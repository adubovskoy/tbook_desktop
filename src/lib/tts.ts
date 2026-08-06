// Read-aloud for the tapped word or its sentence — port of `data/tts/Speaker.kt`.
//
// Two backends, picked per utterance. The WebView's own Web Speech API is the
// primary one: WebView2 (Windows) and WKWebView (macOS) both back it with the
// system voices, at no cost in binary size or dependencies. WebKitGTK builds
// speech synthesis behind a compile-time flag most distributions leave off, so
// on Linux `getVoices()` usually comes back empty; there the Rust `tts_*`
// commands drive whatever speech CLI is installed instead (see src-tauri/tts.rs).
//
// When neither can speak the source language, `canSpeak` says no and the popup
// leaves the speaker button out — the same contract as Android's
// `Speaker.canSpeak`, where voice data is often missing too.

import { invoke } from "@tauri-apps/api/core";
import type { Accent } from "./settings";

/** Which backend spoke last, so `stop()` knows where to send the silence. */
let lastBackend: "web" | "native" | null = null;

const synth = (): SpeechSynthesis | null =>
  typeof window !== "undefined" && "speechSynthesis" in window ? window.speechSynthesis : null;

/**
 * The WebView's voice list. Chromium populates it asynchronously and fires
 * `voiceschanged` when it lands, so the first call may have to wait; an empty
 * list after the timeout means this WebView has no speech synthesis at all.
 */
function webVoices(timeoutMs = 1200): Promise<SpeechSynthesisVoice[]> {
  const s = synth();
  if (!s) return Promise.resolve([]);
  const now = s.getVoices();
  if (now.length > 0) return Promise.resolve(now);
  return new Promise((resolve) => {
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      s.removeEventListener("voiceschanged", finish);
      resolve(s.getVoices());
    };
    s.addEventListener("voiceschanged", finish);
    setTimeout(finish, timeoutMs);
  });
}

/**
 * The language tag to speak in.
 *
 * Books are usually tagged plain "en", which would leave the engine on whatever
 * English it defaults to — American on most systems. Pin it to the accent the
 * transcription is written in, so what is heard matches what is shown. A book
 * tagged en-GB/en-US keeps its own region.
 */
export function resolveLang(lang: string, accent: Accent): string {
  const tag = lang.trim().replace("_", "-");
  if (tag.toLowerCase() === "en") return accent === "british" ? "en-GB" : "en-US";
  return tag;
}

const normalize = (tag: string) => tag.toLowerCase().replace("_", "-");

/**
 * The best voice for `tag`: an exact region match wins, otherwise any voice of
 * the same base language (a Russian sentence read by the one Russian voice
 * installed). Within a group the engine's own default comes first, then local
 * voices — a network voice is silent while offline, which is most of the time
 * for this reader.
 */
function pickVoice(voices: SpeechSynthesisVoice[], tag: string): SpeechSynthesisVoice | null {
  const want = normalize(tag);
  const base = want.split("-")[0];
  if (!base) return null;
  const rank = (v: SpeechSynthesisVoice): number => {
    const l = normalize(v.lang);
    if (l !== want && l.split("-")[0] !== base) return -1;
    return (l === want ? 4 : 0) + (v.default ? 2 : 0) + (v.localService ? 1 : 0);
  };
  let best: SpeechSynthesisVoice | null = null;
  let bestRank = -1;
  for (const v of voices) {
    const r = rank(v);
    if (r > bestRank) {
      best = v;
      bestRank = r;
    }
  }
  return bestRank >= 0 ? best : null;
}

/**
 * Whether anything on this machine can read `lang` aloud. The Web Speech API is
 * asked first; a WebView without it (or without a voice for the language) falls
 * through to the system CLI.
 */
export async function canSpeak(lang: string, accent: Accent): Promise<boolean> {
  if (!lang.trim()) return false;
  const tag = resolveLang(lang, accent);
  if (pickVoice(await webVoices(), tag)) return true;
  try {
    return await invoke<boolean>("tts_can_speak", { lang: tag });
  } catch {
    return false;
  }
}

/** Speak `text` in `lang`, cutting off anything already playing. */
export async function speak(text: string, lang: string, accent: Accent): Promise<void> {
  if (!text.trim()) return;
  const tag = resolveLang(lang, accent);
  stop();
  const voice = pickVoice(await webVoices(), tag);
  const s = synth();
  if (voice && s) {
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.voice = voice;
    utterance.lang = voice.lang;
    lastBackend = "web";
    s.speak(utterance);
    return;
  }
  lastBackend = "native";
  try {
    await invoke("tts_speak", { text, lang: tag });
  } catch {
    // Nothing to say about a silent speaker button: the reader keeps reading.
  }
}

/** Silence whatever is playing. */
export function stop(): void {
  synth()?.cancel();
  if (lastBackend === "native") void invoke("tts_stop").catch(() => {});
}
