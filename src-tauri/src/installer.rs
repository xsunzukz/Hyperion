use crate::error::{AppError, Result};
use crate::manifest::Manifest;
use crate::models::{InstalledGame, VerificationResult};
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use tracing::{info, warn};

/// Per-install journal of files already on disk and matching the manifest.
pub const RESUME_JOURNAL: &str = ".hyperion_resume.txt";

/// Joins a manifest-relative file name onto an install folder.
///
/// Manifests from community mirrors are untrusted: an absolute name, a drive
/// prefix or a `..` component would let a crafted manifest write (or delete)
/// files anywhere the user can, so anything but plain relative names is refused.
pub fn safe_join(base: &Path, relative: &str) -> Result<PathBuf> {
    let normalised = relative.replace('\\', "/");
    let rel = Path::new(&normalised);
    let mut has_name = false;
    for component in rel.components() {
        match component {
            Component::Normal(_) => has_name = true,
            Component::CurDir => {}
            _ => {
                return Err(AppError::Manifest(format!(
                    "Refusing unsafe file path in manifest: {}",
                    relative
                )))
            }
        }
    }
    if !has_name {
        return Err(AppError::Manifest(format!(
            "Refusing empty file path in manifest: {:?}",
            relative
        )));
    }
    Ok(base.join(rel))
}

/// Folder name used for a title inside the library root.
pub fn folder_name_for(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn normalise_dir(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

pub fn same_dir(a: &Path, b: &Path) -> bool {
    normalise_dir(a) == normalise_dir(b)
}

/// True when `dir` is `other` or one of its parents.
fn is_same_or_ancestor(dir: &Path, other: &Path) -> bool {
    let dir = normalise_dir(dir);
    let other = normalise_dir(other);
    !dir.is_empty() && (other == dir || other.starts_with(&format!("{}\\", dir)))
}

/// Folders a whole-folder delete must never touch: drive roots, the user's own
/// folders, system folders, Hyperion's data, and anything in `extra` (library
/// roots, other games' install folders) or containing one of them.
pub fn is_protected_dir(dir: &Path, extra: &[PathBuf]) -> bool {
    if !dir.is_absolute() || dir.parent().is_none() {
        return true;
    }

    let mut known: Vec<PathBuf> = [
        dirs::home_dir(),
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
        dirs::data_dir(),
        dirs::data_local_dir(),
        dirs::picture_dir(),
        dirs::video_dir(),
        dirs::audio_dir(),
        Some(crate::state::data_dir()),
    ]
    .into_iter()
    .flatten()
    .collect();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramData", "SystemRoot", "PUBLIC"] {
        if let Some(value) = std::env::var_os(var) {
            known.push(PathBuf::from(value));
        }
    }

    known
        .iter()
        .chain(extra.iter())
        .any(|protected| is_same_or_ancestor(dir, protected))
}

/// The folder a title is installed into.
///
/// A requested folder is used as-is only when it is the game's own folder: the
/// existing install, an empty or new folder, one already named after the game,
/// or one holding an unfinished Hyperion download. A library root or any other
/// populated folder gets a per-game subfolder, so a later uninstall can never
/// take unrelated files with it.
pub fn resolve_install_dir(
    requested: Option<&str>,
    default_root: &Path,
    title: &str,
    existing_install: Option<&Path>,
) -> PathBuf {
    let folder = folder_name_for(title);
    let Some(dir) = requested
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
    else {
        return default_root.join(&folder);
    };

    if existing_install.is_some_and(|existing| same_dir(existing, &dir)) {
        return dir;
    }

    let library_root = dir.parent().is_none() || same_dir(&dir, default_root);
    let named_for_game = dir
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(&folder));
    let occupied = std::fs::read_dir(&dir)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    let resumable = dir.join(RESUME_JOURNAL).exists();

    if library_root || (occupied && !named_for_game && !resumable) {
        dir.join(folder)
    } else {
        dir
    }
}

/// Removes empty directories under `root` (deepest first), then `root` itself
/// if nothing is left in it.
pub fn remove_empty_dirs(root: &Path) {
    fn prune(dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    prune(&path);
                }
            }
        }
        let _ = std::fs::remove_dir(dir); // fails (and is ignored) unless empty
    }
    prune(root);
}

pub struct Installer;

