use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::models::{DownloadProgress, DownloadStatus, InstalledGame};

/// Steam Store search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamStoreItem {
    pub id: u32,
    pub name: String,
    pub tiny_image: Option<String>,
    pub header_image: Option<String>,
    pub capsule_image: Option<String>,
    pub price: Option<String>,
}

/// Detailed Steam app information from Store API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamStoreDetails {
    pub id: u32,
    pub name: String,
    pub app_type: Option<String>,
    pub short_description: Option<String>,
    pub header_image: Option<String>,
    pub capsule_image: Option<String>,
    pub developers: Option<Vec<String>>,
    pub publishers: Option<Vec<String>>,
}

/// Searches the Steam Store for games by title or keywords
pub async fn search_steam_store(query: &str) -> Result<Vec<SteamStoreItem>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://store.steampowered.com/api/storesearch/")
        .query(&[("term", query), ("l", "english"), ("cc", "US")])
        .send()
        .await
        .map_err(|e| format!("Steam Store search failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Steam Store returned HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Steam Store search JSON: {}", e))?;

    let mut items = Vec::new();
    if let Some(arr) = json.get("items").and_then(|v| v.as_array()) {
        for entry in arr {
            if let Some(id) = entry.get("id").and_then(|v| v.as_u64()) {
                let id_u32 = id as u32;
                let name = entry
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown Game")
                    .to_string();
                let tiny = entry
                    .get("tiny_image")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let price = entry.get("price").and_then(|p| {
                    if let Some(fin) = p.get("final").and_then(|v| v.as_i64()) {
                        if fin == 0 {
                            Some("Free to Play".to_string())
                        } else {
                            Some(format!("${:.2}", fin as f64 / 100.0))
                        }
                    } else {
                        None
                    }
                });

                items.push(SteamStoreItem {
                    id: id_u32,
                    name,
                    tiny_image: tiny,
                    header_image: Some(format!(
                        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg",
                        id_u32
                    )),
                    capsule_image: Some(format!(
                        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
                        id_u32
                    )),
                    price,
                });
            }
        }
    }

    Ok(items)
}

/// Retrieves game details from Steam's official public appdetails API
pub async fn get_steam_app_details(app_id: u32) -> Result<Option<SteamStoreDetails>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://store.steampowered.com/api/appdetails")
        .query(&[("appids", &app_id.to_string())])
        .send()
        .await
        .map_err(|e| format!("Steam app details request failed: {}", e))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Steam app details: {}", e))?;

    let app_str = app_id.to_string();
    let app_obj = match json.get(&app_str) {
        Some(v) => v,
        None => return Ok(None),
    };

    if !app_obj.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Ok(None);
    }

    let data = match app_obj.get("data") {
        Some(d) => d,
        None => return Ok(None),
    };

    let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let app_type = data.get("type").and_then(|v| v.as_str()).map(|s| s.to_string());
    let short_desc = data
        .get("short_description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let header_image = data
        .get("header_image")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let developers = data.get("developers").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });
    let publishers = data.get("publishers").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });

    Ok(Some(SteamStoreDetails {
        id: app_id,
        name,
        app_type,
        short_description: short_desc,
        header_image: header_image.or_else(|| {
            Some(format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg",
                app_id
            ))
        }),
        capsule_image: Some(format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
            app_id
        )),
        developers,
        publishers,
    }))
}

/// Tracks active SteamCMD download processes
pub struct SteamDownloadManager {
    /// Maps app_id -> process ID (PID)
    active_pids: Arc<Mutex<HashMap<u32, u32>>>,
    /// AppIDs whose download should stop (covers Steam client mirrors that have no PID)
    cancelled: Arc<Mutex<HashSet<u32>>>,
    /// Set when the cached SteamCMD session is rejected, so the next attempt
    /// falls back to the password instead of retrying the stale token.
    session_invalid: Arc<Mutex<bool>>,
    /// Titles for in-flight installs, so the tray can name what is downloading
    /// instead of showing a bare AppID.
    active_titles: Arc<Mutex<HashMap<u32, String>>>,
    /// Latest SteamCMD percentage per AppID, for the same reason.
    active_percent: Arc<Mutex<HashMap<u32, f32>>>,
}

impl SteamDownloadManager {
    pub fn new() -> Self {
        Self {
            active_pids: Arc::new(Mutex::new(HashMap::new())),
            cancelled: Arc::new(Mutex::new(HashSet::new())),
            session_invalid: Arc::new(Mutex::new(false)),
            active_titles: Arc::new(Mutex::new(HashMap::new())),
            active_percent: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Steam installs currently running, sorted by AppID, with a percentage once
    /// the SteamCMD poller has reported one.
    pub async fn active_downloads(&self) -> Vec<(u32, String, Option<f32>)> {
        let titles = self.active_titles.lock().await;
        let percent = self.active_percent.lock().await;

        let mut list: Vec<(u32, String, Option<f32>)> = titles
            .iter()
            .map(|(id, title)| (*id, title.clone(), percent.get(id).copied()))
            .collect();
        list.sort_by_key(|(id, _, _)| *id);
        list
    }

    /// Location SteamCMD keeps its own installation, config and cached session.
    fn steamcmd_dir() -> PathBuf {
        crate::state::data_dir().join("steamcmd")
    }
}

/// Serialises the one-time SteamCMD install.
///
/// Two downloads started together used to race on the same `steamcmd.zip` and
/// extract over each other, which can leave a truncated engine on disk that then
/// looks installed forever.
static STEAMCMD_INSTALL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Valve's official SteamCMD archive (~775 KB; the engine self-updates on first run).
const STEAMCMD_ZIP_URL: &str = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip";

impl SteamDownloadManager {
    /// True when the *engine* is present, not just the downloaded bootstrapper.
    ///
    /// Valve's `steamcmd.zip` contains one file — a ~1.6 MB `steamcmd.exe` — and
    /// that stub downloads the real engine (`steamclient.dll`, `bin\`, `package\`,
    /// ~30 MB) the first time it runs. Treating the stub as "installed" made the
    /// first Steam sign-in pay for that download inside the 180s Steam Guard
    /// window, so the login timed out.
    pub fn steamcmd_complete() -> bool {
        let dir = Self::steamcmd_dir();
        dir.join("steamcmd.exe").exists()
            && (dir.join("steamclient.dll").exists() || dir.join("steamclient64.dll").exists())
    }

    /// True when the engine is already installed for this user.
    pub fn steamcmd_installed() -> bool {
        Self::steamcmd_complete()
    }

    /// Ensures the full SteamCMD engine is downloaded and ready to sign in.
    pub async fn ensure_steamcmd() -> Result<PathBuf, String> {
        let steamcmd_dir = Self::steamcmd_dir();
        let steamcmd_exe = steamcmd_dir.join("steamcmd.exe");

        if Self::steamcmd_complete() {
            return Ok(steamcmd_exe);
        }

        // One install at a time. Whoever waited re-checks below and, if the other
        // call already finished it, skips the work entirely.
        let _installing = STEAMCMD_INSTALL.lock().await;
        if Self::steamcmd_complete() {
            return Ok(steamcmd_exe);
        }

        tokio::fs::create_dir_all(&steamcmd_dir)
            .await
            .map_err(|e| format!("Failed to create steamcmd directory: {}", e))?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| format!("Could not create the SteamCMD download client: {}", e))?;

        let mut last_error = String::from("unknown error");

        for attempt in 1..=3 {
            if attempt > 1 {
                tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
            }

            // The bootstrapper may already be on disk from an earlier launch, so
            // extraction is conditional; the first-run engine download is not.
            let result = async {
                if !steamcmd_exe.exists() {
                    Self::install_steamcmd(&client, &steamcmd_dir, &steamcmd_exe).await?;
                }

                Self::bootstrap_engine(&steamcmd_exe).await?;

                if !Self::steamcmd_complete() {
                    return Err(
                        "the engine files are still missing after its first run".to_string()
                    );
                }

                Ok(())
            }
            .await;

            match result {
                Ok(()) => {
                    info!(
                        "Valve SteamCMD engine is ready: {:?}",
                        steamcmd_exe
                    );
                    return Ok(steamcmd_exe);
                }
                Err(e) => {
                    warn!("SteamCMD setup attempt {}/3 failed: {}", attempt, e);
                    last_error = e;
                }
            }
        }

        Err(format!(
            "Could not install SteamCMD after 3 attempts: {}. Check the connection (or an antivirus quarantine) and retry.",
            last_error
        ))
    }

    /// Runs the freshly extracted bootstrapper once so it fetches the real engine.
    ///
    /// `steamcmd +quit` performs the update ("Checking for available updates…")
    /// and exits. Doing it here means the first sign-in starts against a complete
    /// engine instead of downloading 30 MB while the Steam Guard prompt waits.
    async fn bootstrap_engine(steamcmd_exe: &Path) -> Result<(), String> {
        if Self::steamcmd_complete() {
            return Ok(());
        }

        info!("First run: letting SteamCMD fetch its engine (~30 MB)…");

        let mut cmd = tokio::process::Command::new(steamcmd_exe);
        cmd.arg("+quit")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Could not start the SteamCMD bootstrap: {}", e))?;

        // Generous: a slow link can take minutes for the engine plus its update check.
        let status = tokio::time::timeout(std::time::Duration::from_secs(900), child.wait())
            .await
            .map_err(|_| "SteamCMD bootstrap timed out after 15 minutes".to_string())?
            .map_err(|e| format!("SteamCMD bootstrap failed: {}", e))?;

        info!(
            "SteamCMD bootstrap finished (exit {:?})",
            status.code()
        );

        Ok(())
    }

    /// One download + extract round trip. Leaves nothing half-written on failure.
    async fn install_steamcmd(
        client: &reqwest::Client,
        steamcmd_dir: &Path,
        steamcmd_exe: &Path,
    ) -> Result<(), String> {
        let zip_path = steamcmd_dir.join("steamcmd.zip");
        let _ = tokio::fs::remove_file(&zip_path).await;

        let resp = client
            .get(STEAMCMD_ZIP_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to download steamcmd.zip from Valve CDN: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Valve CDN returned error code: {}", resp.status()));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read steamcmd.zip bytes: {}", e))?;

        // The real archive is ~775 KB. Anything tiny is an error page or a body
        // that was cut off, and extracting it would produce a broken engine.
        if bytes.len() < 100_000 {
            return Err(format!(
                "steamcmd.zip looks truncated ({} bytes)",
                bytes.len()
            ));
        }

        info!("Extracting steamcmd.zip ({} bytes)...", bytes.len());

        let dest = steamcmd_dir.to_path_buf();
        let extracted = tokio::task::spawn_blocking(move || extract_zip(&bytes, &dest))
            .await
            .map_err(|e| format!("steamcmd.zip extraction did not finish: {}", e))?;
        if let Err(e) = extracted {
            // A half-extracted tree can leave a truncated steamcmd.exe behind;
            // drop it so the next attempt does not mistake it for a real install.
            let _ = tokio::fs::remove_file(steamcmd_exe).await;
            return Err(e);
        }

        let _ = tokio::fs::remove_file(&zip_path).await;

        if !steamcmd_exe.exists() {
            return Err("steamcmd.exe was not produced after extraction".to_string());
        }

        Ok(())
    }

