//! Offline dictionaries (`.tdict`) — registry, lookup and downloads.
//!
//! Port of Android's `data/dict/` package (spec: `../android/docs/tdict-format.md`,
//! producer: `../dictionaries/`). A dictionary is installed as `<id>.sqlite`
//! plus a JSON sidecar `<id>.json` ([`InstalledDict`]) in
//! `<app data>/dictionaries/`; the sidecar is written last, so its presence
//! means the pair is complete — metadata lives and dies atomically with the
//! database file.

pub mod download;
pub mod manifest;
pub mod models;
pub mod normalize;
pub mod store;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::AppHandle;

use models::{base_lang, DictEntry, InstalledDict, RemoteDict};
use store::Store;

/// Prefix marking a file as a download in progress: anything left over belongs
/// to a process that died, and is swept on startup (same idiom as the library).
pub const STAGING_PREFIX: &str = ".staging-";

/// Only `<source>-<target>` ids ever become file names.
fn valid_id(id: &str) -> bool {
    let mut parts = id.split('-');
    let ok = |part: Option<&str>| {
        part.is_some_and(|p| {
            (2..=3).contains(&p.len()) && p.chars().all(|c| c.is_ascii_lowercase())
        })
    };
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

pub struct Dictionaries {
    dir: PathBuf,
    /// Open read-only handles, one per installed dictionary.
    handles: Mutex<HashMap<String, Arc<Store>>>,
    /// Cancel flags of the downloads in flight, keyed by dictionary id.
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Dictionaries {
    pub fn new(data_dir: &Path) -> Self {
        let dir = data_dir.join("dictionaries");
        let _ = std::fs::create_dir_all(&dir);
        // Downloads orphaned by a killed process.
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with(STAGING_PREFIX) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Dictionaries {
            dir,
            handles: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn manifest_cache(&self) -> PathBuf {
        self.dir.join("manifest.json")
    }

    fn sqlite_file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.sqlite"))
    }

    fn sidecar_file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Installed = sidecar present AND database present; strays are ignored.
    ///
    /// Read from disk on each call rather than cached: it is a handful of small
    /// files, and there is then no state to invalidate when a download lands or
    /// a dictionary is deleted.
    pub fn installed(&self) -> Vec<InstalledDict> {
        let mut out: Vec<InstalledDict> = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&path) else { continue };
            let Ok(meta) = serde_json::from_str::<InstalledDict>(&body) else { continue };
            let named_for_itself = path.file_name().and_then(|n| n.to_str())
                == Some(&format!("{}.json", meta.id));
            if named_for_itself && self.sqlite_file(&meta.id).is_file() {
                out.push(meta);
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// The installed dictionary for a language pair, matched on base subtags
    /// ("en-US" reads the "en" dictionary).
    pub fn installed_for(&self, source: &str, target: &str) -> Option<InstalledDict> {
        let (s, t) = (base_lang(source), base_lang(target));
        self.installed()
            .into_iter()
            .find(|d| base_lang(&d.source) == s && base_lang(&d.target) == t)
    }

    /// Dictionary article(s) for a tapped word: candidates from
    /// [`normalize::candidates`] tried in order, first hit wins.
    ///
    /// Returns `None` when no dictionary is installed for the pair, and an empty
    /// list when one is but the word isn't in it — the reader shows a download
    /// hint for the first and "no entry" for the second.
    pub fn lookup(&self, source: &str, target: &str, word: &str) -> Option<Vec<DictEntry>> {
        let dict = self.installed_for(source, target)?;
        let store = self.store_for(&dict.id);
        for candidate in normalize::candidates(word) {
            match store.lookup(&candidate) {
                Ok(entries) if !entries.is_empty() => return Some(entries),
                Ok(_) => {}
                Err(_) => {
                    // A dictionary that can't be read is gone for good: drop it so
                    // the Dictionaries screen offers a fresh download instead of
                    // failing forever.
                    let _ = self.delete(&dict.id);
                    return None;
                }
            }
        }
        Some(Vec::new())
    }

    fn store_for(&self, id: &str) -> Arc<Store> {
        let mut handles = self.handles.lock().expect("dictionary handles poisoned");
        handles
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Store::new(self.sqlite_file(id))))
            .clone()
    }

    /// Install a downloaded-and-validated database: drop any handle on the old
    /// version, move the staged file into place, then write the sidecar that
    /// makes it visible.
    pub fn commit(&self, staged_sqlite: &Path, meta: InstalledDict) -> Result<(), String> {
        self.handles
            .lock()
            .expect("dictionary handles poisoned")
            .remove(&meta.id);
        let dest = self.sqlite_file(&meta.id);
        if std::fs::rename(staged_sqlite, &dest).is_err() {
            // A rename across filesystems fails; a copy always works.
            std::fs::copy(staged_sqlite, &dest).map_err(|e| format!("Install failed: {e}"))?;
            let _ = std::fs::remove_file(staged_sqlite);
        }
        let body = serde_json::to_string(&meta).map_err(|e| format!("Install failed: {e}"))?;
        let staging = self.dir.join(format!("{STAGING_PREFIX}{}.json", meta.id));
        std::fs::write(&staging, body).map_err(|e| format!("Install failed: {e}"))?;
        let sidecar = self.sidecar_file(&meta.id);
        if std::fs::rename(&staging, &sidecar).is_err() {
            std::fs::copy(&staging, &sidecar).map_err(|e| format!("Install failed: {e}"))?;
            let _ = std::fs::remove_file(&staging);
        }
        Ok(())
    }

    /// Remove an installed dictionary (its handle, database, and sidecar).
    pub fn delete(&self, id: &str) -> Result<(), String> {
        if !valid_id(id) {
            return Err("Unknown dictionary.".into());
        }
        self.handles.lock().expect("dictionary handles poisoned").remove(id);
        let _ = std::fs::remove_file(self.sqlite_file(id));
        let _ = std::fs::remove_file(self.sidecar_file(id));
        Ok(())
    }

    /// The catalog, from the network or the offline cache.
    pub fn manifest(&self) -> manifest::ManifestResult {
        manifest::load(&self.manifest_cache(), manifest::MANIFEST_URL)
    }

    /// Start (or restart after a failure) the download of `id`, resolved against
    /// the cached catalog. Progress arrives as `dict-progress` events.
    pub fn start_download(&self, app: &AppHandle, id: &str) -> Result<(), String> {
        if !valid_id(id) {
            return Err("Unknown dictionary.".into());
        }
        let dict: RemoteDict = manifest::cached_dict(&self.manifest_cache(), id)
            .ok_or("That dictionary is no longer listed. Refresh the list and try again.")?;
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut jobs = self.jobs.lock().expect("dictionary jobs poisoned");
            if jobs.contains_key(id) {
                return Ok(()); // already downloading
            }
            jobs.insert(id.to_string(), cancel.clone());
        }
        // The download outlives the Dictionaries screen, so it gets its own
        // thread and re-resolves the service from the app handle.
        let app = app.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            let state = app.state::<crate::AppState>();
            download::run(&app, &state.dictionaries, &dict, &cancel);
            state
                .dictionaries
                .jobs
                .lock()
                .expect("dictionary jobs poisoned")
                .remove(&id);
        });
        Ok(())
    }

    /// Cancel an in-flight download; the row returns to its idle state.
    pub fn cancel_download(&self, id: &str) {
        if let Some(flag) = self.jobs.lock().expect("dictionary jobs poisoned").get(id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

// `tauri::Manager` brings `AppHandle::state` into scope for start_download.
use tauri::Manager;

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dirs(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("treader-dicts-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn meta(id: &str, version: i64) -> InstalledDict {
        let (source, target) = id.split_once('-').unwrap();
        InstalledDict {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            name: format!("{source} → {target}"),
            version,
            size_bytes: 1,
            license: "CC BY-SA 4.0".into(),
            attribution: "Wiktionary".into(),
        }
    }

    #[test]
    fn ids_that_would_escape_the_dictionary_folder_are_rejected() {
        assert!(valid_id("en-ru"));
        assert!(valid_id("ukr-eng"));
        assert!(!valid_id("../etc/passwd"));
        assert!(!valid_id("en_ru"));
        assert!(!valid_id("EN-RU"));
        assert!(!valid_id("en-ru-extra"));
        assert!(!valid_id(""));
    }

    #[test]
    fn only_a_sidecar_plus_database_counts_as_installed() {
        let data = temp_dirs("installed");
        let dicts = Dictionaries::new(&data);
        assert!(dicts.installed().is_empty());

        // Sidecar without a database: a stray, ignored.
        std::fs::write(
            dicts.sidecar_file("en-ru"),
            serde_json::to_string(&meta("en-ru", 1)).unwrap(),
        )
        .unwrap();
        assert!(dicts.installed().is_empty());

        std::fs::write(dicts.sqlite_file("en-ru"), b"db").unwrap();
        let installed = dicts.installed();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, "en-ru");
        // Matched on base subtags, so a book tagged en-US finds it.
        assert!(dicts.installed_for("en-US", "ru").is_some());
        assert!(dicts.installed_for("de", "ru").is_none());

        dicts.delete("en-ru").unwrap();
        assert!(dicts.installed().is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn commit_moves_the_staged_file_and_publishes_the_sidecar_last() {
        let data = temp_dirs("commit");
        let dicts = Dictionaries::new(&data);
        let staged = dicts.dir().join(format!("{STAGING_PREFIX}en-ru.sqlite"));
        std::fs::write(&staged, b"database").unwrap();

        dicts.commit(&staged, meta("en-ru", 3)).unwrap();
        assert!(!staged.exists());
        assert!(dicts.sqlite_file("en-ru").is_file());
        let installed = dicts.installed();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version, 3);
        assert_eq!(installed[0].attribution, "Wiktionary");
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn staging_leftovers_are_swept_on_startup() {
        let data = temp_dirs("sweep");
        let dir = data.join("dictionaries");
        std::fs::create_dir_all(&dir).unwrap();
        let orphan = dir.join(format!("{STAGING_PREFIX}en-ru.gz"));
        std::fs::write(&orphan, b"half a download").unwrap();

        let _dicts = Dictionaries::new(&data);
        assert!(!orphan.exists());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn lookup_without_an_installed_dictionary_is_none_not_empty() {
        let data = temp_dirs("lookup");
        let dicts = Dictionaries::new(&data);
        // None = "no dictionary for this pair" (the download hint);
        // Some(vec![]) would mean "installed, but no such word".
        assert!(dicts.lookup("en", "ru", "cat").is_none());
        std::fs::remove_dir_all(&data).ok();
    }
}
