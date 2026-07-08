//! Random-access reader over a `.tbook` ZIP (port of `TbookReader.kt`).
//!
//! A `.tbook` is a ZIP of `manifest.json`, `chapters/chN.json`, and an optional
//! cover image. We open the archive per call (cheap, and chapter loads are
//! infrequent) and read entries by name.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use zip::ZipArchive;

use crate::models::{Manifest, OpenedBook};

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

/// Parse just `manifest.json` (used for listing / validation).
pub fn manifest_of(path: &Path) -> Result<Manifest, String> {
    let mut zip = open_archive(path)?;
    let bytes = read_entry_bytes(&mut zip, "manifest.json")?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse manifest: {e}"))
}

/// Manifest plus per-chapter compressed sizes (in manifest order).
pub fn opened_book(path: &Path) -> Result<OpenedBook, String> {
    let mut zip = open_archive(path)?;
    let bytes = read_entry_bytes(&mut zip, "manifest.json")?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("parse manifest: {e}"))?;
    let mut chapter_sizes = Vec::with_capacity(manifest.chapters.len());
    for c in &manifest.chapters {
        let size = zip.by_name(&c.file).map(|e| e.compressed_size()).unwrap_or(0);
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

/// Read the bytes of an arbitrary entry (used for the cover image).
pub fn read_entry(path: &Path, name: &str) -> Result<Vec<u8>, String> {
    let mut zip = open_archive(path)?;
    read_entry_bytes(&mut zip, name)
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

    #[test]
    fn parses_bundled_sample() {
        let ob = opened_book(&sample()).expect("open sample");
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
}