    /// Checks a Steam account+password by actually signing in with SteamCMD.
    ///
    /// Valve has no "verify password" API, so this is the same login a download
    /// would perform — minus `+app_update`, so nothing is installed. Callers use
    /// it to reject wrong credentials before anything is stored or downloaded.
    ///
    /// Steam Guard approvals arrive on the user's phone and can take a while, so
    /// each attempt keeps the process alive (stdin open, output streamed) and we
    /// retry a couple of times if the approval did not land in time — SteamCMD
    /// gives up on its own timer, which a single attempt cannot extend.
    pub async fn verify_login(
        username: &str,
        password: &str,
        guard_code: Option<&str>,
    ) -> Result<(), String> {
        const ATTEMPTS: usize = 3;
        // Hard ceiling across all attempts, so a forgotten dialog cannot spin forever
        const OVERALL_BUDGET: std::time::Duration = std::time::Duration::from_secs(300);

        let username = username.trim();
        if username.is_empty() || password.is_empty() {
            return Err("Steam account name and password are both required".to_string());
        }

        let started = tokio::time::Instant::now();
        let mut last_error = String::new();

        // A typed Guard code is single-use and rotates every 30 seconds, so a
        // retry could only ever replay a code that is already dead. Only the
        // "waiting for the phone approval" path is worth repeating.
        let typed_code = guard_code.map(|c| !c.trim().is_empty()).unwrap_or(false);
        let attempts = if typed_code { 1 } else { ATTEMPTS };

        for attempt in 1..=attempts {
            match Self::login_once(username, password, guard_code).await {
                Ok(()) => return Ok(()),
                Err(reason) => {
                    // A wrong password, a rate limit or a network problem will not
                    // be fixed by asking Valve again — only the pending approval can.
                    let retryable = !typed_code && mentions_guard(&reason);
                    last_error = reason;

                    if !retryable
                        || attempt == attempts
                        || started.elapsed() + std::time::Duration::from_secs(10) >= OVERALL_BUDGET
                    {
                        break;
                    }

                    info!(
                        "Steam sign-in not confirmed (attempt {}/{}): {} — asking again",
                        attempt, attempts, last_error
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
            }
        }

        Err(last_error)
    }

    /// One SteamCMD login round-trip, waiting up to three minutes for the
    /// Steam Guard approval to arrive on the user's phone.
    async fn login_once(
        username: &str,
        password: &str,
        guard_code: Option<&str>,
    ) -> Result<(), String> {
        const APPROVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

        let steamcmd_exe = Self::ensure_steamcmd().await?;

        let mut cmd = Command::new(&steamcmd_exe);
        cmd.arg("+login").arg(username).arg(password);
        if let Some(code) = guard_code.map(str::trim).filter(|c| !c.is_empty()) {
            cmd.arg(code);
        }
        cmd.arg("+quit")
            // stdin stays open: SteamCMD reads the Guard prompt from it, and an
            // immediate EOF makes it abort before the phone approval lands.
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Could not start SteamCMD to check the sign-in: {}", e))?;

        let mut stdin = child.stdin.take();
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill().await;
            return Err("Could not read SteamCMD output while checking the sign-in".to_string());
        };

        let mut lines = BufReader::new(stdout).lines();
        let mut transcript = String::new();
        let deadline = tokio::time::Instant::now() + APPROVAL_TIMEOUT;
        // Set once Valve accepts the sign-in. From then on the engine only needs
        // a moment to flush its session files before it quits on its own.
        let mut confirmed = false;
        let mut shutdown_deadline: Option<tokio::time::Instant> = None;

        loop {
            let limit = shutdown_deadline.unwrap_or(deadline);
            let remaining = limit.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                let _ = child.kill().await;
                // A sign-in that already succeeded must not be reported as a
                // timeout just because the shutdown took longer than we waited.
                return if confirmed {
                    Ok(())
                } else {
                    Err(guard_timeout_message(&transcript))
                };
            }

            let line = match tokio::time::timeout(remaining, lines.next_line()).await {
                Err(_) => {
                    let _ = child.kill().await;
                    return if confirmed {
                        Ok(())
                    } else {
                        Err(guard_timeout_message(&transcript))
                    };
                }
                Ok(Ok(Some(line))) => line,
                // EOF or read error: SteamCMD is done talking
                Ok(Ok(None)) | Ok(Err(_)) => break,
            };

            transcript.push_str(&line);
            transcript.push('\n');
            debug!("steamcmd login: {}", line);

            let lower = line.to_lowercase();

            // SteamCMD announces a successful auth as "Logging in user 'x' ... to
            // Steam Public...OK" — there is no separate "Logged in OK" line.
            //
            // Do NOT kill the engine on this line. SteamCMD only writes the
            // reusable session — the remembered account in `config/config.vdf`
            // and this machine's `ssfn*` Steam Guard sentry — while it shuts
            // down normally. Killing it here threw that token away, so the next
            // download had to sign in with the password and ping the phone for a
            // brand-new approval. Keep draining until it exits by itself.
            if login_confirmed(&line) {
                if !confirmed {
                    info!("Steam confirmed the sign-in; letting SteamCMD persist the session");
                }
                confirmed = true;
                shutdown_deadline =
                    Some(tokio::time::Instant::now() + std::time::Duration::from_secs(45));
                continue;
            }

            // A code prompt: feed through the code typed in the dialog, if any
            if lower.contains("steam guard code")
                || lower.contains("two-factor code")
                || lower.contains("auth code")
            {
                if let (Some(code), Some(sink)) = (guard_code, stdin.as_mut()) {
                    let code = code.trim();
                    if !code.is_empty() {
                        let _ = sink.write_all(format!("{}\n", code).as_bytes()).await;
                        let _ = sink.flush().await;
                        info!("Sent the Steam Guard code to SteamCMD");
                        stdin = None;
                    }
                }
            }

            if lower.contains("invalid password") || lower.contains("account not found") {
                break;
            }
        }

        let _ = child.wait().await;

        if confirmed {
            if cached_session_exists(username) {
                info!(
                    "SteamCMD kept a reusable session for '{}'; later downloads authenticate silently",
                    username
                );
            } else {
                warn!(
                    "SteamCMD signed in but left no reusable session behind — the next download may ask for a Steam Guard approval again"
                );
            }
            return Ok(());
        }

        classify_login_output(&transcript)
    }

