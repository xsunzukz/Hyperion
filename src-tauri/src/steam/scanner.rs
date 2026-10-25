use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::models::GameItem;
use crate::steam::vdf::parse_vdf;

/// Discovers the primary Steam installation path on the host system
pub fn detect_steam_root() -> Option<PathBuf> {
    // 1. Windows registry (read directly instead of shelling out to `reg query`)
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
        use winreg::RegKey;

        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(r"Software\Valve\Steam", KEY_READ)
        {
            if let Ok(value) = key.get_value::<String, _>("SteamPath") {
                let path = PathBuf::from(value.trim().replace('/', "\\"));
                if path.exists() {
                    debug!("Found Steam root via registry: {:?}", path);
                    return Some(path);
                }
            }
        }
    }

    // 2. Fallback to standard Steam paths across common drives
    let standard_candidates = [
        r"C:\Program Files (x86)\Steam",
        r"C:\Program Files\Steam",
        r"D:\Steam",
        r"E:\Steam",
        r"F:\Steam",
        r"G:\Steam",
        r"D:\SteamLibrary",
        r"E:\SteamLibrary",
        r"F:\SteamLibrary",
    ];

    for candidate in &standard_candidates {
        let p = PathBuf::from(candidate);
        if p.exists() && p.join("steamapps").exists() {
            debug!("Found Steam root via standard directory scan: {:?}", p);
            return Some(p);
        }
    }

    None
}

/// Discovers all Steam library folders configured across all drives
pub fn discover_library_folders(steam_root: &Path) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();

    // Include the main Steam root directory itself
    if steam_root.exists() {
        let canonical_str = steam_root.to_string_lossy().to_lowercase().replace('/', "\\");
        visited.insert(canonical_str);
        folders.push(steam_root.to_path_buf());
    }

    let libraryfolders_vdf = steam_root.join("steamapps").join("libraryfolders.vdf");
    if !libraryfolders_vdf.exists() {
        return folders;
    }

    let content = match std::fs::read_to_string(&libraryfolders_vdf) {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not read libraryfolders.vdf at {:?}: {}", libraryfolders_vdf, e);
            return folders;
        }
    };

    let root = parse_vdf(&content);
    if let Some(libs) = root.get("libraryfolders").and_then(|v| v.as_object()) {
        for (_k, v) in libs {
            if let Some(obj) = v.as_object() {
                if let Some(path_val) = obj.get("path").and_then(|p| p.as_str()) {
                    let path = PathBuf::from(path_val.replace('/', "\\"));
                    let canonical_str = path.to_string_lossy().to_lowercase();
                    if path.exists() && !visited.contains(&canonical_str) {
                        visited.insert(canonical_str);
                        folders.push(path);
                    }
                }
            }
        }
    }

    info!("Discovered {} Steam library folder(s)", folders.len());
    folders
}

/// Scans all configured Steam libraries and returns installed games
pub fn scan_installed_steam_games() -> Vec<GameItem> {
    let steam_root = match detect_steam_root() {
        Some(r) => r,
        None => {
            debug!("Steam root not found on this system");
            return Vec::new();
        }
    };

    let libraries = discover_library_folders(&steam_root);
    let mut games = Vec::new();
    let mut seen_app_ids = HashSet::new();

    for lib_folder in libraries {
        let steamapps = lib_folder.join("steamapps");
        if !steamapps.exists() {
            continue;
        }

        let entries = match std::fs::read_dir(&steamapps) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            // Look for `appmanifest_<appid>.acf`
            if name_str.starts_with("appmanifest_") && name_str.ends_with(".acf") {
                if let Some(game) = parse_appmanifest_file(&entry.path(), &lib_folder) {
                    if let Some(app_id) = game.steam_app_id {
                        if !seen_app_ids.contains(&app_id) {
                            seen_app_ids.insert(app_id);
                            games.push(game);
                        }
                    }
                }
            }
        }
    }

    info!("Discovered {} installed Steam game(s)", games.len());
    games
}

/// Filter out background tools, runtime environments, and non-game components
pub fn is_ignored_steam_app(app_id: u32) -> bool {
    matches!(
        app_id,
        228980   // Steamworks Common Redistributables
        | 1070560 // Steam Linux Runtime
        | 1391110 // SteamVR
        | 202355  // Remote Play Client
        | 3277530 // CS2 Workshop Tools
        | 480     // Spacewar
        | 2279720 // CS2 depot
        | 2279721 // CS2 depot
    )
}

