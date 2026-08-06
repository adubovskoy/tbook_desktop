//! Read-aloud fallback for WebViews without the Web Speech API.
//!
//! The frontend speaks through `window.speechSynthesis` wherever it works —
//! WebView2 on Windows and WKWebView on macOS both back it with the system
//! voices, for no binary weight and no dependency. WebKitGTK builds speech
//! synthesis behind a compile-time flag most distributions leave off, so on
//! Linux `getVoices()` usually comes back empty; this module is what the
//! frontend falls back to there: whichever speech CLI the system has.
//!
//! No new build dependency, no daemon of our own — if nothing is installed,
//! [`NativeTts::can_speak`] says no and the reader hides the speaker button
//! (the contract of Android's `Speaker.canSpeak`).
//!
//! The utterance text never goes through a shell: it is passed as one argument
//! or written to the child's stdin.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// The speech CLI this machine has, in preference order.
#[derive(Clone, Debug, PartialEq)]
enum Engine {
    /// macOS `say` — always present; voices are per-language.
    Say,
    /// speech-dispatcher's `spd-say`: the desktop Linux standard, and the same
    /// daemon Orca uses, so it is configured on any accessibility-ready system.
    SpdSay,
    /// `espeak-ng` (or the older `espeak`) driving the sound card directly.
    Espeak(PathBuf),
    /// Windows SAPI through PowerShell. Only a backstop — WebView2 has the
    /// Web Speech API, so the frontend never gets this far on Windows.
    Powershell(PathBuf),
}

/// First match in `PATH`. Deliberately hand-rolled: one env lookup beats a
/// dependency, and `which(1)` isn't on every system either.
fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

fn engine() -> Option<&'static Engine> {
    static ENGINE: OnceLock<Option<Engine>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            if cfg!(target_os = "macos") && PathBuf::from("/usr/bin/say").is_file() {
                return Some(Engine::Say);
            }
            if find_in_path("spd-say").is_some() {
                return Some(Engine::SpdSay);
            }
            for bin in ["espeak-ng", "espeak"] {
                if let Some(path) = find_in_path(bin) {
                    return Some(Engine::Espeak(path));
                }
            }
            if cfg!(windows) {
                if let Some(path) = find_in_path("powershell.exe") {
                    return Some(Engine::Powershell(path));
                }
            }
            None
        })
        .as_ref()
}

/// "en-GB" → "en": the CLIs take a base language (`spd-say -l`, `espeak -v`).
fn base_lang(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

/// `say -v '?'` once, parsed into (voice name, language tag) pairs. The listing
/// is `Name   lang_REGION  # example phrase`, with names that can contain
/// spaces ("Grandma (Enhanced)"), so the language tag anchors the split.
fn say_voices() -> &'static Vec<(String, String)> {
    static VOICES: OnceLock<Vec<(String, String)>> = OnceLock::new();
    VOICES.get_or_init(|| {
        let out = match Command::new("/usr/bin/say").arg("-v").arg("?").output() {
            Ok(out) if out.status.success() => out,
            _ => return Vec::new(),
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                let head = line.split('#').next()?.trim_end();
                let (name, tag) = head.rsplit_once(char::is_whitespace)?;
                let tag = tag.trim();
                // A language tag, not another word of the voice's name.
                if tag.len() < 2 || !tag.chars().next()?.is_ascii_alphabetic() {
                    return None;
                }
                Some((name.trim().to_string(), tag.replace('_', "-")))
            })
            .collect()
    })
}

/// The `say` voice for `tag`: an exact region match first, then any voice of the
/// same base language (Russian text read by the one Russian voice installed).
fn say_voice_for(tag: &str) -> Option<String> {
    let want = tag.to_lowercase();
    let base = base_lang(tag);
    let voices = say_voices();
    voices
        .iter()
        .find(|(_, l)| l.to_lowercase() == want)
        .or_else(|| voices.iter().find(|(_, l)| base_lang(l) == base))
        .map(|(name, _)| name.clone())
}

/// Whether espeak lists a voice for this language (`--voices=ru` prints a table
/// of matching voices, or nothing at all).
fn espeak_has_voice(binary: &PathBuf, tag: &str) -> bool {
    let base = base_lang(tag);
    if base.is_empty() {
        return false;
    }
    match Command::new(binary).arg(format!("--voices={base}")).output() {
        // A header line is always printed; a real voice adds at least one more.
        Ok(out) => String::from_utf8_lossy(&out.stdout).lines().count() > 1,
        Err(_) => false,
    }
}

/// Hide the console window PowerShell would otherwise flash on Windows.
#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