    /// Spawns a background SteamCMD process to download and verify the given game
    #[allow(clippy::too_many_arguments)]
    pub async fn start_download(
        self: Arc<Self>,
        app_handle: AppHandle,
        app_id: u32,
        title: String,
        target_dir: PathBuf,
        credentials: Option<crate::models::SteamCredentials>,
        depot_id: Option<u32>,
        manifest_id: Option<u64>,
        storage: Arc<crate::storage::Storage>,
    ) -> Result<(), String> {
        let steamcmd_exe = Self::ensure_steamcmd().await?;

        // Clear any stale cancellation flag left over from a previous attempt
        self.cancelled.lock().await.remove(&app_id);

        // Refuse to stack a second SteamCMD run on the same AppID: concurrent
        // logins make Valve time out (ERROR (Timeout)) and the downloads fight.
        {
            let pids = self.active_pids.lock().await;
            if pids.contains_key(&app_id) {
                return Err(format!(
                    "A download for AppID {} is already running",
                    app_id
                ));
            }
        }

        tokio::fs::create_dir_all(&target_dir)
            .await
            .map_err(|e| format!("Failed to create game target dir: {}", e))?;

        // Pass each command argument separately to prevent Windows quoting issues
        let mut args: Vec<String> = Vec::new();
        args.push("+force_install_dir".to_string());
        args.push(target_dir.to_string_lossy().to_string());

        // Prefer SteamCMD's cached session. Logging in with the password on every
        // download makes Valve treat each one as a new device login, which pings
        // the Steam Mobile app with a fresh sign-in approval each time. A cached
        // session authenticates silently and sends no notification.
        let cached_ok = match credentials.as_ref() {
            Some(creds) => {
                let invalid = *self.session_invalid.lock().await;
                !invalid && cached_session_exists(&creds.username)
            }
            None => false,
        };
        let used_cached_session = cached_ok;

        let mut logged_in_as_account = false;
        match credentials.as_ref() {
            Some(creds) if cached_ok => {
                args.push("+@NoPromptForPassword".to_string());
                args.push("1".to_string());
                args.push("+login".to_string());
                args.push(creds.username.trim().to_string());
                logged_in_as_account = true;
            }
            Some(creds)
                if !creds.username.trim().is_empty() && !creds.password.is_empty() =>
            {
                args.push("+login".to_string());
                args.push(creds.username.trim().to_string());
                args.push(creds.password.clone());
                if let Some(code) = creds
                    .guard_code
                    .as_ref()
                    .map(|c| c.trim())
                    .filter(|c| !c.is_empty())
                {
                    args.push(code.to_string());
                }
                logged_in_as_account = true;
            }
            _ => {
                // Anonymous mode: free-to-play titles, demos, servers and public depots
                args.push("+login".to_string());
                args.push("anonymous".to_string());
            }
        }

        // Support direct depot and manifest downloading if specified
        if let Some(depot) = depot_id {
            args.push("+download_depot".to_string());
            args.push(app_id.to_string());
            args.push(depot.to_string());
            if let Some(manifest) = manifest_id {
                args.push(manifest.to_string());
            }
        } else {
            args.push("+app_update".to_string());
            args.push(app_id.to_string());
            args.push("validate".to_string());
        }
        args.push("+quit".to_string());

        info!(
            "Starting SteamCMD in-launcher download for AppID {} ('{}') into {:?}",
            app_id, title, target_dir
        );

        let mut cmd = Command::new(&steamcmd_exe);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn steamcmd process: {}", e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture steamcmd stdout".to_string())?;

        let stderr = child.stderr.take();

        let pid = child.id().unwrap_or(0);
        {
            let mut pids = self.active_pids.lock().await;
            pids.insert(app_id, pid);
            self.active_titles
                .lock()
                .await
                .insert(app_id, title.clone());
        }

        // Drain stderr asynchronously to prevent pipe deadlocks
        if let Some(err_stream) = stderr {
            tokio::spawn(async move {
                let mut err_reader = BufReader::new(err_stream).lines();
                while let Ok(Some(err_line)) = err_reader.next_line().await {
                    debug!("SteamCMD stderr: {}", err_line);
                }
            });
        }

        let active_pids_clone = Arc::clone(&self.active_pids);
        let active_titles_clone = Arc::clone(&self.active_titles);
        let active_percent_clone = Arc::clone(&self.active_percent);
        let session_invalid = Arc::clone(&self.session_invalid);
        let app_name = format!("steam_{}", app_id);
        let title_clone = title.clone();
        let target_dir_clone = target_dir.clone();
        let app_handle_clone = app_handle.clone();

