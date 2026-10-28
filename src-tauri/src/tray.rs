//! System tray: a live menu with the active downloads and the installed games.
//!
//! Two rules keep the menu usable, and they matter more than the data does:
//!
//! 1. **Never replace an open menu.** `set_menu()` destroys the native popup, so
//!    a periodic rebuild reads as the tray menu "closing itself". Rows are
//!    therefore updated in place with `MenuItem::set_text()`, and the menu is
//!    only rebuilt when rows are actually added or removed.
//! 2. **Never rebuild right after the user touched the tray.** There is no
//!    menu-opened/closed event in Tauri 2, so a short grace window after any
//!    tray interaction stands in for it: a download starting just as you
//!    right-click cannot yank the menu out from under you.

use crate::models::{DownloadProgress, DownloadStatus};
use crate::state::AppState;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

/// Identifier used to look the tray icon back up when refreshing its menu.
pub const TRAY_ID: &str = "hyperion-tray";

/// Upper bound on games listed inline; the window shows the full library.
const MAX_GAMES: usize = 30;

/// How often the tray re-reads state. Cheap: two in-memory locks and one query.
const REFRESH_INTERVAL: Duration = Duration::from_millis(1000);

/// How long to leave the menu alone after the user interacts with the tray.
const INTERACTION_GRACE: Duration = Duration::from_secs(8);

/// One download line: `key` is stable identity, `label` is what is displayed.
struct Row {
    key: String,
    label: String,
}

/// Relabel bookkeeping, deliberately free of menu types so it stays unit
/// testable: a test binary that links the native menu code fails to start on
/// Windows, because only the real app carries Tauri's Common Controls v6
/// manifest.
#[derive(Default)]
struct RelabelState {
    /// Set when a relabel was refused, so the next safe tick rebuilds instead of
    /// leaving a row stuck on stale text.
    force_rebuild: AtomicBool,
    /// Throttles warnings so a platform quirk cannot log once a second.
    warned_at: Mutex<Option<Instant>>,
}

impl RelabelState {
    /// Reports a refused relabel once a minute, and asks for a rebuild.
    fn note_failure(&self, error: &impl std::fmt::Display) {
        self.force_rebuild.store(true, Ordering::Relaxed);

        let mut last = self.warned_at.lock().unwrap_or_else(|e| e.into_inner());

        let due = last
            .map(|at| at.elapsed() >= Duration::from_secs(60))
            .unwrap_or(true);

        if due {
            tracing::warn!(
                "Could not relabel a tray row ({}); rebuilding the menu instead. \
                 Further relabel errors are suppressed for a minute.",
                error
            );
            *last = Some(Instant::now());
        }
    }
}

/// Menu handles kept alive so rows can be re-labelled without rebuilding.
#[derive(Default)]
struct TrayMenu {
    rows: Mutex<Vec<MenuItem<Wry>>>,
    signature: Mutex<String>,
    last_interaction_ms: AtomicU64,
    relabel: RelabelState,
}

impl TrayMenu {
    fn note_interaction(&self) {
        self.last_interaction_ms
            .store(now_ms(), Ordering::Relaxed);
    }

    fn interacted_recently(&self) -> bool {
        now_ms().saturating_sub(self.last_interaction_ms.load(Ordering::Relaxed))
            < INTERACTION_GRACE.as_millis() as u64
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Everything the menu displays at one point in time.
#[derive(Default)]
struct Snapshot {
    rows: Vec<Row>,
    games: Vec<(String, String)>,
}

impl Snapshot {
    /// Identity of the menu's *structure* — deliberately excludes percentages,
    /// speeds and statuses, which change constantly and are handled by
    /// re-labelling rows instead of rebuilding the menu.
    fn signature(&self) -> String {
        let mut signature = String::new();

        for row in &self.rows {
            signature.push_str(&row.key);
            signature.push('\u{1}');
        }

        signature.push('\u{2}');

        for (app_name, title) in &self.games {
            signature.push_str(app_name);
            signature.push('\u{1}');
            signature.push_str(title);
            signature.push('\u{1}');
        }

        signature
    }
}

/// Builds the tray icon and starts the loop that keeps its menu current.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    app.manage(TrayMenu::default());

    // Placeholder menu; the first refresh fills it in immediately.
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "show",
        "Show Hyperion",
        true,
        None::<&str>,
    )?)?;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Hyperion Launcher")
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event);

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    spawn_refresher(app.clone());

    Ok(())
}

