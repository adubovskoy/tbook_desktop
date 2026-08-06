//! English word → IPA pronunciation.
//!
//! Port of `data/dict/PronunciationDictionary.kt` + `EnglishSpelling.kt`. Two
//! bundled dictionaries, one per accent (`resources/en_ipa_us.tsv`,
//! `en_ipa_gb.tsv`, built by the Android project's `tools/build_ipa_dict.py`
//! from open-dict-data/ipa-dict (MIT) and Wiktionary via kaikki.org (CC BY-SA)).
//! Both files use the same notation, so switching accents changes the
//! pronunciation, not the typography.
//!
//! Loaded lazily on first lookup, and only one accent is ever resident (~180k
//! entries). Kept in Rust rather than shipped to the WebView to keep memory
//! tight, and lookups are then O(1) plus a few cheap fallbacks: the other
//! accent's spelling, regular inflections, and compounds.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Which English is transcribed. Serialized as the frontend's setting values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    American,
    British,
}

impl Accent {
    fn file(self) -> &'static str {
        match self {
            Accent::American => "en_ipa_us.tsv",
            Accent::British => "en_ipa_gb.tsv",
        }
    }
}

type Entries = HashMap<String, String>;

/// The resident dictionary: at most one accent at a time.
#[derive(Default)]
pub struct Pronunciations {
    loaded: Mutex<Option<(Accent, Entries)>>,
}

impl Pronunciations {
    /// IPA (without slashes) for `word` in `accent`, or `None` if unknown.
    /// `dir` is the resource directory holding the two TSVs.
    pub fn lookup(&self, dir: &Path, accent: Accent, word: &str) -> Option<String> {
        let mut guard = self.loaded.lock().ok()?;
        if guard.as_ref().map(|(a, _)| *a) != Some(accent) {
            // Drop the other accent's map before loading, so only one is resident.
            *guard = None;
            *guard = Some((accent, load(&dir.join(accent.file()))));
        }
        let map = &guard.as_ref()?.1;
        resolve(map, accent, word)
    }
}

/// Parse a `word<TAB>ipa` TSV into a lookup map. A missing/unreadable file
/// yields an empty map (lookups then just return `None`).
pub fn load(path: &Path) -> Entries {
    let mut map = Entries::with_capacity(180_000);
    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            if let Some(tab) = line.find('\t') {
                if tab > 0 {
                    map.insert(line[..tab].to_string(), line[tab + 1..].to_string());
                }
            }
        }
    }
    map
}

/// `WordNormalizer::strip` plus lowercase — the TSVs are keyed lowercase.
fn normalize(word: &str) -> Option<String> {
    crate::dict::normalize::strip(word).map(|w| w.to_lowercase())
}

/// The whole fallback chain, in the order Android tries it.
fn resolve(map: &Entries, accent: Accent, word: &str) -> Option<String> {
    let key = normalize(word)?;
    if let Some(found) = lookup_key(map, &key) {
        return Some(found);
    }
    // The other side's spelling of a word this file only has in its own:
    // colour → color for the American file, center → centre for the British.
    if let Some(variant) = spelling_variant(&key, accent) {
        if let Some(found) = lookup_key(map, &variant) {
            return Some(found);
        }
    }
    // Hyphenated compound: pronounce it part by part (all-important →
    // ˈɔl ɪmˈpɔɹtənt). Only if every part is known.
    if key.contains('-') {
        let parts: Vec<&str> = key.split('-').filter(|p| !p.is_empty()).collect();
        if parts.len() >= 2 {
            let mut out = Vec::with_capacity(parts.len());
            for part in &parts {
                out.push(lookup_key(map, part)?);
            }
            return Some(out.join(" "));
        }
    }
    // Last resort — closed compound of two known words (cashbox → cash box,
    // busybody → busy body). Direct hits only, no synthesis. Splits near the
    // middle are tried first: balanced halves are real words more often than
    // the 3-letter scraps at the edges (cas|hbox).
    let chars: Vec<char> = key.chars().collect();
    let n = chars.len();
    if n >= 6 {
        let mut splits: Vec<usize> = (3..=n - 3).collect();
        splits.sort_by_key(|i| ((2 * *i) as isize - n as isize).abs() * 1000 - *i as isize);
        for i in splits {
            let head: String = chars[..i].iter().collect();
            let tail: String = chars[i..].iter().collect();
            if let (Some(a), Some(b)) = (map.get(&head), map.get(&tail)) {
                return Some(format!("{a} {b}"));
            }
        }
    }
    None
}