        // SteamCMD block-buffers its console output when stdout is a pipe, so
        // progress lines arrive late or in bursts. Mirror the real numbers from
        // its own log file and the install manifest instead, once a second.
        let log_path = Self::steamcmd_dir().join("logs").join("console_log.txt");
        let log_offset = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);

        let progress_task = tokio::spawn(poll_download_progress(
            target_dir_clone.clone(),
            app_id,
            app_name.clone(),
            title_clone.clone(),
            app_handle_clone.clone(),
            Arc::clone(&self.cancelled),
            Arc::clone(&self.active_percent),
            log_path,
            log_offset,
        ));

        // Spawn background async streaming reader
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            let mut last_progress_time = Instant::now();
            let mut last_downloaded_bytes: u64 = 0;
            let mut speed_bytes_per_sec: u64 = 0;
            let mut subscription_error = false;
            let mut specific_error_msg: Option<String> = None;
            let mut installed_ok = false;

            // Initial queued event
            let initial_p = DownloadProgress {
                app_name: app_name.clone(),
                title: title_clone.clone(),
                status: DownloadStatus::Analyzing,
                total_bytes: 0,
                downloaded_bytes: 0,
                total_chunks: 100,
                downloaded_chunks: 0,
                speed_bytes_per_sec: 0,
                eta_seconds: 0,
                progress_percent: 0.0,
                current_file: Some("Initializing Steam download engine...".to_string()),
                error_message: None,
            };
            let _ = app_handle_clone.emit("download-progress", initial_p);

            while let Ok(Some(line)) = reader.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // A silent cached login keeps Valve from pinging the phone again.
                // SteamCMD reports it inline ("…to Steam Public...OK") on current
                // builds and as "Logged in OK" on older ones.
                if login_confirmed(&line) {
                    *session_invalid.lock().await = false;
                    continue;
                }

                // The cached token was rejected: fall back to the stored password
                // instead of asking for a Steam Guard code.
                if used_cached_session
                    && (line.contains("No cached credentials")
                        || line.contains("Invalid Password")
                        || requires_steam_guard(&line))
                {
                    *session_invalid.lock().await = true;
                    warn!(
                        "Cached SteamCMD session was rejected for AppID {}; password fallback required",
                        app_id
                    );
                    let err_p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Failed,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: Some("Steam session expired".to_string()),
                        error_message: Some(
                            "STEAM_LOGIN_FAILED: The saved Steam session expired. Enter your Steam password once to re-authorise; after that downloads stay silent."
                                .to_string(),
                        ),
                    };
                    let _ = app_handle_clone.emit("download-progress", err_p);
                    break;
                }

                // Stream live status updates to UI during startup / analysis
                if trimmed.starts_with("Connecting")
                    || trimmed.starts_with("Waiting")
                    || trimmed.starts_with("Checking for")
                    || trimmed.starts_with("Verifying")
                    || trimmed.starts_with("Loading")
                    || trimmed.starts_with("Logging in")
                {
                    let p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Analyzing,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: Some(trimmed.to_string()),
                        error_message: None,
                    };
                    let _ = app_handle_clone.emit("download-progress", p);
                } else if line.contains("downloading, progress:")
                    || line.contains("preallocating, progress:")
                {
                    if let Some((percent, current, total)) = parse_steamcmd_progress_line(&line) {
                        let now = Instant::now();
                        let elapsed = now.duration_since(last_progress_time).as_secs_f64();
                        if elapsed >= 0.5 {
                            if current > last_downloaded_bytes {
                                speed_bytes_per_sec =
                                    ((current - last_downloaded_bytes) as f64 / elapsed) as u64;
                            }
                            last_downloaded_bytes = current;
                            last_progress_time = now;
                        }

                        let eta = if speed_bytes_per_sec > 0 && total > current {
                            (total - current) / speed_bytes_per_sec
                        } else {
                            0
                        };

                        let p = DownloadProgress {
                            app_name: app_name.clone(),
                            title: title_clone.clone(),
                            status: DownloadStatus::Downloading,
                            total_bytes: total,
                            downloaded_bytes: current,
                            total_chunks: 100,
                            downloaded_chunks: percent as u64,
                            speed_bytes_per_sec,
                            eta_seconds: eta,
                            progress_percent: percent as f32,
                            current_file: Some("Downloading game files from Steam CDN...".to_string()),
                            error_message: None,
                        };
                        let _ = app_handle_clone.emit("download-progress", p);
                    }
                } else if line.contains("validating, progress:") {
                    if let Some((percent, current, total)) = parse_steamcmd_progress_line(&line) {
                        let p = DownloadProgress {
                            app_name: app_name.clone(),
                            title: title_clone.clone(),
                            status: DownloadStatus::Verifying,
                            total_bytes: total,
                            downloaded_bytes: current,
                            total_chunks: 100,
                            downloaded_chunks: percent as u64,
                            speed_bytes_per_sec: 0,
                            eta_seconds: 0,
                            progress_percent: percent as f32,
                            current_file: Some("Verifying Steam chunk integrity...".to_string()),
                            error_message: None,
                        };
                        let _ = app_handle_clone.emit("download-progress", p);
                    }
                } else if line.contains("Success! App") || line.contains("fully installed") {
                    info!("Steam download completed for AppID {}", app_id);
                    installed_ok = true;
                } else if requires_steam_guard(&line) {
                    warn!("SteamCMD reported Steam Guard required for AppID {}", app_id);
                    subscription_error = true;
                    specific_error_msg = Some(
                        "STEAM_GUARD_REQUIRED: Steam Guard needs the current authenticator code. Enter it and retry the download.".to_string(),
                    );
                    let err_p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Failed,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: Some("Authentication: Steam Guard approval required".to_string()),
                        error_message: specific_error_msg.clone(),
                    };
                    let _ = app_handle_clone.emit("download-progress", err_p);
                    break;
                } else if line.contains("No subscription") || line.contains("No license") {
                    warn!(
                        "SteamCMD has no license for AppID {} (anonymous session).",
                        app_id
                    );

                    // Hyperion downloads in-app only: no hand-off to the Steam client.
                    subscription_error = true;
                    specific_error_msg = Some(if logged_in_as_account {
                        format!(
                            "Steam refused the licence for '{}' (AppID {}): this account does not own it, or it is a family-shared copy that must be activated in the Steam client once.",
                            title_clone, app_id
                        )
                    } else {
                        format!(
                            "Valve requires an authenticated Steam session to allocate a licence for '{}' (AppID {}). Sign in to Steam in Hyperion and retry.",
                            title_clone, app_id
                        )
                    });
                    let err_p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Failed,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: Some("Ownership / license allocation failed".to_string()),
                        error_message: specific_error_msg.clone(),
                    };
                    let _ = app_handle_clone.emit("download-progress", err_p);
                    break;
                } else if line.contains("Invalid Password") || line.contains("invalid password") {
                    warn!("SteamCMD rejected the password for AppID {}", app_id);
                    subscription_error = true;
                    specific_error_msg = Some(
                        "STEAM_LOGIN_FAILED: Steam rejected this password. Re-enter your Steam credentials.".to_string(),
                    );
                    let err_p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Failed,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: Some("Authentication failed".to_string()),
                        error_message: specific_error_msg.clone(),
                    };
                    let _ = app_handle_clone.emit("download-progress", err_p);
                    break;
                } else if line.contains("Rate Limit Exceeded") || line.contains("Account Login Denied") {
                    warn!("SteamCMD login rate-limited for AppID {}", app_id);
                    subscription_error = true;
                    specific_error_msg = Some(
                        "STEAM_LOGIN_FAILED: Steam is rate-limiting sign-ins for this account. Wait a few minutes and retry."
                            .to_string(),
                    );
                    let err_p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Failed,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: Some("Authentication rate-limited".to_string()),
                        error_message: specific_error_msg.clone(),
                    };
                    let _ = app_handle_clone.emit("download-progress", err_p);
                    break;
                } else if line.contains("ERROR!") || line.contains("FAILED") {
                    warn!("SteamCMD reported error: {}", line);
                    specific_error_msg = Some(trimmed.to_string());
                }
            }

            let status = child.wait().await;
            progress_task.abort();
            {
                let mut pids = active_pids_clone.lock().await;
                pids.remove(&app_id);
            }
            active_titles_clone.lock().await.remove(&app_id);
            active_percent_clone.lock().await.remove(&app_id);

            match status {
                Ok(exit_code)
                    if exit_code.success()
                        && (last_downloaded_bytes > 0
                            || installed_ok
                            || dir_has_content(&target_dir_clone)) =>
                {
                    info!("Steam download process exited successfully for AppID {}", app_id);

                    // Discover the most plausible game executable
                    let found_exe = pick_launch_exe(&target_dir_clone, &title_clone);

                    // Steamworks games look for steam_appid.txt next to the binary.
                    // Without it the game boots the Steam client and asks to install
                    // itself again instead of just starting.
                    ensure_steam_appid_file(&target_dir_clone, app_id);

                    // Save installed game into SQLite storage so it immediately persists in the library
                    let installed_game = InstalledGame {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        version: "Steam".to_string(),
                        install_path: target_dir_clone.to_string_lossy().to_string(),
                        executable: found_exe,
                        launch_parameters: String::new(),
                        install_size: last_downloaded_bytes,
                        can_run_offline: true,
                        requires_ot: false,
                        platform: "steam".to_string(),
                        is_dlc: false,
                        base_urls: Vec::new(),
                        installed_at: Some(chrono::Utc::now().to_rfc3339()),
                    };

                    if let Err(e) = storage.save_installed_game(&installed_game) {
                        warn!("Failed to save installed Steam game to database: {}", e);
                    } else {
                        info!("Successfully saved installed Steam game {} to database", app_name);
                    }

                    let final_p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Completed,
                        total_bytes: last_downloaded_bytes,
                        downloaded_bytes: last_downloaded_bytes,
                        total_chunks: 100,
                        downloaded_chunks: 100,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 100.0,
                        current_file: Some("Download & Installation Completed".to_string()),
                        error_message: None,
                    };
                    let _ = app_handle_clone.emit("download-progress", final_p);
                }
                Ok(exit_code) => {
                    let err_msg = if let Some(msg) = specific_error_msg {
                        msg
                    } else if subscription_error {
                        format!("Valve requires the official Steam client to allocate a license for this title, but no Steam client could be reached. Install Steam, sign in with the owning account, then retry.")
                    } else {
                        format!("SteamCMD exited with status {:?}.", exit_code)
                    };
                    warn!("{}", err_msg);
                    let err_p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Failed,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: Some("Installation stopped".to_string()),
                        error_message: Some(err_msg),
                    };
                    let _ = app_handle_clone.emit("download-progress", err_p);
                }
                Err(e) => {
                    error!("Steam download wait error: {}", e);
                    let err_p = DownloadProgress {
                        app_name: app_name.clone(),
                        title: title_clone.clone(),
                        status: DownloadStatus::Failed,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: None,
                        error_message: Some(format!("SteamCMD process error: {}", e)),
                    };
                    let _ = app_handle_clone.emit("download-progress", err_p);
                }
            }
        });

        Ok(())
    }

    /// Cancels an active Steam download (a SteamCMD process, or a Steam client mirror)
    pub async fn cancel_download(&self, app_id: u32, app_handle: &AppHandle) -> Result<(), String> {
        let pid_opt = {
            let mut pids = self.active_pids.lock().await;
            pids.remove(&app_id)
        };
        self.active_titles.lock().await.remove(&app_id);

        // Flag the cancellation so a Steam client mirror loop stops even when no
        // SteamCMD process is associated with the AppID.
        self.cancelled.lock().await.insert(app_id);

        if let Some(pid) = pid_opt {
            info!("Killing Steam download process PID {} for AppID {}", pid, app_id);
            crate::winproc::kill_tree(pid);
        }

        let p = DownloadProgress {
            app_name: format!("steam_{}", app_id),
            title: format!("Steam App {}", app_id),
            status: DownloadStatus::Cancelled,
            total_bytes: 0,
            downloaded_bytes: 0,
            total_chunks: 0,
            downloaded_chunks: 0,
            speed_bytes_per_sec: 0,
            eta_seconds: 0,
            progress_percent: 0.0,
            current_file: None,
            error_message: None,
        };
        let _ = app_handle.emit("download-progress", p);
        Ok(())
    }

    /// Monitors the official Steam Desktop Client download and emits live progress inside Hyperion
    pub async fn start_monitoring_client_download(
        &self,
        app_id: u32,
        title: String,
        app_handle: AppHandle,
        storage: Arc<crate::storage::Storage>,
    ) {
        let app_name = format!("steam_{}", app_id);
        let title_clone = title.clone();

        // A fresh monitor run supersedes any earlier cancellation for this AppID
        self.cancelled.lock().await.remove(&app_id);
        let cancelled = Arc::clone(&self.cancelled);

        // Initial progress state
        let initial_p = DownloadProgress {
            app_name: app_name.clone(),
            title: title_clone.clone(),
            status: DownloadStatus::Analyzing,
            total_bytes: 0,
            downloaded_bytes: 0,
            total_chunks: 100,
            downloaded_chunks: 0,
            speed_bytes_per_sec: 0,
            eta_seconds: 0,
            progress_percent: 0.0,
            current_file: Some("Connecting to Steam client & allocating download...".to_string()),
            error_message: None,
        };
        let _ = app_handle.emit("download-progress", initial_p);

        tokio::spawn(async move {
            let steam_root = match crate::steam::detect_steam_root() {
                Some(r) => r,
                None => {
                    let _ = app_handle.emit(
                        "download-progress",
                        DownloadProgress {
                            app_name: app_name.clone(),
                            title: title_clone.clone(),
                            status: DownloadStatus::Failed,
                            total_bytes: 0,
                            downloaded_bytes: 0,
                            total_chunks: 100,
                            downloaded_chunks: 0,
                            speed_bytes_per_sec: 0,
                            eta_seconds: 0,
                            progress_percent: 0.0,
                            current_file: Some("Steam client not found".to_string()),
                            error_message: Some(
                                "Steam is not installed on this system. Install the Steam client and sign in to download this title."
                                    .to_string(),
                            ),
                        },
                    );
                    return;
                }
            };

            let mut last_downloaded: u64 = 0;
            let mut last_time = Instant::now();
            let mut not_found_count = 0;

            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                if cancelled.lock().await.contains(&app_id) {
                    debug!("Steam client download monitor cancelled for AppID {}", app_id);
                    break;
                }

                // Discover all library folders (e.g. F:\steam, D:\Games, etc.)
                let libraries = crate::steam::discover_library_folders(&steam_root);
                let manifest_filename = format!("appmanifest_{}.acf", app_id);

                let mut found_manifest = None;
                for lib in &libraries {
                    let manifest_path = lib.join("steamapps").join(&manifest_filename);
                    if manifest_path.exists() {
                        if let Ok(content) = tokio::fs::read_to_string(&manifest_path).await {
                            found_manifest = Some((lib.clone(), content));
                            break;
                        }
                    }
                }

                if let Some((lib_path, content)) = found_manifest {
                    not_found_count = 0;
                    let vdf = crate::steam::vdf::parse_vdf(&content);
                    if let Some(app_state) = vdf.get("AppState").and_then(|a| a.as_object()) {
                        let bytes_downloaded: u64 = app_state
                            .get("BytesDownloaded")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);

                        let bytes_to_download: u64 = app_state
                            .get("BytesToDownload")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);

                        let state_flags: u64 = app_state
                            .get("StateFlags")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);

                        let install_dir_name = app_state
                            .get("installdir")
                            .and_then(|v| v.as_str())
                            .unwrap_or("game");

                        let target_game_dir = lib_path.join("steamapps").join("common").join(install_dir_name);

                        let now = Instant::now();
                        let elapsed = now.duration_since(last_time).as_secs_f64();
                        let speed = if elapsed >= 0.8 && bytes_downloaded > last_downloaded {
                            ((bytes_downloaded - last_downloaded) as f64 / elapsed) as u64
                        } else {
                            0
                        };
                        last_downloaded = bytes_downloaded;
                        last_time = now;

                        let percent = if bytes_to_download > 0 {
                            ((bytes_downloaded as f64 / bytes_to_download as f64) * 100.0) as f32
                        } else {
                            0.0
                        };

                        let eta = if speed > 0 && bytes_to_download > bytes_downloaded {
                            (bytes_to_download - bytes_downloaded) / speed
                        } else {
                            0
                        };

                        // Check if installed (StateFlags & 4 != 0 indicates StateInstalled)
                        let is_fully_installed = (state_flags & 4) != 0 || (bytes_to_download > 0 && bytes_downloaded >= bytes_to_download && state_flags == 4);

                        if is_fully_installed {
                            info!("Steam client finished downloading AppID {}", app_id);

                            let found_exe = pick_launch_exe(&target_game_dir, &title_clone);

                            let installed_game = InstalledGame {
                                app_name: app_name.clone(),
                                title: title_clone.clone(),
                                version: "Steam".to_string(),
                                install_path: target_game_dir.to_string_lossy().to_string(),
                                executable: found_exe,
                                launch_parameters: String::new(),
                                install_size: if bytes_to_download > 0 { bytes_to_download } else { bytes_downloaded },
                                can_run_offline: true,
                                requires_ot: false,
                                platform: "steam".to_string(),
                                is_dlc: false,
                                base_urls: Vec::new(),
                                installed_at: Some(chrono::Utc::now().to_rfc3339()),
                            };

                            let _ = storage.save_installed_game(&installed_game);

                            let final_p = DownloadProgress {
                                app_name: app_name.clone(),
                                title: title_clone.clone(),
                                status: DownloadStatus::Completed,
                                total_bytes: bytes_to_download,
                                downloaded_bytes: bytes_downloaded,
                                total_chunks: 100,
                                downloaded_chunks: 100,
                                speed_bytes_per_sec: 0,
                                eta_seconds: 0,
                                progress_percent: 100.0,
                                current_file: Some("Installation Completed via Steam Client".to_string()),
                                error_message: None,
                            };
                            let _ = app_handle.emit("download-progress", final_p);
                            break;
                        } else {
                            let p = DownloadProgress {
                                app_name: app_name.clone(),
                                title: title_clone.clone(),
                                status: DownloadStatus::Downloading,
                                total_bytes: bytes_to_download,
                                downloaded_bytes: bytes_downloaded,
                                total_chunks: 100,
                                downloaded_chunks: percent as u64,
                                speed_bytes_per_sec: speed,
                                eta_seconds: eta,
                                progress_percent: percent,
                                current_file: Some(format!("Downloading via Steam Client ({})...", install_dir_name)),
                                error_message: None,
                            };
                            let _ = app_handle.emit("download-progress", p);
                        }
                    }
                } else {
                    not_found_count += 1;

                    // The user may still be confirming the install prompt in the
                    // Steam client, so keep waiting instead of dropping the mirror.
                    if not_found_count > 900 {
                        warn!("Timed out waiting for the Steam manifest of AppID {}", app_id);
                        let _ = app_handle.emit(
                            "download-progress",
                            DownloadProgress {
                                app_name: app_name.clone(),
                                title: title_clone.clone(),
                                status: DownloadStatus::Failed,
                                total_bytes: 0,
                                downloaded_bytes: 0,
                                total_chunks: 100,
                                downloaded_chunks: 0,
                                speed_bytes_per_sec: 0,
                                eta_seconds: 0,
                                progress_percent: 0.0,
                                current_file: Some("Timed out waiting for Steam".to_string()),
                                error_message: Some(
                                    "Steam did not start the download. Confirm the install prompt in the Steam client, then click Retry."
                                        .to_string(),
                                ),
                            },
                        );
                        break;
                    } else if not_found_count % 15 == 0 {
                        let _ = app_handle.emit(
                            "download-progress",
                            DownloadProgress {
                                app_name: app_name.clone(),
                                title: title_clone.clone(),
                                status: DownloadStatus::Analyzing,
                                total_bytes: 0,
                                downloaded_bytes: 0,
                                total_chunks: 100,
                                downloaded_chunks: 0,
                                speed_bytes_per_sec: 0,
                                eta_seconds: 0,
                                progress_percent: 0.0,
                                current_file: Some(
                                    "Waiting for Steam client to start the download...".to_string(),
                                ),
                                error_message: None,
                            },
                        );
                    }
                }
            }
        });
    }
}

