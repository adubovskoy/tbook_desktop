//! Book library on disk (port of `BookRepository.kt` / `SampleBookInstaller.kt`).
//!
//! Books are plain files at `<app-data>/books/<id>.tbook`; the id is the file
//! stem. Listing scans the directory and reads each manifest.

use std::fs;
use std::path::{Path, PathBuf};

use crate::models::BookSummary;
use crate::tbook;

/// Absolute path of the book with the given id.
pub fn file_for_id(books_dir: &Path, id: &str) -> PathBuf {
    books_dir.join(format!("{id}.tbook"))
}

fn summary(id: String, m: crate::models::Manifest) -> BookSummary {
    BookSummary {
        id,
        title: m.title,
        author: m.author,
        source_lang: m.source_lang,
        target_langs: m.target_langs,
        has_cover: m.cover.as_deref().map_or(false, |c| !c.is_empty()),
    }
}

/// All books in the library, sorted by title (case-insensitive). Unreadable
/// files are skipped.
pub fn list_books(books_dir: &Path) -> Vec<BookSummary> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(books_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tbook") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(m) = tbook::manifest_of(&path) {
            out.push(summary(id.to_string(), m));
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

fn sanitize_id(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "book".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Validate and copy an external `.tbook` into the library, returning its row.
pub fn import_book(books_dir: &Path, src: &Path) -> Result<BookSummary, String> {
    let manifest = tbook::manifest_of(src)?; // validates it's a real .tbook
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("book");
    let base_id = sanitize_id(stem);
    let mut id = base_id.clone();
    let mut n = 1;
    while file_for_id(books_dir, &id).exists() {
        id = format!("{base_id}-{n}");
        n += 1;
    }
    let dest = file_for_id(books_dir, &id);
    fs::copy(src, &dest).map_err(|e| format!("copy into library: {e}"))?;
    Ok(summary(id, manifest))
}

/// Remove a book file from the library.
pub fn delete_book(books_dir: &Path, id: &str) -> Result<(), String> {
    let path = file_for_id(books_dir, id);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("delete: {e}"))?;
    }
    Ok(())
}

/// Cover image as a `data:` URL for an `<img>` src, or `None` if the book has
/// no cover.
pub fn cover_data_url(books_dir: &Path, id: &str) -> Result<Option<String>, String> {
    use base64::Engine;
    let path = file_for_id(books_dir, id);
    let manifest = tbook::manifest_of(&path)?;
    let cover = match manifest.cover {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(None),
    };
    let bytes = tbook::read_entry(&path, &cover)?;
    let mime = if cover.to_lowercase().ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(format!("data:{mime};base64,{b64}")))
}