/// A direct hit, the possessive base, or a synthesized regular inflection.
fn lookup_key(map: &Entries, key: &str) -> Option<String> {
    if let Some(found) = map.get(key) {
        return Some(found.clone());
    }
    // Possessives: dog's / bankers' / James'.
    if let Some(base) = key.strip_suffix("'s") {
        if let Some(found) = map.get(base) {
            return Some(found.clone());
        }
    }
    let trimmed = key.trim_end_matches('\'');
    if trimmed != key {
        if let Some(found) = map.get(trimmed) {
            return Some(found.clone());
        }
    }
    synthesize_inflection(map, key)
}

// Final phonemes deciding the plural suffix: -ɪz after sibilants (tʃ/dʒ end in
// ʃ/ʒ), voiceless -s, otherwise voiced -z.
const SIBILANTS: [char; 4] = ['s', 'z', 'ʃ', 'ʒ'];
const VOICELESS: [char; 5] = ['p', 't', 'k', 'f', 'θ'];

fn last_char(s: &str) -> Option<char> {
    s.chars().next_back()
}

fn s_suffix(base: &str) -> &'static str {
    match last_char(base) {
        Some(c) if SIBILANTS.contains(&c) => "ɪz",
        Some(c) if VOICELESS.contains(&c) => "s",
        _ => "z",
    }
}

/// Regular inflections missing from the dictionary, derived from the base word
/// with the standard voicing rules: -s/-es (abbots, boxes, ladies), -ed
/// (clattered, hoped, carried), -ing (buttoning, running), -ly.
fn synthesize_inflection(map: &Entries, key: &str) -> Option<String> {
    if key.len() < 4 {
        return None;
    }
    if key.ends_with('s') && !key.ends_with("ss") {
        if let Some(base) = map.get(&key[..key.len() - 1]) {
            return Some(format!("{base}{}", s_suffix(base)));
        }
        if key.ends_with("es") {
            if let Some(base) = map.get(&key[..key.len() - 2]) {
                if last_char(base).is_some_and(|c| SIBILANTS.contains(&c)) {
                    return Some(format!("{base}ɪz"));
                }
            }
        }
        if key.ends_with("ies") {
            if let Some(base) = map.get(&format!("{}y", &key[..key.len() - 3])) {
                return Some(format!("{base}z"));
            }
        }
    }
    if key.ends_with("ed") && key.len() > 4 {
        if key.ends_with("ied") {
            if let Some(base) = map.get(&format!("{}y", &key[..key.len() - 3])) {
                return Some(format!("{base}d"));
            }
        }
        if let Some(base) = inflection_stem(map, &key[..key.len() - 2]) {
            let suffix = match last_char(&base) {
                Some('t') | Some('d') => "ɪd",
                Some(c) if VOICELESS.contains(&c) => "t",
                _ => "d",
            };
            return Some(format!("{base}{suffix}"));
        }
    }
    if key.ends_with("ing") && key.len() > 5 {
        if let Some(base) = inflection_stem(map, &key[..key.len() - 3]) {
            return Some(format!("{base}ɪŋ"));
        }
    }
    if key.ends_with("ly") {
        if let Some(base) = map.get(&key[..key.len() - 2]) {
            return Some(format!("{base}li"));
        }
    }
    None
}

/// Pronunciation of an -ed/-ing stem. A doubled final consonant means the
/// spelling doubled it (running → run) and a silent e was never dropped;
/// otherwise the e-restored form is tried first (hoping → hope, not hop).
fn inflection_stem(map: &Entries, stem: &str) -> Option<String> {
    let chars: Vec<char> = stem.chars().collect();
    let doubled = chars.len() > 2 && chars[chars.len() - 1] == chars[chars.len() - 2];
    if doubled {
        let undoubled: String = chars[..chars.len() - 1].iter().collect();
        map.get(stem).or_else(|| map.get(&undoubled)).cloned()
    } else {
        map.get(&format!("{stem}e")).or_else(|| map.get(stem)).cloned()
    }
}