impl Installer {
    /// Computes the SHA1 hash of a file on disk using a streaming 64 KiB buffer
    pub fn compute_file_sha1<P: AsRef<Path>>(path: P) -> Result<[u8; 20]> {
        let mut file = File::open(path)?;
        let mut hasher = Sha1::new();
        let mut buffer = [0u8; 65536];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let hash: [u8; 20] = hasher.finalize().into();
        Ok(hash)
    }

    /// Verifies all files in an installed game against its manifest
    pub fn verify_installed_game(
        game: &InstalledGame,
        manifest: &Manifest,
    ) -> Result<VerificationResult> {
        let install_path = Path::new(&game.install_path);
        let mut valid_count = 0;
        let mut missing_files = Vec::new();
        let mut corrupt_files = Vec::new();

        info!("Verifying game files for: {}", game.app_name);

        for file_manifest in &manifest.file_manifest_list {
            let Ok(file_path) = safe_join(install_path, &file_manifest.filename) else {
                corrupt_files.push(file_manifest.filename.clone());
                continue;
            };

            if !file_path.exists() {
                missing_files.push(file_manifest.filename.clone());
                continue;
            }

            // Check file size first
            if let Ok(metadata) = std::fs::metadata(&file_path) {
                if metadata.len() != file_manifest.file_size {
                    corrupt_files.push(file_manifest.filename.clone());
                    continue;
                }
            } else {
                corrupt_files.push(file_manifest.filename.clone());
                continue;
            }

            // Compute hash
            match Self::compute_file_sha1(&file_path) {
                Ok(hash) => {
                    if hash == file_manifest.hash {
                        valid_count += 1;
                    } else {
                        corrupt_files.push(file_manifest.filename.clone());
                    }
                }
                Err(_) => {
                    corrupt_files.push(file_manifest.filename.clone());
                }
            }
        }

        let is_valid = missing_files.is_empty() && corrupt_files.is_empty();
        info!(
            "Verification result for {}: valid={}, missing={}, corrupt={}",
            game.app_name,
            valid_count,
            missing_files.len(),
            corrupt_files.len()
        );

        Ok(VerificationResult {
            app_name: game.app_name.clone(),
            total_files: manifest.file_manifest_list.len(),
            valid_files: valid_count,
            corrupt_files,
            missing_files,
            is_valid,
        })
    }

