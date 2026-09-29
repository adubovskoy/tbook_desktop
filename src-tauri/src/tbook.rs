//! Random-access reader over a `.tbook` ZIP (port of `TbookReader.kt`).
//!
//! A `.tbook` is a ZIP of JSON entries plus optional images. Version 1 keeps a
//! `manifest.json` and `chapters/chN.json`; version 2 keeps a `manifest.json`,
//! language-free skeletons `text/chN.json` and per-(chapter, language) overlays
//! `gloss/chN.<lang>.json`. We open the archive per call (cheap, and chapter
//! loads are infrequent) and read entries by name — for version 2 only through
//! the manifest, never by pattern (v2 spec §2.1).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::models::{
    BlockV2, GlossParagraphV2, Locator, Manifest, ManifestV2, NoteV2, NotesOverlayV2, OpenedBook,
    OverlayV2, SkeletonV2,
};

fn open_archive(path: &Path) -> Result<ZipArchive<File>, String> {
    let f = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    ZipArchive::new(f).map_err(|e| format!("not a valid .tbook ({}): {e}", path.display()))
}

fn read_entry_bytes(zip: &mut ZipArchive<File>, name: &str) -> Result<Vec<u8>, String> {
    let mut entry = zip
        .by_name(name)
        .map_err(|_| format!("missing entry: {name}"))?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// The `manifest.formatVersion` values this reader implements (v2 spec §10.1).
pub const SUPPORTED_FORMAT_VERSIONS: [i64; 2] = [1, 2];

/// The tokenizer this consumer implements (v2 spec §5); pinned by its id.
pub const TOKENIZER: &str = "tbook-w2";

/// Feature ids of `manifest.requires` this consumer implements (v2 spec §3.2).
pub const KNOWN_FEATURES: [&str; 3] = ["text/paragraph", "align/token", "tokenizer/tbook-w2"];

/// A parsed manifest of either version — the one place the version split lives.
pub enum Book {
    V1(Manifest),
    V2(ManifestV2),
}

impl Book {
    /// The version-independent view the WebView and the library list consume.
    pub fn normalized(&self) -> Manifest {
        match self {
            Book::V1(m) => m.clone(),
            Book::V2(m) => m.normalized(),
        }
    }
}

/// `requires` ids we do not implement (v2 spec §3.2, §11.3).
fn unknown_features(requires: &[String]) -> Vec<String> {
    requires
        .iter()
        .filter(|id| !KNOWN_FEATURES.contains(&id.as_str()))
        .cloned()
        .collect()
}

/// Parse `manifest.json` bytes, dispatching on `formatVersion` before any other
/// field is decoded (v2 spec §10.1): 1 is the version-1 path, 2 the version-2
/// one, anything else — or a missing field — is refused by name, so a newer
/// file fails with a readable message instead of opening as an empty book.
fn parse_manifest(bytes: &[u8]) -> Result<Book, String> {
    let raw: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("parse manifest: {e}"))?;
    // Version-1 files in the wild predate the field; only a v1-shaped manifest
    // (one with `chapters`) may default to 1, never a version-2 one (§10.1).
    let version = match raw.get("formatVersion") {
        Some(v) => v
            .as_i64()
            .ok_or_else(|| "manifest formatVersion is not an integer".to_string())?,
        None if raw.get("chapters").is_some() => 1,
        None => return Err("manifest has no formatVersion".to_string()),
    };
    match version {
        1 => serde_json::from_value(raw)
            .map(Book::V1)
            .map_err(|e| format!("parse manifest: {e}")),
        2 => {
            let m: ManifestV2 =
                serde_json::from_value(raw).map_err(|e| format!("parse manifest: {e}"))?;
            // §3.3: both values are constants under version 2, and both change
            // how every offset in the file is read.
            if !m.text.tokenizer.is_empty() && m.text.tokenizer != TOKENIZER {
                return Err(format!(
                    "this .tbook is tokenized with {:?}, this app implements {TOKENIZER:?}",
                    m.text.tokenizer
                ));
            }
            if !m.text.offsets.is_empty() && m.text.offsets != "codepoint" {
                return Err(format!(
                    "this .tbook measures offsets in {:?}, this app implements code points",
                    m.text.offsets
                ));
            }
            let unknown = unknown_features(&m.requires);
            if !unknown.is_empty() {
                return Err(format!(
                    "this .tbook requires features this app does not implement: {}",
                    unknown.join(", ")
                ));
            }
            Ok(Book::V2(m))
        }
        other => Err(format!(
            "unsupported .tbook format version {other} (this app reads versions {})",
            SUPPORTED_FORMAT_VERSIONS
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(" and ")
        )),
    }
}

/// Parse just `manifest.json`, in whichever version it is written.
fn book_of(path: &Path) -> Result<Book, String> {
    let mut zip = open_archive(path)?;
    let bytes = read_entry_bytes(&mut zip, "manifest.json")?;
    parse_manifest(&bytes)
}

/// Parse just `manifest.json` (used for listing / validation), normalized.
pub fn manifest_of(path: &Path) -> Result<Manifest, String> {
    Ok(book_of(path)?.normalized())
}

/// Manifest plus per-chapter compressed sizes (in manifest order). For a
/// version-2 book that is the skeleton's size — the source text, which is what
/// reading progress should be weighted by.
pub fn opened_book(path: &Path) -> Result<OpenedBook, String> {
    let mut zip = open_archive(path)?;
    let bytes = read_entry_bytes(&mut zip, "manifest.json")?;
    let book = parse_manifest(&bytes)?;
    if let Book::V2(m) = &book {
        check_referenced(&zip, m)?;
    }
    let manifest = book.normalized();
    let mut chapter_sizes = Vec::with_capacity(manifest.chapters.len());
    for c in &manifest.chapters {
        let size = zip
            .by_name(&c.file)
            .map(|e| e.compressed_size())
            .unwrap_or(0);
        chapter_sizes.push(size);
    }
    Ok(OpenedBook {
        manifest,
        chapter_sizes,
    })
}