/// Mirrors SteamCMD's on-disk progress once a second.
///
/// The install manifest SteamCMD writes into `<target>/steamapps` carries
/// `BytesDownloaded` / `BytesToDownload`, which stays accurate no matter how the
/// child's stdout is buffered. Falls back to the staging folder size while the
/// manifest is still being written.
async fn poll_download_progress(
    target_dir: PathBuf,
    app_id: u32,
    app_name: String,
    title: String,
    app_handle: AppHandle,
    cancelled: Arc<Mutex<HashSet<u32>>>,
    // Shared with the tray, so an in-flight SteamCMD install reports progress.
    progress: Arc<Mutex<HashMap<u32, f32>>>,
    log_path: PathBuf,
    log_offset: u64,
) {
    let acf_path = target_dir
        .join("steamapps")
        .join(format!("appmanifest_{}.acf", app_id));

    let mut last_bytes: u64 = 0;
    let mut last_time = Instant::now();
    let mut last_logged_percent: i32 = -1;
    let mut quiet_ticks = 0u32;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        if cancelled.lock().await.contains(&app_id) {
            break;
        }

        // SteamCMD writes the ACF only at state transitions, so its counters are
        // stale mid-transfer; the console log is the live source. Both measure the
        // *staged* (uncompressed) scale, but the actual network transfer is
        // BytesToDownload (compressed) — which is the figure Steam itself shows
        // while downloading. So: compressed size during the transfer, staged size
        // while verifying.
        let mut bytes_downloaded: u64 = 0;
        let mut bytes_to_download: u64 = 0;
        let mut bytes_to_stage: u64 = 0;
        let mut installed = false;

        if let Ok(content) = tokio::fs::read_to_string(&acf_path).await {
            let vdf = crate::steam::vdf::parse_vdf(&content);
            if let Some(state) = vdf.get("AppState").and_then(|a| a.as_object()) {
                let read_u64 = |key: &str| -> u64 {
                    state
                        .get(key)
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0)
                };
                bytes_downloaded = read_u64("BytesDownloaded");
                bytes_to_download = read_u64("BytesToDownload");
                bytes_to_stage = read_u64("BytesToStage");
                installed = read_u64("StateFlags") & 4 != 0;
            }
        }

        let mut downloaded: u64 = 0;
        let mut total: u64 = 0;
        let mut verifying = false;
        let mut percent_hint: Option<f64> = None;

        // Best source: SteamCMD's own console log, flushed live to disk
        if let Some(log) = last_logged_progress(&log_path, log_offset) {
            if log.total > 0 && (bytes_to_stage == 0 || log.total == bytes_to_stage) {
                downloaded = log.current;
                total = log.total;
                verifying = log.verifying;
                percent_hint = Some(log.percent);
            }
        }

        // Fall back to the compressed transfer counters
        if downloaded == 0 && bytes_downloaded > 0 && bytes_to_download > 0 {
            downloaded = bytes_downloaded;
            total = bytes_to_download;
        }

        // During the transfer, report the compressed download size (Steam's own
        // behaviour); the staged size is only meaningful once verifying starts.
        if !verifying && bytes_to_download > 0 && total > 0 {
            let percent = percent_hint.unwrap_or_else(|| {
                ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
            });
            downloaded = ((percent / 100.0) * bytes_to_download as f64) as u64;
            total = bytes_to_download;
            percent_hint = Some(percent);
        }

        // Still logging in / resolving the manifest: keep the UI honest instead
        // of leaving a stale "Initializing" label forever.
        if total == 0 && downloaded == 0 {
            quiet_ticks += 1;
            if quiet_ticks == 6 || quiet_ticks % 15 == 0 {
                let _ = app_handle.emit(
                    "download-progress",
                    DownloadProgress {
                        app_name: app_name.clone(),
                        title: title.clone(),
                        status: DownloadStatus::Analyzing,
                        total_bytes: 0,
                        downloaded_bytes: 0,
                        total_chunks: 100,
                        downloaded_chunks: 0,
                        speed_bytes_per_sec: 0,
                        eta_seconds: 0,
                        progress_percent: 0.0,
                        current_file: Some("Signing in to Steam and resolving the manifest…".to_string()),
                        error_message: None,
                    },
                );
            }
            continue;
        }

        quiet_ticks = 0;

        let now = Instant::now();
        let elapsed = now.duration_since(last_time).as_secs_f64();

        // SteamCMD restarts its counters when a new run begins; treat a drop as
        // a reset instead of freezing the speed at zero forever.
        if downloaded < last_bytes {
            last_bytes = 0;
            last_time = now;
        }

        let mut speed = 0u64;
        if elapsed >= 0.8 && downloaded > last_bytes {
            speed = ((downloaded - last_bytes) as f64 / elapsed) as u64;
        }
        if downloaded > last_bytes {
            last_bytes = downloaded;
            last_time = now;
        }

        let percent = match percent_hint {
            Some(p) => p.clamp(0.0, 100.0) as f32,
            None if total > 0 => {
                (((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0)) as f32
            }
            None => 0.0,
        };
        let eta = if speed > 0 && total > downloaded {
            (total - downloaded) / speed
        } else {
            0
        };

        progress.lock().await.insert(app_id, percent);

        if percent as i32 != last_logged_percent {
            last_logged_percent = percent as i32;
            debug!(
                "Steam progress AppID {}: {:.2}% ({}/{} bytes, {}/s, verifying={})",
                app_id, percent, downloaded, total, speed, verifying
            );
        }

        let _ = app_handle.emit(
            "download-progress",
            DownloadProgress {
                app_name: app_name.clone(),
                title: title.clone(),
                status: if verifying || installed {
                    DownloadStatus::Verifying
                } else {
                    DownloadStatus::Downloading
                },
                total_bytes: total,
                downloaded_bytes: downloaded,
                total_chunks: 100,
                downloaded_chunks: percent as u64,
                speed_bytes_per_sec: speed,
                eta_seconds: eta,
                progress_percent: percent,
                current_file: Some(if verifying || installed {
                    "Verifying downloaded files…".to_string()
                } else {
                    "Downloading from Steam CDN…".to_string()
                }),
                error_message: None,
            },
        );
    }

    // Stop reporting progress once the poller is done with this AppID.
    progress.lock().await.remove(&app_id);
}