    /// Uninstalls a game.
    ///
    /// With the manifest it was installed from, only that manifest's files are
    /// deleted (plus folders left empty), so saves or mods the user added stay.
    /// Without one, the folder is removed whole — but never when it is a drive
    /// root, a user/system folder, a library root or holds another game.
    pub fn uninstall_game(
        game: &InstalledGame,
        manifest: Option<&Manifest>,
        protected: &[PathBuf],
    ) -> Result<()> {
        if game.install_path.trim().is_empty() {
            return Err(AppError::Install(format!(
                "{} has no install folder on record",
                game.title
            )));
        }

        let install_path = Path::new(&game.install_path);
        if !install_path.exists() {
            info!(
                "Install path {} already does not exist",
                install_path.display()
            );
            return Ok(());
        }

        info!(
            "Uninstalling game {} from {}",
            game.app_name,
            install_path.display()
        );

        match manifest {
            Some(manifest) => {
                for file in &manifest.file_manifest_list {
                    match safe_join(install_path, &file.filename) {
                        Ok(path) => {
                            if let Err(e) = std::fs::remove_file(&path) {
                                if e.kind() != std::io::ErrorKind::NotFound {
                                    warn!("Could not delete {}: {}", path.display(), e);
                                }
                            }
                        }
                        Err(e) => warn!("Skipping {}", e),
                    }
                }
                let _ = std::fs::remove_file(install_path.join(RESUME_JOURNAL));
                remove_empty_dirs(install_path);
            }
            None => {
                if is_protected_dir(install_path, protected) {
                    return Err(AppError::Install(format!(
                        "Refusing to delete {}: it is a drive root, a system or library folder, or it contains other games. Delete the game's files there yourself.",
                        install_path.display()
                    )));
                }
                std::fs::remove_dir_all(install_path)?;
            }
        }

        info!("Uninstalled {} successfully", game.app_name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_keeps_files_inside_the_install_folder() {
        let base = Path::new("D:\\Games\\Dauntless");
        assert!(safe_join(base, "Engine/Binaries/Win64/game.exe").is_ok());
        assert!(safe_join(base, "Engine\\Content\\a.pak").is_ok());
        assert!(safe_join(base, "./readme.txt").is_ok());

        assert!(safe_join(base, "../evil.dll").is_err());
        assert!(safe_join(base, "Engine/../../evil.dll").is_err());
        assert!(safe_join(base, "C:\\Windows\\evil.dll").is_err());
        assert!(safe_join(base, "C:evil.dll").is_err());
        assert!(safe_join(base, "\\Users\\x\\evil.dll").is_err());
        assert!(safe_join(base, "/etc/passwd").is_err());
        assert!(safe_join(base, "\\\\server\\share\\evil.dll").is_err());
        assert!(safe_join(base, "").is_err());
    }

    #[test]
    fn library_roots_and_populated_folders_get_a_game_subfolder() {
        let root = tempfile::tempdir().unwrap();
        let library = root.path().join("Games");
        std::fs::create_dir_all(library.join("Other Game")).unwrap();

        // Default library root
        let lib = library.to_string_lossy().to_string();
        assert_eq!(
            resolve_install_dir(Some(&lib), &library, "Dauntless", None),
            library.join("Dauntless")
        );

        // A populated folder that is not this game's
        let other_lib = root.path().join("Elsewhere");
        std::fs::create_dir_all(other_lib.join("stuff")).unwrap();
        let other = other_lib.to_string_lossy().to_string();
        assert_eq!(
            resolve_install_dir(Some(&other), &library, "Dauntless", None),
            other_lib.join("Dauntless")
        );

        // New or empty folder, the game's own folder, and the existing install stay as given
        let fresh = root.path().join("Fresh").to_string_lossy().to_string();
        assert_eq!(
            resolve_install_dir(Some(&fresh), &library, "Dauntless", None),
            PathBuf::from(&fresh)
        );
        let own = library.join("Dauntless");
        std::fs::create_dir_all(own.join("Engine")).unwrap();
        assert_eq!(
            resolve_install_dir(Some(&own.to_string_lossy()), &library, "Dauntless", None),
            own
        );
        assert_eq!(
            resolve_install_dir(Some(&other), &library, "Dauntless", Some(&other_lib)),
            other_lib
        );

        // Nothing requested: library root + title
        assert_eq!(
            resolve_install_dir(None, &library, "Dauntless", None),
            library.join("Dauntless")
        );
    }

    #[test]
    fn protected_dirs_cover_roots_libraries_and_folders_holding_other_games() {
        let root = tempfile::tempdir().unwrap();
        let library = root.path().join("Games");
        let other_game = library.join("Other");

        assert!(is_protected_dir(Path::new("C:\\"), &[]));
        assert!(is_protected_dir(Path::new("relative\\dir"), &[]));
        assert!(is_protected_dir(&library, &[library.clone()]));
        assert!(is_protected_dir(&library, &[other_game.clone()]));
        assert!(!is_protected_dir(&library.join("Dauntless"), &[library.clone(), other_game]));
        if let Some(home) = dirs::home_dir() {
            assert!(is_protected_dir(&home, &[]));
            assert!(is_protected_dir(home.parent().unwrap(), &[]));
        }
    }

    #[test]
    fn manifest_uninstall_keeps_user_files() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("Game");
        std::fs::create_dir_all(install.join("Bin")).unwrap();
        std::fs::create_dir_all(install.join("Saves")).unwrap();
        std::fs::write(install.join("Bin/game.exe"), b"x").unwrap();
        std::fs::write(install.join("Saves/slot1.sav"), b"keep").unwrap();

        let mut manifest = Manifest::default();
        manifest.file_manifest_list.push(crate::manifest::FileManifest {
            filename: "Bin/game.exe".to_string(),
            ..Default::default()
        });
        let game = InstalledGame {
            app_name: "Game".to_string(),
            title: "Game".to_string(),
            install_path: install.to_string_lossy().to_string(),
            ..Default::default()
        };

        Installer::uninstall_game(&game, Some(&manifest), &[]).unwrap();
        assert!(!install.join("Bin").exists());
        assert!(install.join("Saves/slot1.sav").exists());
    }
}