/// Decode a chapter entry as raw JSON, passed through verbatim to the WebView.
pub fn read_chapter_value(path: &Path, file: &str) -> Result<serde_json::Value, String> {
    let mut zip = open_archive(path)?;
    let bytes = read_entry_bytes(&mut zip, file)?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse chapter {file}: {e}"))
}

/// A version-2 chapter skeleton as raw JSON (spec §4). The WebView tokenizes
/// and renders it; here we only resolve the entry through the manifest.
pub fn read_skeleton_value(path: &Path, index: usize) -> Result<serde_json::Value, String> {
    let mut b = V2Book::open(path)?;
    let entry =
        b.m.spine
            .get(index)
            .map(|c| c.text.clone())
            .ok_or_else(|| format!("chapter {index} is not in the spine"))?;
    let bytes = b.read(&entry)?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse skeleton {entry}: {e}"))
}

/// One version-2 overlay as raw JSON (spec §6), bound against its skeleton
/// (§6.1). `Ok(None)` means the chapter has no overlay in that language — a
/// partial conversion, which §3.4 allows; an `Err` means the overlay exists but
/// does not bind, and the chapter must then be shown without it.
pub fn read_overlay_value(
    path: &Path,
    index: usize,
    lang: &str,
) -> Result<Option<serde_json::Value>, String> {
    let mut b = V2Book::open(path)?;
    let chapter =
        b.m.spine
            .get(index)
            .cloned()
            .ok_or_else(|| format!("chapter {index} is not in the spine"))?;
    let Some(entry) = chapter.gloss.get(lang) else {
        return Ok(None);
    };
    let skeleton: SkeletonV2 = {
        let bytes = b.read(&chapter.text)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse skeleton {}: {e}", chapter.text))?
    };
    // A digest mismatch (§9.1) rejects this overlay like a binding failure.
    let bytes = b.read(entry)?;
    // Parsed once: the typed view checks the binding, the value travels on.
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("parse overlay {entry}: {e}"))?;
    let overlay =
        OverlayV2::deserialize(&value).map_err(|e| format!("parse overlay {entry}: {e}"))?;
    check_binding(&skeleton, &overlay, lang).map_err(|e| format!("{entry}: {e}"))?;
    Ok(Some(value))
}

/// The binding rules of v2 spec §6.1: language, chapter id, paragraph count,
/// paragraph ids and sentence counts (rows and cells for a table). A failure
/// rejects the whole overlay — never a partial gloss. `lang` is required, so an
/// empty or missing one never matches.
pub fn check_binding(skel: &SkeletonV2, ov: &OverlayV2, lang: &str) -> Result<(), String> {
    if ov.lang != lang {
        return Err(format!(
            "overlay language {:?} is not the requested {lang:?}",
            ov.lang
        ));
    }
    if ov.chapter != skel.id {
        return Err(format!(
            "overlay binds chapter {:?}, skeleton is {:?}",
            ov.chapter, skel.id
        ));
    }
    check_paragraphs(&skel.paragraphs, &ov.paragraphs, true)
}

/// §6.1 rules 2, 4 and 6, plus rule 3 (paragraph ids) when `check_ids`.
fn check_paragraphs(
    blocks: &[BlockV2],
    gloss: &[GlossParagraphV2],
    check_ids: bool,
) -> Result<(), String> {
    if gloss.len() != blocks.len() {
        return Err(format!(
            "overlay has {} paragraphs, skeleton has {}",
            gloss.len(),
            blocks.len()
        ));
    }
    for (p, (sk, gl)) in blocks.iter().zip(gloss).enumerate() {
        if check_ids && gl.id != sk.id {
            return Err(format!(
                "paragraph {p}: overlay id {:?}, skeleton id {:?}",
                gl.id, sk.id
            ));
        }
        match (&sk.table, &gl.rows) {
            (Some(table), Some(rows)) => {
                if table.rows.len() != rows.len() {
                    return Err(format!(
                        "paragraph {p}: overlay has {} rows, skeleton has {}",
                        rows.len(),
                        table.rows.len()
                    ));
                }
                for (r, (srow, grow)) in table.rows.iter().zip(rows).enumerate() {
                    if srow.len() != grow.len() {
                        return Err(format!(
                            "paragraph {p} row {r}: overlay has {} cells, skeleton has {}",
                            grow.len(),
                            srow.len()
                        ));
                    }
                    for (c, (scell, gcell)) in srow.iter().zip(grow).enumerate() {
                        if scell.sents.len() != gcell.s.len() {
                            return Err(format!(
                                "paragraph {p} cell {r}/{c}: overlay has {} translations, skeleton has {} sentences",
                                gcell.s.len(),
                                scell.sents.len()
                            ));
                        }
                    }
                }
            }
            (Some(_), None) => {
                return Err(format!(
                    "paragraph {p}: skeleton is a table, overlay is not"
                ))
            }
            (None, Some(_)) => {
                return Err(format!(
                    "paragraph {p}: overlay is a table, skeleton is not"
                ))
            }
            (None, None) => {
                let got = gl.s.as_ref().map_or(0, |s| s.len());
                if got != sk.sents.len() {
                    return Err(format!(
                        "paragraph {p}: overlay has {got} translations, skeleton has {} sentences",
                        sk.sents.len()
                    ));
                }
            }
        }
    }
    Ok(())
}

