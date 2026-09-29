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
#[serde(rename_all = "camelCase")]
pub struct ChapterRef {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    /// Entry name of the chapter body — a v1 chapter file, a v2 skeleton.
    #[serde(default)]
    pub file: String,
    /// Version 2 only: the languages this chapter has an overlay for (§3.4
    /// allows a listed target to be missing from a chapter).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gloss_langs: Vec<String>,
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

// ---------------------------------------------------------------------------
// Format version 2 (doc/specs/tbook-format-v2.md)
//
// Only what this consumer needs is modelled; serde ignores every other field,
// as §3.1/§10.2 require. Offsets here are Unicode code points (§7) — the
// WebView converts them to UTF-16 before slicing.
// ---------------------------------------------------------------------------

use std::collections::BTreeMap;

/// `manifest.json` of a version-2 file (spec §3).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestV2 {
    pub format_version: i64,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub languages: LanguagesV2,
    #[serde(default)]
    pub text: TextDeclV2,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub spine: Vec<ChapterRefV2>,
    /// Footnote entries (§3.7): always an object, `{}` when there are none.
    #[serde(default)]
    pub notes: NotesRefV2,
    /// Entry name of the shipped JSON Schema (§3.9).
    #[serde(default)]
    pub schema: String,
    /// `entry -> "sha256:<hex>"` for every entry but `mimetype` and
    /// `manifest.json` (§9.1), checked on every entry read.
    #[serde(default)]
    pub digests: BTreeMap<String, String>,
    // `source` and `meta` are informative and carry no reader semantics.
}

/// `manifest.notes` (§3.7): the footnote bodies and one overlay per language.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NotesRefV2 {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub gloss: BTreeMap<String, String>,
}

/// One footnote body of `text/notes.json` (§4.12), as binding needs it.
#[derive(Debug, Clone, Deserialize)]
pub struct NoteV2 {
    #[serde(default)]
    pub paragraphs: Vec<BlockV2>,
}

/// `gloss/notes.<lang>.json` (§6.12), as binding needs it.
#[derive(Debug, Clone, Deserialize)]
pub struct NotesOverlayV2 {
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub notes: BTreeMap<String, Vec<GlossParagraphV2>>,
}

/// `manifest.languages` (spec §3.4): the source pivot and the gloss languages.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LanguagesV2 {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub targets: Vec<String>,
}

/// `manifest.text` (spec §3.3). Every value is a constant under version 2;
/// `normalization` is always NFC and there is nothing for a reader to do with
/// it, so it is not modelled.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TextDeclV2 {
    #[serde(default)]
    pub tokenizer: String,
    #[serde(default)]
    pub offsets: String,
}

/// One `manifest.spine` entry (spec §3.5).
#[derive(Debug, Clone, Deserialize)]
pub struct ChapterRefV2 {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    /// Entry name of the language-free skeleton.
    #[serde(default)]
    pub text: String,
    /// `lang -> overlay entry name`.
    #[serde(default)]
    pub gloss: BTreeMap<String, String>,
}

/// A chapter skeleton `text/chN.json` (spec §4), in the shape the binding
/// rules and full-book search need.
#[derive(Debug, Clone, Deserialize)]
pub struct SkeletonV2 {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub paragraphs: Vec<BlockV2>,
}

/// A paragraph, table cell or footnote paragraph (spec §4.2).
#[derive(Debug, Clone, Deserialize)]
pub struct BlockV2 {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    /// Sentence ranges `[a, b)` into `text`, in code points.
    #[serde(default)]
    pub sents: Vec<[i64; 2]>,
    #[serde(default)]
    pub table: Option<TableV2>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TableV2 {
    #[serde(default)]
    pub rows: Vec<Vec<BlockV2>>,
}

/// An overlay `gloss/chN.<lang>.json` (spec §6). The translations themselves
/// travel to the WebView as raw JSON; here we only need what binds them.
#[derive(Debug, Clone, Deserialize)]
pub struct OverlayV2 {
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub chapter: String,
    #[serde(default)]
    pub paragraphs: Vec<GlossParagraphV2>,
}

/// `{id, s}` for a text block, `{id, rows}` for a table block (spec §6.1).
#[derive(Debug, Clone, Deserialize)]
pub struct GlossParagraphV2 {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub s: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub rows: Option<Vec<Vec<GlossCellV2>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlossCellV2 {
    #[serde(default)]
    pub s: Vec<serde_json::Value>,
}

/// Where a locator resolved to (§3.5.2): indices into the spine and its
/// paragraphs. `paragraph` is `None` for the chapter's start; `sentence` and
/// `word` are handed back as stored for the WebView to check against the
/// tokenized paragraph. `exact` is true when chapter and paragraph both
/// resolved by id — the same file, so the saved indices still apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Locator {
    pub chapter: usize,
    pub paragraph: Option<usize>,
    pub sentence: Option<usize>,
    pub word: Option<usize>,
    pub exact: bool,
}

impl ManifestV2 {
    /// The version-independent view the WebView and the library list consume.
    /// `chapters[].file` is the skeleton entry; the per-chapter gloss languages
    /// ride along so the reader knows which overlays a chapter actually has.
    pub fn normalized(&self) -> Manifest {
        Manifest {
            format_version: self.format_version,
            title: self.title.clone(),
            author: self.authors.join(", "),
            source_lang: self.languages.source.clone(),
            target_langs: self.languages.targets.clone(),
            cover: self.cover.clone(),
            // Footnotes are not rendered for version-2 books yet; the v1 note
            // shape would mis-parse a v2 notes entry, so none is offered.
            notes: None,
            chapters: self
                .spine
                .iter()
                .map(|c| ChapterRef {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    file: c.text.clone(),
                    gloss_langs: c.gloss.keys().cloned().collect(),
                })
                .collect(),
        }
    }
}
