//! Streams `.tdict.gz` files from the dictionary server into the registry.
//!
//! Port of `data/dict/DictionaryDownloader.kt`: download to a staging file
//! (hashing on the fly) → verify sha256 → gunzip → sanity-check the SQLite →
//! atomic commit. Progress is emitted as `dict-progress` events, so leaving the
//! Dictionaries screen doesn't interrupt a 40 MB download — the work runs on its
//! own thread, app-scoped by construction. A killed process simply loses the
//! download; staging files are swept when the registry is next created.

use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use flate2::read::GzDecoder;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use super::models::{InstalledDict, RemoteDict};
use super::{Dictionaries, STAGING_PREFIX};

/// Event name the Dictionaries screen listens on.
pub const PROGRESS_EVENT: &str = "dict-progress";

/// Emitted often enough to animate a progress bar, rarely enough not to flood
/// the WebView bridge (Android throttles the same way, for e-ink's sake).
const PROGRESS_STEP: u64 = 256 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub id: String,
    /// "downloading", "installing", "failed", "done" or "cancelled".
    pub phase: &'static str,
    pub bytes_read: u64,
    pub total_bytes: u64,
    pub message: String,
}

impl Progress {
    fn downloading(id: &str, bytes_read: u64, total_bytes: u64) -> Self {
        Progress {
            id: id.to_string(),
            phase: "downloading",
            bytes_read,
            total_bytes,
            message: String::new(),
        }
    }

    fn phase(id: &str, phase: &'static str) -> Self {
        Progress {
            id: id.to_string(),
            phase,
            bytes_read: 0,
            total_bytes: 0,
            message: String::new(),
        }
    }

    fn failed(id: &str, message: impl Into<String>) -> Self {
        Progress {
            id: id.to_string(),
            phase: "failed",
            bytes_read: 0,
            total_bytes: 0,
            message: message.into(),
        }
    }
}

/// Where progress goes. The app emits Tauri events; tests collect.
type Emit<'a> = dyn Fn(Progress) + 'a;

/// Download, verify and install `dict`, reporting progress as `dict-progress`
/// events. Runs to completion on the calling thread;
/// [`Dictionaries::start_download`] gives it one of its own.
pub fn run(app: &AppHandle, dicts: &Dictionaries, dict: &RemoteDict, cancel: &Arc<AtomicBool>) {
    let emit = |progress: Progress| {
        let _ = app.emit(PROGRESS_EVENT, progress);
    };
    install(&emit, dicts, dict, cancel);
}

/// The whole download, minus the Tauri event plumbing.
fn install(emit: &Emit, dicts: &Dictionaries, dict: &RemoteDict, cancel: &Arc<AtomicBool>) {
    let gz = dicts.dir().join(format!("{STAGING_PREFIX}{}.gz", dict.id));
    let sqlite = dicts.dir().join(format!("{STAGING_PREFIX}{}.sqlite", dict.id));
    let outcome = download(emit, dicts, dict, cancel, &gz, &sqlite);
    let _ = std::fs::remove_file(&gz);
    let _ = std::fs::remove_file(&sqlite);
    match outcome {
        Ok(()) => emit(Progress::phase(&dict.id, "done")),
        Err(Abort::Cancelled) => emit(Progress::phase(&dict.id, "cancelled")),
        Err(Abort::Failed(message)) => emit(Progress::failed(&dict.id, message)),
    }
}

enum Abort {
    Cancelled,
    Failed(String),
}

impl From<String> for Abort {
    fn from(message: String) -> Self {
        Abort::Failed(message)
    }
}

