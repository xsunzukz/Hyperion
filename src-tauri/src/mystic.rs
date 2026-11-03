//! Dauntless for Mystic Paradox: one pinned build, downloaded from Epic's CDN.
//!
//! Mystic Paradox only runs Dauntless 1.14.7 (changelist 647472). Epic hands out
//! 2.1.1 for the game today, so nothing here asks Epic which build or which CDN
//! to use: the manifest is checked against its published SHA-1 and the chunks
//! come from the fixed Epic CDN path in Mystic Paradox's install guide
//! (docs/INSTALLING_DAUNTLESS.md in pranav158/Mystic-Paradox). The manifest's own
//! host (api.mysticfox.dev) serves the manifest only, never chunks.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha1::Sha1;
use sha2::{Digest, Sha256};

/// Epic's codename for Dauntless; the app name inside the manifest.
pub const APP_NAME: &str = "Jackal";
pub const TITLE: &str = "Dauntless";
pub const BUILD_VERSION: &str = "1.14.7.647472";

pub const MANIFEST_URL: &str = "https://api.mysticfox.dev/v2/Dauntless_1.14.7.647472.manifest";
/// SHA-1 of the manifest file, as published by Mystic Paradox.
pub const MANIFEST_SHA1: &str = "aa70748e4c0a776a991f2bac10ad342adaa31476";

/// Epic CDN hosts that serve this build, tried in order. The path is the same
/// on each; the first is the one Mystic Paradox's guide uses.
pub const CHUNK_BASES: &[&str] = &[
    "https://egs-cloudfront-chunks.epicgamescdn.com/Builds/Org/o-krlzxj88qrtb69fredeuaf887bl5az/Jackal/default",
    "https://egdownload.fastly-edge.com/Builds/Org/o-krlzxj88qrtb69fredeuaf887bl5az/Jackal/default",
    "https://epicgames-download1.akamaized.net/Builds/Org/o-krlzxj88qrtb69fredeuaf887bl5az/Jackal/default",
    "https://fastly-download.epicgames.com/Builds/Org/o-krlzxj88qrtb69fredeuaf887bl5az/Jackal/default",
    "https://download.epicgames.com/Builds/Org/o-krlzxj88qrtb69fredeuaf887bl5az/Jackal/default",
];

/// The executable the Mystic Paradox launcher hashes before it lets the game
/// start, and the SHA-256 it accepts for 1.14.7.
pub const GAME_EXE: &str = "Archon/Binaries/Win64/Dauntless-Win64-Shipping.exe";
pub const GAME_EXE_SHA256: &str = "f3f95fbf22709ef077a11632ed95d675b7209b7e0172d9ecb9ba7d02a1fbb66b";

pub fn is_dauntless(app_name: &str) -> bool {
    app_name.eq_ignore_ascii_case(APP_NAME)
}

/// True only for the exact 1.14.7 manifest file.
pub fn is_pinned_manifest(bytes: &[u8]) -> bool {
    hex::encode(Sha1::digest(bytes)) == MANIFEST_SHA1
}

/// A folder that already holds a copy of the game (a legendary install, an
/// older Hyperion one, a copy from another PC).
pub fn holds_install(dir: &Path) -> bool {
    dir.join("Dauntless.exe").is_file() || dir.join("Archon").is_dir()
}

/// The copy a picked folder points at: the folder itself, a `Dauntless` folder
/// inside it, or the game folder around a picked `Archon` folder.
pub fn find_existing(chosen: &Path) -> Option<PathBuf> {
    if holds_install(chosen) {
        return Some(chosen.to_path_buf());
    }
    let nested = chosen.join(TITLE);
    if holds_install(&nested) {
        return Some(nested);
    }
    let picked_archon = chosen
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("Archon"));
    chosen
        .parent()
        .filter(|parent| picked_archon && holds_install(parent))
        .map(Path::to_path_buf)
}