/// `key` rewritten in the spelling `accent`'s dictionary uses, or `None` if
/// nothing applies (the word already looks native to that accent).
///
/// Deliberately blunt (port of `EnglishSpelling.kt`): it is only ever tried
/// after a direct lookup has already missed, so a transform that invents a word
/// (doctor → doctour) just misses again.
fn spelling_variant(key: &str, accent: Accent) -> Option<String> {
    let mut k = match accent {
        Accent::American => key.replace("our", "or"),
        // Only word-final -or is the American half of -our: color, colors,
        // colored — never "world" or "storm".
        Accent::British => american_or_to_our(key),
    };
    let suffixes: &[(&str, &str)] = match accent {
        Accent::American => &UK_SUFFIXES,
        Accent::British => &US_SUFFIXES,
    };
    for (from, to) in suffixes {
        if k.ends_with(from) {
            k = format!("{}{to}", &k[..k.len() - from.len()]);
            break;
        }
    }
    if k == key {
        None
    } else {
        Some(k)
    }
}

/// Word-final American -or(s|ed|…) → -our(…): the half of -our worth flipping.
fn american_or_to_our(key: &str) -> String {
    const TAILS: [&str; 8] = ["", "s", "ed", "ing", "ful", "less", "ous", "able"];
    // Longest tail first, so "colorful" isn't matched as "color" + "ful" twice.
    let mut best: Option<(usize, &str)> = None;
    for tail in TAILS {
        let needle = format!("or{tail}");
        if key.ends_with(&needle) && key.len() > needle.len() {
            if best.is_none_or(|(len, _)| needle.len() > len) {
                best = Some((needle.len(), tail));
            }
        }
    }
    match best {
        Some((len, tail)) => format!("{}our{tail}", &key[..key.len() - len]),
        None => key.to_string(),
    }
}

// Longest match first: -ises before -ise; -re last, it's the loosest.
const UK_SUFFIXES: [(&str, &str); 10] = [
    ("ising", "izing"),
    ("ises", "izes"),
    ("ised", "ized"),
    ("ise", "ize"),
    ("ysing", "yzing"),
    ("yses", "yzes"),
    ("ysed", "yzed"),
    ("yse", "yze"),
    ("res", "ers"),
    ("re", "er"),
];

