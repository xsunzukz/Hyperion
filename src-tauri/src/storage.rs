use crate::error::Result;
use crate::models::{AppSettings, InstalledGame, Session};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct CachedCatalogItem {
    pub app_name: String,
    pub title: String,
    pub namespace: String,
    pub catalog_item_id: String,
    pub json_data: String,
}

pub struct Storage {
    conn: Mutex<Connection>,
    pub db_path: PathBuf,
}

impl Storage {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        info!("Opening database at {}", db_path.display());
        let conn = Connection::open(&db_path)?;
        let storage = Self {
            conn: Mutex::new(conn),
            db_path,
        };
        storage.init_tables()?;
        Ok(storage)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS installed_games (
                app_name TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                version TEXT NOT NULL,
                install_path TEXT NOT NULL,
                executable TEXT NOT NULL,
                launch_parameters TEXT NOT NULL DEFAULT '',
                install_size INTEGER NOT NULL DEFAULT 0,
                can_run_offline INTEGER NOT NULL DEFAULT 1,
                requires_ot INTEGER NOT NULL DEFAULT 0,
                platform TEXT NOT NULL DEFAULT 'Windows',
                is_dlc INTEGER NOT NULL DEFAULT 0,
                base_urls TEXT NOT NULL DEFAULT '[]',
                installed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS catalog_cache (
                app_name TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                namespace TEXT NOT NULL,
                catalog_item_id TEXT NOT NULL,
                json_data TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS auth_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                account_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                refresh_token TEXT NOT NULL,
                session_json TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;

        debug!("Database schema initialized successfully");
        Ok(())
    }

    // Installed Games methods
    pub fn save_installed_game(&self, game: &InstalledGame) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let base_urls_json = serde_json::to_string(&game.base_urls)?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO installed_games 
            (app_name, title, version, install_path, executable, launch_parameters, 
             install_size, can_run_offline, requires_ot, platform, is_dlc, base_urls, installed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, COALESCE(?13, CURRENT_TIMESTAMP))
            "#,
            params![
                game.app_name,
                game.title,
                game.version,
                game.install_path,
                game.executable,
                game.launch_parameters,
                game.install_size as i64,
                if game.can_run_offline { 1 } else { 0 },
                if game.requires_ot { 1 } else { 0 },
                game.platform,
                if game.is_dlc { 1 } else { 0 },
                base_urls_json,
                game.installed_at
            ],
        )?;
        Ok(())
    }

    pub fn get_installed_game(&self, app_name: &str) -> Result<Option<InstalledGame>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT app_name, title, version, install_path, executable, launch_parameters,
                   install_size, can_run_offline, requires_ot, platform, is_dlc, base_urls, installed_at
            FROM installed_games WHERE app_name = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![app_name])?;
        if let Some(row) = rows.next()? {
            let base_urls_str: String = row.get(11)?;
            let base_urls: Vec<String> = serde_json::from_str(&base_urls_str).unwrap_or_default();

            Ok(Some(InstalledGame {
                app_name: row.get(0)?,
                title: row.get(1)?,
                version: row.get(2)?,
                install_path: row.get(3)?,
                executable: row.get(4)?,
                launch_parameters: row.get(5)?,
                install_size: row.get::<_, i64>(6)? as u64,
                can_run_offline: row.get::<_, i32>(7)? != 0,
                requires_ot: row.get::<_, i32>(8)? != 0,
                platform: row.get(9)?,
                is_dlc: row.get::<_, i32>(10)? != 0,
                base_urls,
                installed_at: row.get(12)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_installed_games(&self) -> Result<Vec<InstalledGame>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT app_name, title, version, install_path, executable, launch_parameters,
                   install_size, can_run_offline, requires_ot, platform, is_dlc, base_urls, installed_at
            FROM installed_games ORDER BY title ASC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let base_urls_str: String = row.get(11)?;
            let base_urls: Vec<String> = serde_json::from_str(&base_urls_str).unwrap_or_default();

            Ok(InstalledGame {
                app_name: row.get(0)?,
                title: row.get(1)?,
                version: row.get(2)?,
                install_path: row.get(3)?,
                executable: row.get(4)?,
                launch_parameters: row.get(5)?,
                install_size: row.get::<_, i64>(6)? as u64,
                can_run_offline: row.get::<_, i32>(7)? != 0,
                requires_ot: row.get::<_, i32>(8)? != 0,
                platform: row.get(9)?,
                is_dlc: row.get::<_, i32>(10)? != 0,
                base_urls,
                installed_at: row.get(12)?,
            })
        })?;

        let mut games = Vec::new();
        for r in rows {
            games.push(r?);
        }
        Ok(games)
    }

    pub fn delete_installed_game(&self, app_name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM installed_games WHERE app_name = ?1",
            params![app_name],
        )?;
        Ok(())
    }

    // Catalog Cache methods
    pub fn cache_catalog_item(
        &self,
        app_name: &str,
        title: &str,
        namespace: &str,
        catalog_item_id: &str,
        json_data: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO catalog_cache (app_name, title, namespace, catalog_item_id, json_data, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
            "#,
            params![app_name, title, namespace, catalog_item_id, json_data],
        )?;
        Ok(())
    }

    pub fn get_cached_catalog_items(&self) -> Result<Vec<CachedCatalogItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT app_name, title, namespace, catalog_item_id, json_data FROM catalog_cache",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CachedCatalogItem {
                app_name: row.get(0)?,
                title: row.get(1)?,
                namespace: row.get(2)?,
                catalog_item_id: row.get(3)?,
                json_data: row.get(4)?,
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn get_cached_catalog_item(&self, app_name: &str) -> Result<Option<CachedCatalogItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT app_name, title, namespace, catalog_item_id, json_data FROM catalog_cache WHERE app_name = ?1",
        )?;
        let mut rows = stmt.query(params![app_name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(CachedCatalogItem {
                app_name: row.get(0)?,
                title: row.get(1)?,
                namespace: row.get(2)?,
                catalog_item_id: row.get(3)?,
                json_data: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn clear_steam_catalog_cache(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM catalog_cache WHERE namespace = 'steam' OR namespace = 'steam_ignored' OR app_name LIKE 'steam_%'",
            [],
        )?;
        Ok(())
    }

    // Settings methods
    pub fn get_settings(&self) -> Result<AppSettings> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut settings = AppSettings::default();
        for r in rows {
            let (key, val) = r?;
            match key.as_str() {
                "default_install_dir" => {
                    if val.contains("AppData") || val.trim().is_empty() {
                        settings.default_install_dir = crate::models::detect_default_install_dir();
                    } else {
                        settings.default_install_dir = val;
                    }
                }
                "max_workers" => settings.max_workers = val.parse().unwrap_or(16),
                "throttle_mbps" => settings.throttle_mbps = val.parse().unwrap_or(0),
                "language" => settings.language = val,
                "auto_update" => settings.auto_update = val == "true",
                "skip_update_check" => settings.skip_update_check = val == "true",
                "platform" => settings.platform = val,
                "steam_account" => settings.steam_account = if val.is_empty() { None } else { Some(val) },
                "steam_id" => settings.steam_id = if val.is_empty() { None } else { Some(val) },
                "steam_token" => settings.steam_token = if val.is_empty() { None } else { Some(val) },
                "steam_password_enc" => {
                    settings.steam_password_enc = if val.is_empty() { None } else { Some(val) }
                }
                "steam_library_scan" => settings.steam_library_scan = val == "true",
                _ => {}
            }
        }
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let pairs = [
            ("default_install_dir", settings.default_install_dir.clone()),
            ("max_workers", settings.max_workers.to_string()),
            ("throttle_mbps", settings.throttle_mbps.to_string()),
            ("language", settings.language.clone()),
            ("auto_update", settings.auto_update.to_string()),
            ("skip_update_check", settings.skip_update_check.to_string()),
            ("platform", settings.platform.clone()),
            ("steam_account", settings.steam_account.clone().unwrap_or_default()),
            ("steam_id", settings.steam_id.clone().unwrap_or_default()),
            ("steam_token", settings.steam_token.clone().unwrap_or_default()),
            (
                "steam_password_enc",
                settings.steam_password_enc.clone().unwrap_or_default(),
            ),
            ("steam_library_scan", settings.steam_library_scan.to_string()),
        ];

        for (k, v) in pairs {
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![k, v],
            )?;
        }
        Ok(())
    }

    // Auth State storage
    pub fn save_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let session_json = serde_json::to_string(session)?;
        let display_name = session
            .display_name
            .clone()
            .unwrap_or_else(|| "User".to_string());
        let refresh_token = session.refresh_token.clone().unwrap_or_default();

        conn.execute(
            r#"
            INSERT OR REPLACE INTO auth_state (id, account_id, display_name, refresh_token, session_json, updated_at)
            VALUES (1, ?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
            "#,
            params![session.account_id, display_name, refresh_token, session_json],
        )?;
        Ok(())
    }

    pub fn get_saved_session(&self) -> Result<Option<Session>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT session_json FROM auth_state WHERE id = 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let json_str: String = row.get(0)?;
            let session: Session = serde_json::from_str(&json_str)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    pub fn clear_session(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM auth_state WHERE id = 1", [])?;
        Ok(())
    }
}
