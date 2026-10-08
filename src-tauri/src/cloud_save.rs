use crate::api::EpicApiClient;
use crate::error::Result;
use crate::models::{CloudSaveState, InstalledGame, Session};
use std::path::PathBuf;

pub struct CloudSaveManager;

impl CloudSaveManager {
    /// Resolves template variables in the CloudSaveFolder attribute
    pub fn resolve_save_path(
        raw_pattern: &str,
        game: &InstalledGame,
        session: Option<&Session>,
    ) -> Option<PathBuf> {
        let mut path_str = raw_pattern.replace('\\', "/");
        let epic_id = session
            .map(|s| s.account_id.as_str())
            .unwrap_or("00000000000000000000000000000000");

        path_str = path_str.replace("{installdir}", &game.install_path.replace('\\', "/"));
        path_str = path_str.replace("{epicid}", epic_id);

        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
                path_str = path_str.replace("{appdata}", &appdata.replace('\\', "/"));
            }
            if let Ok(userprofile) = std::env::var("USERPROFILE") {
                let userdir = format!("{}/Documents", userprofile.replace('\\', "/"));
                let usersaved = format!("{}/Saved Games", userprofile.replace('\\', "/"));
                path_str = path_str.replace("{userdir}", &userdir);
                path_str = path_str.replace("{userprofile}", &userprofile.replace('\\', "/"));
                path_str = path_str.replace("{usersavedgames}", &usersaved);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home) = dirs::home_dir() {
                let home_str = home.to_string_lossy().to_string();
                path_str = path_str.replace("{appdata}", &format!("{}/.local/share", home_str));
                path_str = path_str.replace("{userdir}", &format!("{}/Documents", home_str));
                path_str = path_str.replace("{userprofile}", &home_str);
                path_str =
                    path_str.replace("{usersavedgames}", &format!("{}/Saved Games", home_str));
            }
        }

        let p = PathBuf::from(path_str);
        Some(p)
    }

    /// Checks local save game directory and cloud metadata
    pub async fn get_save_status(
        app_name: &str,
        save_pattern: Option<&str>,
        game: Option<&InstalledGame>,
        session: Option<&Session>,
        api_client: &EpicApiClient,
    ) -> Result<CloudSaveState> {
        let local_path = if let (Some(pat), Some(g)) = (save_pattern, game) {
            Self::resolve_save_path(pat, g, session)
        } else {
            None
        };

        let mut local_files_count = 0;
        let mut local_updated_at = None;

        if let Some(ref lp) = local_path {
            if lp.exists() && lp.is_dir() {
                if let Ok(entries) = std::fs::read_dir(lp) {
                    let mut latest_time = None;
                    for entry in entries.flatten() {
                        if let Ok(meta) = entry.metadata() {
                            if meta.is_file() {
                                local_files_count += 1;
                                if let Ok(mod_time) = meta.modified() {
                                    if latest_time.is_none() || Some(mod_time) > latest_time {
                                        latest_time = Some(mod_time);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(t) = latest_time {
                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                        local_updated_at = Some(dt.to_rfc3339());
                    }
                }
            }
        }

        // Check remote cloud save manifests
        let mut cloud_files_count = 0;
        let mut cloud_updated_at = None;

        if session.is_some() {
            if let Ok(manifest_val) = api_client.get_cloud_save_manifests(app_name).await {
                if let Some(files) = manifest_val["files"].as_array() {
                    cloud_files_count = files.len();
                    if let Some(first) = files.first() {
                        if let Some(last_mod) = first["lastModified"].as_str() {
                            cloud_updated_at = Some(last_mod.to_string());
                        }
                    }
                }
            }
        }

        let in_sync = local_files_count > 0 && cloud_files_count > 0;
        let status_message = if local_files_count == 0 && cloud_files_count == 0 {
            "No save data found locally or in cloud".to_string()
        } else if local_files_count > 0 && cloud_files_count == 0 {
            "Local saves ready to upload".to_string()
        } else if local_files_count == 0 && cloud_files_count > 0 {
            "Cloud saves available to restore".to_string()
        } else {
            "Cloud and local saves synced".to_string()
        };

        Ok(CloudSaveState {
            app_name: app_name.to_string(),
            local_path: local_path.map(|p| p.to_string_lossy().to_string()),
            local_files_count,
            local_updated_at,
            cloud_files_count,
            cloud_updated_at,
            in_sync,
            status_message,
        })
    }
}