// The mirror image, for the British file. British headwords keep -ize
// (Wiktionary and the OED both prefer it), so only -yze, the doubled-l
// inflections and -ense are worth flipping, plus -er → -re last.
const US_SUFFIXES: [(&str, &str); 11] = [
    ("yzing", "ysing"),
    ("yzes", "yses"),
    ("yzed", "ysed"),
    ("yze", "yse"),
    ("eling", "elling"),
    ("eled", "elled"),
    ("elers", "ellers"),
    ("eler", "eller"),
    ("ense", "ence"),
    ("ers", "res"),
    ("er", "re"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn dict() -> Entries {
        let mut m = Entries::new();
        for (word, ipa) in [
            ("hello", "həˈloʊ"),
            ("cat", "kæt"),
            ("box", "bɑks"),
            ("lady", "ˈleɪdi"),
            ("hope", "hoʊp"),
            ("run", "rʌn"),
            ("clatter", "ˈklætəɹ"),
            ("carry", "ˈkæɹi"),
            ("quick", "kwɪk"),
            ("all", "ˈɔl"),
            ("important", "ɪmˈpɔɹtənt"),
            ("cash", "kæʃ"),
            ("color", "ˈkʌləɹ"),
            ("centre", "ˈsɛntəɹ"),
            ("realise", "ˈɹiəlaɪz"),
        ] {
            m.insert(word.into(), ipa.into());
        }
        m
    }

    #[test]
    fn direct_hits_and_possessives() {
        let m = dict();
        assert_eq!(resolve(&m, Accent::American, "Hello").as_deref(), Some("həˈloʊ"));
        assert_eq!(resolve(&m, Accent::American, "“Hello,”").as_deref(), Some("həˈloʊ"));
        assert_eq!(resolve(&m, Accent::American, "cat’s").as_deref(), Some("kæt"));
        assert_eq!(resolve(&m, Accent::American, "   "), None);
        assert_eq!(resolve(&m, Accent::American, "zzzzzz"), None);
    }

    #[test]
    fn synthesizes_regular_inflections() {
        let m = dict();
        // -s voicing: after a voiceless stop, /s/; otherwise /z/; after a sibilant, /ɪz/.
        assert_eq!(resolve(&m, Accent::American, "cats").as_deref(), Some("kæts"));
        assert_eq!(resolve(&m, Accent::American, "runs").as_deref(), Some("rʌnz"));
        assert_eq!(resolve(&m, Accent::American, "boxes").as_deref(), Some("bɑksɪz"));
        assert_eq!(resolve(&m, Accent::American, "ladies").as_deref(), Some("ˈleɪdiz"));
        // -ed: /ɪd/ after t/d, /t/ after voiceless, /d/ otherwise.
        assert_eq!(resolve(&m, Accent::American, "clattered").as_deref(), Some("ˈklætəɹd"));
        assert_eq!(resolve(&m, Accent::American, "hoped").as_deref(), Some("hoʊpt"));
        assert_eq!(resolve(&m, Accent::American, "carried").as_deref(), Some("ˈkæɹid"));
        // -ing, with the doubled consonant and the restored silent e.
        assert_eq!(resolve(&m, Accent::American, "running").as_deref(), Some("rʌnɪŋ"));
        assert_eq!(resolve(&m, Accent::American, "hoping").as_deref(), Some("hoʊpɪŋ"));
        // -ly
        assert_eq!(resolve(&m, Accent::American, "quickly").as_deref(), Some("kwɪkli"));
    }

    #[test]
    fn compounds_are_pronounced_part_by_part() {
        let m = dict();
        assert_eq!(
            resolve(&m, Accent::American, "all-important").as_deref(),
            Some("ˈɔl ɪmˈpɔɹtənt")
        );
        // Closed compound, split at the balanced point.
        assert_eq!(resolve(&m, Accent::American, "cashbox").as_deref(), Some("kæʃ bɑks"));
        // An unknown part means no transcription at all, rather than a wrong one.
        assert_eq!(resolve(&m, Accent::American, "all-zzzzz"), None);
    }

    #[test]
    fn the_other_accents_spelling_is_tried_once() {
        let m = dict();
        // The American file has "color"; a British edition spells it "colour".
        assert_eq!(resolve(&m, Accent::American, "colour").as_deref(), Some("ˈkʌləɹ"));
        // The British file has "centre"; an American edition spells it "center".
        assert_eq!(resolve(&m, Accent::British, "center").as_deref(), Some("ˈsɛntəɹ"));
        // British -ise where the file (here) has -ise already: still a hit.
        assert_eq!(resolve(&m, Accent::British, "realise").as_deref(), Some("ˈɹiəlaɪz"));
    }

    #[test]
    fn spelling_variants_only_fire_when_something_applies() {
        assert_eq!(spelling_variant("colour", Accent::American).as_deref(), Some("color"));
        assert_eq!(spelling_variant("colors", Accent::British).as_deref(), Some("colours"));
        assert_eq!(spelling_variant("center", Accent::British).as_deref(), Some("centre"));
        assert_eq!(spelling_variant("analyse", Accent::American).as_deref(), Some("analyze"));
        // Not the American half of -our, and no suffix applies.
        assert_eq!(spelling_variant("world", Accent::British), None);
        assert_eq!(spelling_variant("storm", Accent::British), None);
        assert_eq!(spelling_variant("cat", Accent::American), None);
    }

    #[test]
    fn the_bundled_dictionaries_load_and_answer() {
        // Guards the resource names and the TSV format, not the data.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let pron = Pronunciations::default();
        let us = pron.lookup(&dir, Accent::American, "water").expect("US water");
        let gb = pron.lookup(&dir, Accent::British, "water").expect("GB water");
        assert!(!us.is_empty() && !gb.is_empty());
        assert_ne!(us, gb, "the two accents should transcribe water differently");
        // Switching accents swaps the resident map rather than mixing them.
        assert_eq!(pron.lookup(&dir, Accent::American, "water").as_deref(), Some(us.as_str()));
    }
}
