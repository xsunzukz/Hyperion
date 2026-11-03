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
pub mod mystic;
pub mod network;
pub mod secret;
pub mod state;
pub mod steam;
pub mod storage;
pub mod tray;
pub mod winproc;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use tauri::{Emitter, Manager, WindowEvent};
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
            commands::game_get_download_networks,
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
            commands::networks_list,
            commands::mystic_launcher_status,
            commands::launch_mystic_launcher,
            commands::dauntless_install,
            commands::dauntless_pick_existing,
            commands::app_hide_to_tray,
            commands::app_quit,
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
        .on_window_event(|window, event| {
            // Closing quits Hyperion like any desktop app. While a download is
            // running the window stays open and the UI asks first: keep
            // downloading in the tray, or quit. The "always keep running in the
            // tray" preference skips the question.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                api.prevent_close();

                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<Arc<crate::state::AppState>>();
                    let always_tray = state
                        .storage
                        .get_settings()
                        .map(|settings| settings.close_to_tray)
                        .unwrap_or(false);
                    let active = crate::commands::active_download_count(&state).await;

                    if always_tray {
                        if let Some(main) = app.get_webview_window("main") {
                            let _ = main.hide();
                        }
                    } else if active > 0 {
                        tracing::info!("Close requested with {} active download(s); asking", active);
                        let _ = app.emit_to("main", "close-requested", active);
                    } else {
                        tracing::info!("Window closed; exiting Hyperion");
                        app.exit(0);
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running hyperion application");
}
