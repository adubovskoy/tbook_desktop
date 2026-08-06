//! Models for downloadable offline dictionaries (`.tdict`).
//!
//! Port of `data/dict/DictionaryModels.kt`. The format is specified in
//! `../android/docs/tdict-format.md`: a per-language-pair SQLite file, served
//! gzipped, listed by the manifest at dictionary.tbook.dev. An installed
//! dictionary is that SQLite file plus a JSON sidecar holding its
//! [`InstalledDict`] metadata.

use serde::{Deserialize, Serialize};

/// Manifest schema versions this app understands (tdict §4).
pub const SUPPORTED_MANIFEST_VERSION: u32 = 1;

/// On-device `.tdict` format versions this app can open (tdict §2).
pub const SUPPORTED_TDICT_FORMAT: u32 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDictManifest {
    #[serde(default = "one")]
    pub format_version: u32,
    #[serde(default)]
    pub dictionaries: Vec<RemoteDict>,
}

fn one() -> u32 {
    1
}

/// One downloadable dictionary. `sha256` and `download_size_bytes` describe the
/// gzip file as transferred; `size_bytes` the decompressed SQLite file.
/// `version` is monotonic — greater than the installed one means an update.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDict {
    pub id: String,
    pub source: String,
    pub target: String,
    pub name: String,
    pub version: i64,
    #[serde(default = "one")]
    pub min_format_version: u32,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub download_size_bytes: u64,
    pub sha256: String,
    pub url: String,
    #[serde(default)]
    pub entry_count: u64,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub attribution: String,
}

/// Sidecar (`<id>.json`) written next to `<id>.sqlite` on successful install.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledDict {
    pub id: String,
    pub source: String,
    pub target: String,
    pub name: String,
    pub version: i64,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub attribution: String,
}

/// One numbered meaning of a dictionary article. For source→English
/// dictionaries the gloss itself is the translation and `translations` is empty.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DictSense {
    #[serde(default)]
    pub gloss: String,
    #[serde(default)]
    pub translations: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
}

/// A decoded dictionary article (one homonym).
#[derive(Debug, Clone, Serialize)]
pub struct DictEntry {
    pub lemma: String,
    pub pos: Option<String>,
    pub gender: Option<String>,
    pub ipa: Option<String>,
    pub senses: Vec<DictSense>,
}

/// "en-US" → "en": dictionaries are keyed by base language subtags.
pub fn base_lang(code: &str) -> String {
    code.split('-').next().unwrap_or("").trim().to_lowercase()
}

/// Decode an entry's `senses_json` column. Unknown fields are ignored so a v1
/// reader survives additive changes (tdict §3).
pub fn parse_senses(senses_json: &str) -> Vec<DictSense> {
    serde_json::from_str(senses_json).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_manifest_leniently() {
        let body = r#"{"formatVersion":1,"dictionaries":[{"id":"en-ru","source":"en",
            "target":"ru","name":"English → Russian","version":20260722,
            "sizeBytes":41234567,"downloadSizeBytes":19876543,"sha256":"abc",
            "url":"https://example.test/en-ru.tdict.gz","somethingNew":42}]}"#;
        let m: RemoteDictManifest = serde_json::from_str(body).unwrap();
        assert_eq!(m.format_version, 1);
        assert_eq!(m.dictionaries.len(), 1);
        let d = &m.dictionaries[0];
        assert_eq!(d.id, "en-ru");
        assert_eq!(d.download_size_bytes, 19_876_543);
        // Absent optional fields fall back rather than failing the parse.
        assert_eq!(d.min_format_version, 1);
        assert_eq!(d.license, "");
    }

    #[test]
    fn parses_senses_and_survives_junk() {
        let senses = parse_senses(
            r#"[{"gloss":"challenge","translations":["про́ба"],"unknown":1},{"gloss":"book"}]"#,
        );
        assert_eq!(senses.len(), 2);
        assert_eq!(senses[0].translations, vec!["про́ба"]);
        assert!(senses[1].translations.is_empty());
        assert!(parse_senses("not json").is_empty());
    }

    #[test]
    fn base_lang_strips_region() {
        assert_eq!(base_lang("en-US"), "en");
        assert_eq!(base_lang(" RU "), "ru");
    }
}
