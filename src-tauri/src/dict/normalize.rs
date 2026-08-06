//! Turns a tapped token into dictionary lookup keys.
//!
//! Port of `data/dict/WordNormalizer.kt`, and the algorithm tdict §6 specifies
//! for consumers. Kept separate from the store so it stays trivially testable.

/// Tokens longer than this are punctuation runs or degenerate "words".
const MAX_LEN: usize = 64;

/// Trim whitespace; fold the typographic apostrophe books actually use into the
/// ASCII one dictionaries key on (can’t → can't); drop surrounding chars that
/// aren't letters/apostrophe/hyphen. Case is preserved — proper nouns match
/// their capitalized lemma directly.
pub fn strip(word: &str) -> Option<String> {
    if word.chars().count() > MAX_LEN {
        return None;
    }
    let folded = word.trim().replace('’', "'");
    let trimmed = folded.trim_matches(|c: char| !c.is_alphabetic() && c != '\'' && c != '-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Ordered lookup candidates for a tapped word: as stripped (proper nouns),
/// lowercased (sentence-initial capitals), and with the English possessive
/// removed (dog's / bankers'). Callers try each in turn and take the first hit.
pub fn candidates(word: &str) -> Vec<String> {
    let Some(base) = strip(word) else {
        return Vec::new();
    };
    let lower = base.to_lowercase();
    let mut out: Vec<String> = Vec::with_capacity(4);
    let mut push = |candidate: String| {
        if !candidate.is_empty() && !out.contains(&candidate) {
            out.push(candidate);
        }
    };
    push(base.clone());
    push(lower.clone());
    for variant in [base, lower] {
        if let Some(stem) = variant.strip_suffix("'s") {
            push(stem.to_string());
        }
        let trimmed = variant.trim_end_matches('\'');
        if trimmed != variant {
            push(trimmed.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_punctuation_but_keeps_case_and_inner_marks() {
        assert_eq!(strip("“Hello,”").as_deref(), Some("Hello"));
        assert_eq!(strip("can’t").as_deref(), Some("can't"));
        assert_eq!(strip("well-known").as_deref(), Some("well-known"));
        assert_eq!(strip("   "), None);
        assert_eq!(strip("…—…"), None);
        assert_eq!(strip(&"a".repeat(65)), None);
    }

    #[test]
    fn candidates_are_ordered_and_deduplicated() {
        assert_eq!(candidates("Holmes"), vec!["Holmes", "holmes"]);
        assert_eq!(candidates("dog’s"), vec!["dog's", "dog"]);
        assert_eq!(candidates("Bankers'"), vec!["Bankers'", "bankers'", "Bankers", "bankers"]);
        // Already lowercase and unpossessed: one candidate, not four.
        assert_eq!(candidates("cat"), vec!["cat"]);
        assert!(candidates("...").is_empty());
    }
}