/// Where Dauntless goes for the folder the user picked. An existing copy is
/// used as it is, so its matching files are kept; anything else follows the
/// normal rule (a library folder gets a `Dauntless` subfolder).
pub fn target_dir(chosen: &Path, default_root: &Path, existing_install: Option<&Path>) -> PathBuf {
    if holds_install(chosen) {
        return chosen.to_path_buf();
    }
    // A library folder that already has the game one level down
    let nested = chosen.join(TITLE);
    if holds_install(&nested) {
        return nested;
    }
    // `<copy>\Dauntless` typed (or added by a folder picker) under an existing copy
    if !chosen.exists() {
        if let Some(parent) = chosen.parent().filter(|p| holds_install(p)) {
            return parent.to_path_buf();
        }
    }
    let chosen = chosen.to_string_lossy();
    crate::installer::resolve_install_dir(
        Some(chosen.as_ref()),
        default_root,
        TITLE,
        existing_install,
    )
}

/// Whether the installed executable is the one the launcher accepts. `None`
/// when it cannot be read (missing files are reported by the normal verify).
pub fn exe_matches_launcher(install_dir: &Path) -> Option<bool> {
    let mut file = File::open(install_dir.join(GAME_EXE)).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(hex::encode(hasher.finalize()) == GAME_EXE_SHA256)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_published_manifest_is_accepted() {
        assert!(!is_pinned_manifest(b"not a manifest"));
        assert!(!is_pinned_manifest(&[]));
    }

    #[test]
    fn an_existing_copy_is_used_where_it_is() {
        let root = tempfile::tempdir().unwrap();
        let copy = root.path().join("NSA").join("Mystic").join("Dauntless");
        std::fs::create_dir_all(copy.join("Archon")).unwrap();
        std::fs::write(copy.join("Dauntless.exe"), b"exe").unwrap();

        // Picking the copy itself, the folder around it, or a `Dauntless`
        // folder that does not exist inside it all land on the copy
        assert_eq!(target_dir(&copy, Path::new("D:\\Games"), None), copy);
        assert_eq!(target_dir(copy.parent().unwrap(), Path::new("D:\\Games"), None), copy);
        assert_eq!(target_dir(&copy.join(TITLE), Path::new("D:\\Games"), None), copy);
    }

    #[test]
    fn an_existing_copy_is_found_from_the_folder_the_user_picks() {
        let root = tempfile::tempdir().unwrap();
        let copy = root.path().join("Games").join(TITLE);
        std::fs::create_dir_all(copy.join("Archon").join("Binaries")).unwrap();
        std::fs::write(copy.join("Dauntless.exe"), b"exe").unwrap();

        assert_eq!(find_existing(&copy), Some(copy.clone()));
        assert_eq!(find_existing(copy.parent().unwrap()), Some(copy.clone()));
        assert_eq!(find_existing(&copy.join("Archon")), Some(copy.clone()));

        // Nothing to reuse: no download is started into an empty folder
        assert_eq!(find_existing(&copy.join("Archon").join("Binaries")), None);
        let empty = root.path().join("Empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(find_existing(&empty), None);
    }

    #[test]
    fn a_folder_without_the_game_gets_its_own_subfolder() {
        let root = tempfile::tempdir().unwrap();
        let library = root.path().join("Games");
        std::fs::create_dir_all(library.join("SomeOtherGame")).unwrap();

        assert_eq!(
            target_dir(&library, Path::new("D:\\Elsewhere"), None),
            library.join(TITLE)
        );
    }

    /// Runs against the real file when HYPERION_DAUNTLESS_MANIFEST points at a
    /// downloaded copy of MANIFEST_URL.
    #[test]
    fn the_published_manifest_is_build_1_14_7() {
        let Ok(path) = std::env::var("HYPERION_DAUNTLESS_MANIFEST") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        assert!(is_pinned_manifest(&bytes));

        let manifest = crate::manifest::Manifest::parse(&bytes).unwrap();
        assert_eq!(manifest.meta.app_name, APP_NAME);
        assert_eq!(manifest.meta.build_version, BUILD_VERSION);
        assert_eq!(manifest.file_manifest_list.len(), 611);
        assert!(manifest.file_manifest_list.iter().any(|f| f.filename == GAME_EXE));
    }

    #[test]
    fn every_chunk_base_is_an_epic_cdn_path_for_this_build() {
        for base in CHUNK_BASES {
            assert!(base.starts_with("https://"));
            assert!(base.ends_with("/o-krlzxj88qrtb69fredeuaf887bl5az/Jackal/default"));
            assert!(!base.contains("mysticfox"));
        }
    }
}