/// Writes `steam_appid.txt` into an install folder.
///
/// Steamworks titles read that file to learn which AppID they belong to. Without
/// it they initialise the Steam client, which does not know about Hyperion's copy
/// and offers to download the game again instead of just starting.
pub fn ensure_steam_appid_file(dir: &Path, app_id: u32) {
    let path = dir.join("steam_appid.txt");
    if path.exists() {
        return;
    }

    match std::fs::write(&path, app_id.to_string()) {
        Ok(_) => debug!("Wrote steam_appid.txt for AppID {} into {:?}", app_id, dir),
        Err(e) => warn!("Could not write {:?}: {}", path, e),
    }
}

/// Unpacks a zip archive into `dest`, in-process.
///
/// Entries whose names would land outside `dest` (absolute paths, `..`) are
/// refused rather than skipped: a tampered archive is not a SteamCMD install.
fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("steamcmd.zip is not a valid archive: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Could not read steamcmd.zip entry {}: {}", i, e))?;
        let name = entry
            .name()
            .map(|name| name.into_owned())
            .unwrap_or_else(|_| format!("entry {i}"));
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| format!("steamcmd.zip has an unsafe entry: {}", name))?;
        let out = dest.join(relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&out)
                .map_err(|e| format!("Could not create {}: {}", out.display(), e))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create {}: {}", parent.display(), e))?;
        }
        let mut file = std::fs::File::create(&out)
            .map_err(|e| format!("Could not write {}: {}", out.display(), e))?;
        std::io::copy(&mut entry, &mut file)
            .map_err(|e| format!("Could not extract {}: {}", out.display(), e))?;
    }
    Ok(())
}

/// True when SteamCMD already holds a cached session for this account.
///
/// When one is present, a bare `+login <user>` authenticates with the stored
/// token instead of the password, which is what keeps Valve from sending a new
/// sign-in approval to the Steam Mobile app on every download.
///
/// Three things can carry that session, and all three have to be honoured:
/// the `ssfn*` sentry (this machine's Steam Guard trust token), the Steam
/// client-style `config/loginusers.vdf`, and — on SteamCMD itself — the
/// remembered account stored in `config/config.vdf`. Checking only
/// `loginusers.vdf` never matched a SteamCMD install, so every download fell
/// back to a password sign-in and a fresh phone approval.
pub fn cached_session_exists(username: &str) -> bool {
    let dir = SteamDownloadManager::steamcmd_dir();

    if sentry_exists_in(&dir) {
        return true;
    }

    let config = dir.join("config");

    if let Ok(content) = std::fs::read_to_string(config.join("loginusers.vdf")) {
        if loginusers_has_account(&content, username) {
            return true;
        }
    }

    match std::fs::read_to_string(config.join("config.vdf")) {
        Ok(content) => config_vdf_holds_token(&content, username),
        Err(_) => false,
    }
}

/// True when SteamCMD's `config.vdf` holds a *usable* login for this account.
///
/// The `Accounts` list on its own is not enough — it only remembers the account
/// name, so trusting it made the engine attempt a bare `+login` that dies with
/// "Cached credentials not found" and aborts the download. The actual token
/// lives in `ConnectCache`, next to that list.
fn config_vdf_holds_token(content: &str, username: &str) -> bool {
    if !accounts_has_account(content, username) {
        return false;
    }

    let vdf = crate::steam::vdf::parse_vdf(content);
    vdf.get("InstallConfigStore")
        .and_then(|store| store.get("Software"))
        .and_then(|software| software.get("Valve"))
        .and_then(|valve| valve.get("Steam"))
        .and_then(|steam| steam.get("ConnectCache"))
        .and_then(|cache| cache.as_object())
        .map(|cache| !cache.is_empty())
        .unwrap_or(false)
}

/// True when an `ssfn*` sentry sits in `dir` or in `dir/config`.
fn sentry_exists_in(dir: &Path) -> bool {
    fn has_sentry(path: &Path) -> bool {
        std::fs::read_dir(path)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .to_lowercase()
                        .starts_with("ssfn")
                })
            })
            .unwrap_or(false)
    }

    has_sentry(dir) || has_sentry(&dir.join("config"))
}

/// Parses SteamCMD's `config.vdf` and reports whether it remembers this account.
///
/// SteamCMD keeps `"Accounts" { "<account>" { "SteamID" … } }` under
/// `InstallConfigStore/Software/Valve/Steam`, unlike the Steam client which
/// writes `loginusers.vdf`.
fn accounts_has_account(content: &str, username: &str) -> bool {
    let vdf = crate::steam::vdf::parse_vdf(content);
    vdf.get("InstallConfigStore")
        .and_then(|store| store.get("Software"))
        .and_then(|software| software.get("Valve"))
        .and_then(|valve| valve.get("Steam"))
        .and_then(|steam| steam.get("Accounts"))
        .and_then(|accounts| accounts.as_object())
        .map(|accounts| {
            accounts
                .keys()
                .any(|name| name.eq_ignore_ascii_case(username))
        })
        .unwrap_or(false)
}

/// Makes SteamCMD forget the remembered account, so "Disconnect" really logs
/// out: the next download has to sign in again instead of reusing the cached
/// session behind the UI's back.
pub fn forget_cached_session() {
    let dir = SteamDownloadManager::steamcmd_dir();
    forget_session_in(&dir.join("config"));
    // Sentries normally sit next to steamcmd.exe rather than in `config\`, so
    // clearing only the config directory would leave the account silently
    // signed in after a "Disconnect".
    forget_sentries_in(&dir);
}