/// Pulls the current downloads and library out of app state.
async fn snapshot(app: &AppHandle) -> Snapshot {
    let state = app.state::<Arc<AppState>>();

    let mut rows: Vec<Row> = state
        .downloader
        .get_all_active()
        .await
        .iter()
        .map(|progress| Row {
            key: format!("epic:{}", progress.app_name),
            label: format_download(progress),
        })
        .collect();

    // Steam installs are driven by SteamCMD outside the Epic download manager.
    for (app_id, title, percent) in state.steam_downloader.active_downloads().await {
        rows.push(Row {
            key: format!("steam:{}", app_id),
            label: match percent {
                Some(percent) => format!("{} — Steam · {:.0}%", title, percent.clamp(0.0, 100.0)),
                // Before the first poll tick there is no percentage yet.
                None => format!("{} — Steam (AppID {})", title, app_id),
            },
        });
    }

    let mut games: Vec<(String, String)> = state
        .storage
        .get_all_installed_games()
        .unwrap_or_default()
        .into_iter()
        .map(|game| (game.app_name, game.title))
        .collect();
    games.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

    Snapshot { rows, games }
}

/// One row of the downloads section.
fn format_download(progress: &DownloadProgress) -> String {
    let percent = progress.progress_percent.clamp(0.0, 100.0);
    let detail = match progress.status {
        DownloadStatus::Downloading => format!(
            "{:.0}% · {}",
            percent,
            human_speed(progress.speed_bytes_per_sec)
        ),
        DownloadStatus::Paused => format!("paused at {:.0}%", percent),
        DownloadStatus::Verifying => "verifying files".to_string(),
        DownloadStatus::Analyzing => "preparing".to_string(),
        DownloadStatus::Queued => "queued".to_string(),
        DownloadStatus::Installing => "installing".to_string(),
        DownloadStatus::Completed => "finishing".to_string(),
        DownloadStatus::Failed => "failed".to_string(),
        DownloadStatus::Cancelled => "cancelled".to_string(),
    };

    format!("{} — {}", progress.title, detail)
}

fn human_speed(bytes_per_sec: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    let mib = bytes_per_sec as f64 / MIB;

    if mib >= 1.0 {
        format!("{:.1} MB/s", mib)
    } else {
        format!("{:.0} KB/s", bytes_per_sec as f64 / 1024.0)
    }
}

/// Renders a snapshot into a menu, returning the row handles for later
/// re-labelling.
fn build_menu(app: &AppHandle, snapshot: &Snapshot) -> tauri::Result<(Menu<Wry>, Vec<MenuItem<Wry>>)> {
    let menu = Menu::new(app)?;
    let mut rows = Vec::with_capacity(snapshot.rows.len());

    menu.append(&MenuItem::with_id(
        app,
        "show",
        "Show Hyperion",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    if snapshot.rows.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "no-downloads",
            "No active downloads",
            false,
            None::<&str>,
        )?)?;
    } else {
        menu.append(&MenuItem::with_id(
            app,
            "downloads-header",
            format!("Downloads ({})", snapshot.rows.len()),
            false,
            None::<&str>,
        )?)?;

        for (index, row) in snapshot.rows.iter().enumerate() {
            let item = MenuItem::with_id(
                app,
                format!("download:{index}"),
                &row.label,
                true,
                None::<&str>,
            )?;
            menu.append(&item)?;
            rows.push(item);
        }
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    if snapshot.games.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "no-games",
            "No games installed",
            false,
            None::<&str>,
        )?)?;
    } else {
        let submenu = Submenu::with_id(
            app,
            "games",
            format!("Installed games ({})", snapshot.games.len()),
            true,
        )?;

        for (app_name, title) in snapshot.games.iter().take(MAX_GAMES) {
            submenu.append(&MenuItem::with_id(
                app,
                format!("game:{app_name}"),
                title,
                true,
                None::<&str>,
            )?)?;
        }

        if snapshot.games.len() > MAX_GAMES {
            let rest = snapshot.games.len() - MAX_GAMES;
            submenu.append(&PredefinedMenuItem::separator(app)?)?;
            submenu.append(&MenuItem::with_id(
                app,
                "more-games",
                format!("…and {rest} more in the launcher"),
                false,
                None::<&str>,
            )?)?;
        }

        menu.append(&submenu)?;
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        "Quit Hyperion",
        true,
        None::<&str>,
    )?)?;

    Ok((menu, rows))
}

