//! English word → IPA pronunciation (port of `PronunciationDictionary.kt`).
//!
//! Loads `en_ipa.tsv` (~126k `word<TAB>ipa` lines) into a HashMap, lazily and
//! once. Kept in Rust (not shipped to the WebView) to keep memory tight.

use std::collections::HashMap;
use std::path::Path;

/// Parse the TSV into a lookup map. A missing/unreadable file yields an empty
/// map (lookups then just return `None`), matching the Android behavior.
pub fn load(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::with_capacity(140_000);
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

/// Lowercase, then strip surrounding chars that aren't letters / `'` / `’` / `-`.
fn normalize(word: &str) -> Option<String> {
    let lowered = word.trim().to_lowercase();
    let trimmed =
        lowered.trim_matches(|c: char| !c.is_alphabetic() && c != '\'' && c != '’' && c != '-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// IPA (without slashes) for `word`, or `None` if unknown. Falls back to the
/// possessive base (`cats'` → `cat`... i.e. drops a trailing `'s`/`’s`).
pub fn lookup(map: &HashMap<String, String>, word: &str) -> Option<String> {
    let key = normalize(word)?;
    if let Some(v) = map.get(&key) {
        return Some(v.clone());
    }
    for suffix in ["'s", "’s"] {
        if let Some(base) = key.strip_suffix(suffix) {
            if base != key {
                if let Some(v) = map.get(base) {
                    return Some(v.clone());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("hello".into(), "həˈloʊ".into());
        m.insert("cat".into(), "kæt".into());
        m
    }

    #[test]
    fn normalizes_and_looks_up() {
        let m = dict();
        assert_eq!(lookup(&m, "Hello").as_deref(), Some("həˈloʊ"));
        assert_eq!(lookup(&m, "“Hello,”").as_deref(), Some("həˈloʊ"));
        assert_eq!(lookup(&m, "cat's").as_deref(), Some("kæt"));
        assert_eq!(lookup(&m, "missing"), None);
        assert_eq!(lookup(&m, "   "), None);
    }
}
