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

/// `%LOCALAPPDATA%/Hyperion/installed` — the exact manifest each install came
/// from. Verify, repair, updates and uninstall all need the build that is on
/// disk, not whatever the URL or Epic serves today.
pub fn installed_dir() -> PathBuf {
    crate::state::data_dir().join("installed")
}

/// Filename-safe, collision-proof name for an app's stored manifest.
pub fn installed_file_name(app_name: &str) -> String {
    let readable: String = app_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(64)
        .collect();
    let mut hasher = Sha1::new();
    hasher.update(app_name.as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("{}-{}.manifest", readable, &digest[..10])
}

pub fn read_installed_from(dir: &Path, app_name: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(installed_file_name(app_name))).ok()
}

/// Atomic, like the URL cache: a torn write must never stand in for the build.
pub fn write_installed_to(dir: &Path, app_name: &str, bytes: &[u8]) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(installed_file_name(app_name));
    let tmp = path.with_extension("manifest.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

pub fn read_installed(app_name: &str) -> Option<Vec<u8>> {
    read_installed_from(&installed_dir(), app_name)
}

pub fn write_installed(app_name: &str, bytes: &[u8]) -> Result<PathBuf> {
    write_installed_to(&installed_dir(), app_name, bytes)
}

pub fn remove_installed(app_name: &str) {
    let _ = std::fs::remove_file(installed_dir().join(installed_file_name(app_name)));
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
    fn installed_manifests_are_kept_per_app() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_installed_from(dir.path(), "Jackal").is_none());

        write_installed_to(dir.path(), "Jackal", b"build-a").unwrap();
        write_installed_to(dir.path(), "jackal", b"other-app").unwrap();
        write_installed_to(dir.path(), "Jackal", b"build-b").unwrap();

        assert_eq!(read_installed_from(dir.path(), "Jackal").unwrap(), b"build-b");
        assert_eq!(read_installed_from(dir.path(), "jackal").unwrap(), b"other-app");
        assert_ne!(installed_file_name("a b"), installed_file_name("a_b"));
        assert!(!installed_file_name("../../x").contains('/'));
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