/// Keeps row labels current, and rebuilds only when rows are added or removed.
fn spawn_refresher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(REFRESH_INTERVAL).await;

            let snapshot = snapshot(&app).await;
            let signature = snapshot.signature();
            let state = app.state::<TrayMenu>();

            let structure_changed = state.relabel.force_rebuild.load(Ordering::Relaxed) || {
                let current = state.signature.lock().unwrap_or_else(|e| e.into_inner());
                *current != signature
            };

            if structure_changed {
                // Replacing the menu closes it if it is open, so wait out the
                // grace window and retry on the next tick.
                if state.interacted_recently() {
                    continue;
                }

                match build_menu(&app, &snapshot) {
                    Ok((menu, rows)) => {
                        let Some(tray) = app.tray_by_id(TRAY_ID) else {
                            tracing::warn!("Tray icon {} disappeared", TRAY_ID);
                            continue;
                        };

                        if let Err(e) = tray.set_menu(Some(menu)) {
                            tracing::warn!("Could not update the tray menu: {}", e);
                            continue;
                        }

                        *state.rows.lock().unwrap_or_else(|e| e.into_inner()) = rows;
                        *state.signature.lock().unwrap_or_else(|e| e.into_inner()) = signature;
                        state.relabel.force_rebuild.store(false, Ordering::Relaxed);
                    }
                    Err(e) => tracing::warn!("Could not build the tray menu: {}", e),
                }

                continue;
            }

            // Same rows: relabel in place. This keeps percentages live without
            // touching the menu itself, so an open menu stays open. It is still
            // skipped inside the grace window, because even a text change reaches
            // the native popup and could disturb it mid-interaction.
            if state.interacted_recently() {
                continue;
            }

            let rows = state.rows.lock().unwrap_or_else(|e| e.into_inner());

            for (item, row) in rows.iter().zip(snapshot.rows.iter()) {
                let unchanged = item.text().map(|text| text == row.label).unwrap_or(false);

                if !unchanged {
                    // A refused relabel used to vanish silently and leave the row
                    // on stale text forever; now it is logged and the next safe
                    // tick rebuilds the menu so the text is correct again.
                    if let Err(e) = item.set_text(&row.label) {
                        state.relabel.note_failure(&e);
                    }
                }
            }
        }
    });
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    app.state::<TrayMenu>().note_interaction();

    let id = event.id.as_ref().to_string();

    match id.as_str() {
        "show" => reveal_main_window(app),
        "quit" => {
            tracing::info!("Quit requested from the tray");
            app.exit(0);
        }
        _ => match id.strip_prefix("game:") {
            // Dauntless must go through Mystic Paradox's launcher, never
            // straight to the game executable. Missing launcher -> releases.
            Some("Jackal") => {
                if let Err(e) = crate::commands::launch_mystic_launcher_process() {
                    tracing::warn!(
                        "Mystic Paradox Launcher could not start ({e}); opening releases"
                    );
                    if let Err(open_err) = crate::commands::open_mystic_paradox_releases() {
                        tracing::warn!("Could not open Mystic Paradox releases: {}", open_err);
                    }
                }
            }
            // Clicking an installed game starts it, like the launcher's PLAY.
            Some(app_name) => {
                let app_name = app_name.to_string();
                let handle = app.clone();

                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<Arc<AppState>>();

                    match crate::commands::game_launch(app_name.clone(), false, Vec::new(), state)
                        .await
                    {
                        Ok(pid) => tracing::info!(
                            "Launched '{}' from the tray (pid {})",
                            app_name,
                            pid
                        ),
                        Err(e) => {
                            tracing::warn!("Tray launch of '{}' failed: {}", app_name, e);
                            // Surface the reason instead of failing silently.
                            reveal_main_window(&handle);
                        }
                    }
                });
            }
            // Download rows (and anything unknown) just raise the window.
            None => reveal_main_window(app),
        },
    }
}