/// Discovers all App IDs owned by the user across Steam userdata folders
pub fn discover_owned_steam_app_ids(steam_root: &Path) -> Vec<u32> {
    let mut owned_ids = HashSet::new();
    let userdata_dir = steam_root.join("userdata");

    if !userdata_dir.exists() {
        return Vec::new();
    }

    if let Ok(user_folders) = std::fs::read_dir(&userdata_dir) {
        for user_folder in user_folders.flatten() {
            if !user_folder.path().is_dir() {
                continue;
            }

            // 1. Scan userdata/<account_id>/config/librarycache/
            let librarycache_dir = user_folder.path().join("config").join("librarycache");
            if librarycache_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&librarycache_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                                if file_name.ends_with(".json") {
                                    let stem = file_name.trim_end_matches(".json");
                                    if let Ok(app_id) = stem.parse::<u32>() {
                                        if !is_ignored_steam_app(app_id) {
                                            owned_ids.insert(app_id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 2. Scan userdata/<account_id>/config/localconfig.vdf
            let localconfig_path = user_folder.path().join("config").join("localconfig.vdf");
            if localconfig_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&localconfig_path) {
                    let root = parse_vdf(&content);
                    if let Some(apps_obj) = root
                        .get("UserLocalConfigStore")
                        .and_then(|s| s.get("apps"))
                        .and_then(|a| a.as_object())
                    {
                        for (key, _) in apps_obj {
                            if let Ok(app_id) = key.parse::<u32>() {
                                if !is_ignored_steam_app(app_id) {
                                    owned_ids.insert(app_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    info!("Discovered {} owned Steam game AppIDs in userdata", owned_ids.len());
    let mut list: Vec<u32> = owned_ids.into_iter().collect();
    list.sort_unstable();
    list
}

/// Discovers and resolves all Steam games (both installed locally and owned in userdata),
/// fetching real metadata from official Valve API and caching to SQLite.
pub async fn resolve_all_steam_library_games(
    storage: &Arc<crate::storage::Storage>,
    force_refresh: bool,
) -> Vec<GameItem> {
    let steam_root = match detect_steam_root() {
        Some(r) => r,
        None => {
            debug!("Steam root not found on this system");
            return Vec::new();
        }
    };

    // 1. Get all locally installed games (read from local appmanifest_<appid>.acf files)
    let installed_games = scan_installed_steam_games();
    let mut seen_app_ids: HashSet<u32> = HashSet::new();
    let mut games = Vec::new();

    for g in installed_games {
        if let Some(id) = g.steam_app_id {
            seen_app_ids.insert(id);
        }
        games.push(g);
    }

    // 2. Discover owned App IDs from user's Steam userdata
    let owned_ids = discover_owned_steam_app_ids(&steam_root);
    let uninstalled_ids: Vec<u32> = owned_ids
        .into_iter()
        .filter(|id| !seen_app_ids.contains(id) && !is_ignored_steam_app(*id))
        .collect();

    let mut to_fetch: Vec<u32> = Vec::new();

    for app_id in uninstalled_ids {
        let app_key = format!("steam_{}", app_id);
        if !force_refresh {
            if let Ok(Some(cached)) = storage.get_cached_catalog_item(&app_key) {
                if cached.namespace == "steam_ignored" {
                    continue; // Skip tools or non-games
                }

                // Check if title is valid and not generic fallback
                if !cached.title.is_empty() && !cached.title.starts_with("Steam App ") {
                    games.push(GameItem {
                        app_name: app_key.clone(),
                        title: cached.title,
                        namespace: "steam".to_string(),
                        catalog_item_id: app_id.to_string(),
                        description: Some("Steam Library Game".to_string()),
                        developer: None,
                        publisher: Some("Steam".to_string()),
                        icon_url: Some(format!(
                            "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
                            app_id
                        )),
                        banner_url: Some(format!(
                            "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg",
                            app_id
                        )),
                        logo_url: Some(format!(
                            "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/logo.png",
                            app_id
                        )),
                        is_installed: false,
                        is_dlc: false,
                        build_version: None,
                        install_path: None,
                        install_size: None,
                        executable: None,
                        can_run_offline: true,
                        supports_cloud_saves: true,
                        platform: "steam".to_string(),
                        steam_app_id: Some(app_id),
                    });
                    continue;
                }
            }
        }

        to_fetch.push(app_id);
    }

    // 3. Concurrently fetch metadata for uncached games from Valve's official API
    if !to_fetch.is_empty() {
        info!("Fetching official Steam Store details for {} uncached games...", to_fetch.len());
        let mut futures_vec = Vec::new();
        for app_id in to_fetch {
            futures_vec.push(async move {
                let res = crate::steam::downloader::get_steam_app_details(app_id).await;
                (app_id, res)
            });
        }

        let results = futures_util::future::join_all(futures_vec).await;

        for (app_id, res) in results {
            let app_key = format!("steam_{}", app_id);
            match res {
                Ok(Some(details)) => {
                    let is_game = details.app_type.as_deref().unwrap_or("game") == "game";
                    if is_game && !details.name.is_empty() {
                        let json_str = serde_json::to_string(&details).unwrap_or_default();
                        let _ = storage.cache_catalog_item(
                            &app_key,
                            &details.name,
                            "steam",
                            &app_id.to_string(),
                            &json_str,
                        );

                        games.push(GameItem {
                            app_name: app_key,
                            title: details.name,
                            namespace: "steam".to_string(),
                            catalog_item_id: app_id.to_string(),
                            description: details.short_description.or(Some("Steam Library Game".to_string())),
                            developer: details.developers.and_then(|d| d.first().cloned()),
                            publisher: details.publishers.and_then(|p| p.first().cloned()).or(Some("Steam".to_string())),
                            icon_url: details.capsule_image.or_else(|| {
                                Some(format!(
                                    "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
                                    app_id
                                ))
                            }),
                            banner_url: details.header_image.or_else(|| {
                                Some(format!(
                                    "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg",
                                    app_id
                                ))
                            }),
                            logo_url: Some(format!(
                                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/logo.png",
                                app_id
                            )),
                            is_installed: false,
                            is_dlc: false,
                            build_version: None,
                            install_path: None,
                            install_size: None,
                            executable: None,
                            can_run_offline: true,
                            supports_cloud_saves: true,
                            platform: "steam".to_string(),
                            steam_app_id: Some(app_id),
                        });
                    } else {
                        // Mark tool / advertising / DLC / soundtrack as ignored
                        let _ = storage.cache_catalog_item(
                            &app_key,
                            &details.name,
                            "steam_ignored",
                            &app_id.to_string(),
                            "{}",
                        );
                    }
                }
                _ => {
                    // Store API returned 404 or error - mark as ignored to avoid spamming
                    let _ = storage.cache_catalog_item(
                        &app_key,
                        "Unknown",
                        "steam_ignored",
                        &app_id.to_string(),
                        "{}",
                    );
                }
            }
        }
    }

    // 4. Mark titles that Hyperion downloaded natively (they live outside the Steam
    //    library, so the local appmanifest scan above cannot see them).
    let installed_locally: HashMap<String, crate::models::InstalledGame> = storage
        .get_all_installed_games()
        .unwrap_or_default()
        .into_iter()
        .map(|ig| (ig.app_name.clone(), ig))
        .collect();

    for game in games.iter_mut() {
        if let Some(ig) = installed_locally.get(&game.app_name) {
            game.is_installed = true;
            game.install_path = Some(ig.install_path.clone());
            game.install_size = Some(ig.install_size);
            game.executable = Some(ig.executable.clone());
            game.build_version = Some(ig.version.clone());
        }
    }

    info!("Total Steam games (installed + verified owned): {}", games.len());
    games
}

/// Fallback synchronous scan of Steam games
pub fn scan_all_steam_library_games() -> Vec<GameItem> {
    scan_installed_steam_games()
}

/// Lists the packages SteamCMD installed next to a game.
///
/// A `+app_update` run pulls dependency apps (Steamworks Common
/// Redistributables, shared runtimes, …) into the same `steamapps` folder. Each
/// one leaves its own `appmanifest_<id>.acf`, so the folder is the source of
/// truth for "what else came with this game".
pub fn scan_extra_content(install_dir: &Path, main_app_id: u32) -> Vec<crate::models::SteamExtraContent> {
    let steamapps = install_dir.join("steamapps");
    let mut extras = Vec::new();

    let Ok(entries) = std::fs::read_dir(&steamapps) else {
        return extras;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(id_part) = file_name
            .strip_prefix("appmanifest_")
            .and_then(|rest| rest.strip_suffix(".acf"))
        else {
            continue;
        };
        let Ok(app_id) = id_part.parse::<u32>() else {
            continue;
        };
        if app_id == main_app_id {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let vdf = parse_vdf(&content);
        let Some(state) = vdf.get("AppState") else {
            continue;
        };

        let name = state
            .get_str("name")
            .unwrap_or("Additional content")
            .to_string();
        let installdir = state.get_str("installdir").unwrap_or("");
        let state_flags: u64 = state
            .get_str("StateFlags")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut size_bytes: u64 = state
            .get_str("SizeOnDisk")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if size_bytes == 0 && !installdir.is_empty() {
            size_bytes = dir_size(&steamapps.join("common").join(installdir));
        }

        extras.push(crate::models::SteamExtraContent {
            app_id,
            name,
            size_bytes,
            installed: state_flags & 4 != 0 || size_bytes > 0,
        });
    }

    extras.sort_by_key(|extra| extra.app_id);
    extras
}

/// Total size of everything under a directory (missing folder → 0).
fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += dir_size(&path);
        } else if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    total
}

/// Parses a single appmanifest file into a unified GameItem
fn parse_appmanifest_file(manifest_path: &Path, library_root: &Path) -> Option<GameItem> {
    let content = std::fs::read_to_string(manifest_path).ok()?;
    let root = parse_vdf(&content);
    let app_state = root.get("AppState")?;

    let app_id_str = app_state.get_str("appid")?;
    let app_id: u32 = app_id_str.parse().ok()?;

    // Filter out Steam Common Redistributables, SteamVR runtime, Proton, Steam Linux Runtime
    // (Common Tool App IDs: 228980 is Steamworks Common Redistributables)
    if app_id == 228980 || app_id == 1070560 || app_id == 1391110 {
        return None;
    }

    let title = app_state.get_str("name")?.to_string();
    let installdir = app_state.get_str("installdir").unwrap_or("");
    let size_on_disk: Option<u64> = app_state.get_str("SizeOnDisk").and_then(|s| s.parse().ok());
    let state_flags: u32 = app_state.get_str("StateFlags").and_then(|s| s.parse().ok()).unwrap_or(4);

    // StateFlags bitmask: 4 = Fully Installed, 1024 = Update Required, 2 = Update Running, etc.
    // If state_flags has the 4 bit set or is > 0 and common folder exists, mark as installed
    let install_path = library_root.join("steamapps").join("common").join(installdir);
    let is_installed = (state_flags & 4 != 0) || install_path.exists();

    // High quality Steam CDN artwork (public CDN URLs, zero auth required)
    let icon_url = Some(format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
        app_id
    ));
    let banner_url = Some(format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg",
        app_id
    ));
    let logo_url = Some(format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/logo.png",
        app_id
    ));

    Some(GameItem {
        app_name: format!("steam_{}", app_id),
        title,
        namespace: "steam".to_string(),
        catalog_item_id: format!("steam_{}", app_id),
        description: None,
        developer: None,
        publisher: None,
        icon_url,
        banner_url,
        logo_url,
        is_installed,
        is_dlc: false,
        build_version: None,
        install_path: Some(install_path.to_string_lossy().to_string()),
        install_size: size_on_disk,
        executable: None,
        can_run_offline: true,
        supports_cloud_saves: true,
        platform: "steam".to_string(),
        steam_app_id: Some(app_id),
    })
}

/// Launches a Steam game using the official Steam client protocol
pub fn launch_steam_game(app_id: u32) -> Result<(), String> {
    let url = format!("steam://rungameid/{}", app_id);
    info!("Launching Steam game via protocol: {}", url);
    open::that_detached(&url).map_err(|e| format!("Failed to launch Steam game (AppID {}): {}", app_id, e))
}



#[cfg(test)]
mod tests {
    use super::scan_extra_content;

    fn write_manifest(dir: &std::path::Path, file: &str, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(file), body).unwrap();
    }

    #[test]
    fn lists_dependency_manifests_and_skips_the_game() {
        let root = tempfile::tempdir().unwrap();
        let steamapps = root.path().join("steamapps");

        write_manifest(
            &steamapps,
            "appmanifest_367520.acf",
            r#"
"AppState"
{
    "appid"     "367520"
    "name"      "Hollow Knight"
    "installdir"    "Hollow Knight"
    "StateFlags"    "4"
    "SizeOnDisk"    "500"
}
"#,
        );
        write_manifest(
            &steamapps,
            "appmanifest_228980.acf",
            r#"
"AppState"
{
    "appid"     "228980"
    "name"      "Steamworks Common Redistributables"
    "installdir"    "Steamworks Shared"
    "StateFlags"    "4"
    "SizeOnDisk"    "1234"
}
"#,
        );

        let extras = scan_extra_content(root.path(), 367520);
        assert_eq!(extras.len(), 1);
        assert_eq!(extras[0].app_id, 228980);
        assert_eq!(extras[0].name, "Steamworks Common Redistributables");
        assert_eq!(extras[0].size_bytes, 1234);
        assert!(extras[0].installed);
    }

    #[test]
    fn missing_steamapps_folder_yields_nothing() {
        let root = tempfile::tempdir().unwrap();
        assert!(scan_extra_content(root.path(), 1).is_empty());
    }
}
