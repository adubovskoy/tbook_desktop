//! Full-book text extraction for search.
//!
//! The actual case-insensitive matching runs in the frontend (TS port of
//! `BookSearch.searchParagraphs`) so the match offsets share one coordinate
//! system (JS UTF-16) with the renderer's highlight ranges. Here we just read
//! every chapter and produce each paragraph's *joined* text — sentences joined
//! by a single space, exactly like `buildParagraphRender`.

use std::path::Path;

use crate::models::{ChapterTexts, Manifest};
use crate::tbook;

/// `[chapter][paragraph] -> joined source text`.
pub fn book_texts(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let manifest = tbook::manifest_of(path)?;
    let mut out: Vec<Vec<String>> = Vec::with_capacity(manifest.chapters.len());
    read_into(path, &manifest, &mut out)?;
    Ok(out)
}

fn read_into(path: &Path, manifest: &Manifest, out: &mut Vec<Vec<String>>) -> Result<(), String> {
    for c in &manifest.chapters {
        let value = tbook::read_chapter_value(path, &c.file)?;
        let chapter: ChapterTexts =
            serde_json::from_value(value).map_err(|e| format!("parse chapter {}: {e}", c.file))?;
        let paras = chapter
            .paragraphs
            .iter()
            .map(|p| {
                p.iter()
                    .map(|s| s.src.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        out.push(paras);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn extracts_joined_paragraph_texts() {
        let texts = book_texts(Path::new("resources/sample.tbook")).expect("book_texts");
        assert!(!texts.is_empty(), "should have chapters");
        let total: usize = texts.iter().map(|c| c.len()).sum();
        assert!(total > 0, "should have paragraphs");
        // The opening line should be findable in chapter 0.
        let joined = texts[0].join("\n");
        assert!(joined.contains("Sherlock"), "expected 'Sherlock' in chapter 1");
    }
}
