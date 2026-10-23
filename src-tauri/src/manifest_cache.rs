//! On-disk cache for `.manifest` files pulled from third-party APIs.
//!
//! Community builds often publish only the manifest, so it is fetched
//! once (when the launcher opens for the first time) and served from disk on
//! every later install — the install click no longer needs the API at all.

use crate::error::Result;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};

/// `%LOCALAPPDATA%/Hyperion/manifests`
pub fn cache_dir() -> PathBuf {
    crate::state::data_dir().join("manifests")
}

/// Stable, filename-safe key for a manifest URL.
pub fn file_name(url: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(url.as_bytes());
    format!("{}.manifest", hex::encode(hasher.finalize()))
}

pub fn path_in(dir: &Path, url: &str) -> PathBuf {
    dir.join(file_name(url))
}

pub fn cache_path(url: &str) -> PathBuf {
    path_in(&cache_dir(), url)
}

pub fn read_from(dir: &Path, url: &str) -> Option<Vec<u8>> {
    std::fs::read(path_in(dir, url)).ok()
}

/// Writes atomically, so an interrupted write can never leave a truncated
/// manifest that would later fail to parse.
pub fn write_to(dir: &Path, url: &str, bytes: &[u8]) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = path_in(dir, url);
    let tmp = path.with_extension("manifest.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

pub fn read(url: &str) -> Option<Vec<u8>> {
    read_from(&cache_dir(), url)
}

pub fn write(url: &str, bytes: &[u8]) -> Result<PathBuf> {
    write_to(&cache_dir(), url, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_stable_and_url_specific() {
        let a = file_name("https://api.example.dev/v2/Game_1.0.manifest");
        let b = file_name("https://api.example.dev/v2/Game_1.0.manifest");
        let c = file_name("https://api.example.dev/v2/Game_1.1.manifest");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.ends_with(".manifest"));
        assert_eq!(a.len(), ".manifest".len() + 40); // sha1 hex
    }

    #[test]
    fn cached_bytes_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://api.example.dev/v2/Game_1.0.manifest";
        let payload = b"0\x00\x00\x00not-really-a-manifest".to_vec();

        assert!(read_from(dir.path(), url).is_none());
        write_to(dir.path(), url, &payload).expect("write cache");
        assert_eq!(read_from(dir.path(), url).unwrap(), payload);

        // No temp files left behind
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