fn download(
    emit: &Emit,
    dicts: &Dictionaries,
    dict: &RemoteDict,
    cancel: &Arc<AtomicBool>,
    gz: &Path,
    sqlite: &Path,
) -> Result<(), Abort> {
    let total = if dict.download_size_bytes > 0 {
        dict.download_size_bytes
    } else {
        dict.size_bytes
    };
    emit(Progress::downloading(&dict.id, 0, total));

    // --- transfer, hashing as it goes ---
    let mut response = super::manifest::agent()
        .get(&dict.url)
        .call()
        .map_err(|e| Abort::Failed(format!("Download failed: {e}")))?;
    if response.status() != 200 {
        return Err(Abort::Failed(format!(
            "Download failed (HTTP {}).",
            response.status()
        )));
    }
    {
        let body = response.body_mut();
        let mut reader = body.as_reader();
        let mut out = std::fs::File::create(gz)
            .map_err(|e| Abort::Failed(format!("Can't write to the dictionary folder: {e}")))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut read_total: u64 = 0;
        let mut last_emitted: u64 = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Abort::Cancelled);
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| Abort::Failed(format!("Download failed: {e}")))?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut out, &buf[..n])
                .map_err(|e| Abort::Failed(format!("Can't write the download: {e}")))?;
            hasher.update(&buf[..n]);
            read_total += n as u64;
            if read_total - last_emitted >= PROGRESS_STEP {
                last_emitted = read_total;
                emit(Progress::downloading(
                    &dict.id,
                    read_total,
                    total.max(read_total),
                ));
            }
        }
        let hex = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if !hex.eq_ignore_ascii_case(dict.sha256.trim()) {
            return Err(Abort::Failed("Download corrupted — try again.".into()));
        }
    }

    // --- unpack, check, install ---
    emit(Progress::phase(&dict.id, "installing"));
    let packed = std::fs::File::open(gz).map_err(|e| Abort::Failed(format!("Install failed: {e}")))?;
    let mut decoder = GzDecoder::new(std::io::BufReader::new(packed));
    let mut out = std::fs::File::create(sqlite)
        .map_err(|e| Abort::Failed(format!("Install failed: {e}")))?;
    std::io::copy(&mut decoder, &mut out)
        .map_err(|_| Abort::Failed("The downloaded file isn't a dictionary.".to_string()))?;
    drop(out);
    let _ = std::fs::remove_file(gz);
    if cancel.load(Ordering::Relaxed) {
        return Err(Abort::Cancelled);
    }
    super::store::validate(sqlite, &dict.id).map_err(Abort::Failed)?;

    dicts
        .commit(
            sqlite,
            InstalledDict {
                id: dict.id.clone(),
                source: dict.source.clone(),
                target: dict.target.clone(),
                name: dict.name.clone(),
                version: dict.version,
                size_bytes: dict.size_bytes,
                license: dict.license.clone(),
                attribution: dict.attribution.clone(),
            },
        )
        .map_err(Abort::Failed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;

    /// Serves `body` once at 127.0.0.1, then stops. Returns the URL.
    fn serve(body: Vec<u8>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/en-ru.tdict.gz", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                // Read just the request head; the body of a GET is empty.
                let mut head = [0u8; 1024];
                let _ = std::io::Read::read(&mut stream, &mut head);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/gzip\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });
        url
    }

    /// A real (tiny) .tdict, gzipped as the server would serve it. The scratch
    /// file is numbered because tests run in parallel in one process.
    fn packed_tdict(pair: &str) -> Vec<u8> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "treader-dl-src-{}-{pair}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&path);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE entry(id INTEGER PRIMARY KEY, lemma TEXT NOT NULL, pos TEXT,
                                gender TEXT, ipa TEXT, senses_json TEXT NOT NULL);
             CREATE TABLE form(form TEXT NOT NULL, entry_id INTEGER NOT NULL);",
        )
        .unwrap();
        conn.execute("INSERT INTO meta VALUES ('format_version','1')", []).unwrap();
        conn.execute("INSERT INTO meta VALUES ('pair', ?1)", [pair]).unwrap();
        conn.execute(
            "INSERT INTO entry VALUES (1,'cat','noun',NULL,NULL,
             '[{\"gloss\":\"animal\",\"translations\":[\"кот\"]}]')",
            [],
        )
        .unwrap();
        drop(conn);
        let raw = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).ok();
        let mut encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&raw).unwrap();
        encoder.finish().unwrap()
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    fn dicts(name: &str) -> (std::path::PathBuf, Dictionaries) {
        let data = std::env::temp_dir().join(format!("treader-dl-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&data).unwrap();
        (data.clone(), Dictionaries::new(&data))
    }

    fn remote(url: String, sha256: String, packed_len: u64) -> RemoteDict {
        RemoteDict {
            id: "en-ru".into(),
            source: "en".into(),
            target: "ru".into(),
            name: "English → Russian".into(),
            version: 5,
            min_format_version: 1,
            size_bytes: 40_960,
            download_size_bytes: packed_len,
            sha256,
            url,
            entry_count: 1,
            license: "CC BY-SA 4.0".into(),
            attribution: "Wiktionary".into(),
        }
    }

    /// Collects the emitted progress so a test can assert on the sequence.
    fn collector() -> (Arc<Mutex<Vec<Progress>>>, impl Fn(Progress)) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        (seen, move |p: Progress| sink.lock().unwrap().push(p))
    }

    #[test]
    fn downloads_verifies_unpacks_and_installs() {
        let (data, store) = dicts("happy");
        let packed = packed_tdict("en-ru");
        let dict = remote(
            serve(packed.clone()),
            sha256_hex(&packed),
            packed.len() as u64,
        );
        let (seen, emit) = collector();

        install(&emit, &store, &dict, &Arc::new(AtomicBool::new(false)));

        let phases: Vec<&str> = seen.lock().unwrap().iter().map(|p| p.phase).collect();
        assert_eq!(phases.first(), Some(&"downloading"));
        assert_eq!(phases.last(), Some(&"done"), "phases were {phases:?}");
        // Installed, and the article is actually readable through the registry.
        let installed = store.installed();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version, 5);
        let entries = store.lookup("en", "ru", "Cat").expect("pair installed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].senses[0].translations, vec!["кот"]);
        // No staging leftovers.
        let staging: Vec<_> = std::fs::read_dir(store.dir())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(STAGING_PREFIX))
            .collect();
        assert!(staging.is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_corrupted_transfer_is_rejected_and_installs_nothing() {
        let (data, store) = dicts("corrupt");
        let packed = packed_tdict("en-ru");
        let dict = remote(
            serve(packed.clone()),
            "00".repeat(32), // the hash the manifest promised, which this isn't
            packed.len() as u64,
        );
        let (seen, emit) = collector();

        install(&emit, &store, &dict, &Arc::new(AtomicBool::new(false)));

        let last = seen.lock().unwrap().last().cloned().unwrap();
        assert_eq!(last.phase, "failed");
        assert!(last.message.contains("corrupted"), "message was {}", last.message);
        assert!(store.installed().is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_dictionary_for_another_pair_is_refused() {
        let (data, store) = dicts("mismatch");
        // The server hands out a de-ru file for the en-ru row.
        let packed = packed_tdict("de-ru");
        let dict = remote(
            serve(packed.clone()),
            sha256_hex(&packed),
            packed.len() as u64,
        );
        let (seen, emit) = collector();

        install(&emit, &store, &dict, &Arc::new(AtomicBool::new(false)));

        let last = seen.lock().unwrap().last().cloned().unwrap();
        assert_eq!(last.phase, "failed");
        assert!(store.installed().is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_cancelled_download_installs_nothing() {
        let (data, store) = dicts("cancel");
        let packed = packed_tdict("en-ru");
        let dict = remote(
            serve(packed.clone()),
            sha256_hex(&packed),
            packed.len() as u64,
        );
        let (seen, emit) = collector();

        // Already cancelled when it starts: the first loop check aborts it.
        install(&emit, &store, &dict, &Arc::new(AtomicBool::new(true)));

        assert_eq!(seen.lock().unwrap().last().unwrap().phase, "cancelled");
        assert!(store.installed().is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    /// The real thing, against dictionary.tbook.dev: catalog → download →
    /// verify → install → look a word up. Ignored by default (it needs the
    /// network and moves a megabyte); run it after touching this module with
    ///
    /// ```text
    /// cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "hits dictionary.tbook.dev"]
    fn installs_a_real_dictionary_from_the_server() {
        let (data, store) = dicts("live");
        let catalog = super::super::manifest::load(
            &store.dir().join("manifest.json"),
            super::super::manifest::MANIFEST_URL,
        );
        assert_eq!(catalog.status, "fresh", "{}", catalog.message);

        // The smallest one on offer keeps the test quick.
        let dict = catalog
            .dictionaries
            .iter()
            .min_by_key(|d| d.download_size_bytes.max(d.size_bytes))
            .expect("the catalog lists dictionaries")
            .clone();
        println!(
            "downloading {} ({:.1} MB packed)",
            dict.id,
            dict.download_size_bytes as f64 / 1e6
        );
        let (seen, emit) = collector();
        install(&emit, &store, &dict, &Arc::new(AtomicBool::new(false)));

        let last = seen.lock().unwrap().last().cloned().unwrap();
        assert_eq!(last.phase, "done", "{}", last.message);
        assert_eq!(store.installed().len(), 1);
        // A word the dictionary certainly has: its own source language's article
        // for a very common word. Any hit proves the SQLite made it through
        // gunzip and validation intact.
        let entries = store
            .lookup(&dict.source, &dict.target, "casa")
            .expect("pair installed");
        println!("{} articles for “casa”", entries.len());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_body_that_is_not_gzip_fails_without_installing() {
        let (data, store) = dicts("notgz");
        let body = b"<html>404 from a proxy</html>".to_vec();
        let dict = remote(serve(body.clone()), sha256_hex(&body), body.len() as u64);
        let (seen, emit) = collector();

        install(&emit, &store, &dict, &Arc::new(AtomicBool::new(false)));

        assert_eq!(seen.lock().unwrap().last().unwrap().phase, "failed");
        assert!(store.installed().is_empty());
        std::fs::remove_dir_all(&data).ok();
    }
}
