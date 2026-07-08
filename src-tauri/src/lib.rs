mod ipa;
mod library;
mod models;
mod search;
mod tbook;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use tauri::path::BaseDirectory;
use tauri::Manager;

use models::{BookSummary, OpenedBook};

/// Process-wide services resolved once at startup.
pub struct AppState {
    books_dir: PathBuf,
    ipa_path: PathBuf,
    ipa: OnceLock<HashMap<String, String>>,
}

impl AppState {
    fn ipa_map(&self) -> &HashMap<String, String> {
        self.ipa.get_or_init(|| ipa::load(&self.ipa_path))
    }
}

#[tauri::command]
fn list_books(state: tauri::State<AppState>) -> Vec<BookSummary> {
    library::list_books(&state.books_dir)
}

#[tauri::command]
fn open_book(state: tauri::State<AppState>, id: String) -> Result<OpenedBook, String> {
    tbook::opened_book(&library::file_for_id(&state.books_dir, &id))
}

#[tauri::command]
fn read_chapter(
    state: tauri::State<AppState>,
    id: String,
    file: String,
) -> Result<serde_json::Value, String> {
    tbook::read_chapter_value(&library::file_for_id(&state.books_dir, &id), &file)
}

#[tauri::command]
fn book_texts(state: tauri::State<AppState>, id: String) -> Result<Vec<Vec<String>>, String> {
    search::book_texts(&library::file_for_id(&state.books_dir, &id))
}

/// Footnote bodies (`notes.json`) as raw JSON, or `None` when the book has none.
#[tauri::command]
fn book_notes(
    state: tauri::State<AppState>,
    id: String,
) -> Result<Option<serde_json::Value>, String> {
    let path = library::file_for_id(&state.books_dir, &id);
    let manifest = tbook::manifest_of(&path)?;
    match manifest.notes {
        Some(entry) if !entry.is_empty() => tbook::read_notes_value(&path, &entry).map(Some),
        _ => Ok(None),
    }
}

/// A body-image entry as a base64 `data:` URL (figures, spec §4.2).
#[tauri::command]
fn book_image(state: tauri::State<AppState>, id: String, entry: String) -> Result<String, String> {
    tbook::image_data_url(&library::file_for_id(&state.books_dir, &id), &entry)
}

#[tauri::command]
fn cover_data_url(state: tauri::State<AppState>, id: String) -> Result<Option<String>, String> {
    library::cover_data_url(&state.books_dir, &id)
}

#[tauri::command]
fn import_book(state: tauri::State<AppState>, path: String) -> Result<BookSummary, String> {
    library::import_book(&state.books_dir, std::path::Path::new(&path))
}

#[tauri::command]
fn delete_book(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    library::delete_book(&state.books_dir, &id)
}

#[tauri::command]
fn ipa_for(state: tauri::State<AppState>, word: String) -> Option<String> {
    ipa::lookup(state.ipa_map(), &word)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let books_dir = data_dir.join("books");
            std::fs::create_dir_all(&books_dir).ok();

            let ipa_path = app
                .path()
                .resolve("resources/en_ipa.tsv", BaseDirectory::Resource)?;

            // First-run: copy the bundled sample into the library. A marker file
            // means deleting the sample later doesn't bring it back.
            let marker = data_dir.join(".sample_installed");
            if !marker.exists() {
                if let Ok(sample) = app
                    .path()
                    .resolve("resources/sample.tbook", BaseDirectory::Resource)
                {
                    let dest = books_dir.join("sample.tbook");
                    if sample.exists() && !dest.exists() {
                        std::fs::copy(&sample, &dest).ok();
                    }
                }
                std::fs::write(&marker, b"1").ok();
            }

            app.manage(AppState {
                books_dir,
                ipa_path,
                ipa: OnceLock::new(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_books,
            open_book,
            read_chapter,
            book_texts,
            book_notes,
            book_image,
            cover_data_url,
            import_book,
            delete_book,
            ipa_for,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
