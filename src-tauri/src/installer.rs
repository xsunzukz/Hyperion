use crate::error::Result;
use crate::manifest::Manifest;
use crate::models::{InstalledGame, VerificationResult};
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tracing::info;

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
            let file_path = install_path.join(&file_manifest.filename);

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

    /// Safely uninstalls a game by deleting installed files and empty directories
    pub fn uninstall_game(game: &InstalledGame) -> Result<()> {
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

        // Remove files
        std::fs::remove_dir_all(install_path)?;
        info!("Uninstalled {} successfully", game.app_name);
        Ok(())
    }
}
