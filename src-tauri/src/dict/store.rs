//! One installed `.tdict` dictionary: a read-only SQLite file.
//!
//! Port of `data/dict/DictionaryStore.kt`. Opened lazily on first lookup and
//! kept open — read-only handles are cheap and users hold a handful of
//! dictionaries. SQLite is compiled in (rusqlite `bundled`), so no system
//! library has to be present on the reader's machine.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags};

use super::models::{parse_senses, DictEntry};

/// Mirrors dictc's validation query (tdict §6): direct lemma hit unioned with
/// the inflected-form table, capped to keep the article sheet sane.
const QUERY: &str = "\
SELECT id, lemma, pos, gender, ipa, senses_json FROM entry WHERE lemma = ?1
UNION
SELECT e.id, e.lemma, e.pos, e.gender, e.ipa, e.senses_json
  FROM form f JOIN entry e ON e.id = f.entry_id WHERE f.form = ?1
ORDER BY id LIMIT 8";

pub struct Store {
    path: PathBuf,
    /// `Connection` is `Send` but not `Sync`; the mutex is what lets the store
    /// be shared. Lookups are sub-millisecond, so contention is a non-issue.
    conn: Mutex<Option<Connection>>,
}

impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Store {
            path: path.into(),
            conn: Mutex::new(None),
        }
    }

    /// Open (once) read-only. `SQLITE_OPEN_READ_ONLY` also means a missing file
    /// is an error rather than a freshly created empty database.
    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
        let mut guard = self.conn.lock().map_err(|_| "dictionary lock poisoned".to_string())?;
        if guard.is_none() {
            let conn = Connection::open_with_flags(
                &self.path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|e| format!("Can't open dictionary: {e}"))?;
            *guard = Some(conn);
        }
        f(guard.as_ref().expect("just opened")).map_err(|e| format!("Dictionary read failed: {e}"))
    }

    /// Entries matching `word` exactly, either as a lemma or via the form table
    /// (pre-lowercased inflected forms). Homonyms come back in entry-id order
    /// (Wiktionary's editorial noun/verb/… ordering). Empty when the word isn't
    /// in the dictionary; `Err` when the file can't be read.
    pub fn lookup(&self, word: &str) -> Result<Vec<DictEntry>, String> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare_cached(QUERY)?;
            let rows = stmt.query_map([word], |row| {
                Ok(DictEntry {
                    lemma: row.get::<_, String>(1)?,
                    pos: row.get::<_, Option<String>>(2)?,
                    gender: row.get::<_, Option<String>>(3)?,
                    ipa: row.get::<_, Option<String>>(4)?,
                    senses: parse_senses(&row.get::<_, String>(5)?),
                })
            })?;
            rows.collect()
        })
    }

    /// `meta.format_version`, or 0 when unreadable.
    pub fn format_version(&self) -> u32 {
        self.with_conn(|conn| {
            conn.query_row("SELECT value FROM meta WHERE key='format_version'", [], |r| {
                r.get::<_, String>(0)
            })
        })
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
    }

    /// `meta.pair` ("en-ru"), lowercased; empty when unreadable.
    pub fn pair(&self) -> String {
        self.with_conn(|conn| {
            conn.query_row("SELECT value FROM meta WHERE key='pair'", [], |r| {
                r.get::<_, String>(0)
            })
        })
        .map(|v| v.trim().to_lowercase())
        .unwrap_or_default()
    }
}

/// Validate a freshly downloaded database the way the reader will open it, before
/// letting it into the registry: the format must be one this app understands and
/// the file must be the pair it was downloaded as.
pub fn validate(path: &Path, id: &str) -> Result<(), String> {
    let store = Store::new(path);
    let format = store.format_version();
    if format == 0 {
        return Err("The downloaded file isn't a dictionary.".into());
    }
    if format > super::models::SUPPORTED_TDICT_FORMAT {
        return Err("This dictionary needs a newer version of TReader.".into());
    }
    let pair = store.pair();
    if !pair.is_empty() && pair != id.to_lowercase() {
        return Err("The downloaded dictionary is for a different language pair.".into());
    }
    // A dictionary that parses but holds nothing would look installed and never
    // answer a lookup.
    store.lookup("a").map_err(|_| "The downloaded dictionary is unreadable.".to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal .tdict, built to the schema in tdict §2.
    fn write_tdict(path: &Path, pair: &str, format: &str) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE entry(id INTEGER PRIMARY KEY, lemma TEXT NOT NULL, pos TEXT,
                                gender TEXT, ipa TEXT, senses_json TEXT NOT NULL);
             CREATE INDEX entry_lemma_idx ON entry(lemma);
             CREATE TABLE form(form TEXT NOT NULL, entry_id INTEGER NOT NULL);
             CREATE INDEX form_idx ON form(form);",
        )
        .unwrap();
        conn.execute("INSERT INTO meta VALUES ('pair', ?1)", [pair]).unwrap();
        conn.execute("INSERT INTO meta VALUES ('format_version', ?1)", [format])
            .unwrap();
        conn.execute(
            "INSERT INTO entry VALUES (1, 'teach', 'verb', NULL, '/tiːtʃ/',
             '[{\"gloss\":\"to instruct\",\"translations\":[\"учить\"]}]')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO form VALUES ('taught', 1)", []).unwrap();
        conn.execute("INSERT INTO form VALUES ('teach', 1)", []).unwrap();
    }

    fn temp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("treader-test-{}-{}.sqlite", std::process::id(), name));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn looks_up_by_lemma_and_by_inflected_form() {
        let path = temp_path("lookup");
        write_tdict(&path, "en-ru", "1");
        let store = Store::new(&path);

        let direct = store.lookup("teach").unwrap();
        assert_eq!(direct.len(), 1);
        assert_eq!(direct[0].lemma, "teach");
        assert_eq!(direct[0].senses[0].translations, vec!["учить"]);

        // The form table resolves an inflection to the same single entry.
        let inflected = store.lookup("taught").unwrap();
        assert_eq!(inflected.len(), 1);
        assert_eq!(inflected[0].lemma, "teach");

        assert!(store.lookup("absent").unwrap().is_empty());
        assert_eq!(store.format_version(), 1);
        assert_eq!(store.pair(), "en-ru");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn validation_rejects_wrong_pair_and_future_format() {
        let ok = temp_path("valid");
        write_tdict(&ok, "en-ru", "1");
        assert!(validate(&ok, "en-ru").is_ok());
        assert!(validate(&ok, "de-ru").is_err());
        std::fs::remove_file(&ok).ok();

        let future = temp_path("future");
        write_tdict(&future, "en-ru", "2");
        assert!(validate(&future, "en-ru").is_err());
        std::fs::remove_file(&future).ok();

        let garbage = temp_path("garbage");
        std::fs::write(&garbage, b"not a database at all").unwrap();
        assert!(validate(&garbage, "en-ru").is_err());
        std::fs::remove_file(&garbage).ok();
    }

    #[test]
    fn a_missing_file_is_an_error_not_an_empty_dictionary() {
        let store = Store::new(temp_path("missing"));
        assert!(store.lookup("teach").is_err());
    }
}
