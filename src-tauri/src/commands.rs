use crate::cloud_save::CloudSaveManager;
use crate::error::{AppError, Result};
use crate::installer::Installer;
use crate::launcher::Launcher;
use crate::manifest::Manifest;
use crate::models::{
    AppSettings, CloudSaveState, CustomInstallRequest, DownloadProgress, DownloadStatus, GameItem,
    InstalledGame, ManifestInfo, SteamAuthStatus, SteamCredentialStatus, SteamCredentials,
    SystemDiagnostics, UserProfile, VerificationResult,
};
use crate::state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use sysinfo::{Disks, System};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tracing::{info, warn};

// ==========================================
// Authentication Commands
// ==========================================

#[tauri::command]
pub async fn auth_get_login_url(state: State<'_, Arc<AppState>>) -> Result<String> {
    Ok(state.api_client.get_login_url())
}

const EPIC_AUTH_INJECTION_SCRIPT: &str = r#"
(function() {
  // 1. Strictly disable default browser context menus (Back, Refresh, Save as, Print)
  window.addEventListener('contextmenu', function(e) {
    e.preventDefault();
    e.stopPropagation();
    return false;
  }, true);

  // 2. Disable browser reload and print shortcuts
  window.addEventListener('keydown', function(e) {
    if (e.key === 'F5' || (e.ctrlKey && (e.key === 'r' || e.key === 'R' || e.key === 'p' || e.key === 'P'))) {
      e.preventDefault();
      e.stopPropagation();
    }
  }, true);

  var handled = false;
  function handleDetectedCode(code) {
    if (handled) return;
    handled = true;

    // Immediately replace page with sleek Hyperion completion screen
    try {
      if (document.body) {
        document.body.innerHTML = `
          <div style="position:fixed;inset:0;background:#09090b;color:#f4f4f5;display:flex;flex-direction:column;align-items:center;justify-content:center;font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;z-index:9999999;">
            <div style="width:44px;height:44px;border:3px solid #ff6600;border-top-color:transparent;border-radius:50%;animation:hyperion-spin 0.8s linear infinite;margin-bottom:18px;"></div>
            <style>@keyframes hyperion-spin{to{transform:rotate(360deg)}}</style>
            <h2 style="font-size:20px;font-weight:700;margin:0 0 8px;color:#ff7700;">Sign In Complete</h2>
            <p style="font-size:13px;color:#a1a1aa;margin:0;">Connecting your Epic Games account...</p>
          </div>
        `;
      }
    } catch(e) {}

    // Backup title signal
    document.title = 'AUTH_SUCCESS:' + code;

    // Instant internal callback navigation (intercepted by native layer)
    window.location.href = 'https://hyperion-auth-callback.local/?code=' + encodeURIComponent(code);
  }

  function checkContent() {
    if (handled) return true;
    try {
      // 1. Check URL parameters
      var urlParams = new URLSearchParams(window.location.search);
      var code = urlParams.get('code') || urlParams.get('authorizationCode');
      if (code && typeof code === 'string' && code.length >= 20) {
        handleDetectedCode(code);
        return true;
      }
      if (window.location.hash) {
        var hashParams = new URLSearchParams(window.location.hash.substring(1));
        var hCode = hashParams.get('code') || hashParams.get('authorizationCode');
        if (hCode && typeof hCode === 'string' && hCode.length >= 20) {
          handleDetectedCode(hCode);
          return true;
        }
      }

      // 2. Check document body text / JSON (Epic /id/api/redirect renders JSON with authorizationCode)
      var text = (document.body && document.body.innerText) || (document.documentElement && document.documentElement.innerText) || '';
      text = text.trim();
      if (text.length > 0) {
        if (text.startsWith('{') && text.endsWith('}')) {
          try {
            var data = JSON.parse(text);
            if (data.authorizationCode && typeof data.authorizationCode === 'string' && data.authorizationCode.length >= 20) {
              handleDetectedCode(data.authorizationCode);
              return true;
            }
          } catch(e) {}
        }

        // Substring / regex match for "authorizationCode": "..."
        var mAuth = text.match(/"authorizationCode"\s*:\s*"([a-fA-F0-9]{20,})"/);
        if (mAuth && mAuth[1]) {
          handleDetectedCode(mAuth[1]);
          return true;
        }
      }
    } catch(err) {}
    return false;
  }

  // Fast check and continuous polling
  if (!checkContent()) {
    var timer = setInterval(function() {
      if (checkContent() || handled) {
        clearInterval(timer);
      }
    }, 100);

    document.addEventListener('DOMContentLoaded', checkContent);
    window.addEventListener('load', checkContent);
  }
})();
"#;

#[tauri::command]
pub async fn auth_start_automatic_login(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    // 1. Close any existing auth window
    if let Some(existing) = app.get_webview_window("epic_auth") {
        let _ = existing.close();
    }

    let login_url_str = state.api_client.get_login_url();
    let parsed_url: url::Url = login_url_str
        .parse()
        .map_err(|e| AppError::Auth(format!("Invalid Epic login URL: {}", e)))?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let tx_clone = tx.clone();

    // 2. Build dedicated clean native auth window
    let builder = WebviewWindowBuilder::new(
        &app,
        "epic_auth",
        WebviewUrl::External(parsed_url),
    )
    .title("Sign In - Epic Games")
    .inner_size(500.0, 700.0)
    .center()
    .resizable(false)
    .initialization_script(EPIC_AUTH_INJECTION_SCRIPT)
    .on_navigation(move |nav_url: &url::Url| {
        // Intercept our dummy callback host
        if nav_url.host_str() == Some("hyperion-auth-callback.local") {
            for (k, v) in nav_url.query_pairs() {
                if k == "code" && v.len() >= 20 {
                    info!("Captured OAuth code directly from webview intercept: {}", &v);
                    let _ = tx_clone.send(v.to_string());
                    break;
                }
            }
            return false; // Cancel navigation to dummy host
        }

        // Also check if redirected with query params
        if let Some(_query) = nav_url.query() {
            for (k, v) in nav_url.query_pairs() {
                if (k == "code" || k == "authorizationCode") && v.len() >= 20 {
                    info!("Captured OAuth code via query parameter: {}", &v);
                    let _ = tx_clone.send(v.to_string());
                    return false;
                }
            }
        }

        true
    });

    builder
        .build()
        .map_err(|e| AppError::Auth(format!("Failed to open sign-in window: {}", e)))?;

    // 3. Background token receiver & session initializer
    let app_handle = app.clone();
    let state_arc = Arc::clone(&state);

    tokio::spawn(async move {
        let mut rx = rx;
        let mut poll_interval = tokio::time::interval(std::time::Duration::from_millis(200));
        let mut final_code: Option<String> = None;
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(300); // 5 min timeout

        loop {
            if start_time.elapsed() > timeout {
                warn!("Authentication timed out after 5 minutes");
                let _ = app_handle.emit("auth-login-failed", "Sign-in timed out. Please try again.");
                break;
            }

            // Check if window is still open
            let win = match app_handle.get_webview_window("epic_auth") {
                Some(w) => w,
                None => {
                    info!("Sign-in window closed by user");
                    break;
                }
            };

            // Check if code was intercepted
            if let Ok(code) = rx.try_recv() {
                final_code = Some(code);
                break;
            }

            // Backup check: window title
            if let Ok(title) = win.title() {
                if let Some(code) = title.strip_prefix("AUTH_SUCCESS:") {
                    let code = code.trim().to_string();
                    if code.len() >= 20 {
                        info!("Captured OAuth code via window title: {}", &code);
                        final_code = Some(code);
                        break;
                    }
                }
            }

            poll_interval.tick().await;
        }

        if let Some(code) = final_code {
            info!("Authenticating session with fresh authorization code...");
            let login_res = state_arc.api_client.login_with_code(&code).await;

            match login_res {
                Ok(session) => {
                    if let Err(e) = state_arc.storage.save_session(&session) {
                        warn!("Failed to save session to database: {:?}", e);
                    }

                    let profile = UserProfile {
                        account_id: session.account_id,
                        display_name: session
                            .display_name
                            .unwrap_or_else(|| "Epic Gamer".to_string()),
                        email: None,
                        country: None,
                        logged_in: true,
                    };

                    // Automatically close the sign-in window
                    if let Some(win) = app_handle.get_webview_window("epic_auth") {
                        let _ = win.close();
                    }

                    info!("Native zero-copy authentication successful for {}", &profile.display_name);
                    let _ = app_handle.emit("auth-login-success", &profile);
                }
                Err(err) => {
                    warn!("Failed to authenticate with captured code: {:?}", err);
                    let _ = app_handle.emit(
                        "auth-login-failed",
                        format!("Authentication failed: {}", err),
                    );
                }
            }
        }
    });

    Ok(())
}


#[tauri::command]
pub async fn auth_cancel_automatic_login(app: AppHandle) -> Result<()> {
    if let Some(win) = app.get_webview_window("epic_auth") {
        let _ = win.close();
    }
    Ok(())
}

const STEAM_AUTH_INJECTION_SCRIPT: &str = r#"
(function() {
  var handled = false;
  function checkSteamLogin() {
    if (handled) return;
    try {
      var cookies = document.cookie || '';
      var m = cookies.match(/steamLoginSecure=([0-9]+)%7C%7C([^;]+)/);
      if (!m) {
        m = cookies.match(/steamLoginSecure=([^;]+)/);
      }

      var steamId = '';
      if (m && m[1]) {
        steamId = m[1];
      } else if (window.g_steamID && window.g_steamID !== '0') {
        steamId = String(window.g_steamID);
      }

      var accountName = '';
      var userElem = document.querySelector('.persona') || document.querySelector('#account_pulldown') || document.querySelector('.user_avatar');
      if (userElem && userElem.textContent) {
        accountName = userElem.textContent.trim();
      }

      if (m || (steamId && !window.location.href.includes('/login'))) {
        handled = true;
        var token = m ? m[0] : ('steamLoginSecure=' + steamId);

        try {
          if (document.body) {
            document.body.innerHTML = `
              <div style="position:fixed;inset:0;background:#09090b;color:#f4f4f5;display:flex;flex-direction:column;align-items:center;justify-content:center;font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;z-index:9999999;">
                <div style="width:44px;height:44px;border:3px solid #0ea5e9;border-top-color:transparent;border-radius:50%;animation:hyperion-spin 0.8s linear infinite;margin-bottom:18px;"></div>
                <style>@keyframes hyperion-spin{to{transform:rotate(360deg)}}</style>
                <h2 style="font-size:20px;font-weight:700;margin:0 0 8px;color:#38bdf8;">Steam Sign In Complete</h2>
                <p style="font-size:13px;color:#a1a1aa;margin:0;">Connecting your Steam account to Hyperion...</p>
              </div>
            `;
          }
        } catch(e) {}

        window.location.href = 'https://hyperion-steam-callback.local/?token=' + encodeURIComponent(token) + '&steamid=' + encodeURIComponent(steamId) + '&account=' + encodeURIComponent(accountName);
      }
    } catch(err) {}
  }

  setInterval(checkSteamLogin, 400);
  document.addEventListener('DOMContentLoaded', checkSteamLogin);
  window.addEventListener('load', checkSteamLogin);
})();
"#;

