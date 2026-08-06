mod dict;
mod ipa;
mod library;
mod models;
mod search;
mod tbook;
mod tts;

use std::path::PathBuf;

use tauri::path::BaseDirectory;
use tauri::Manager;

use models::{BookSummary, OpenedBook};

/// Process-wide services resolved once at startup.
pub struct AppState {
    books_dir: PathBuf,
    /// Where the bundled dictionaries and fonts live.
    resource_dir: PathBuf,
    /// English → IPA, one accent resident at a time.
    ipa: ipa::Pronunciations,
    /// Read-aloud for WebViews without the Web Speech API (see `tts`).
    tts: tts::NativeTts,
    /// Installed offline dictionaries, their catalog and their downloads.
    dictionaries: dict::Dictionaries,
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

/// English IPA for a word, in the accent the reader picked. Async so the first
/// call — which loads ~180k entries off disk — doesn't block the UI thread.
#[tauri::command]
async fn ipa_for(
    state: tauri::State<'_, AppState>,
    word: String,
    accent: ipa::Accent,
) -> Result<Option<String>, String> {
    Ok(state.ipa.lookup(&state.resource_dir, accent, &word))
}

/// Whether the system speech CLI can read `lang` aloud. Only consulted when the
/// WebView has no Web Speech API of its own.
#[tauri::command]
fn tts_can_speak(state: tauri::State<AppState>, lang: String) -> bool {
    state.tts.can_speak(&lang)
}

#[tauri::command]
fn tts_speak(state: tauri::State<AppState>, text: String, lang: String) -> Result<(), String> {
    state.tts.speak(&text, &lang)
}

#[tauri::command]
fn tts_stop(state: tauri::State<AppState>) {
    state.tts.stop();
}

/// The downloadable-dictionary catalog, from the network or the offline cache.
#[tauri::command]
async fn dict_manifest(
    state: tauri::State<'_, AppState>,
) -> Result<dict::manifest::ManifestResult, String> {
    // On the async runtime, not the main thread: this one talks to the network.
    Ok(state.dictionaries.manifest())
}

#[tauri::command]
fn dict_installed(state: tauri::State<AppState>) -> Vec<dict::models::InstalledDict> {
    state.dictionaries.installed()
}

#[tauri::command]
fn dict_installed_for(
    state: tauri::State<AppState>,
    source: String,
    target: String,
) -> Option<dict::models::InstalledDict> {
    state.dictionaries.installed_for(&source, &target)
}

/// Start a download; progress arrives as `dict-progress` events.
#[tauri::command]
fn dict_download(app: tauri::AppHandle, state: tauri::State<AppState>, id: String) -> Result<(), String> {
    state.dictionaries.start_download(&app, &id)
}

#[tauri::command]
fn dict_cancel(state: tauri::State<AppState>, id: String) {
    state.dictionaries.cancel_download(&id);
}

#[tauri::command]
fn dict_delete(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    state.dictionaries.delete(&id)
}

/// Article(s) for a tapped word. `None` means no dictionary is installed for the
/// pair; an empty list means one is, but the word isn't in it.
#[tauri::command]
async fn dict_lookup(
    state: tauri::State<'_, AppState>,
    source: String,
    target: String,
    word: String,
) -> Result<Option<Vec<dict::models::DictEntry>>, String> {
    Ok(state.dictionaries.lookup(&source, &target, &word))
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

            let resource_dir = app.path().resolve("resources", BaseDirectory::Resource)?;

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
                resource_dir,
                ipa: ipa::Pronunciations::default(),
                tts: tts::NativeTts::default(),
                dictionaries: dict::Dictionaries::new(&data_dir),
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
            tts_can_speak,
            tts_speak,
            tts_stop,
            dict_manifest,
            dict_installed,
            dict_installed_for,
            dict_download,
            dict_cancel,
            dict_delete,
            dict_lookup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
