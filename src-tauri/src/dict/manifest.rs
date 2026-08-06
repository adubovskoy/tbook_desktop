//! The dictionary catalog: fetch, and keep the last good copy on disk.
//!
//! Port of `data/dict/DictionaryManifestClient.kt`. Caching is what lets the
//! Dictionaries screen render offline — and installed dictionaries keep working
//! regardless, since only the *catalog* needs the network. This and the download
//! are the app's only network use.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use super::models::{RemoteDict, RemoteDictManifest, SUPPORTED_MANIFEST_VERSION};

pub const MANIFEST_URL: &str = "https://dictionary.tbook.dev/v1/manifest.json";

/// What the Dictionaries screen shows above the list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestResult {
    /// "fresh" (just fetched), "cached" (offline, last good copy) or
    /// "unavailable" (no network and no cache, or a newer schema).
    pub status: &'static str,
    pub message: String,
    pub dictionaries: Vec<RemoteDict>,
}

impl ManifestResult {
    fn fresh(manifest: RemoteDictManifest) -> Self {
        ManifestResult {
            status: "fresh",
            message: String::new(),
            dictionaries: manifest.dictionaries,
        }
    }

    fn cached(manifest: RemoteDictManifest) -> Self {
        ManifestResult {
            status: "cached",
            message: String::new(),
            dictionaries: manifest.dictionaries,
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        ManifestResult {
            status: "unavailable",
            message: message.into(),
            dictionaries: Vec::new(),
        }
    }
}

pub fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_body(Some(Duration::from_secs(30)))
            .build(),
    )
}

fn fetch(url: &str) -> Result<String, String> {
    let mut response = agent().get(url).call().map_err(|e| e.to_string())?;
    if response.status() != 200 {
        return Err(format!("HTTP {} for {url}", response.status()));
    }
    response.body_mut().read_to_string().map_err(|e| e.to_string())
}

/// The catalog: from the network when reachable, otherwise the last good copy.
pub fn load(cache_file: &Path, url: &str) -> ManifestResult {
    let body = match fetch(url) {
        Ok(body) => body,
        Err(_) => {
            return cached_or(
                cache_file,
                "Can't reach the dictionary server. Check your connection.",
            )
        }
    };
    let Ok(manifest) = serde_json::from_str::<RemoteDictManifest>(&body) else {
        return cached_or(
            cache_file,
            "The dictionary server returned an unexpected response.",
        );
    };
    if manifest.format_version > SUPPORTED_MANIFEST_VERSION {
        return cached_or(cache_file, "Update TReader to see the available dictionaries.");
    }
    write_cache(cache_file, &body);
    ManifestResult::fresh(manifest)
}

/// The cached catalog, or `fallback` when there isn't a usable one.
fn cached_or(cache_file: &Path, fallback: &str) -> ManifestResult {
    match cached(cache_file) {
        Some(manifest) => ManifestResult::cached(manifest),
        None => ManifestResult::unavailable(fallback),
    }
}

/// The last successfully fetched catalog, if it is one this app understands.
pub fn cached(cache_file: &Path) -> Option<RemoteDictManifest> {
    let body = std::fs::read_to_string(cache_file).ok()?;
    serde_json::from_str::<RemoteDictManifest>(&body)
        .ok()
        .filter(|m| m.format_version <= SUPPORTED_MANIFEST_VERSION)
}

/// The entry for `id` in the cached catalog. Downloads resolve their URL here
/// rather than taking one from the WebView, so what is fetched is always
/// something our own server listed.
pub fn cached_dict(cache_file: &Path, id: &str) -> Option<RemoteDict> {
    cached(cache_file)?.dictionaries.into_iter().find(|d| d.id == id)
}

fn write_cache(cache_file: &Path, body: &str) {
    let Some(dir) = cache_file.parent() else { return };
    let staging = dir.join(format!("{}manifest", super::STAGING_PREFIX));
    if std::fs::write(&staging, body).is_ok() && std::fs::rename(&staging, cache_file).is_err() {
        let _ = std::fs::copy(&staging, cache_file);
        let _ = std::fs::remove_file(&staging);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "treader-manifest-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const BODY: &str = r#"{"formatVersion":1,"dictionaries":[{"id":"en-ru","source":"en",
        "target":"ru","name":"English → Russian","version":7,"sizeBytes":10,
        "sha256":"ab","url":"https://example.test/en-ru.tdict.gz"}]}"#;

    #[test]
    fn falls_back_to_the_cache_when_the_server_is_unreachable() {
        let dir = temp_dir("offline");
        let cache = dir.join("manifest.json");
        std::fs::write(&cache, BODY).unwrap();

        // A port nothing listens on stands in for "no network".
        let result = load(&cache, "http://127.0.0.1:1/v1/manifest.json");
        assert_eq!(result.status, "cached");
        assert_eq!(result.dictionaries.len(), 1);
        assert_eq!(cached_dict(&cache, "en-ru").unwrap().target, "ru");
        assert!(cached_dict(&cache, "de-ru").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_cache_and_no_network_is_unavailable_with_a_message() {
        let dir = temp_dir("nothing");
        let result = load(&dir.join("manifest.json"), "http://127.0.0.1:1/v1/manifest.json");
        assert_eq!(result.status, "unavailable");
        assert!(result.message.contains("connection"));
        assert!(result.dictionaries.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_newer_schema_is_not_read_from_the_cache() {
        let dir = temp_dir("newer");
        let cache = dir.join("manifest.json");
        std::fs::write(&cache, r#"{"formatVersion":99,"dictionaries":[]}"#).unwrap();
        assert!(cached(&cache).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