fn on_tray_icon_event(tray: &TrayIcon<Wry>, event: TrayIconEvent) {
    // Any interaction (including the right click that opens the menu) suppresses
    // rebuilds briefly — there is no menu-opened event to rely on.
    tray.app_handle().state::<TrayMenu>().note_interaction();

    // Left click toggles the window; the menu stays on right click.
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        let app = tray.app_handle();
        let visible = app
            .get_webview_window("main")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);

        if visible {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        } else {
            reveal_main_window(app);
        }
    }
}

/// Brings the main window back from the tray.
pub fn reveal_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // A window that was minimised then hidden stays minimised internally,
        // so restore before showing or it comes back still collapsed.
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: &str, label: &str) -> Row {
        Row {
            key: key.to_string(),
            label: label.to_string(),
        }
    }

    /// The bug this replaces: percentages live in the label, so a signature that
    /// included them changed on every tick and rebuilt (and closed) the menu.
    #[test]
    fn signature_ignores_volatile_row_text() {
        let mut before = Snapshot::default();
        before
            .rows
            .push(row("epic:dauntless", "Dauntless — 10% · 1.0 MB/s"));

        let mut after = Snapshot::default();
        after
            .rows
            .push(row("epic:dauntless", "Dauntless — 42% · 12.3 MB/s"));

        assert_eq!(before.signature(), after.signature());
    }

    #[test]
    fn signature_changes_when_rows_or_games_change() {
        let mut base = Snapshot::default();
        base.rows.push(row("epic:dauntless", "Dauntless — 10%"));

        let mut extra_row = Snapshot::default();
        extra_row.rows.push(row("epic:dauntless", "Dauntless — 10%"));
        extra_row.rows.push(row("steam:730", "Counter-Strike — Steam"));
        assert_ne!(base.signature(), extra_row.signature());

        let mut renamed = Snapshot::default();
        renamed.rows.push(row("epic:other", "Dauntless — 10%"));
        assert_ne!(base.signature(), renamed.signature());

        let mut with_game = Snapshot::default();
        with_game.rows.push(row("epic:dauntless", "Dauntless — 10%"));
        with_game
            .games
            .push(("control".to_string(), "Control".to_string()));
        assert_ne!(base.signature(), with_game.signature());
    }

    /// A refused relabel must not vanish: it schedules a rebuild.
    #[test]
    fn a_refused_relabel_requests_a_rebuild() {
        let relabel = RelabelState::default();
        assert!(!relabel.force_rebuild.load(Ordering::Relaxed));

        relabel.note_failure(&"platform said no");
        assert!(relabel.force_rebuild.load(Ordering::Relaxed));
    }

    /// The warning is throttled, so a persistent failure cannot log every tick.
    #[test]
    fn relabel_warnings_are_throttled() {
        let relabel = RelabelState::default();
        relabel.note_failure(&"first");

        let first = *relabel
            .warned_at
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert!(first.is_some());

        relabel.note_failure(&"second");

        let second = *relabel
            .warned_at
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            first, second,
            "a second failure inside a minute must not re-log"
        );
    }
}