/// Speaks through the system CLI, one utterance at a time.
#[derive(Default)]
pub struct NativeTts {
    /// The utterance currently playing, so the next one can cut it off —
    /// `TextToSpeech.QUEUE_FLUSH` on Android.
    current: Mutex<Option<Child>>,
}

impl NativeTts {
    /// Whether this machine can speak `tag` (a BCP-47 language tag).
    ///
    /// Cheap after the first call: the engine and its voice list are resolved
    /// once and cached. `spd-say` and SAPI can't be interrogated per language
    /// without a round trip through the daemon, so they answer yes and let the
    /// engine substitute a voice — the same best-effort Android's
    /// `isLanguageAvailable` gives on devices with partial voice data.
    pub fn can_speak(&self, tag: &str) -> bool {
        match engine() {
            Some(Engine::Say) => say_voice_for(tag).is_some(),
            Some(Engine::SpdSay) | Some(Engine::Powershell(_)) => !base_lang(tag).is_empty(),
            Some(Engine::Espeak(binary)) => espeak_has_voice(binary, tag),
            None => false,
        }
    }

    /// Speak `text` in `tag`, cutting off anything already playing.
    pub fn speak(&self, text: &str, tag: &str) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let engine = engine().ok_or("No speech engine installed.")?;
        self.stop();

        let base = base_lang(tag);
        let mut stdin_text: Option<&str> = None;
        let mut cmd = match engine {
            Engine::Say => {
                let mut cmd = Command::new("/usr/bin/say");
                if let Some(voice) = say_voice_for(tag) {
                    cmd.arg("-v").arg(voice);
                }
                // Read the text from stdin: as an argument it would have to
                // survive `say`'s own option parsing (dialogue opening with a
                // dash is ordinary prose in a novel).
                cmd.arg("-f").arg("/dev/stdin");
                stdin_text = Some(text);
                cmd
            }
            Engine::SpdSay => {
                let mut cmd = Command::new("spd-say");
                cmd.arg("-l").arg(&base).arg("--").arg(text);
                cmd
            }
            Engine::Espeak(binary) => {
                let mut cmd = Command::new(binary);
                cmd.arg("-v").arg(&base).arg("--stdin");
                stdin_text = Some(text);
                cmd
            }
            Engine::Powershell(binary) => {
                let mut cmd = Command::new(binary);
                cmd.args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    // The text arrives on stdin, so no quoting of book prose
                    // into a PowerShell string literal.
                    &format!(
                        "Add-Type -AssemblyName System.Speech; \
                         $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
                         try {{ $s.SelectVoiceByHints([System.Speech.Synthesis.VoiceGender]::NotSet, \
                         [System.Speech.Synthesis.VoiceAge]::NotSet, 0, \
                         [System.Globalization.CultureInfo]::GetCultureInfo('{base}')) }} catch {{ }}; \
                         $s.Speak([Console]::In.ReadToEnd())"
                    ),
                ]);
                stdin_text = Some(text);
                cmd
            }
        };
        no_window(&mut cmd);
        cmd.stdin(if stdin_text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null());

        let mut child = cmd.spawn().map_err(|e| format!("Speech failed: {e}"))?;
        if let Some(text) = stdin_text {
            // Ignore a broken pipe: the engine having quit early is a silent
            // utterance, not an error worth surfacing over the book.
            if let Some(stdin) = child.stdin.take() {
                let mut stdin = stdin;
                let _ = stdin.write_all(text.as_bytes());
            }
        }
        *self.current.lock().unwrap() = Some(child);
        Ok(())
    }

    /// Silence whatever is playing.
    pub fn stop(&self) {
        if let Some(mut child) = self.current.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // spd-say only hands the text to the speech-dispatcher daemon, which
        // keeps talking after the client is gone — cancel it at the daemon.
        if engine() == Some(&Engine::SpdSay) {
            let mut cmd = Command::new("spd-say");
            cmd.arg("-C").stdout(Stdio::null()).stderr(Stdio::null());
            no_window(&mut cmd);
            if let Ok(mut child) = cmd.spawn() {
                let _ = child.wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_lang_strips_region() {
        assert_eq!(base_lang("en-GB"), "en");
        assert_eq!(base_lang("ru"), "ru");
        assert_eq!(base_lang("zh_CN"), "zh");
        assert_eq!(base_lang(""), "");
    }

    #[test]
    fn empty_text_is_a_no_op() {
        // No engine required: blank text never reaches one.
        assert!(NativeTts::default().speak("   ", "en-US").is_ok());
    }

    #[test]
    fn unknown_language_cannot_be_spoken() {
        // With no engine installed nothing can be spoken; with one, a blank
        // tag still can't. Both leave the speaker button hidden.
        assert!(!NativeTts::default().can_speak(""));
    }
}