/// Removes this machine's Steam Guard sentries from `dir`.
fn forget_sentries_in(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.starts_with("ssfn") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Removes the account files from a SteamCMD `config` directory.
fn forget_session_in(config: &std::path::Path) {
    if !config.is_dir() {
        return;
    }

    for name in ["loginusers.vdf", "config.vdf"] {
        let path = config.join(name);
        if path.exists() {
            match std::fs::remove_file(&path) {
                Ok(()) => info!("Forgot cached SteamCMD session file {:?}", path),
                Err(e) => warn!("Could not remove {:?}: {}", path, e),
            }
        }
    }

    // `ssfn*` sentries are this machine's Steam Guard trust tokens
    if let Ok(entries) = std::fs::read_dir(config) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("ssfn") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Parses `loginusers.vdf` and reports whether the account is remembered.
fn loginusers_has_account(content: &str, username: &str) -> bool {
    let vdf = crate::steam::vdf::parse_vdf(content);
    vdf.get("users")
        .and_then(|users| users.as_object())
        .map(|users| {
            users.values().any(|user| {
                user.as_object()
                    .and_then(|obj| obj.get("AccountName"))
                    .and_then(|name| name.as_str())
                    .map(|name| name.eq_ignore_ascii_case(username))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// True when SteamCMD's output mentions Steam Guard or a phone confirmation.
fn mentions_guard(output: &str) -> bool {
    let lower = output.to_lowercase();
    lower.contains("steam guard")
        || lower.contains("two-factor")
        || lower.contains("confirmation")
        || lower.contains("authenticator")
        || lower.contains("invalidloginauthcode")
}

/// Error text for the case where Steam never finished the sign-in.
fn guard_timeout_message(output: &str) -> String {
    if mentions_guard(output) {
        "Steam is still waiting for the Steam Guard approval. Approve the sign-in in the Steam app (or enter the current code) and try again.".to_string()
    } else {
        "Steam did not answer in time — check your connection and try again".to_string()
    }
}

/// True when SteamCMD reported a completed authentication.
///
/// Modern builds announce it inline — `Logging in user 'x' [U:1:…] to Steam
/// Public...OK` — and then `Waiting for client config...OK`, `Waiting for user
/// info...OK`, `Unloading Steam API...OK`. Older builds print `Logged in OK`.
fn login_confirmed(output: &str) -> bool {
    let lower = output.to_lowercase();
    lower.contains("logged in ok")
        || lower.contains("to steam public...ok")
        || lower.contains("waiting for user info")
        || lower.contains("waiting for client config")
}

/// Turns SteamCMD's console output into a verdict on the sign-in attempt.
///
/// SteamCMD only ever prints text, so the outcome has to be read from the log:
/// a confirmation line on success, and a handful of well-known failure strings.
fn classify_login_output(output: &str) -> Result<(), String> {
    let lower = output.to_lowercase();

    if login_confirmed(output) {
        return Ok(());
    }

    let reason = if lower.contains("invalid password") {
        "Steam rejected that password for this account"
    } else if lower.contains("two-factor code mismatch") || lower.contains("two factor code mismatch") {
        "Steam rejected that Guard code — codes change every 30 seconds, copy a fresh one from the Steam app and try again"
    } else if lower.contains("invalid login auth code")
        || lower.contains("two-factor code")
        || lower.contains("steam guard code")
        || lower.contains("needs to be confirmed")
    {
        "Steam Guard didn't complete — approve the request in the Steam app, or enter the current code, then try again"
    } else if lower.contains("rate limit exceeded") {
        "Too many sign-in attempts — Valve rate-limited this account. Wait a few minutes and try again"
    } else if lower.contains("account login denied") || lower.contains("accountlogondenied") {
        "Valve denied the login for this account"
    } else if lower.contains("account not found") || lower.contains("invalid account name") {
        "That Steam account name does not exist"
    } else if lower.contains("timeout") {
        "Valve timed out the sign-in — check your connection and try again"
    } else if lower.contains("failed to install") || lower.contains("error (") {
        "SteamCMD reported an error while signing in"
    } else {
        "Steam did not confirm the sign-in — check the account name and password"
    };

    Err(reason.to_string())
}

/// One `progress:` sample from SteamCMD's log, with the phase it belongs to.
struct SteamLogProgress {
    percent: f64,
    current: u64,
    /// For SteamCMD this is the *staged* (uncompressed) total, not the download
    total: u64,
    verifying: bool,
}

/// Reads the newest `progress:` line SteamCMD appended to its console log.
///
/// Unlike the child's stdout (block-buffered when piped), the log file is
/// flushed continuously, so it is the only live progress source available.
/// `from_offset` is the log length captured when the download started, which
/// keeps stale lines from previous runs out of the picture.
fn last_logged_progress(path: &Path, from_offset: u64) -> Option<SteamLogProgress> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len <= from_offset {
        return None;
    }

    // Never read more than the tail of the log
    let window_start = len.saturating_sub(64 * 1024).max(from_offset);
    file.seek(SeekFrom::Start(window_start)).ok()?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);

    let line = text.lines().rev().find(|line| line.contains("progress:"))?;
    let (percent, current, total) = parse_steamcmd_progress_line(line)?;

    Some(SteamLogProgress {
        percent,
        current,
        total,
        verifying: line.contains("verifying") || line.contains("validating"),
    })
}

/// True when SteamCMD is genuinely asking for a Steam Guard code.
///
/// SteamCMD also prints "Steam Guard code provided." on a *successful* login
/// that used a code, so a bare "Steam Guard" substring match turns every good
/// sign-in into a false failure (which is what re-opened the guard prompt after
/// a completed download).
fn requires_steam_guard(line: &str) -> bool {
    const PROMPTS: &[&str] = &[
        "protected by Steam Guard",
        "Steam Guard code required",
        "Enter the Steam Guard code",
        "Steam Guard code needed",
        "Invalid Login Auth Code",
        "Two-factor code",
        "NeedTwoFactorCode",
    ];

    if line.contains("code provided") {
        return false;
    }

    PROMPTS.iter().any(|prompt| line.contains(prompt))
}

/// True when a folder exists and holds at least one file, one level deep.
/// Used to recognise a finished install even if no progress line was parsed.
fn dir_has_content(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };

    entries.flatten().any(|entry| {
        let path = entry.path();
        path.is_file()
            || (path.is_dir()
                && std::fs::read_dir(&path)
                    .map(|mut inner| inner.next().is_some())
                    .unwrap_or(false))
    })
}

/// Picks the most plausible game executable in a freshly downloaded folder.
///
/// SteamCMD drops redistributables, crash handlers and helpers next to the real
/// binary, so prefer the executable whose name resembles the game title and
/// penalise known non-game names.
pub fn pick_launch_exe(dir: &Path, title: &str) -> String {
    const NOISE: &[&str] = &[
        "unins",
        "crash",
        "handler",
        "vcredist",
        "vc_redist",
        "dxsetup",
        "dxwebsetup",
        "directx",
        "dotnet",
        "setup",
        "install",
        "updater",
        "update",
        "helper",
        "report",
        "editor",
        "server",
        "easyanticheat",
        "eac_",
        "battleye",
        "prereq",
        "launcher_config",
        "redist",
    ];

    let normalize = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect()
    };

    let title_key = normalize(title);
    let mut best: Option<(i32, String)> = None;

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return String::new(),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let lower = name.to_lowercase();
        if !lower.ends_with(".exe") {
            continue;
        }

        let stem = normalize(lower.trim_end_matches(".exe"));
        let mut score = 1;

        if NOISE.iter().any(|n| lower.contains(n)) {
            score -= 100;
        }
        if !title_key.is_empty()
            && (stem == title_key || stem.contains(&title_key) || title_key.contains(&stem))
        {
            score += 50;
        }

        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, name.to_string()));
        }
    }

    best.map(|(_, name)| name).unwrap_or_default()
}

/// Helper that extracts (percent, current_bytes, total_bytes) from a steamcmd progress line.
///
/// SteamCMD emits lines such as:
/// `Update state (0x61) downloading, progress: 45.67 (456789012 / 1000000000)`
/// — note there is no `%` sign, so the percentage has to be derived from the
/// byte counters when it is absent.
fn parse_steamcmd_progress_line(line: &str) -> Option<(f64, u64, u64)> {
    const MARKER: &str = "progress:";
    let idx = line.find(MARKER)?;
    let rest = line[idx + MARKER.len()..].trim();

    // Split "12.34" from "(123 / 456)"
    let (head, tail) = match rest.find('(') {
        Some(p) => (&rest[..p], Some(&rest[p + 1..])),
        None => (rest, None),
    };

    let percent = head
        .trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .ok();

    let mut current = 0u64;
    let mut total = 0u64;
    if let Some(tail) = tail {
        if let Some(close) = tail.find(')') {
            let mut parts = tail[..close].split('/');
            if let (Some(c), Some(t)) = (parts.next(), parts.next()) {
                current = c.trim().replace([',', ' '], "").parse().unwrap_or(0);
                total = t.trim().replace([',', ' '], "").parse().unwrap_or(0);
            }
        }
    }

    // Prefer the reported figure; otherwise derive it from the byte counters
    let percent = match percent {
        Some(p) => {
            // Some builds report the raw fraction as a percentage pointer
            if p.is_nan() {
                return None;
            }
            p
        }
        None if total > 0 => (current as f64 / total as f64) * 100.0,
        None => return None,
    };

    Some((percent, current, total))
}

#[cfg(test)]
mod tests {
    use super::{
        accounts_has_account, classify_login_output, config_vdf_holds_token, forget_session_in,
        last_logged_progress, login_confirmed, loginusers_has_account,
        parse_steamcmd_progress_line, pick_launch_exe, requires_steam_guard,
    };

    #[test]
    fn reads_progress_from_the_steamcmd_log_tail() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("console_log.txt");
        std::fs::write(&path, "stale run\n").unwrap();
        let offset = std::fs::metadata(&path).unwrap().len();

        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(
            file,
            "[2026-10-20 20:31:53]  Update state (0x61) downloading, progress: 4.52 (236361223 / 5231995691)"
        )
        .unwrap();
        writeln!(
            file,
            "[2026-10-20 20:31:55]  Update state (0x61) downloading, progress: 6.93 (362325575 / 5231995691)"
        )
        .unwrap();

        let sample = last_logged_progress(&path, offset).expect("progress line");
        assert!((sample.percent - 6.93).abs() < 0.01);
        assert_eq!(sample.current, 362_325_575);
        assert_eq!(sample.total, 5_231_995_691);
        assert!(!sample.verifying);