#[tauri::command]
pub async fn steam_start_automatic_login(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    if let Some(existing) = app.get_webview_window("steam_auth") {
        let _ = existing.close();
    }

    let login_url = url::Url::parse("https://store.steampowered.com/login/")
        .map_err(|e| AppError::Auth(format!("Invalid Steam login URL: {}", e)))?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<(String, String, String)>();
    let tx_clone = tx.clone();

    let builder = WebviewWindowBuilder::new(
        &app,
        "steam_auth",
        WebviewUrl::External(login_url),
    )
    .title("Sign In with Steam - Official Valve Web Login")
    .inner_size(520.0, 720.0)
    .center()
    .resizable(false)
    .initialization_script(STEAM_AUTH_INJECTION_SCRIPT)
    .on_navigation(move |nav_url: &url::Url| {
        if nav_url.host_str() == Some("hyperion-steam-callback.local") {
            let mut token = String::new();
            let mut steam_id = String::new();
            let mut account = String::new();

            for (k, v) in nav_url.query_pairs() {
                if k == "token" { token = v.to_string(); }
                if k == "steamid" { steam_id = v.to_string(); }
                if k == "account" { account = v.to_string(); }
            }

            info!("Captured Steam webview token! SteamID: {}", steam_id);
            let _ = tx_clone.send((token, steam_id, account));
            return false;
        }
        true
    });

    builder
        .build()
        .map_err(|e| AppError::Auth(format!("Failed to open Steam sign-in window: {}", e)))?;

    let app_handle = app.clone();
    let state_arc = Arc::clone(&state);

    tokio::spawn(async move {
        let mut rx = rx;
        let mut poll_interval = tokio::time::interval(std::time::Duration::from_millis(250));
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(300);

        loop {
            if start_time.elapsed() > timeout {
                warn!("Steam authentication timed out after 5 minutes");
                let _ = app_handle.emit("steam-login-failed", "Steam sign-in timed out. Please try again.");
                break;
            }

            let win = match app_handle.get_webview_window("steam_auth") {
                Some(w) => w,
                None => {
                    info!("Steam sign-in window closed by user");
                    break;
                }
            };

            if let Ok((token, steam_id, account)) = rx.try_recv() {
                let mut settings = match state_arc.storage.get_settings() {
                    Ok(s) => s,
                    Err(_) => AppSettings::default(),
                };

                let effective_account = if !account.is_empty() {
                    Some(account)
                } else {
                    settings.steam_account
                };

                let effective_steam_id = if !steam_id.is_empty() {
                    Some(steam_id)
                } else {
                    settings.steam_id
                };

                settings.steam_token = Some(token);
                settings.steam_account = effective_account.clone();
                settings.steam_id = effective_steam_id.clone();

                if let Err(e) = state_arc.storage.save_settings(&settings) {
                    warn!("Failed to save Steam token credentials to storage: {}", e);
                }

                let status = SteamAuthStatus {
                    logged_in: true,
                    account_name: effective_account,
                    steam_id: effective_steam_id,
                };

                let _ = app_handle.emit("steam-login-success", &status);
                info!("Steam token-based sign-in succeeded!");

                let _ = win.close();
                break;
            }

            poll_interval.tick().await;
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn steam_cancel_automatic_login(app: AppHandle) -> Result<()> {
    if let Some(win) = app.get_webview_window("steam_auth") {
        let _ = win.close();
    }
    Ok(())
}

#[tauri::command]
pub async fn steam_get_auth_status(state: State<'_, Arc<AppState>>) -> Result<SteamAuthStatus> {
    // Only what the user signed in with is reported — Steam's own account files
    // (config/loginusers.vdf) are never opened, which is exactly the pattern
    // antivirus engines flag in Steam-session stealers.
    let settings = state.storage.get_settings()?;

    let account_name = settings.steam_account.clone();
    let steam_id = settings.steam_id.clone();
    let logged_in = settings.steam_token.is_some() || account_name.is_some();

    Ok(SteamAuthStatus {
        logged_in,
        account_name,
        steam_id,
    })
}

/// Forgets the Steam account completely: in-memory session, DPAPI-protected
/// password, persisted account/token and SteamCMD's remembered login.
async fn clear_steam_session(state: &AppState) -> Result<()> {
    *state.steam_credentials.lock().await = None;

    // Drop the remembered password (a no-op when nothing was stored)
    crate::secret::forget(&state.storage)?;

    let mut settings = state.storage.get_settings()?;
    settings.steam_token = None;
    settings.steam_account = None;
    settings.steam_id = None;
    state.storage.save_settings(&settings)?;

    // Make Valve's engine forget the account too, so the next download cannot
    // silently authenticate with the cached session we just disconnected.
    crate::steam::downloader::forget_cached_session();

    Ok(())
}

#[tauri::command]
pub async fn steam_logout(state: State<'_, Arc<AppState>>) -> Result<()> {
    clear_steam_session(&state).await?;
    info!("Disconnected the Steam account");
    Ok(())
}

// ==========================================
// Headless Steam engine credentials
// ==========================================

/// Returns whether the headless engine has a Steam session in memory.
/// Only the account name is exposed — the password never leaves the process.
#[tauri::command]
pub async fn steam_get_credential_status(
    state: State<'_, Arc<AppState>>,
) -> Result<SteamCredentialStatus> {
    let creds = state.steam_credentials.lock().await;
    let settings = state.storage.get_settings().ok();
    let remembered = settings
        .as_ref()
        .map(|s| s.steam_password_enc.is_some())
        .unwrap_or(false);

    let account = creds
        .as_ref()
        .map(|c| c.username.clone())
        .or_else(|| settings.and_then(|s| s.steam_account));

    // A cached SteamCMD session is what actually authorises downloads, whether
    // or not Hyperion still holds the password.
    let engine_session_cached = account
        .as_deref()
        .map(crate::steam::downloader::cached_session_exists)
        .unwrap_or(false);

    Ok(SteamCredentialStatus {
        has_credentials: creds.is_some(),
        username: account,
        remembered,
        engine_session_cached,
    })
}

/// Stores Steam credentials for the headless engine.
///
/// With `remember` the password is written to the local database **encrypted
/// with Windows DPAPI** (bound to the current Windows user); without it, the
/// credential stays in process memory for this run only.
#[tauri::command]
pub async fn steam_set_credentials(
    username: String,
    password: String,
    guard_code: Option<String>,
    remember: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<SteamCredentialStatus> {
    let username = username.trim().to_string();
    if username.is_empty() || password.is_empty() {
        return Err(AppError::Auth(
            "Steam username and password are both required".to_string(),
        ));
    }

    let guard_code = guard_code
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());

    let remember = remember.unwrap_or(false);

    // Validate against Valve before anything is stored or downloaded: Steam has
    // no "check password" API, so this is a throwaway SteamCMD sign-in (+quit,
    // no +app_update). A rejected password never reaches the database.
    info!("Verifying Steam credentials for '{}'", username);
    match crate::steam::downloader::SteamDownloadManager::verify_login(
        &username,
        &password,
        guard_code.as_deref(),
    )
    .await
    {
        Ok(()) => info!("Steam confirmed the credentials for '{}'", username),
        // Connectivity problems must not lock the user out of offline installs
        // from an already-cached SteamCMD session.
        Err(reason)
            if reason.starts_with("Valve timed out") || reason.starts_with("Could not start SteamCMD") =>
        {
            warn!("Could not verify Steam sign-in ({}); keeping the credentials", reason);
        }
        Err(reason) => {
            return Err(AppError::Auth(format!("Steam sign-in failed: {}", reason)));
        }
    }

    // Remember which account signed in, so later runs can show it without ever
    // opening Steam's own account files. Signing in also turns on the local
    // library scan: the whole point of connecting is seeing the Steam games, and
    // it stays switchable from Settings.
    if let Ok(mut settings) = state.storage.get_settings() {
        settings.steam_account = Some(username.clone());
        if !settings.steam_library_scan {
            settings.steam_library_scan = true;
            info!("Enabled the Steam library scan after a successful sign-in");
        }
        if let Err(e) = state.storage.save_settings(&settings) {
            warn!("Could not persist the Steam account name: {}", e);
        }
    }

    if remember {
        crate::secret::remember(&state.storage, &username, &password)?;
        info!("Remembered DPAPI-protected Steam credential for '{}'", username);
    } else {
        crate::secret::forget(&state.storage)?;
        info!("Stored in-memory Steam session for '{}'", username);
    }

    let mut creds = state.steam_credentials.lock().await;
    *creds = Some(SteamCredentials {
        username: username.clone(),
        password,
        guard_code,
    });

    Ok(SteamCredentialStatus {
        has_credentials: true,
        engine_session_cached: crate::steam::downloader::cached_session_exists(&username),
        username: Some(username),
        remembered: remember,
    })
}

/// Drops the in-memory Steam session and any remembered password.
#[tauri::command]
pub async fn steam_clear_credentials(state: State<'_, Arc<AppState>>) -> Result<()> {
    clear_steam_session(&state).await?;
    info!("Cleared Steam session and remembered credential");
    Ok(())
}

/// Attaches a fresh Steam Guard code to the session already held in memory,
/// so a guard challenge does not force the password to be typed again.
#[tauri::command]
pub async fn steam_set_guard_code(
    guard_code: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SteamCredentialStatus> {
    let code = guard_code.trim().to_string();
    if code.is_empty() {
        return Err(AppError::Auth("Steam Guard code is required".to_string()));
    }

    let remembered = state
        .storage
        .get_settings()
        .map(|s| s.steam_password_enc.is_some())
        .unwrap_or(false);

    let mut creds = state.steam_credentials.lock().await;
    match creds.as_mut() {
        Some(existing) => {
            existing.guard_code = Some(code);
            Ok(SteamCredentialStatus {
                has_credentials: true,
                engine_session_cached: crate::steam::downloader::cached_session_exists(
                    &existing.username,
                ),
                username: Some(existing.username.clone()),
                remembered,
            })
        }
        None => Err(AppError::Auth(
            "No Steam session in memory — sign in again".to_string(),
        )),
    }
}

#[tauri::command]
pub async fn auth_login_code(code: String, state: State<'_, Arc<AppState>>) -> Result<UserProfile> {
    let session = state.api_client.login_with_code(&code).await?;
    state.storage.save_session(&session)?;

    Ok(UserProfile {
        account_id: session.account_id,
        display_name: session
            .display_name
            .unwrap_or_else(|| "Epic Gamer".to_string()),
        email: None,
        country: None,
        logged_in: true,
    })
}

#[tauri::command]
pub async fn auth_login_exchange(
    code: String,
    state: State<'_, Arc<AppState>>,
) -> Result<UserProfile> {
    let session = state.api_client.login_with_exchange_code(&code).await?;
    state.storage.save_session(&session)?;

    Ok(UserProfile {
        account_id: session.account_id,
        display_name: session
            .display_name
            .unwrap_or_else(|| "Epic Gamer".to_string()),
        email: None,
        country: None,
        logged_in: true,
    })
}

#[tauri::command]
pub async fn auth_resume_session(state: State<'_, Arc<AppState>>) -> Result<Option<UserProfile>> {
    let saved_session = state.storage.get_saved_session()?;
    if let Some(session) = saved_session {
        info!(
            "Attempting to resume saved session for {}",
            session.account_id
        );

        let is_valid = state
            .api_client
            .verify_session(&session.access_token)
            .await
            .unwrap_or(false);

        if is_valid {
            state.api_client.set_session(Some(session.clone())).await;
            return Ok(Some(UserProfile {
                account_id: session.account_id,
                display_name: session.display_name.unwrap_or_else(|| "User".to_string()),
                email: None,
                country: None,
                logged_in: true,
            }));
        }

        // Try refresh token if available
        if let Some(ref rt) = session.refresh_token {
            info!("Access token expired, attempting token refresh");
            if let Ok(new_session) = state.api_client.refresh_session(rt).await {
                state.storage.save_session(&new_session)?;
                return Ok(Some(UserProfile {
                    account_id: new_session.account_id,
                    display_name: new_session
                        .display_name
                        .unwrap_or_else(|| "User".to_string()),
                    email: None,
                    country: None,
                    logged_in: true,
                }));
            }
        }
    }

    Ok(None)
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, Arc<AppState>>) -> Result<()> {
    info!("User logging out");
    state.api_client.set_session(None).await;
    state.storage.clear_session()?;
    Ok(())
}

#[tauri::command]
pub async fn auth_get_profile(state: State<'_, Arc<AppState>>) -> Result<Option<UserProfile>> {
    if let Some(session) = state.api_client.get_session().await {
        Ok(Some(UserProfile {
            account_id: session.account_id,
            display_name: session.display_name.unwrap_or_else(|| "User".to_string()),
            email: None,
            country: None,
            logged_in: true,
        }))
    } else {
        Ok(None)
    }
}

// ==========================================
// Catalog and Library Commands
// ==========================================

/// `GameItem` rows for locally installed titles, built without any network call.
///
/// Steam entries are skipped here: the Steam resolver emits them with the right
/// platform, artwork and AppID, so including them twice would surface a
/// `steam_*` title as an Epic card.
fn installed_game_items(installed: &std::collections::HashMap<String, InstalledGame>) -> Vec<GameItem> {
    installed
        .values()
        .filter(|ig| ig.platform != "steam" && !ig.app_name.starts_with("steam_"))
        .map(|ig| GameItem {
            app_name: ig.app_name.clone(),
            title: ig.title.clone(),
            namespace: "local".to_string(),
            catalog_item_id: String::new(),
            description: Some("Installed local game".to_string()),
            developer: None,
            publisher: None,
            icon_url: None,
            banner_url: None,
            logo_url: None,
            is_installed: true,
            is_dlc: ig.is_dlc,
            build_version: Some(ig.version.clone()),
            install_path: Some(ig.install_path.clone()),
            install_size: Some(ig.install_size),
            executable: Some(ig.executable.clone()),
            can_run_offline: ig.can_run_offline,
            supports_cloud_saves: false,
            platform: "epic".to_string(),
            steam_app_id: None,
        })
        .collect()
}

/// Drops duplicate entries for the same app, keeping the first occurrence.
fn dedupe_games(games: Vec<GameItem>) -> Vec<GameItem> {
    let mut seen = std::collections::HashSet::new();
    games
        .into_iter()
        .filter(|game| seen.insert(game.app_name.clone()))
        .collect()
}

/// One step of the startup flow, streamed to the boot screen.
#[derive(Clone, serde::Serialize)]
pub struct BootstrapProgress {
    pub stage: String,
    pub label: String,
    pub detail: Option<String>,
    /// `None` means "working, but no meaningful percentage" (e.g. an archive download).
    pub percent: Option<f32>,
    pub finished: bool,
}

/// What the boot screen reports once everything is ready.
#[derive(Clone, serde::Serialize)]
pub struct BootstrapReport {
    pub epic_games: usize,
    pub steam_games: usize,
    pub steamcmd_ready: bool,
    pub first_run: bool,
}

/// First-launch bootstrap: runtime files, SteamCMD, then the whole library.
///
/// Emits `bootstrap-progress` events so the splash screen can show real stages
/// instead of a spinner that means nothing. SteamCMD is only installed on a first
/// run (or when it is missing) — later launches go straight to the library.
#[tauri::command]
pub async fn app_bootstrap(
    force: bool,
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<BootstrapReport> {
    let mut settings = state.storage.get_settings()?;
    let first_run = force || !settings.bootstrap_done;

    let emit = |stage: &str, label: &str, detail: Option<String>, percent: Option<f32>, finished: bool| {
        let _ = app_handle.emit(
            "bootstrap-progress",
            BootstrapProgress {
                stage: stage.to_string(),
                label: label.to_string(),
                detail,
                percent,
                finished,
            },
        );
    };

    emit(
        "files",
        "Preparing Hyperion",
        Some("Checking local runtime files".to_string()),
        Some(0.0),
        false,
    );

    let mut steamcmd_ready = crate::steam::downloader::SteamDownloadManager::steamcmd_installed();

    if first_run && !steamcmd_ready {
        // Deliberately vague: the splash should not narrate which engine is being
        // fetched or where it comes from. The real reason stays in the log.
        emit(
            "steamcmd",
            "Setting this up",
            Some("Preparing the download engine".to_string()),
            None,
            false,
        );

        match crate::steam::downloader::SteamDownloadManager::ensure_steamcmd().await {
            Ok(_) => {
                steamcmd_ready = true;
                emit("steamcmd", "Setting this up", None, Some(100.0), true);
            }
            Err(e) => {
                // Not fatal: Epic downloads keep working, Steam installs retry later.
                warn!("SteamCMD install failed during bootstrap: {}", e);
                emit(
                    "steamcmd",
                    "Setting this up",
                    Some("Skipped for now — it will retry when needed".to_string()),
                    Some(100.0),
                    true,
                );
            }
        }
    } else {
        emit("steamcmd", "Setting this up", None, Some(100.0), true);
    }

    emit(
        "library",
        "Loading your library",
        Some("Reading the cached Epic catalog".to_string()),
        None,
        false,
    );

    let games = load_catalog(&state, false).await?;
    let steam_games = games.iter().filter(|game| game.platform == "steam").count();
    let epic_games = games.len().saturating_sub(steam_games);

    emit(
        "library",
        "Library ready",
        Some(format!("{} Epic · {} Steam titles", epic_games, steam_games)),
        Some(100.0),
        false,
    );

    if !settings.bootstrap_done {
        settings.bootstrap_done = true;
        if let Err(e) = state.storage.save_settings(&settings) {
            warn!("Could not persist the bootstrap flag: {}", e);
        }
    }

    emit("done", "Starting Hyperion", None, Some(100.0), true);

    Ok(BootstrapReport {
        epic_games,
        steam_games,
        steamcmd_ready,
        first_run,
    })
}

#[tauri::command]
pub async fn catalog_get_games(
    force_refresh: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<GameItem>> {
    load_catalog(&state, force_refresh).await
}

/// Builds the launcher library: cached Epic catalog first, then installed titles
/// and the Steam library. Shared by the UI command and the startup bootstrap so
/// the first screen and the library cannot disagree.
async fn load_catalog(state: &AppState, force_refresh: bool) -> Result<Vec<GameItem>> {
    let settings = state.storage.get_settings()?;
    let installed_games = state.storage.get_all_installed_games()?;
    let installed_map: std::collections::HashMap<String, InstalledGame> = installed_games
        .into_iter()
        .map(|g| (g.app_name.clone(), g))
        .collect();

    let session = state.api_client.get_session().await;

    // Read the cache before anything network-bound: it is the only source that
    // still works when the session is gone, expired, or the machine is offline.
    // Returning only installed games whenever there was no session is exactly
    // what made a populated library look empty.
    let cached = state.storage.get_cached_catalog_items()?;
    // Serve the cache unless a live refresh was asked for *and* we could actually
    // perform one; logged out there is nothing to refresh from, so the cache is
    // the library.
    if !cached.is_empty() && (!force_refresh || session.is_none()) {
        let mut items = Vec::new();
        for item in cached {
            let app_name = item.app_name;
            let title = item.title;
            let namespace = item.namespace;
            let catalog_item_id = item.catalog_item_id;

            // The Steam metadata fetcher caches its rows in the same catalog
            // table. They are added below by the Steam resolver (with the right
            // platform, artwork and install state), so emitting them here would
            // duplicate every Steam title as an "Epic" card.
            if namespace == "steam"
                || namespace == "steam_ignored"
                || app_name.starts_with("steam_")
            {
                continue;
            }

            let metadata: Option<crate::models::GameMetadata> =
                serde_json::from_str(&item.json_data).ok();
            let mut icon_url = None;
            let mut banner_url = None;
            let mut logo_url = None;
            let mut desc = None;
            let mut dev = None;
            let mut publ = None;

            if let Some(m) = metadata {
                desc = m.description;
                dev = m.developer;
                publ = m.publisher;
                if let Some(images) = m.key_images {
                    for img in images {
                        match img.image_type.as_str() {
                            "DieselGameBox" | "Thumbnail" | "BoxArt" => icon_url = Some(img.url),
                            "DieselGameBoxTall" | "OfferImageTall" => {
                                if icon_url.is_none() {
                                    icon_url = Some(img.url);
                                }
                            }
                            "DieselGameBoxWide" | "OfferImageWide" => banner_url = Some(img.url),
                            "Logo" => logo_url = Some(img.url),
                            _ => {}
                        }
                    }
                }
            }

            let is_installed = installed_map.contains_key(&app_name);
            let ig = installed_map.get(&app_name);

            items.push(GameItem {
                app_name: app_name.clone(),
                title,
                namespace,
                catalog_item_id,
                description: desc,
                developer: dev,
                publisher: publ,
                icon_url,
                banner_url,
                logo_url,
                is_installed,
                is_dlc: ig.map(|g| g.is_dlc).unwrap_or(false),
                build_version: ig.map(|g| g.version.clone()),
                install_path: ig.map(|g| g.install_path.clone()),
                install_size: ig.map(|g| g.install_size),
                executable: ig.map(|g| g.executable.clone()),
                can_run_offline: ig.map(|g| g.can_run_offline).unwrap_or(true),
                supports_cloud_saves: true,
                platform: "epic".to_string(),
                steam_app_id: None,
            });
        }
        items.extend(local_steam_library(&state, &settings, force_refresh).await);

        // Merge any installed Steam games from database that weren't detected via Steam client
        for (app_name, ig) in &installed_map {
            if (ig.platform == "steam" || app_name.starts_with("steam_"))
                && !items.iter().any(|item| &item.app_name == app_name)
            {
                let app_id = app_name.strip_prefix("steam_").and_then(|s| s.parse::<u32>().ok());
                items.push(GameItem {
                    app_name: app_name.clone(),
                    title: ig.title.clone(),
                    namespace: "steam".to_string(),
                    catalog_item_id: app_id.map(|id| id.to_string()).unwrap_or_default(),
                    description: Some("Steam Game installed via Hyperion".to_string()),
                    developer: None,
                    publisher: None,
                    icon_url: app_id.map(|id| format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg", id)),
                    banner_url: app_id.map(|id| format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg", id)),
                    logo_url: app_id.map(|id| format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/logo.png", id)),
                    is_installed: true,
                    is_dlc: false,
                    build_version: Some(ig.version.clone()),
                    install_path: Some(ig.install_path.clone()),
                    install_size: Some(ig.install_size),
                    executable: Some(ig.executable.clone()),
                    can_run_offline: true,
                    supports_cloud_saves: false,
                    platform: "steam".to_string(),
                    steam_app_id: app_id,
                });
            }
        }

        return Ok(dedupe_games(items));
    }

    // Nothing cached and nobody signed in: show what is installed on this PC.
    if session.is_none() {
        let mut items = installed_game_items(&installed_map);
        items.extend(local_steam_library(&state, &settings, force_refresh).await);
        return Ok(dedupe_games(items));
    }

    // Fetch real assets from Epic Games API
    info!("Fetching live catalog assets from Epic Games Store API...");
    let assets = match state.api_client.get_game_assets(&settings.platform).await {
        Ok(a) => a,
        Err(e) => {
            warn!(
                "Failed to fetch live assets: {}. Falling back to the installed library.",
                e
            );
            // Previously this returned the Steam library alone, so an expired
            // token blanked every installed Epic title too.
            let mut fallback_items = installed_game_items(&installed_map);
            fallback_items.extend(local_steam_library(&state, &settings, false).await);
            return Ok(dedupe_games(fallback_items));
        }
    };

    let mut game_items = Vec::new();

    for asset in assets {
        if asset.namespace == "ue" {
            continue; // Skip Unreal Engine engine components
        }

        let metadata = state
            .api_client
            .get_catalog_item(&asset.namespace, &asset.catalog_item_id)
            .await
            .unwrap_or(None);

        let title = metadata
            .as_ref()
            .map(|m| m.title.clone())
            .unwrap_or_else(|| asset.app_name.clone());

        let mut icon_url = None;
        let mut banner_url = None;
        let mut logo_url = None;
        let mut desc = None;
        let mut dev = None;
        let mut publ = None;

        if let Some(ref m) = metadata {
            desc = m.description.clone();
            dev = m.developer.clone();
            publ = m.publisher.clone();
            if let Some(ref images) = m.key_images {
                for img in images {
                    match img.image_type.as_str() {
                        "DieselGameBox" | "Thumbnail" | "BoxArt" => {
                            icon_url = Some(img.url.clone())
                        }
                        "DieselGameBoxTall" | "OfferImageTall" => {
                            if icon_url.is_none() {
                                icon_url = Some(img.url.clone());
                            }
                        }
                        "DieselGameBoxWide" | "OfferImageWide" => {
                            banner_url = Some(img.url.clone())
                        }
                        "Logo" => logo_url = Some(img.url.clone()),
                        _ => {}
                    }
                }
            }

            // Cache to SQLite
            if let Ok(json_str) = serde_json::to_string(m) {
                let _ = state.storage.cache_catalog_item(
                    &asset.app_name,
                    &title,
                    &asset.namespace,
                    &asset.catalog_item_id,
                    &json_str,
                );
            }
        }

        let is_installed = installed_map.contains_key(&asset.app_name);
        let ig = installed_map.get(&asset.app_name);

        game_items.push(GameItem {
            app_name: asset.app_name,
            title,
            namespace: asset.namespace,
            catalog_item_id: asset.catalog_item_id,
            description: desc,
            developer: dev,
            publisher: publ,
            icon_url,
            banner_url,
            logo_url,
            is_installed,
            is_dlc: ig.map(|g| g.is_dlc).unwrap_or(false),
            build_version: Some(asset.build_version),
            install_path: ig.map(|g| g.install_path.clone()),
            install_size: ig.map(|g| g.install_size),
            executable: ig.map(|g| g.executable.clone()),
            can_run_offline: ig.map(|g| g.can_run_offline).unwrap_or(true),
            supports_cloud_saves: true,
            platform: "epic".to_string(),
            steam_app_id: None,
        });
    }

    game_items.extend(local_steam_library(&state, &settings, force_refresh).await);

    // Merge any installed Steam games from database that weren't detected via Steam client
    for (app_name, ig) in &installed_map {
        if (ig.platform == "steam" || app_name.starts_with("steam_"))
            && !game_items.iter().any(|item| &item.app_name == app_name)
        {
            let app_id = app_name.strip_prefix("steam_").and_then(|s| s.parse::<u32>().ok());
            game_items.push(GameItem {
                app_name: app_name.clone(),
                title: ig.title.clone(),
                namespace: "steam".to_string(),
                catalog_item_id: app_id.map(|id| id.to_string()).unwrap_or_default(),
                description: Some("Steam Game installed via Hyperion".to_string()),
                developer: None,
                publisher: None,
                icon_url: app_id.map(|id| format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg", id)),
                banner_url: app_id.map(|id| format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg", id)),
                logo_url: app_id.map(|id| format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/logo.png", id)),
                is_installed: true,
                is_dlc: false,
                build_version: Some(ig.version.clone()),
                install_path: Some(ig.install_path.clone()),
                install_size: Some(ig.install_size),
                executable: Some(ig.executable.clone()),
                can_run_offline: true,
                supports_cloud_saves: false,
                platform: "steam".to_string(),
                steam_app_id: app_id,
            });
        }
    }

    Ok(dedupe_games(game_items))
}

#[tauri::command]
pub async fn catalog_get_game_details(
    app_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<GameItem>> {
    let games = catalog_get_games(false, state).await?;
    Ok(games.into_iter().find(|g| g.app_name == app_name))
}

// ==========================================
// Steam install routing helpers
// ==========================================

/// Where a title installs: the requested folder when it is the game's own, else
/// a per-game folder inside it (or inside the library root). An existing install
/// of the same app keeps its folder, so updates land in place.
fn install_target(
    state: &AppState,
    app_name: &str,
    requested: Option<&str>,
    title: &str,
) -> PathBuf {
    let default_root = state
        .storage
        .get_settings()
        .map(|s| PathBuf::from(s.default_install_dir))
        .unwrap_or_default();
    let existing = state
        .storage
        .get_installed_game(app_name)
        .ok()
        .flatten()
        .map(|g| PathBuf::from(g.install_path));
    crate::installer::resolve_install_dir(requested, &default_root, title, existing.as_deref())
}

/// Headless install engine: Valve's SteamCMD, driven with the user's Steam
/// session. Runs entirely inside Hyperion — no Steam client window and
/// no Steam library involvement; files land in the folder Hyperion was given.
async fn start_steamcmd_download(
    app_handle: AppHandle,
    app_id: u32,
    title: String,
    install_dir: Option<String>,
    credentials: Option<SteamCredentials>,
    state: &Arc<AppState>,
) -> Result<()> {
    let target_dir = install_target(state, &format!("steam_{}", app_id), install_dir.as_deref(), &title);

    state
        .steam_downloader
        .clone()
        .start_download(
            app_handle,
            app_id,
            title,
            target_dir,
            credentials,
            None,
            None,
            state.storage.clone(),
        )
        .await
        .map_err(AppError::Install)
}

// ==========================================
// Installation & Download Commands
// ==========================================

#[tauri::command]
pub async fn game_install(
    app_handle: AppHandle,
    app_name: String,
    install_dir: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    info!("Requested installation for {}", app_name);

    let settings = state.storage.get_settings()?;

    // Steam titles download headlessly inside Hyperion through Valve's
    // SteamCMD engine. SteamCMD can only allocate a depot licence to an
    // authenticated session, so a one-time Steam sign-in is required; the
    // credentials live in memory for this app run only.
    if let Some(app_id) = app_name
        .strip_prefix("steam_")
        .and_then(|s| s.parse::<u32>().ok())
    {
        let title = match state.storage.get_cached_catalog_item(&app_name) {
            Ok(Some(cached)) if !cached.title.is_empty() => cached.title,
            _ => match crate::steam::get_steam_app_details(app_id).await {
                Ok(Some(details)) => details.name,
                _ => format!("Steam App {}", app_id),
            },
        };

        let credentials = state.steam_credentials.lock().await.clone();
        if credentials.is_none() {
            return Err(AppError::Install(
                "STEAM_LOGIN_REQUIRED: Sign in to Steam once so Hyperion can download this title in-app."
                    .to_string(),
            ));
        }

        return start_steamcmd_download(app_handle, app_id, title, install_dir, credentials, &state)
            .await;
    }

    let games = catalog_get_games(false, state.clone()).await?;
    let game = games
        .into_iter()
        .find(|g| g.app_name == app_name)
        .ok_or_else(|| AppError::Install(format!("Game {} not found in library", app_name)))?;

    // Steam game discovered without the "steam_<appid>" naming convention
    if game.platform == "steam" {
        let app_id = game
            .steam_app_id
            .ok_or_else(|| AppError::Install(format!("Invalid Steam AppID for {}", app_name)))?;

        let credentials = state.steam_credentials.lock().await.clone();
        if credentials.is_none() {
            return Err(AppError::Install(
                "STEAM_LOGIN_REQUIRED: Sign in to Steam once so Hyperion can download this title in-app."
                    .to_string(),
            ));
        }

        return start_steamcmd_download(
            app_handle,
            app_id,
            game.title,
            install_dir,
            credentials,
            &state,
        )
        .await;
    }

    // 1. Fetch Manifest URLs from Epic API
    let manifest_resp = state
        .api_client
        .get_manifest_response(
            &game.namespace,
            &game.catalog_item_id,
            &game.app_name,
            &settings.platform,
        )
        .await?;

    let first_elem = manifest_resp
        .elements
        .first()
        .ok_or_else(|| AppError::Manifest("No manifest element returned by API".to_string()))?;

    // 2. Download manifest binary via CDN mirrors with signed query tokens
    let (manifest_bytes, base_urls) = state
        .api_client
        .download_manifest_from_element(first_elem)
        .await?;

    let base_url = base_urls
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Manifest("No base URL derived from CDN manifests".to_string()))?;

    let manifest = Manifest::parse(&manifest_bytes)?;

    // 3. Determine target directory
    let target_dir = install_target(&state, &game.app_name, install_dir.as_deref(), &game.title);

    let executable = if !manifest.meta.launch_exe.is_empty() {
        manifest.meta.launch_exe.clone()
    } else {
        format!("{}.exe", game.app_name)
    };

    let total_size: u64 = manifest
        .file_manifest_list
        .iter()
        .map(|f| f.file_size)
        .sum();

    // 4. Save record to installed_games SQLite table
    let installed = InstalledGame {
        app_name: game.app_name.clone(),
        title: game.title.clone(),
        version: manifest.meta.build_version.clone(),
        install_path: target_dir.to_string_lossy().to_string(),
        executable,
        launch_parameters: manifest.meta.launch_command.clone(),
        install_size: total_size,
        can_run_offline: true,
        requires_ot: false,
        platform: settings.platform.clone(),
        is_dlc: game.is_dlc,
        base_urls: vec![base_url.clone()],
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    // 5. Start chunk download pipeline (game record is saved upon 100% completion)
    state
        .downloader
        .clone()
        .start_download(
            app_handle,
            game.app_name,
            game.title,
            manifest,
            manifest_bytes,
            base_url,
            state.api_client.get_session().await.map(|s| s.access_token),
            target_dir,
            settings.max_workers,
            Some(installed),
            state.storage.clone(),
        )
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn select_manifest_file() -> Result<Option<String>> {
    let path = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Select Epic Games Manifest File (.manifest)")
            .add_filter("Epic Manifest", &["manifest", "bin"])
            .add_filter("All Files", &["*"])
            .pick_file()
    })
    .await
    .map_err(|e| AppError::General(e.to_string()))?;

    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn select_directory(
    default_path: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<String>> {
    let settings_dir = state
        .storage
        .get_settings()
        .ok()
        .map(|s| s.default_install_dir);

    let target_dir = default_path
        .filter(|p| !p.trim().is_empty())
        .or(settings_dir)
        .unwrap_or_else(|| "D:\\Games".to_string());

    let path_buf = std::path::PathBuf::from(&target_dir);
    // Ensure folder exists so Windows FileDialog opens directly into it
    if !path_buf.exists() {
        let _ = std::fs::create_dir_all(&path_buf);
    }

    let dir_to_open = if path_buf.exists() {
        path_buf
    } else if std::path::Path::new("D:\\").exists() {
        std::path::PathBuf::from("D:\\")
    } else {
        std::path::PathBuf::from("C:\\")
    };

    let path = tokio::task::spawn_blocking(move || {
        rfd::FileDialog::new()
            .set_title("Select Games Installation Directory")
            .set_directory(&dir_to_open)
            .pick_folder()
    })
    .await
    .map_err(|e| AppError::General(e.to_string()))?;

    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

/// Picks the CDN that actually serves this build's chunks.
///
/// Manifests often come from an API that has no game data, so candidates are
/// tried in order — explicit base, Epic's catalog/assets API (by every known
/// name for the build), then the manifest's own directory — and the first one
/// that answers for the first chunk wins.
async fn pick_chunk_base(
    state: &AppState,
    manifest: &Manifest,
    aliases: &[String],
    url_base: Option<&str>,
    explicit: Option<&str>,
    epic_token: Option<&str>,
) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    let mut push = |base: &str| {
        let base = base.trim_end_matches('/').to_string();
        if !base.is_empty() && !candidates.contains(&base) {
            candidates.push(base);
        }
    };

    if let Some(explicit) = explicit {
        push(explicit);
    } else {
        // Cached catalog entry, when the game is already in the library
        for alias in aliases {
            if let Ok(Some(item)) = state.storage.get_cached_catalog_item(alias) {
                if let Ok(settings) = state.storage.get_settings() {
                    if let Ok(resp) = state
                        .api_client
                        .get_manifest_response(
                            &item.namespace,
                            &item.catalog_item_id,
                            alias,
                            &settings.platform,
                        )
                        .await
                    {
                        for element in &resp.elements {
                            if let Some(entry) = element.manifests.first() {
                                push(&entry.base_url());
                            }
                        }
                    }
                }
            }
        }

        // Epic's assets API, by codename and display title
        if let Ok(settings) = state.storage.get_settings() {
            for alias in aliases {
                if let Ok(resp) = state
                    .api_client
                    .get_manifest_response_by_app(&settings.platform, alias)
                    .await
                {
                    for element in &resp.elements {
                        if let Some(entry) = element.manifests.first() {
                            push(&entry.base_url());
                        }
                    }
                }
            }
        }

        // The manifest's own directory last: right for self-hosted builds
        if let Some(dir) = url_base {
            push(dir);
        }
    }

    let first_chunk = manifest
        .chunk_data_list
        .first()
        .map(|c| c.path(manifest.version));

    if let Some(chunk_path) = &first_chunk {
        for candidate in &candidates {
            if chunk_base_responds(candidate, chunk_path, epic_token).await {
                info!("Using chunk CDN {}", candidate);
                return Some(candidate.clone());
            }
            warn!("Chunk CDN {} does not serve {}", candidate, chunk_path);
        }
    }

    // Nothing answered: fall back to the best guess so the caller can report it
    candidates.into_iter().next()
}

/// Manifest bytes for `url`, preferring the on-disk cache.
///
/// A cached copy is only used when it still parses, so a corrupted file quietly
/// falls back to a fresh download; freshly downloaded bytes are cached for the
/// next install (and for offline use).
async fn manifest_bytes_for(state: &AppState, url: &str) -> Result<(Vec<u8>, bool)> {
    if let Some(cached) = crate::manifest_cache::read(url) {
        if Manifest::parse(&cached).is_ok() {
            return Ok((cached, true));
        }
        warn!("Discarding unreadable cached manifest for {}", url);
    }

    let bytes = state.api_client.download_manifest_bytes(url).await?;
    Manifest::parse(&bytes)?; // never cache something we cannot read
    if let Err(e) = crate::manifest_cache::write(url, &bytes) {
        warn!("Could not cache manifest for {}: {}", url, e);
    }
    Ok((bytes, false))
}

/// Fresh manifest bytes for `url`, falling back to a readable cached copy.
///
/// Startup uses this instead of `manifest_bytes_for` so a manifest the sponsor
/// republishes is actually picked up: the cache-first helper would serve the old
/// file forever. The local copy is still the fallback, so an offline launch keeps
/// working, and identical bytes are not rewritten to disk.
async fn refreshed_manifest_bytes(state: &AppState, url: &str) -> Result<(Vec<u8>, bool)> {
    match state.api_client.download_manifest_bytes(url).await {
        Ok(bytes) => {
            Manifest::parse(&bytes)?; // never cache something we cannot read

            let unchanged = crate::manifest_cache::read(url)
                .map(|cached| cached == bytes)
                .unwrap_or(false);

            if !unchanged {
                if let Err(e) = crate::manifest_cache::write(url, &bytes) {
                    warn!("Could not cache manifest for {}: {}", url, e);
                }
                info!("Refreshed the cached manifest for {}", url);
            }

            Ok((bytes, false))
        }
        Err(e) => match crate::manifest_cache::read(url) {
            Some(cached) if Manifest::parse(&cached).is_ok() => {
                warn!(
                    "Manifest refresh failed for {} ({}); using the cached copy",
                    url, e
                );
                Ok((cached, true))
            }
            _ => Err(e),
        },
    }
}

async fn inspect_manifest(
    state: &AppState,
    trimmed: &str,
    refresh: bool,
) -> Result<ManifestInfo> {
    let (manifest_bytes, suggested_base_url) =
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            let (bytes, from_cache) = if refresh {
                refreshed_manifest_bytes(state, trimmed).await?
            } else {
                manifest_bytes_for(state, trimmed).await?
            };
            info!(
                "Manifest {} loaded from {} ({} bytes)",
                trimmed,
                if from_cache { "cache" } else { "network" },
                bytes.len()
            );
            let base = trimmed.rfind('/').map(|i| trimmed[..i].to_string());
            (bytes, base)
        } else {
            let bytes = tokio::fs::read(trimmed).await.map_err(|e| {
                AppError::Io(format!(
                    "Failed to read manifest file from '{}': {}",
                    trimmed, e
                ))
            })?;
            (bytes, None)
        };

    let manifest = Manifest::parse(&manifest_bytes)?;
    let app_name = manifest.meta.app_name.clone();

    // Check if app_name matches any known game in library catalog
    let matched_game_title = if !app_name.is_empty() {
        state
            .storage
            .get_cached_catalog_item(&app_name)
            .ok()
            .flatten()
            .map(|item| item.title)
    } else {
        None
    };

    let total_size: u64 = manifest
        .file_manifest_list
        .iter()
        .map(|f| f.file_size)
        .sum();

    let mut aliases: Vec<String> = Vec::new();
    for candidate in [app_name.as_str(), matched_game_title.as_deref().unwrap_or("")] {
        let candidate = candidate.trim();
        if !candidate.is_empty() && !aliases.iter().any(|a| a.eq_ignore_ascii_case(candidate)) {
            aliases.push(candidate.to_string());
        }
    }

    let epic_token = state.api_client.get_session().await.map(|s| s.access_token);
    let final_base_url = pick_chunk_base(
        state,
        &manifest,
        &aliases,
        suggested_base_url.as_deref(),
        None,
        epic_token.as_deref(),
    )
    .await
    .or(suggested_base_url);

    Ok(ManifestInfo {
        app_name,
        build_version: manifest.meta.build_version,
        launch_exe: manifest.meta.launch_exe,
        launch_command: manifest.meta.launch_command,
        total_size,
        file_count: manifest.file_manifest_list.len(),
        chunk_count: manifest.chunk_data_list.len(),
        suggested_base_url: final_base_url,
        matched_game_title,
    })
}

#[tauri::command]
pub async fn manifest_inspect(
    manifest_path_or_url: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ManifestInfo> {
    let trimmed = manifest_path_or_url.trim();
    if trimmed.is_empty() {
        return Err(AppError::Manifest(
            "Manifest path or URL is required".to_string(),
        ));
    }
    inspect_manifest(&state, trimmed, false).await
}

/// Refreshes a manifest at startup and keeps it on disk.
///
/// The launcher calls this on every launch for the featured build: it downloads
/// the manifest if the network allows, refreshes the local copy, and the INSTALL
/// click then loads that file — no second round-trip, and a manifest the sponsor
/// republished is actually used. Offline launches fall back to the cached copy.
#[tauri::command]
pub async fn manifest_prefetch(
    url: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ManifestInfo> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(AppError::Manifest("Manifest URL is required".to_string()));
    }
    inspect_manifest(&state, trimmed, true).await
}

/// Cheap probe: does this CDN base actually serve the given chunk path?
async fn chunk_base_responds(base: &str, chunk_path: &str, auth_token: Option<&str>) -> bool {
    let url = format!("{}/{}", base.trim_end_matches('/'), chunk_path);
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    else {
        return false;
    };

    let mut request = client
        .get(&url)
        .header(reqwest::header::RANGE, "bytes=0-15");

    // Only Epic's own CDN may see the account token
    if let Some(token) = auth_token {
        if url.contains("epicgames") {
            request = request.header(reqwest::header::AUTHORIZATION, format!("bearer {}", token));
        }
    }

    matches!(request.send().await, Ok(resp) if resp.status().is_success())
}

#[tauri::command]
pub async fn game_install_custom_manifest(
    request: CustomInstallRequest,
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let path_or_url = request.manifest_path_or_url.trim();
    if path_or_url.is_empty() {
        return Err(AppError::Manifest(
            "Manifest path or URL is required".to_string(),
        ));
    }

    // 1. Fetch or read manifest bytes (the cached copy wins, so an install
    //    started from a previously prefetched manifest needs no API call)
    let (manifest_bytes, url_base) =
        if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            let (bytes, from_cache) = manifest_bytes_for(&state, path_or_url).await?;
            info!(
                "Installing from manifest {} ({})",
                path_or_url,
                if from_cache {
                    "cached copy"
                } else {
                    "freshly downloaded"
                }
            );
            let base = path_or_url.rfind('/').map(|i| path_or_url[..i].to_string());
            (bytes, base)
        } else {
            let bytes = tokio::fs::read(path_or_url).await.map_err(|e| {
                AppError::Io(format!(
                    "Failed to read manifest file from '{}': {}",
                    path_or_url, e
                ))
            })?;
            (bytes, None)
        };

    // 2. Parse manifest binary
    let manifest = Manifest::parse(&manifest_bytes)?;

    // 3. Resolve app_name
    let app_name = request
        .app_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if !manifest.meta.app_name.is_empty() {
                Some(manifest.meta.app_name.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            AppError::Manifest(
                "Could not determine App Name from manifest. Please provide an App Name."
                    .to_string(),
            )
        })?;

    // 4. Determine title / settings (needed to look up the CDN below)
    let settings = state.storage.get_settings()?;
    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            state
                .storage
                .get_cached_catalog_item(&app_name)
                .ok()
                .flatten()
                .map(|item| item.title)
        })
        .unwrap_or_else(|| app_name.clone());

    // 5. Resolve the chunk CDN.
    //
    // Mirrors frequently host only the `.manifest` file while the actual game
    // data still lives on Epic's servers, so gather candidates and keep the
    // first one that really serves this build's chunks.
    let epic_token = state.api_client.get_session().await.map(|s| s.access_token);
    let explicit_base = request
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // Names this build can be found under on Epic: the manifest's own codename
    // first (e.g. "Jackal"), then the requested name and the display title.
    let mut aliases: Vec<String> = Vec::new();
    for candidate in [
        manifest.meta.app_name.as_str(),
        app_name.as_str(),
        title.as_str(),
    ] {
        let candidate = candidate.trim();
        if !candidate.is_empty() && !aliases.iter().any(|a| a.eq_ignore_ascii_case(candidate)) {
            aliases.push(candidate.to_string());
        }
    }

    let base_url = pick_chunk_base(
        &state,
        &manifest,
        &aliases,
        url_base.as_deref(),
        explicit_base.as_deref(),
        epic_token.as_deref(),
    )
    .await
    .or(url_base)
    .ok_or_else(|| {
        AppError::Manifest(
            "Could not determine CDN Base URL for downloading chunks. Please specify the Base URL."
                .to_string(),
        )
    })?;

    // 6. Determine target install directory. A library root or someone else's
    //    folder gets the game's own subfolder, so a build never unpacks into it.
    let target_dir = install_target(&state, &app_name, request.install_dir.as_deref(), &title);

    let executable = if !manifest.meta.launch_exe.is_empty() {
        manifest.meta.launch_exe.clone()
    } else {
        format!("{}.exe", app_name)
    };

    let total_size: u64 = manifest
        .file_manifest_list
        .iter()
        .map(|f| f.file_size)
        .sum();

    // 7. Save record to installed_games SQLite table
    let build_version = if !manifest.meta.build_version.is_empty() {
        manifest.meta.build_version.clone()
    } else {
        "custom".to_string()
    };

    let installed = InstalledGame {
        app_name: app_name.clone(),
        title: title.clone(),
        version: build_version,
        install_path: target_dir.to_string_lossy().to_string(),
        executable,
        launch_parameters: manifest.meta.launch_command.clone(),
        install_size: total_size,
        can_run_offline: true,
        requires_ot: false,
        platform: settings.platform.clone(),
        is_dlc: false,
        base_urls: vec![base_url.clone()],
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    // 8. Start chunk download pipeline (record is saved to SQLite upon 100% completion)
    state
        .downloader
        .clone()
        .start_download(
            app_handle,
            app_name,
            title,
            manifest,
            manifest_bytes,
            base_url,
            epic_token,
            target_dir,
            settings.max_workers,
            Some(installed),
            state.storage.clone(),
        )
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn game_cancel_download(
    app_name: String,
    state: State<'_, Arc<AppState>>,
    app_handle: AppHandle,
) -> Result<()> {
    if let Some(steam_id_str) = app_name.strip_prefix("steam_") {
        if let Ok(app_id) = steam_id_str.parse::<u32>() {
            let _ = state.steam_downloader.cancel_download(app_id, &app_handle).await;
            return Ok(());
        }
    }

    // 1. Stop the active download. The pipeline itself removes a fresh
    //    install's partial folder; an existing install is never touched here —
    //    cancelling an update (or dismissing a finished entry) used to delete
    //    the whole working game.
    let _ = state.downloader.cancel_download(&app_name).await;

    // 2. Emit progress as cancelled to frontend
    let p = DownloadProgress {
        app_name: app_name.clone(),
        title: app_name.clone(),
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

    info!("Cancelled and cleaned up download for {}", app_name);
    Ok(())
}

#[tauri::command]
pub async fn game_pause_download(app_name: String, state: State<'_, Arc<AppState>>) -> Result<()> {
    state.downloader.pause_download(&app_name).await
}

#[tauri::command]
pub async fn game_resume_download(app_name: String, state: State<'_, Arc<AppState>>) -> Result<()> {
    state.downloader.resume_download(&app_name).await
}

#[tauri::command]
pub async fn game_get_download_progress(
    app_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<DownloadProgress>> {
    Ok(state.downloader.get_download_progress(&app_name).await)
}

#[tauri::command]
pub async fn game_get_all_downloads(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<DownloadProgress>> {
    Ok(state.downloader.get_all_active().await)
}

#[tauri::command]
pub async fn game_verify(
    app_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<VerificationResult> {
    let installed = state
        .storage
        .get_installed_game(&app_name)?
        .ok_or_else(|| AppError::Verification(format!("Game {} is not installed", app_name)))?;

    let settings = state.storage.get_settings()?;
    let games = catalog_get_games(false, state.clone()).await?;
    let game = games
        .into_iter()
        .find(|g| g.app_name == app_name)
        .ok_or_else(|| AppError::Verification("Game not found in library".to_string()))?;

    let manifest_resp = state
        .api_client
        .get_manifest_response(
            &game.namespace,
            &game.catalog_item_id,
            &game.app_name,
            &settings.platform,
        )
        .await?;

    let first_elem = manifest_resp
        .elements
        .first()
        .ok_or_else(|| AppError::Manifest("No manifest element returned by API".to_string()))?;

    let (manifest_bytes, _base_urls) = state
        .api_client
        .download_manifest_from_element(first_elem)
        .await?;
    let manifest = Manifest::parse(&manifest_bytes)?;

    Installer::verify_installed_game(&installed, &manifest)
}

#[tauri::command]
pub async fn game_uninstall(app_name: String, state: State<'_, Arc<AppState>>) -> Result<()> {
    let installed = state
        .storage
        .get_installed_game(&app_name)?
        .ok_or_else(|| AppError::Install(format!("Game {} is not installed", app_name)))?;

    // Library roots and every other game's folder are off limits for a
    // whole-folder delete (an install that landed in a shared folder).
    let mut protected: Vec<PathBuf> = state
        .storage
        .get_all_installed_games()?
        .into_iter()
        .filter(|g| g.app_name != app_name && !g.install_path.trim().is_empty())
        .map(|g| PathBuf::from(g.install_path))
        .collect();
    if let Ok(settings) = state.storage.get_settings() {
        protected.push(PathBuf::from(settings.default_install_dir));
    }

    let manifest = crate::manifest_cache::read_installed(&app_name)
        .and_then(|bytes| Manifest::parse(&bytes).ok());

    Installer::uninstall_game(&installed, manifest.as_ref(), &protected)?;
    state.storage.delete_installed_game(&app_name)?;
    crate::manifest_cache::remove_installed(&app_name);
    Ok(())
}

// ==========================================
// Game Launching Commands
// ==========================================

#[tauri::command]
pub async fn game_launch(
    app_name: String,
    offline: bool,
    extra_args: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<u32> {
    // If this is a Steam game (app_name format: "steam_<appid>")
    if let Some(steam_id_str) = app_name.strip_prefix("steam_") {
        if let Ok(app_id) = steam_id_str.parse::<u32>() {
            // Remembered so a protocol launch can still be tracked by folder
            let mut tracking_dir: Option<std::path::PathBuf> = None;
            let mut tracking_exe: Option<std::path::PathBuf> = None;
            let mut tracking_title = app_name.clone();

            // Check if installed in database with executable
            if let Ok(Some(installed)) = state.storage.get_installed_game(&app_name) {
                let install_dir = std::path::PathBuf::from(&installed.install_path);
                tracking_dir = Some(install_dir.clone());
                tracking_title = installed.title.clone();

                let mut exe_path = if installed.executable.is_empty() {
                    std::path::PathBuf::new()
                } else {
                    install_dir.join(&installed.executable)
                };

                // Re-discover the binary if the stored name is missing or stale
                if exe_path.as_os_str().is_empty() || !exe_path.exists() {
                    let discovered =
                        crate::steam::downloader::pick_launch_exe(&install_dir, &installed.title);
                    exe_path = if discovered.is_empty() {
                        std::path::PathBuf::new()
                    } else {
                        install_dir.join(discovered)
                    };
                }

                // Make sure Steamworks knows which title this copy is, so the game
                // starts instead of asking the Steam client to install it again.
                crate::steam::downloader::ensure_steam_appid_file(&install_dir, app_id);

                if !exe_path.as_os_str().is_empty() && exe_path.exists() {
                    tracking_exe = Some(exe_path.clone());
                    info!("Launching standalone Steam binary directly: {:?}", exe_path);
                    let mut cmd = std::process::Command::new(&exe_path);
                    cmd.current_dir(&install_dir);
                    match cmd.spawn() {
                        Ok(child) => {
                            let pid = child.id();
                            register_running_game(&state, &app_name, &installed.title, pid).await;
                            return Ok(pid);
                        }
                        Err(e) => {
                            warn!("Failed direct spawn of {:?}: {}, trying Steam protocol...", exe_path, e);
                        }
                    }
                }
            }

            crate::steam::launch_steam_game(app_id)
                .map_err(|e| AppError::Launch(e))?;

            // The Steam client starts the game, so there is no child handle to
            // watch. Locate the game's install folder and poll for a process
            // running from it, otherwise "Running"/Stop never lights up for
            // titles Steam installed itself.
            let install_dir = tracking_dir.or_else(|| {
                crate::steam::scan_installed_steam_games()
                    .into_iter()
                    .find(|g| g.app_name == app_name)
                    .and_then(|g| g.install_path)
                    .map(std::path::PathBuf::from)
            });

            if let Some(dir) = install_dir {
                spawn_protocol_launch_watcher(
                    Arc::clone(&state),
                    app_name.clone(),
                    tracking_title.clone(),
                    dir,
                    tracking_exe,
                );
            }

            return Ok(app_id);
        }
    }

    let installed = state
        .storage
        .get_installed_game(&app_name)?
        .ok_or_else(|| AppError::Launch(format!("Game {} is not installed", app_name)))?;

    let games = catalog_get_games(false, state.clone()).await?;
    let namespace = games
        .into_iter()
        .find(|g| g.app_name == app_name)
        .map(|g| g.namespace)
        .unwrap_or_else(|| "default".to_string());

    let session = state.api_client.get_session().await;
    let settings = state.storage.get_settings()?;

    let pid = Launcher::launch_game(
        &installed,
        &namespace,
        session.as_ref(),
        &state.api_client,
        offline,
        &extra_args,
        Some(&settings.language),
    )
    .await?;

    register_running_game(&state, &app_name, &installed.title, pid).await;
    Ok(pid)
}

/// Records a launched game so the UI can show "Running" and offer Stop.
async fn register_running_game(
    state: &Arc<AppState>,
    app_name: &str,
    title: &str,
    pid: u32,
) {
    let entry = crate::models::RunningGame {
        app_name: app_name.to_string(),
        title: title.to_string(),
        pid,
        started_at: chrono::Utc::now().to_rfc3339(),
    };

    state
        .running_games
        .lock()
        .await
        .insert(app_name.to_string(), entry);

    info!("Tracking running game {} (pid {})", app_name, pid);
}

/// Finds a process whose executable lives inside `dir`.
fn find_game_process(dir: &std::path::Path) -> Option<u32> {
    crate::winproc::find_process_in_dir(dir)
}

/// Finds the running process of an installed game.
///
/// The recorded executable is the strongest signal; when it is unknown, the
/// first binary running from the install folder is used instead.
fn find_installed_game_process(game: &InstalledGame) -> Option<u32> {
    let dir = std::path::PathBuf::from(&game.install_path);
    if dir.as_os_str().is_empty() {
        return None;
    }

    if !game.executable.is_empty() {
        if let Some(pid) = crate::winproc::find_process_by_exe(&dir.join(&game.executable)) {
            return Some(pid);
        }
    }

    crate::winproc::find_process_in_dir(&dir)
}

/// Watches for the game process after a `steam://rungameid` launch.
///
/// The Steam client owns the process in that case, so the only way to report
/// "Running" (and allow Stop) is to spot the game's binary. When the executable
/// is known only processes with that name are opened; otherwise the install
/// folder is scanned.
fn spawn_protocol_launch_watcher(
    state: Arc<AppState>,
    app_name: String,
    title: String,
    install_dir: std::path::PathBuf,
    exe_hint: Option<std::path::PathBuf>,
) {
    tokio::spawn(async move {
        // Steam needs a few seconds before the game binary appears
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

            if state.running_games.lock().await.contains_key(&app_name) {
                return;
            }

            let found = match &exe_hint {
                Some(exe) => crate::winproc::find_process_by_exe(exe),
                None => find_game_process(&install_dir),
            };

            if let Some(pid) = found {
                info!(
                    "Detected protocol-launched game {} at pid {}",
                    app_name, pid
                );
                register_running_game(&state, &app_name, &title, pid).await;
                return;
            }
        }
    });
}

/// Kills a process and its children (games spawn helper/launcher processes).
fn kill_process_tree(pid: u32) -> Result<()> {
    if crate::winproc::kill_tree(pid) {
        return Ok(());
    }

    Err(AppError::Launch(format!(
        "Could not stop process {} — it may have already exited",
        pid
    )))
}

/// Games the launcher started that are still alive.
///
/// Polled by the UI every few seconds, so it only checks the PIDs it already
/// knows about — a handle wait per game, with no process enumeration and no
/// memory/CPU counters. Discovering games that were started elsewhere is a
/// separate, much rarer call (`game_adopt_running`).
#[tauri::command]
pub async fn game_get_running(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::models::RunningGame>> {
    let mut map = state.running_games.lock().await;

    // Drop entries whose process exited on its own
    map.retain(|_, game| crate::winproc::is_running(game.pid));

    let mut running: Vec<crate::models::RunningGame> = map.values().cloned().collect();
    running.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    Ok(running)
}

/// Picks up installed games that are already running without this launcher:
/// started earlier, by Steam, or from the Epic client.
///
/// This walks the process list, so the UI calls it on startup and then at a
/// slow interval instead of on every liveness poll.
#[tauri::command]
pub async fn game_adopt_running(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::models::RunningGame>> {
    let installed_games = state.storage.get_all_installed_games().unwrap_or_default();
    let mut map = state.running_games.lock().await;

    for installed in installed_games {
        if map.contains_key(&installed.app_name) {
            continue;
        }
        if let Some(pid) = find_installed_game_process(&installed) {
            info!(
                "Adopted already-running game {} (pid {})",
                installed.app_name, pid
            );
            map.insert(
                installed.app_name.clone(),
                crate::models::RunningGame {
                    app_name: installed.app_name.clone(),
                    title: installed.title.clone(),
                    pid,
                    started_at: chrono::Utc::now().to_rfc3339(),
                },
            );
        }
    }

    let mut running: Vec<crate::models::RunningGame> = map.values().cloned().collect();
    running.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    Ok(running)
}

/// Stops a game started from the launcher.
#[tauri::command]
pub async fn game_stop(app_name: String, state: State<'_, Arc<AppState>>) -> Result<()> {
    let tracked = state.running_games.lock().await.get(&app_name).cloned();

    // Fall back to the install folder: the game may have been running since
    // before this launcher session started.
    let game = match tracked {
        Some(game) => game,
        None => {
            let installed = state
                .storage
                .get_installed_game(&app_name)?
                .ok_or_else(|| AppError::Launch(format!("{} is not running", app_name)))?;
            let pid = find_installed_game_process(&installed)
                .ok_or_else(|| AppError::Launch(format!("{} is not running", installed.title)))?;
            crate::models::RunningGame {
                app_name: installed.app_name.clone(),
                title: installed.title.clone(),
                pid,
                started_at: chrono::Utc::now().to_rfc3339(),
            }
        }
    };

    match kill_process_tree(game.pid) {
        Ok(()) => {
            state.running_games.lock().await.remove(&app_name);
            info!("Stopped {} (pid {})", game.app_name, game.pid);
            Ok(())
        }
        Err(e) => {
            // Keep the entry so the Stop button stays available for a retry
            let pid = game.pid;
            state.running_games.lock().await.insert(app_name.clone(), game);
            warn!("Could not stop {} (pid {}): {}", app_name, pid, e);
            Err(e)
        }
    }
}

/// Steam games found by scanning this PC's Steam client (library folders, app
/// manifests, `userdata/*/config/localconfig.vdf`).
///
/// Opt-in via `steam_library_scan`: an unsigned app reading Steam's `userdata`
/// unprompted is what Steam stealers do, and antivirus behaviour engines flag
/// and suspend it.
async fn local_steam_library(
    state: &AppState,
    settings: &AppSettings,
    force_refresh: bool,
) -> Vec<GameItem> {
    if !settings.steam_library_scan {
        return Vec::new();
    }
    crate::steam::resolve_all_steam_library_games(&state.storage, force_refresh).await
}

#[tauri::command]
pub async fn steam_get_installed_games(state: State<'_, Arc<AppState>>) -> Result<Vec<GameItem>> {
    let settings = state.storage.get_settings()?;
    Ok(local_steam_library(&state, &settings, false).await)
}

/// Reports the extra packages SteamCMD installed next to a game
/// (Steamworks Common Redistributables, shared runtimes, …).
#[tauri::command]
pub async fn steam_extra_content(
    app_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::models::SteamExtraContent>> {
    let Some(installed) = state.storage.get_installed_game(&app_name)? else {
        return Ok(Vec::new());
    };
    if installed.platform != "steam" {
        return Ok(Vec::new());
    }

    let main_app_id = app_name
        .strip_prefix("steam_")
        .and_then(|id| id.parse::<u32>().ok())
        .unwrap_or(0);

    Ok(crate::steam::scan_extra_content(
        std::path::Path::new(&installed.install_path),
        main_app_id,
    ))
}

#[tauri::command]
pub async fn steam_launch_game(app_id: u32) -> Result<()> {
    crate::steam::launch_steam_game(app_id).map_err(|e| AppError::Launch(e))
}

#[tauri::command]
pub async fn steam_search_games(query: String) -> Result<Vec<crate::steam::SteamStoreItem>> {
    crate::steam::search_steam_store(&query)
        .await
        .map_err(AppError::General)
}

#[tauri::command]
pub async fn steam_get_details(app_id: u32) -> Result<Option<crate::steam::SteamStoreDetails>> {
    crate::steam::get_steam_app_details(app_id)
        .await
        .map_err(AppError::General)
}

#[tauri::command]
pub async fn steam_install_custom(
    app_handle: AppHandle,
    app_id: u32,
    title: Option<String>,
    install_dir: Option<String>,
    depot_id: Option<u32>,
    manifest_id: Option<u64>,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let final_title = if let Some(t) = title {
        t
    } else {
        match crate::steam::get_steam_app_details(app_id).await {
            Ok(Some(details)) => details.name,
            _ => format!("Steam App {}", app_id),
        }
    };

    // The Direct Downloader pre-fills the bare library root; installing there
    // made a later uninstall wipe every game in it.
    let target_dir = install_target(
        &state,
        &format!("steam_{}", app_id),
        install_dir.as_deref(),
        &final_title,
    );

    info!(
        "Initiating native Steam in-launcher download for AppID {} ('{}')",
        app_id, final_title
    );

    // Reuse the in-memory Steam session if the user has signed in
    let credentials = state.steam_credentials.lock().await.clone();

    state
        .steam_downloader
        .clone()
        .start_download(
            app_handle,
            app_id,
            final_title,
            target_dir,
            credentials,
            depot_id,
            manifest_id,
            state.storage.clone(),
        )
        .await
        .map_err(AppError::Install)?;

    Ok(())
}

// ==========================================
// Cloud Save Commands
// ==========================================

#[tauri::command]
pub async fn cloud_save_get_status(
    app_name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<CloudSaveState> {
    let installed = state.storage.get_installed_game(&app_name)?;
    let session = state.api_client.get_session().await;

    // Pattern for cloud save location
    let pattern = "{appdata}/{app_name}/Saved/SaveGames";

    CloudSaveManager::get_save_status(
        &app_name,
        Some(pattern),
        installed.as_ref(),
        session.as_ref(),
        &state.api_client,
    )
    .await
}

#[tauri::command]
pub async fn cloud_save_sync(app_name: String, state: State<'_, Arc<AppState>>) -> Result<String> {
    let status = cloud_save_get_status(app_name, state).await?;
    Ok(status.status_message)
}

// ==========================================
// Settings & Diagnostics Commands
// ==========================================

#[tauri::command]
pub async fn settings_get(state: State<'_, Arc<AppState>>) -> Result<AppSettings> {
    state.storage.get_settings()
}

#[tauri::command]
pub async fn settings_save(settings: AppSettings, state: State<'_, Arc<AppState>>) -> Result<()> {
    state.storage.save_settings(&settings)
}

#[tauri::command]
pub async fn diagnostics_get(state: State<'_, Arc<AppState>>) -> Result<SystemDiagnostics> {
    // Only memory totals are needed here — no process enumeration (sysinfo's
    // new_all() would read every process' memory, which is pointless for this).
    let mut sys = System::new();
    sys.refresh_memory();

    let disks = Disks::new_with_refreshed_list();
    let free_disk_bytes: u64 = disks.iter().map(|d| d.available_space()).sum();
    let free_disk_gb = (free_disk_bytes as f64) / (1024.0 * 1024.0 * 1024.0);

    let settings = state.storage.get_settings()?;

    Ok(SystemDiagnostics {
        os_name: System::name().unwrap_or_else(|| "Windows".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "10 / 11".to_string()),
        arch: std::env::consts::ARCH.to_string(),
        total_memory_mb: sys.total_memory() / (1024 * 1024),
        free_memory_mb: sys.available_memory() / (1024 * 1024),
        default_install_dir: settings.default_install_dir,
        free_disk_space_gb: (free_disk_gb * 10.0).round() / 10.0,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        rust_version: "1.97.1".to_string(),
    })
}

#[tauri::command]
pub async fn open_path(path: String) -> Result<()> {
    open::that(path).map_err(|e| AppError::Io(e.to_string()))
}

#[cfg(test)]
mod game_process_tests {
    use super::*;

    fn installed(app_name: &str, dir: &str, exe: &str) -> InstalledGame {
        InstalledGame {
            app_name: app_name.to_string(),
            title: app_name.to_string(),
            version: "1.0".to_string(),
            install_path: dir.to_string(),
            executable: exe.to_string(),
            launch_parameters: String::new(),
            install_size: 0,
            can_run_offline: true,
            requires_ot: false,
            platform: "epic".to_string(),
            is_dlc: false,
            base_urls: Vec::new(),
            installed_at: None,
        }
    }

    #[test]
    fn install_paths_compare_case_and_separator_insensitively() {
        assert!(crate::installer::same_dir(
            std::path::Path::new("D:\\Games\\Dauntless"),
            std::path::Path::new("d:\\games\\dauntless\\")
        ));
        assert!(!crate::installer::same_dir(
            std::path::Path::new("D:\\Games\\Dauntless"),
            std::path::Path::new("D:\\Games\\Other")
        ));
    }

    #[test]
    fn games_without_an_install_path_are_never_matched() {
        let game = installed("GameD", "", "gamed.exe");
        assert_eq!(find_installed_game_process(&game), None);
    }
}

#[cfg(test)]
mod library_tests {
    use super::*;

    fn installed_with_platform(app_name: &str, platform: &str) -> InstalledGame {
        InstalledGame {
            app_name: app_name.to_string(),
            title: app_name.to_string(),
            version: "1.0".to_string(),
            install_path: "C:\\Games\\game".to_string(),
            executable: "game.exe".to_string(),
            launch_parameters: String::new(),
            install_size: 0,
            can_run_offline: true,
            requires_ot: false,
            platform: platform.to_string(),
            is_dlc: false,
            base_urls: Vec::new(),
            installed_at: None,
        }
    }

    /// Steam rows belong to the Steam resolver; emitting them here as well would
    /// show each Steam title twice, once as a bogus "Epic" card.
    #[test]
    fn local_items_skip_steam_titles() {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "doki".to_string(),
            installed_with_platform("doki", "Windows"),
        );
        map.insert(
            "steam_367520".to_string(),
            installed_with_platform("steam_367520", "steam"),
        );
        map.insert(
            "steam_1091500".to_string(),
            installed_with_platform("steam_1091500", "Windows"),
        );

        let items = installed_game_items(&map);

        assert_eq!(items.len(), 1, "only the Epic install should survive");
        assert_eq!(items[0].app_name, "doki");
        assert_eq!(items[0].platform, "epic");
        assert!(items[0].is_installed);
    }
}

#[cfg(all(test, target_os = "windows"))]
mod running_process_tests {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    #[test]
    fn a_spawned_process_is_tracked_and_can_be_stopped() {
        let mut child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .expect("spawn a throwaway process");

        let pid = child.id();
        assert!(
            crate::winproc::is_running(pid),
            "process should be alive after spawn"
        );

        super::kill_process_tree(pid).expect("stop the process");
        std::thread::sleep(std::time::Duration::from_millis(600));

        assert!(
            !crate::winproc::is_running(pid),
            "process should be gone after stop"
        );
        let _ = child.wait();
    }

    #[test]
    fn stopping_an_unknown_pid_reports_an_error() {
        assert!(super::kill_process_tree(999_999).is_err());
    }
}
