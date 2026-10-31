pub mod api;
pub mod chunk;
pub mod cloud_save;
pub mod commands;
pub mod downloader;
pub mod error;
pub mod installer;
pub mod launcher;
pub mod manifest;
pub mod manifest_cache;
pub mod models;
pub mod secret;
pub mod state;
pub mod steam;
pub mod storage;
pub mod tray;
pub mod winproc;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use tauri::{Manager, WindowEvent};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Keep the main app webview on the bundled UI. The Epic sign-in window is the
/// only webview allowed to navigate to remote hosts; everything else must stay
/// on the Tauri app origin (or localhost during `tauri dev`).
fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("hyperion-navigation")
        .on_navigation(|webview, url| {
            if webview.label() == "epic_auth" {
                return true;
            }

            match url.scheme() {
                "tauri" | "about" => true,
                "http" | "https" => {
                    url.host_str() == Some("tauri.localhost")
                        || (cfg!(debug_assertions) && url.host_str() == Some("localhost"))
                }
                _ => false,
            }
        })
        .build()
}

pub fn run() {
    // Structured logging initialization
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hyperion=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();

    tracing::info!("Initializing Hyperion backend...");

    let app_state = match state::AppState::new() {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("Failed to initialize app state: {}", e);
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        // Must be registered before any other plugin, so a second launch is
        // rejected before it touches the database, the tray or SteamCMD.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tracing::info!("Second launch forwarded to the running instance");
            crate::tray::reveal_main_window(app);
        }))
        .plugin(navigation_guard())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::auth_get_login_url,
            commands::auth_start_automatic_login,
            commands::auth_cancel_automatic_login,
            commands::auth_login_code,
            commands::auth_login_exchange,
            commands::auth_resume_session,
            commands::auth_logout,
            commands::auth_get_profile,
            commands::catalog_get_games,
            commands::app_bootstrap,
            commands::catalog_get_game_details,
            commands::game_install,
            commands::game_cancel_download,
            commands::game_pause_download,
            commands::game_resume_download,
            commands::game_get_download_progress,
            commands::game_get_all_downloads,
            commands::game_verify,
            commands::game_uninstall,
            commands::game_launch,
            commands::game_get_running,
            commands::game_adopt_running,
            commands::game_stop,
            commands::cloud_save_get_status,
            commands::cloud_save_sync,
            commands::settings_get,
            commands::settings_save,
            commands::diagnostics_get,
            commands::mystic_launcher_status,
            commands::launch_mystic_launcher,
            commands::open_path,
            commands::select_manifest_file,
            commands::select_directory,
            commands::manifest_inspect,
            commands::manifest_prefetch,
            commands::game_install_custom_manifest,
            commands::game_repair,
            commands::steam_get_installed_games,
            commands::steam_extra_content,
            commands::steam_launch_game,
            commands::steam_search_games,
            commands::steam_get_details,
            commands::steam_install_custom,
            commands::steam_get_auth_status,
            commands::steam_prepare_engine,
            commands::steam_logout,
            commands::steam_get_credential_status,
            commands::steam_set_credentials,
            commands::steam_clear_credentials,
            commands::steam_set_guard_code,
        ])
        .setup(|app| {
            // Tray icon, plus the loop that keeps its menu listing the active
            // downloads and the installed games.
            crate::tray::setup(app.handle())?;

            tracing::info!("Hyperion Tauri application setup completed");
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing normally parks the launcher in the tray: downloads, the
            // Steam engine and the running-game watcher all keep going. This is
            // a preference, so turning it off lets the window close and the
            // process exit like a normal desktop app. Minimising stays a normal
            // minimise — the taskbar is still where people expect it.
            WindowEvent::CloseRequested { api, .. } => {
                let close_to_tray = window
                    .app_handle()
                    .state::<Arc<crate::state::AppState>>()
                    .storage
                    .get_settings()
                    .map(|settings| settings.close_to_tray)
                    .unwrap_or(true);

                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    tracing::info!("Close-to-tray is disabled; exiting Hyperion");
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running hyperion application");
}