        // Nothing appended past this offset → no progress from this run
        assert!(last_logged_progress(&path, offset + 10_000).is_none());
    }

    #[test]
    fn writes_steam_appid_for_steamworks_games() {
        let dir = tempfile::tempdir().unwrap();
        super::ensure_steam_appid_file(dir.path(), 367520);

        let written = std::fs::read_to_string(dir.path().join("steam_appid.txt")).unwrap();
        assert_eq!(written, "367520");

        // A second call must not clobber the file
        std::fs::write(dir.path().join("steam_appid.txt"), "111").unwrap();
        super::ensure_steam_appid_file(dir.path(), 367520);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("steam_appid.txt")).unwrap(),
            "111"
        );
    }

    #[test]
    fn detects_a_remembered_steam_account() {
        let vdf = r#"
"users"
{
	"76561199517025580"
	{
		"AccountName"		"sunzukz"
		"PersonaName"		"RAiNFALLS"
		"RememberPassword"		"1"
		"AutoLogin"		"1"
	}
}
"#;
        assert!(loginusers_has_account(vdf, "sunzukz"));
        assert!(loginusers_has_account(vdf, "SunZukZ"));
        assert!(!loginusers_has_account(vdf, "someone_else"));
        assert!(!loginusers_has_account("\"users\"\n{\n}\n", "sunzukz"));
    }

    #[test]
    fn guard_success_chatter_is_not_a_prompt() {
        assert!(!requires_steam_guard("Logging in using username/password."));
        assert!(!requires_steam_guard("Steam Guard code provided."));
        assert!(!requires_steam_guard("Logging in user 'x' to Steam Public...OK"));
        assert!(!requires_steam_guard("Success! App '674750' fully installed."));
    }

    #[test]
    fn real_guard_prompts_are_detected() {
        assert!(requires_steam_guard(
            "This account is protected by Steam Guard. Please enter the Steam Guard code from your email."
        ));
        assert!(requires_steam_guard("ERROR (Invalid Login Auth Code)"));
        assert!(requires_steam_guard("Account Login Denied NeedTwoFactorCode"));
        assert!(requires_steam_guard("Enter the Steam Guard code:"));
    }

    #[test]
    fn picks_the_game_binary_and_skips_helpers() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "UnityCrashHandler64.exe",
            "vcredist_x64.exe",
            "Cuphead.exe",
            "readme.txt",
        ] {
            std::fs::write(dir.path().join(name), b"stub").unwrap();
        }
        assert_eq!(pick_launch_exe(dir.path(), "Cuphead"), "Cuphead.exe");
    }

    #[test]
    fn returns_empty_when_no_executable_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"stub").unwrap();
        assert_eq!(pick_launch_exe(dir.path(), "Anything"), "");
    }

    #[test]
    fn parses_downloading_progress_without_percent_sign() {
        let line = "Update state (0x61) downloading, progress: 45.67 (456789012 / 1000000000)";
        assert_eq!(
            parse_steamcmd_progress_line(line),
            Some((45.67, 456_789_012, 1_000_000_000))
        );
    }

    #[test]
    fn derives_percent_when_only_byte_counters_exist() {
        let line = "Update state (0x61) downloading, progress: (250 / 1000)";
        let (percent, current, total) = parse_steamcmd_progress_line(line).unwrap();
        assert!((percent - 25.0).abs() < f64::EPSILON);
        assert_eq!((current, total), (250, 1000));
    }

    #[test]
    fn parses_validating_progress() {
        let line = "Update state (0x81) validating, progress: 45.60 (456 / 1000)";
        assert_eq!(
            parse_steamcmd_progress_line(line),
            Some((45.6, 456, 1000))
        );
    }

    #[test]
    fn tolerates_thousands_separators() {
        let line = "downloading, progress: 1 (1,500 / 300,000)";
        let (percent, current, total) = parse_steamcmd_progress_line(line).unwrap();
        assert!((percent - 1.0).abs() < f64::EPSILON);
        assert_eq!((current, total), (1500, 300_000));
    }

    #[test]
    fn ignores_lines_without_progress() {
        assert_eq!(
            parse_steamcmd_progress_line("Success! App '268910' fully installed."),
            None
        );
        assert_eq!(parse_steamcmd_progress_line(""), None);
    }

    #[test]
    fn accepts_a_confirmed_login() {
        let output = "Logging in user 'sunzukz' to Steam Public...\nLogged in OK\nWaiting for user info...OK";
        assert!(classify_login_output(output).is_ok());
    }

    #[test]
    fn accepts_the_inline_ok_that_steamcmd_actually_prints() {
        // Verbatim stdout from SteamCMD 1792617903 after a Guard code was accepted
        let output = "\
Loading Steam API...OK
Logging in using username/password.
Steam Guard code provided.
Logging in user 'sunzukz' [U:1:1556759852] to Steam Public...OK
Waiting for client config...OK
Waiting for user info...OK
Unloading Steam API...OK";
        assert!(login_confirmed(output));
        assert!(classify_login_output(output).is_ok());
    }

    #[test]
    fn does_not_confirm_on_the_login_start_line_alone() {
        assert!(!login_confirmed("Logging in using username/password."));
        assert!(!login_confirmed("Steam Guard code provided."));
    }

    #[test]
    fn reads_the_account_steamcmd_remembers_in_config_vdf() {
        // Shape SteamCMD actually writes: no loginusers.vdf anywhere, the
        // remembered account lives inside config.vdf.
        let vdf = "\"InstallConfigStore\"
{
\t\"Software\"
\t{
\t\t\"Valve\"
\t\t{
\t\t\t\"Steam\"
\t\t\t{
\t\t\t\t\"Accounts\"
\t\t\t\t{
\t\t\t\t\t\"sunzukz\"
\t\t\t\t\t{
\t\t\t\t\t\t\"SteamID\"\t\t\"76561199517025580\"
\t\t\t\t\t}
\t\t\t\t}
\t\t\t}
\t\t}
\t}
}
";
        assert!(accounts_has_account(vdf, "sunzukz"));
        assert!(accounts_has_account(vdf, "SunZukZ"));
        assert!(!accounts_has_account(vdf, "someone_else"));
        assert!(!accounts_has_account("", "sunzukz"));

        // A remembered name is not a session: without a token a bare `+login`
        // fails with "Cached credentials not found".
        assert!(!config_vdf_holds_token(vdf, "sunzukz"));

        let with_token = vdf.replace(
            "\"Accounts\"",
            "\"ConnectCache\"\n\t\t\t\t{\n\t\t\t\t\t\"76561199517025580\"\t\t\"token\"\n\t\t\t\t}\n\t\t\t\t\"Accounts\"",
        );
        assert!(config_vdf_holds_token(&with_token, "sunzukz"));
        assert!(!config_vdf_holds_token(&with_token, "someone_else"));
    }

    #[test]
    fn disconnecting_removes_cached_account_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path();

        for name in ["loginusers.vdf", "config.vdf", "ssfn1234567890"] {
            std::fs::write(config.join(name), "x").unwrap();
        }
        std::fs::write(config.join("libraryfolders.vdf"), "x").unwrap();

        forget_session_in(config);

        assert!(!config.join("loginusers.vdf").exists());
        assert!(!config.join("config.vdf").exists());
        assert!(!config.join("ssfn1234567890").exists());
        assert!(
            config.join("libraryfolders.vdf").exists(),
            "unrelated SteamCMD files must survive a disconnect"
        );
    }

    #[test]
    fn rejects_a_wrong_password() {
        let output = "Logging in user 'sunzukz' to Steam Public...\nFAILED (Invalid Password)\n";
        let err = classify_login_output(output).unwrap_err();
        assert!(err.contains("rejected that password"), "got: {err}");
    }

    #[test]
    fn rejects_a_missing_guard_code() {
        let output = "FAILED (InvalidLoginAuthCode)\nTwo-factor code is required";
        let err = classify_login_output(output).unwrap_err();
        assert!(err.contains("Steam Guard"), "got: {err}");
    }

    #[test]
    fn reports_rate_limiting_and_unknown_accounts() {
        assert!(classify_login_output("FAILED (Rate Limit Exceeded)")
            .unwrap_err()
            .contains("rate-limited"));
        assert!(classify_login_output("FAILED (Account Not Found)")
            .unwrap_err()
            .contains("does not exist"));
    }

    #[test]
    fn does_not_treat_silence_as_success() {
        assert!(classify_login_output("SteamCMD starting...").is_err());
    }
}

#[cfg(test)]
mod extract_tests {
    use super::extract_zip;
    use std::io::Write;

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn extracts_in_process() {
        let dir = tempfile::tempdir().unwrap();
        let archive = zip_with(&[("steamcmd.exe", b"MZ stub"), ("bin/readme.txt", b"hi")]);
        extract_zip(&archive, dir.path()).unwrap();
        assert_eq!(std::fs::read(dir.path().join("steamcmd.exe")).unwrap(), b"MZ stub");
        assert_eq!(std::fs::read(dir.path().join("bin/readme.txt")).unwrap(), b"hi");
    }

    #[test]
    fn refuses_entries_outside_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("steamcmd");
        let archive = zip_with(&[("../escaped.exe", b"x")]);
        assert!(extract_zip(&archive, &inner).is_err());
        assert!(!dir.path().join("escaped.exe").exists());
        assert!(extract_zip(b"not a zip", &inner).is_err());
    }
}
