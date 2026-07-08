//! `.tbook` data model (mirrors the Android app's `data/model/Models.kt`).
//!
//! `#[serde(rename_all = "camelCase")]` serves double duty: it parses the
//! camelCase `.tbook` JSON *and* serializes camelCase back to the WebView.

use serde::{Deserialize, Serialize};

fn default_format_version() -> i64 {
    1
}

/// `manifest.json` — book metadata + chapter index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    #[serde(default = "default_format_version")]
    pub format_version: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub source_lang: String,
    #[serde(default)]
    pub target_langs: Vec<String>,
    #[serde(default)]
    pub cover: Option<String>,
    /// Entry name of the footnote-bodies file (`"notes.json"`), if any.
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub chapters: Vec<ChapterRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterRef {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub file: String,
}

/// What `open_book` returns: the manifest plus each chapter's compressed size
/// (used to weight reading progress, mirroring `TbookReader.chapterByteSizes()`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedBook {
    pub manifest: Manifest,
    pub chapter_sizes: Vec<u64>,
}

/// One row in the library list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSummary {
    pub id: String,
    pub title: String,
    pub author: String,
    pub source_lang: String,
    pub target_langs: Vec<String>,
    pub has_cover: bool,
}

/// Minimal chapter shape used only for full-book search: we just need each
/// sentence's source text. serde ignores the other fields (`words`/`tr`/`spans`).
#[derive(Debug, Deserialize)]
pub struct ChapterTexts {
    #[serde(default)]
    pub paragraphs: Vec<Vec<SentenceSrc>>,
}

#[derive(Debug, Deserialize)]
pub struct SentenceSrc {
    #[serde(default)]
    pub src: String,
}