/// A stored locator `chapterId/paragraphId/sentence/word` (v2 spec §3.5.2), or
/// a shorter prefix; `None` when the string is not one.
pub fn parse_locator(s: &str) -> Option<(String, Option<String>, Option<usize>, Option<usize>)> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.is_empty() || parts.len() > 4 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let num = |i: usize| -> Option<Option<usize>> {
        match parts.get(i) {
            None => Some(None),
            Some(p) => p.parse::<usize>().ok().map(Some),
        }
    };
    Some((
        parts[0].to_string(),
        parts.get(1).map(|p| p.to_string()),
        num(2)?,
        num(3)?,
    ))
}

/// Resolve a stored locator (v2 spec §3.5.2, normative): find the chapter by
/// its content-derived id, then the paragraph by its own id. When the chapter
/// id is gone — a re-conversion whose title or opening paragraphs changed —
/// the paragraph id is searched **across all chapters** and the first match in
/// spine order wins; paragraph ids survive re-conversion far more often, which
/// is why that fallback is mandatory. A known chapter whose paragraph is gone
/// degrades to that chapter's start, and a locator nothing matches to the book
/// start — never to a wrong word. Sentence and word travel back only with a
/// resolved paragraph; the WebView checks them against its tokenization.
pub fn resolve_locator(path: &Path, locator: &str) -> Result<Locator, String> {
    let book_start = Locator {
        chapter: 0,
        paragraph: None,
        sentence: None,
        word: None,
        exact: false,
    };
    let Some((chapter_id, paragraph_id, sentence, word)) = parse_locator(locator) else {
        return Ok(book_start);
    };
    let mut b = V2Book::open(path)?;
    let spine = b.m.spine.clone();
    let mut read = |entry: &str| -> Option<SkeletonV2> {
        let bytes = b.read(entry).ok()?;
        serde_json::from_slice(&bytes).ok()
    };
    let at = |chapter: usize, paragraph: usize, exact: bool| Locator {
        chapter,
        paragraph: Some(paragraph),
        sentence,
        word: sentence.and(word),
        exact,
    };
    if let Some(i) = spine.iter().position(|c| c.id == chapter_id) {
        let chapter_start = Locator {
            chapter: i,
            paragraph: None,
            sentence: None,
            word: None,
            exact: paragraph_id.is_none(),
        };
        let Some(pid) = paragraph_id else {
            return Ok(chapter_start);
        };
        let found =
            read(&spine[i].text).and_then(|s| s.paragraphs.iter().position(|p| p.id == pid));
        return Ok(found.map_or(chapter_start, |p| at(i, p, true)));
    }
    let Some(pid) = paragraph_id else {
        return Ok(book_start);
    };
    for (i, c) in spine.iter().enumerate() {
        let Some(skeleton) = read(&c.text) else {
            continue;
        };
        if let Some(p) = skeleton.paragraphs.iter().position(|p| p.id == pid) {
            return Ok(at(i, p, false));
        }
    }
    Ok(book_start)
}

/// Every paragraph's text of a version-2 skeleton, for full-book search.
/// Version 1 joins a paragraph's sentences with one space; version 2 stores the
/// paragraph text itself (§4.5).
pub fn paragraph_texts(path: &Path, chapter_file: &str) -> Result<Vec<String>, String> {
    let bytes = V2Book::open(path)?.read(chapter_file)?;
    let skeleton: SkeletonV2 = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse skeleton {chapter_file}: {e}"))?;
    Ok(skeleton.paragraphs.iter().map(block_text).collect())
}

/// A block's searchable text: its own text, or a table's cells joined.
fn block_text(b: &BlockV2) -> String {
    if let Some(table) = &b.table {
        let cells: Vec<&str> = table
            .rows
            .iter()
            .flat_map(|r| r.iter().map(|c| c.text.as_str()))
            .filter(|t| !t.is_empty())
            .collect();
        if !cells.is_empty() {
            return cells.join(" ");
        }
    }
    b.text.clone()
}

