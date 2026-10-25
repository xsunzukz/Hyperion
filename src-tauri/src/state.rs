use crate::api::EpicApiClient;
use crate::downloader::DownloadManager;
use crate::error::Result;
use crate::models::{RunningGame, SteamCredentials};
use crate::storage::Storage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Root of Hyperion's per-user data: `%LOCALAPPDATA%/Hyperion`.
///
/// Installs that predate the rename used `MysticHyperion`; on first launch the
/// old folder (and its database) is moved across so nothing is lost.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("data"));
    let dir = base.join("Hyperion");

    if !dir.exists() {
        let legacy = base.join("MysticHyperion");
        if legacy.exists() {
            if std::fs::rename(&legacy, &dir).is_ok() {
                tracing::info!("Migrated {:?} to {:?}", legacy, dir);
            }
        }
    }

    // Database file was renamed too
    let legacy_db = dir.join("mystichyperion.db");
    let db = dir.join("hyperion.db");
    if legacy_db.exists() && !db.exists() {
        let _ = std::fs::rename(&legacy_db, &db);
    }

    dir
}

pub struct AppState {
    pub api_client: Arc<EpicApiClient>,
    pub storage: Arc<Storage>,
    pub downloader: Arc<DownloadManager>,
    pub steam_downloader: Arc<crate::steam::SteamDownloadManager>,
    /// In-memory credentials for the headless Steam engine (never persisted)
    pub steam_credentials: Mutex<Option<SteamCredentials>>,
    /// Games the launcher started, keyed by app_name (polls for liveness)
    pub running_games: Mutex<HashMap<String, RunningGame>>,
    /// Held while the Epic catalog is fetched live, so overlapping refreshes
    /// (sign-in fires two) share one fetch instead of doubling the requests.
    pub catalog_refresh: Mutex<()>,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let app_dir = data_dir();
        std::fs::create_dir_all(&app_dir).ok();

        let db_path = app_dir.join("hyperion.db");
        let storage = Arc::new(Storage::new(db_path)?);
        let api_client = Arc::new(EpicApiClient::new());
        // A session renewed mid-run is saved right away, so the next start
        // resumes with the new refresh token rather than the spent one.
        let session_store = storage.clone();
        api_client.set_session_sink(move |session| {
            if let Err(e) = session_store.save_session(session) {
                tracing::warn!("Could not save the renewed Epic session: {}", e);
            }
        });
        let downloader = Arc::new(DownloadManager::new());
        let steam_downloader = Arc::new(crate::steam::SteamDownloadManager::new());

        // Restore a remembered Steam session (DPAPI-protected) if one exists
        let remembered = crate::secret::load_credentials(&storage);

        Ok(Self {
            api_client,
            storage,
            downloader,
            steam_downloader,
            steam_credentials: Mutex::new(remembered),
            running_games: Mutex::new(HashMap::new()),
            catalog_refresh: Mutex::new(()),
        })
    }
}