/// Read the bytes of an arbitrary entry (the cover, figure images). A
/// version-2 entry is checked against its digest (§9.1).
pub fn read_entry(path: &Path, name: &str) -> Result<Vec<u8>, String> {
    let mut zip = open_archive(path)?;
    let bytes = read_entry_bytes(&mut zip, name)?;
    let manifest = read_entry_bytes(&mut zip, "manifest.json")?;
    if let Ok(Book::V2(m)) = parse_manifest(&manifest) {
        verify_digest(&m, name, &bytes)?;
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// Version-2 integrity (spec §9.1) and footnotes (§4.12, §6.12)
// ---------------------------------------------------------------------------

/// `"sha256:<hex>"` of an entry's uncompressed bytes (§9.1).
pub fn entry_digest(bytes: &[u8]) -> String {
    let sum = Sha256::digest(bytes);
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for b in sum.iter() {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Check `bytes` of `name` against `manifest.digests`. An entry without a
/// digest is read unverified with a warning, never refused (§9.1
/// digest-missing). `Ok(false)` = not verified.
fn verify_digest(m: &ManifestV2, name: &str, bytes: &[u8]) -> Result<bool, String> {
    match m.digests.get(name) {
        Some(want) if !want.eq_ignore_ascii_case(&entry_digest(bytes)) => Err(format!(
            "this .tbook is damaged: entry {name:?} does not match its digest"
        )),
        Some(_) => Ok(true),
        None => {
            eprintln!(
                "tbook: warning: entry {name:?} has no digest in the manifest; read unverified"
            );
            Ok(false)
        }
    }
}

/// Every entry the manifest names: skeletons, overlays, footnote entries, the
/// cover, the shipped schema and every key of `digests`.
fn referenced_entries(m: &ManifestV2) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for c in &m.spine {
        if !c.text.is_empty() {
            out.insert(c.text.clone());
        }
        out.extend(c.gloss.values().cloned());
    }
    if let Some(t) = &m.notes.text {
        out.insert(t.clone());
    }
    out.extend(m.notes.gloss.values().cloned());
    if let Some(c) = m.cover.as_ref().filter(|c| !c.is_empty()) {
        out.insert(c.clone());
    }
    if !m.schema.is_empty() {
        out.insert(m.schema.clone());
    }
    out.extend(m.digests.keys().cloned());
    out
}

/// §9.1: a consumer MUST refuse a file missing a referenced entry. Checked on
/// the central directory only — no entry is decompressed.
fn check_referenced(zip: &ZipArchive<File>, m: &ManifestV2) -> Result<(), String> {
    let names: BTreeSet<&str> = zip.file_names().collect();
    match referenced_entries(m)
        .into_iter()
        .find(|e| !names.contains(e.as_str()))
    {
        Some(missing) => Err(format!(
            "this .tbook is damaged: entry {missing:?} is missing"
        )),
        None => Ok(()),
    }
}

/// An open version-2 archive whose every read is digest-checked.
struct V2Book {
    zip: ZipArchive<File>,
    m: ManifestV2,
}

impl V2Book {
    fn open(path: &Path) -> Result<Self, String> {
        let mut zip = open_archive(path)?;
        let bytes = read_entry_bytes(&mut zip, "manifest.json")?;
        let m = match parse_manifest(&bytes)? {
            Book::V2(m) => m,
            Book::V1(_) => return Err("not a version-2 .tbook".to_string()),
        };
        check_referenced(&zip, &m)?;
        Ok(V2Book { zip, m })
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, String> {
        let bytes = read_entry_bytes(&mut self.zip, name)?;
        verify_digest(&self.m, name, &bytes)?;
        Ok(bytes)
    }
}

/// Verify a whole book on import (§9.1: a consumer SHOULD verify on import):
/// every entry of a version-2 file against its digest. Version 1 carries none.
pub fn verify_book(path: &Path) -> Result<(), String> {
    if !matches!(book_of(path)?, Book::V2(_)) {
        return Ok(());
    }
    let mut b = V2Book::open(path)?;
    let names: Vec<String> = b.m.digests.keys().cloned().collect();
    for name in names {
        b.read(&name)?;
    }
    Ok(())
}

/// A version-2 book's footnote bodies (`manifest.notes.text`, §4.12) as raw
/// JSON, or `None` when it has none.
pub fn read_notes_v2_value(path: &Path) -> Result<Option<serde_json::Value>, String> {
    let mut b = V2Book::open(path)?;
    let Some(entry) = b.m.notes.text.clone() else {
        return Ok(None);
    };
    let bytes = b.read(&entry)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("parse notes {entry}: {e}"))
}

/// A version-2 footnote overlay (§6.12) as raw JSON, bound note by note
/// against the bodies. `Ok(None)` = no footnote overlay in that language; an
/// `Err` = one exists but is damaged or does not bind, and the notes are then
/// shown without it.
pub fn read_notes_overlay_value(
    path: &Path,
    lang: &str,
) -> Result<Option<serde_json::Value>, String> {
    let mut b = V2Book::open(path)?;
    let (Some(text), Some(entry)) = (b.m.notes.text.clone(), b.m.notes.gloss.get(lang).cloned())
    else {
        return Ok(None);
    };
    let notes: BTreeMap<String, NoteV2> =
        serde_json::from_slice(&b.read(&text)?).map_err(|e| format!("parse notes {text}: {e}"))?;
    let value: serde_json::Value = serde_json::from_slice(&b.read(&entry)?)
        .map_err(|e| format!("parse notes overlay {entry}: {e}"))?;
    let overlay = NotesOverlayV2::deserialize(&value)
        .map_err(|e| format!("parse notes overlay {entry}: {e}"))?;
    check_notes_binding(&notes, &overlay, lang).map_err(|e| format!("{entry}: {e}"))?;
    Ok(Some(value))
}

/// §6.12: `lang` equals the entry's key, every note id of the overlay exists
/// in the bodies, and each note's paragraph list binds by position (§6.1 rules
/// 2 and 4; paragraph ids are not compared).
pub fn check_notes_binding(
    notes: &BTreeMap<String, NoteV2>,
    ov: &NotesOverlayV2,
    lang: &str,
) -> Result<(), String> {
    if ov.lang != lang {
        return Err(format!(
            "notes overlay language {:?} is not the requested {lang:?}",
            ov.lang
        ));
    }
    for (id, paragraphs) in &ov.notes {
        let note = notes
            .get(id)
            .ok_or_else(|| format!("note {id:?} is not in the footnote bodies"))?;
        check_paragraphs(&note.paragraphs, paragraphs, false)
            .map_err(|e| format!("note {id:?}: {e}"))?;
    }
    Ok(())
}

/// Decode the footnote-bodies entry (named by `manifest.notes`) as raw JSON,
/// passed through verbatim to the WebView.
pub fn read_notes_value(path: &Path, entry: &str) -> Result<serde_json::Value, String> {
    let mut zip = open_archive(path)?;
    let bytes = read_entry_bytes(&mut zip, entry)?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse notes {entry}: {e}"))
}

/// Sniff an image MIME type from magic bytes — entry extensions are nominal
/// (spec §3.3 / §4.2), so we never trust them.
fn sniff_image_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/jpeg"
    }
}

/// An image entry (figure body image) as a base64 `data:` URL for an `<img>`.
pub fn image_data_url(path: &Path, entry: &str) -> Result<String, String> {
    use base64::Engine;
    let bytes = read_entry(path, entry)?;
    let mime = sniff_image_mime(&bytes);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // cargo test runs with CWD = crate root (src-tauri).
    fn sample() -> PathBuf {
        Path::new("resources/sample.tbook").to_path_buf()
    }

    /// A real migrated version-2 book, kept outside the repo (3.5 MB). Tests
    /// that need it skip when it is absent.
    fn sample_v2() -> Option<PathBuf> {
        let p = PathBuf::from(
            "/home/adubovskoy/.claude/projects/-home-adubovskoy-Develop-reader-doc/9771d31c-1b9b-41bf-b2fe-cc8ab066d401/panel/out/v2/sample-v2.tbook",
        );
        p.exists().then_some(p)
    }

    #[test]
    fn parses_bundled_sample() {
        let ob = opened_book(&sample()).expect("open sample");
        assert_eq!(ob.manifest.format_version, 1, "the bundled sample is v1");
        assert!(!ob.manifest.title.is_empty(), "title should be set");
        assert!(!ob.manifest.chapters.is_empty(), "should have chapters");
        assert_eq!(
            ob.chapter_sizes.len(),
            ob.manifest.chapters.len(),
            "one size per chapter"
        );
        assert!(ob.chapter_sizes.iter().all(|&s| s > 0), "sizes nonzero");

        // First chapter decodes and has paragraphs with sentences.
        let first = &ob.manifest.chapters[0];
        let value = read_chapter_value(&sample(), &first.file).expect("read chapter");
        let paras = value
            .get("paragraphs")
            .and_then(|p| p.as_array())
            .expect("paragraphs array");
        assert!(!paras.is_empty(), "chapter should have paragraphs");
    }

    #[test]
    fn refuses_unknown_format_version() {
        let path = write_manifest("v3", br#"{"formatVersion":3,"title":"T","spine":[]}"#);
        let err = manifest_of(&path).expect_err("a v3 manifest must be refused");
        assert!(err.contains("format version 3"), "message names it: {err}");
        let err = opened_book(&path).expect_err("opened_book refuses too");
        assert!(err.contains("format version 3"), "{err}");
        cleanup(&path);
    }

    #[test]
    fn refuses_manifest_without_a_version() {
        let path = write_manifest("nover", br#"{"title":"T","spine":[]}"#);
        let err = manifest_of(&path).expect_err("a missing version must not default");
        assert!(err.contains("formatVersion"), "{err}");
        cleanup(&path);
    }

    #[test]
    fn refuses_unknown_requires_id() {
        let path = write_manifest(
            "req",
            br#"{"formatVersion":2,"requires":["text/paragraph","x-holo/deck"],"title":"T","spine":[]}"#,
        );
        let err = manifest_of(&path).expect_err("an unknown requires id must be refused");
        assert!(err.contains("x-holo/deck"), "message names the id: {err}");
        assert!(
            !err.contains("text/paragraph"),
            "known ids stay quiet: {err}"
        );
        cleanup(&path);
    }

    #[test]
    fn refuses_another_tokenizer() {
        let path = write_manifest(
            "tok",
            br#"{"formatVersion":2,"requires":[],"title":"T","spine":[],"text":{"tokenizer":"tbook-w3","offsets":"codepoint"}}"#,
        );
        let err = manifest_of(&path).expect_err("another tokenizer means other token indices");
        assert!(err.contains("tbook-w3"), "message names it: {err}");
        cleanup(&path);
    }

    #[test]
    fn version_two_manifest_normalizes() {
        let Some(path) = sample_v2() else { return };
        let ob = opened_book(&path).expect("open sample-v2");
        let m = &ob.manifest;
        assert_eq!(m.format_version, 2);
        assert_eq!(m.title, "The Adventures of Sherlock Holmes");
        assert_eq!(m.author, "Arthur Conan Doyle", "authors[] joined");
        assert_eq!(m.source_lang, "en");
        assert!(m.target_langs.contains(&"ru".to_string()));
        assert_eq!(m.chapters.len(), 13, "one entry per spine chapter");
        assert!(
            m.chapters[0].file.starts_with("text/"),
            "chapters[].file is the skeleton entry: {}",
            m.chapters[0].file
        );
        assert!(
            m.chapters[0].id.starts_with('c'),
            "content-derived chapter id"
        );
        assert!(
            m.chapters[0].gloss_langs.contains(&"ru".to_string()),
            "per-chapter gloss languages travel with the spine"
        );
        assert_eq!(ob.chapter_sizes.len(), 13);
        assert!(ob.chapter_sizes.iter().all(|&s| s > 0));
        assert!(
            m.notes.is_none(),
            "v2 footnotes are not handed to the v1 note UI"
        );
    }

    #[test]
    fn reads_skeleton_and_bound_overlay() {
        let Some(path) = sample_v2() else { return };
        let skel = read_skeleton_value(&path, 2).expect("skeleton");
        let id = skel
            .get("id")
            .and_then(|v| v.as_str())
            .expect("skeleton id");
        let paras = skel
            .get("paragraphs")
            .and_then(|p| p.as_array())
            .expect("paragraphs");
        assert!(!paras.is_empty());
        assert!(paras[0].get("text").is_some(), "paragraph carries its text");
        assert!(paras[0].get("sents").is_some(), "and its sentence ranges");

        let ov = read_overlay_value(&path, 2, "ru")
            .expect("overlay reads")
            .expect("chapter 2 has a ru overlay");
        assert_eq!(ov.get("chapter").and_then(|v| v.as_str()), Some(id));
        assert_eq!(ov.get("lang").and_then(|v| v.as_str()), Some("ru"));
        assert_eq!(
            ov.get("paragraphs")
                .and_then(|p| p.as_array())
                .map(Vec::len),
            Some(paras.len()),
            "binding rule 2: one gloss paragraph per skeleton paragraph"
        );

        // A language the book does not carry is absent, not an error (§3.4).
        assert!(read_overlay_value(&path, 2, "zz")
            .expect("no error")
            .is_none());
        // A version-1 book has no skeletons at all.
        assert!(read_skeleton_value(&sample(), 0).is_err());
    }

    #[test]
    fn locators_fall_back_to_the_paragraph_id() {
        let Some(path) = sample_v2() else { return };
        let m = manifest_of(&path).expect("manifest");
        let skel = read_skeleton_value(&path, 4).expect("skeleton");
        let paras = skel.get("paragraphs").and_then(|p| p.as_array()).unwrap();
        let para_id = paras[3].get("id").and_then(|v| v.as_str()).unwrap();
        let chapter_id = &m.chapters[4].id;
        let loc = |s: String| resolve_locator(&path, &s).expect("resolves");

        let hit = loc(format!("{chapter_id}/{para_id}/0/2"));
        assert_eq!(
            hit,
            Locator {
                chapter: 4,
                paragraph: Some(3),
                sentence: Some(0),
                word: Some(2),
                exact: true
            }
        );

        // §3.5.2: an unknown chapter id must not lose the position — the
        // paragraph id is searched across all chapters.
        let hit = loc(format!("c00000000/{para_id}/0/2"));
        assert_eq!((hit.chapter, hit.paragraph, hit.exact), (4, Some(3), false));

        // A paragraph that is gone keeps at least its chapter (its start).
        let hit = loc(format!("{chapter_id}/00000000/0/0"));
        assert_eq!((hit.chapter, hit.paragraph, hit.sentence), (4, None, None));

        // Nothing at all resolves to the book start, never to a wrong word.
        let hit = loc("c00000000/00000000/0/0".to_string());
        assert_eq!((hit.chapter, hit.paragraph), (0, None));
        assert_eq!(loc("not a locator//".to_string()).paragraph, None);
    }

    #[test]
    fn locator_strings_parse() {
        assert_eq!(
            parse_locator("cbedb517c/04537723/0/3"),
            Some((
                "cbedb517c".into(),
                Some("04537723".into()),
                Some(0),
                Some(3)
            ))
        );
        assert_eq!(parse_locator("c1"), Some(("c1".into(), None, None, None)));
        assert_eq!(parse_locator("c1/p1/x"), None);
        assert_eq!(parse_locator("c1//0"), None);
        assert_eq!(parse_locator("a/b/0/0/9"), None);
    }

    #[test]
    fn the_migrated_book_verifies_in_full() {
        let Some(path) = sample_v2() else { return };
        verify_book(&path).expect("every digest of the migrated sample matches");
        verify_book(&sample()).expect("a version-1 book has nothing to verify");
    }

    const V2_MANIFEST: &str = r#"{"formatVersion":2,"requires":["text/paragraph","align/token","tokenizer/tbook-w2"],
        "title":"T","authors":[],"languages":{"source":"en","targets":["ru"]},
        "text":{"tokenizer":"tbook-w2","offsets":"codepoint","normalization":"NFC"},"cover":null,
        "notes":{"text":"text/notes.json","gloss":{"ru":"gloss/notes.ru.json"}},
        "spine":[{"id":"c1","title":"T","text":"text/ch1.json","gloss":{"ru":"gloss/ch1.ru.json"}}],
        "schema":"schema/tbook-2.schema.json","digests":{}}"#;
    const V2_SKELETON: &str = r#"{"id":"c1","paragraphs":[{"id":"p1","text":"The cat sat.","sents":[[0,12]],
        "notes":[{"p":12,"id":"n1","label":"1"}]}]}"#;
    const V2_OVERLAY: &str = r#"{"lang":"ru","chapter":"c1","gates":[],"alignDigest":"sha256:0",
        "paragraphs":[{"id":"p1","s":[{"t":"Кот сидел.","a":[1,2]}]}]}"#;
    const V2_NOTES: &str = r#"{"n1":{"label":"1","kind":"note","paragraphs":[
        {"id":"ea90ea77","text":"The two attendants brought up the rear.","sents":[[0,39]]}]}}"#;
    const V2_NOTES_RU: &str = r#"{"lang":"ru","gates":[],"alignDigest":"sha256:0","notes":{"n1":[
        {"id":"ea90ea77","s":[{"t":"Двое сопровождающих замыкали шествие.","a":[1,[0,2],2]}]}]}}"#;

    /// A sealed version-2 book: every entry digested into the manifest, which
    /// goes last; `tamper` then rewrites entries behind the manifest's back.
    fn write_v2(name: &str, entries: &[(&str, &str)], tamper: &[(&str, &str)]) -> PathBuf {
        let mut all: Vec<(String, String)> = vec![
            ("mimetype".into(), "application/vnd.tbook+zip".into()),
            ("schema/tbook-2.schema.json".into(), "{}".into()),
        ];
        all.extend(entries.iter().map(|(n, b)| (n.to_string(), b.to_string())));
        let digests: Vec<String> = all
            .iter()
            .filter(|(n, _)| n != "mimetype")
            .map(|(n, b)| format!("{n:?}:{:?}", entry_digest(b.as_bytes())))
            .collect();
        let manifest = V2_MANIFEST.replace(
            "\"digests\":{}",
            &format!("\"digests\":{{{}}}", digests.join(",")),
        );
        all.push(("manifest.json".into(), manifest));
        for (n, b) in tamper {
            if let Some(e) = all.iter_mut().find(|(name, _)| name == n) {
                e.1 = b.to_string();
            }
        }
        let dir = std::env::temp_dir().join(format!("tbook-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.tbook"));
        let mut w = zip::ZipWriter::new(File::create(&path).unwrap());
        for (n, b) in &all {
            w.start_file(n.as_str(), zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut w, b.as_bytes()).unwrap();
        }
        w.finish().unwrap();
        path
    }

    fn v2_entries() -> Vec<(&'static str, &'static str)> {
        vec![
            ("text/ch1.json", V2_SKELETON),
            ("gloss/ch1.ru.json", V2_OVERLAY),
            ("text/notes.json", V2_NOTES),
            ("gloss/notes.ru.json", V2_NOTES_RU),
        ]
    }

    #[test]
    fn a_sealed_book_verifies_and_reads() {
        let path = write_v2("sealed", &v2_entries(), &[]);
        verify_book(&path).expect("verifies");
        opened_book(&path).expect("opens");
        assert!(read_overlay_value(&path, 0, "ru").unwrap().is_some());
        cleanup(&path);
    }

    #[test]
    fn a_tampered_entry_is_refused_where_it_is_read() {
        // Annex B.4: the overlay's bytes change, the manifest's digest does not.
        let tampered = V2_OVERLAY.replace("[1,2]", "[2,1]");
        let path = write_v2(
            "tamper-ov",
            &v2_entries(),
            &[("gloss/ch1.ru.json", &tampered)],
        );
        let err = verify_book(&path).expect_err("import refuses it");
        assert!(err.contains("gloss/ch1.ru.json"), "{err}");
        let err = read_overlay_value(&path, 0, "ru").expect_err("the overlay is rejected");
        assert!(err.contains("digest"), "{err}");
        // The skeleton is intact and still reads.
        read_skeleton_value(&path, 0).expect("skeleton reads");
        cleanup(&path);

        let path = write_v2(
            "tamper-sk",
            &v2_entries(),
            &[("text/ch1.json", &V2_SKELETON.replace("cat", "dog"))],
        );
        assert!(
            read_skeleton_value(&path, 0).is_err(),
            "a damaged skeleton refuses the chapter"
        );
        cleanup(&path);
    }

    #[test]
    fn a_missing_referenced_entry_refuses_the_file() {
        let mut entries = v2_entries();
        entries.retain(|(n, _)| *n != "gloss/notes.ru.json");
        let path = write_v2("missing", &entries, &[]);
        let err = opened_book(&path).expect_err("refused on open");
        assert!(err.contains("gloss/notes.ru.json"), "{err}");
        cleanup(&path);
    }

    #[test]
    fn v2_footnotes_read_and_bind() {
        let path = write_v2("notes", &v2_entries(), &[]);
        let notes = read_notes_v2_value(&path).unwrap().expect("bodies");
        assert!(notes.get("n1").is_some());
        let ov = read_notes_overlay_value(&path, "ru")
            .unwrap()
            .expect("ru notes");
        assert_eq!(ov.get("lang").and_then(|v| v.as_str()), Some("ru"));
        assert!(
            read_notes_overlay_value(&path, "de").unwrap().is_none(),
            "no de notes"
        );
        cleanup(&path);

        // A note id the bodies do not have rejects the whole overlay (§6.12).
        let bad = V2_NOTES_RU.replace("\"n1\"", "\"n9\"");
        let notes: BTreeMap<String, NoteV2> = serde_json::from_str(V2_NOTES).unwrap();
        let ov: NotesOverlayV2 = serde_json::from_str(&bad).unwrap();
        let err = check_notes_binding(&notes, &ov, "ru").expect_err("unknown note");
        assert!(err.contains("n9"), "{err}");
        // A paragraph too many in a note binds no better than in a chapter.
        let two = V2_NOTES_RU.replace("]}]}}", "]},{\"id\":\"x\",\"s\":[]}]}}");
        let ov: NotesOverlayV2 = serde_json::from_str(&two).unwrap();
        assert!(check_notes_binding(&notes, &ov, "ru").is_err());
        let ov: NotesOverlayV2 = serde_json::from_str(V2_NOTES_RU).unwrap();
        assert!(
            check_notes_binding(&notes, &ov, "de").is_err(),
            "language must match"
        );
    }

    #[test]
    fn note_paragraphs_bind_by_position_not_by_id() {
        // §6.12 / G6: rule 3 does not apply to note paragraphs.
        let notes: BTreeMap<String, NoteV2> = serde_json::from_str(V2_NOTES).unwrap();
        let other_id = V2_NOTES_RU.replace("ea90ea77", "00000000");
        let ov: NotesOverlayV2 = serde_json::from_str(&other_id).unwrap();
        check_notes_binding(&notes, &ov, "ru").expect("a differing paragraph id still binds");
        // Rule 4 still applies: a sentence too many.
        let extra = other_id.replace("]}]}}", ",null]}]}}");
        let ov: NotesOverlayV2 = serde_json::from_str(&extra).unwrap();
        assert!(check_notes_binding(&notes, &ov, "ru").is_err());
    }

    #[test]
    fn an_empty_or_missing_lang_is_a_binding_error() {
        // Decision 17 / G7: `lang` is required and must equal the entry's key.
        let skel = skeleton("c1", &[("p1", 1)]);
        let err =
            check_binding(&skel, &overlay("", "c1", &[("p1", 1)]), "ru").expect_err("empty lang");
        assert!(err.contains("ru"), "{err}");
        let mut no_lang: serde_json::Value = serde_json::from_str(V2_OVERLAY).unwrap();
        no_lang.as_object_mut().unwrap().remove("lang");
        let ov = OverlayV2::deserialize(&no_lang).unwrap();
        assert!(check_binding(&skel, &ov, "ru").is_err(), "missing lang");

        let notes: BTreeMap<String, NoteV2> = serde_json::from_str(V2_NOTES).unwrap();
        for bad in [
            V2_NOTES_RU.replace("\"lang\":\"ru\"", "\"lang\":\"\""),
            V2_NOTES_RU.replace("\"lang\":\"ru\",", ""),
        ] {
            let ov: NotesOverlayV2 = serde_json::from_str(&bad).unwrap();
            assert!(check_notes_binding(&notes, &ov, "ru").is_err(), "{bad}");
        }
    }

    #[test]
    fn an_entry_without_a_digest_is_read_unverified() {
        // §9.1 digest-missing / G2: warned, never refused.
        let mut m: ManifestV2 = serde_json::from_str(V2_MANIFEST).unwrap();
        assert_eq!(verify_digest(&m, "text/ch1.json", b"{}"), Ok(false));
        m.digests
            .insert("text/ch1.json".into(), entry_digest(b"{}"));
        assert_eq!(verify_digest(&m, "text/ch1.json", b"{}"), Ok(true));
        assert!(verify_digest(&m, "text/ch1.json", b"[]").is_err());
    }

    #[test]
    fn damaged_footnote_entries_are_refused_alone() {
        // §9.1 digest-lazy / G1: a damaged notes.gloss hides only its
        // translations; damaged notes.text makes footnotes unavailable.
        let path = write_v2(
            "tamper-notes-ru",
            &v2_entries(),
            &[("gloss/notes.ru.json", &V2_NOTES_RU.replace("Двое", "Трое"))],
        );
        read_notes_v2_value(&path)
            .unwrap()
            .expect("footnotes stay readable");
        let err = read_notes_overlay_value(&path, "ru").expect_err("overlay refused");
        assert!(err.contains("digest"), "{err}");
        assert!(read_overlay_value(&path, 0, "ru").unwrap().is_some());
        cleanup(&path);

        let path = write_v2(
            "tamper-notes",
            &v2_entries(),
            &[("text/notes.json", &V2_NOTES.replace("two", "six"))],
        );
        assert!(read_notes_v2_value(&path).is_err(), "footnotes unavailable");
        read_skeleton_value(&path, 0).expect("chapters still read");
        assert!(read_overlay_value(&path, 0, "ru").unwrap().is_some());
        cleanup(&path);
    }

    #[test]
    fn paragraph_texts_of_a_v2_skeleton() {
        let Some(path) = sample_v2() else { return };
        let m = manifest_of(&path).expect("manifest");
        let texts = paragraph_texts(&path, &m.chapters[2].file).expect("texts");
        assert!(!texts.is_empty());
        assert!(
            texts.iter().any(|t| t.contains("Holmes")),
            "paragraph text is the block text, verbatim"
        );
    }

    fn skeleton(id: &str, paras: &[(&str, usize)]) -> SkeletonV2 {
        let json = serde_json::json!({
            "id": id,
            "paragraphs": paras.iter().map(|(pid, n)| serde_json::json!({
                "id": pid,
                "text": "x",
                "sents": (0..*n).map(|i| [i as i64, i as i64 + 1]).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
        serde_json::from_value(json).unwrap()
    }

    fn overlay(lang: &str, chapter: &str, paras: &[(&str, usize)]) -> OverlayV2 {
        let json = serde_json::json!({
            "lang": lang,
            "chapter": chapter,
            "gates": [],
            "paragraphs": paras.iter().map(|(pid, n)| serde_json::json!({
                "id": pid,
                "s": (0..*n).map(|_| serde_json::json!({"t": "t"})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn binding_rules_reject_a_mis_bound_overlay() {
        let skel = skeleton("c1", &[("p1", 2), ("p2", 1)]);
        let ok = overlay("ru", "c1", &[("p1", 2), ("p2", 1)]);
        check_binding(&skel, &ok, "ru").expect("a matching overlay binds");

        // Rule 1: another chapter's overlay (tamper test B.4.2).
        let err = check_binding(&skel, &overlay("ru", "c9", &[("p1", 2), ("p2", 1)]), "ru")
            .expect_err("chapter id mismatch");
        assert!(err.contains("c9"), "{err}");
        // Rule 2: a paragraph too few.
        assert!(check_binding(&skel, &overlay("ru", "c1", &[("p1", 2)]), "ru").is_err());
        // Rule 3: shifted paragraphs (tamper test B.4.3).
        let err = check_binding(&skel, &overlay("ru", "c1", &[("p2", 1), ("p2", 1)]), "ru")
            .expect_err("paragraph id mismatch");
        assert!(err.contains("paragraph 0"), "{err}");
        // Rule 4: a sentence count that does not match.
        assert!(check_binding(&skel, &overlay("ru", "c1", &[("p1", 3), ("p2", 1)]), "ru").is_err());
        // The overlay's own language must be the one asked for (B.4.1).
        let err = check_binding(&skel, &ok, "tr").expect_err("language mismatch");
        assert!(err.contains("tr"), "{err}");
    }

    #[test]
    fn image_mime_sniffed_from_bytes() {
        let manifest = manifest_of(&sample()).expect("manifest");
        let cover = manifest.cover.expect("sample has a cover");
        let url = image_data_url(&sample(), &cover).expect("cover data url");
        assert!(
            url.starts_with("data:image/jpeg;base64,"),
            "MIME comes from magic bytes, not the entry extension"
        );
    }

    /// A one-entry .tbook holding just this manifest, in a temp dir.
    fn write_manifest(name: &str, body: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tbook-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.tbook"));
        let f = File::create(&path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        w.start_file("manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut w, body).unwrap();
        w.finish().unwrap();
        path
    }

    fn cleanup(path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
