use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OAuth session information returned by Epic Games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub expires_in: i64,
    pub expires_at: Option<String>,
    pub token_type: String,
    pub refresh_token: Option<String>,
    pub refresh_expires: Option<i64>,
    pub refresh_expires_at: Option<String>,
    pub account_id: String,
    pub client_id: Option<String>,
    pub internal_client: Option<bool>,
    pub client_service: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub app: Option<String>,
    pub in_sub: Option<bool>,
}

/// Simplified user info exposed to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub account_id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub country: Option<String>,
    pub logged_in: bool,
}

/// Game asset retrieved from Epic assets API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAsset {
    #[serde(rename = "appName")]
    pub app_name: String,
    #[serde(rename = "catalogItemId")]
    pub catalog_item_id: String,
    pub namespace: String,
    #[serde(rename = "assetId")]
    pub asset_id: Option<String>,
    #[serde(rename = "buildVersion")]
    pub build_version: String,
    pub label_name: Option<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Catalog item image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyImage {
    #[serde(rename = "type")]
    pub image_type: String,
    pub url: String,
    pub md5: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Custom attribute entry on catalog metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAttribute {
    #[serde(rename = "type")]
    pub attr_type: Option<String>,
    pub value: String,
}

/// Detailed catalog item metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMetadata {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "keyImages")]
    pub key_images: Option<Vec<KeyImage>>,
    #[serde(rename = "customAttributes")]
    pub custom_attributes: Option<HashMap<String, CustomAttribute>>,
    #[serde(rename = "developerDisplayName")]
    pub developer: Option<String>,
    #[serde(rename = "publisherDisplayName")]
    pub publisher: Option<String>,
    #[serde(rename = "releaseInfo")]
    pub release_info: Option<Vec<ReleaseInfo>>,
    #[serde(rename = "mainGameItem")]
    pub main_game_item: Option<Box<GameMetadata>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub id: Option<String>,
    #[serde(rename = "appId")]
    pub app_id: Option<String>,
    pub platform: Option<Vec<String>>,
}

/// Frontend Game entity combining catalog metadata and local installation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameItem {
    pub app_name: String,
    pub title: String,
    pub namespace: String,
    pub catalog_item_id: String,
    pub description: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub logo_url: Option<String>,
    pub is_installed: bool,
    pub is_dlc: bool,
    pub build_version: Option<String>,
    pub install_path: Option<String>,
    pub install_size: Option<u64>,
    pub executable: Option<String>,
    pub can_run_offline: bool,
    pub supports_cloud_saves: bool,
    #[serde(default = "default_platform")]
    pub platform: String,
    #[serde(default)]
    pub steam_app_id: Option<u32>,
}

fn default_platform() -> String {
    "epic".to_string()
}

/// Locally installed game record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledGame {
    pub app_name: String,
    pub title: String,
    pub version: String,
    pub install_path: String,
    pub executable: String,
    pub launch_parameters: String,
    pub install_size: u64,
    pub can_run_offline: bool,
    pub requires_ot: bool,
    pub platform: String,
    pub is_dlc: bool,
    pub base_urls: Vec<String>,
    pub installed_at: Option<String>,
}

/// Download engine task and progress status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DownloadStatus {
    Queued,
    Analyzing,
    Downloading,
    Paused,
    Verifying,
    Installing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub app_name: String,
    pub title: String,
    pub status: DownloadStatus,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub total_chunks: u64,
    pub downloaded_chunks: u64,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: u64,
    pub progress_percent: f32,
    pub current_file: Option<String>,
    pub error_message: Option<String>,
}

/// Steam authentication status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamAuthStatus {
    pub logged_in: bool,
    pub account_name: Option<String>,
    pub steam_id: Option<String>,
}

/// Credentials handed to the headless Steam download engine.
///
/// Kept in process memory only: never serialized, never written to SQLite.
#[derive(Debug, Clone)]
pub struct SteamCredentials {
    pub username: String,
    pub password: String,
    pub guard_code: Option<String>,
}

/// Non-sensitive view of the headless engine's credential state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamCredentialStatus {
    pub has_credentials: bool,
    pub username: Option<String>,
    /// True when a DPAPI-protected password is stored in the local database
    #[serde(default)]
    pub remembered: bool,
    /// True when SteamCMD already holds a signed-in session for this account,
    /// which is what actually authorises downloads.
    #[serde(default)]
    pub engine_session_cached: bool,
}

/// A package SteamCMD installs alongside a game (redistributables, shared
/// runtimes, dedicated-server pieces…). These live in the same `steamapps`
/// folder as the game and were previously invisible in the launcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamExtraContent {
    pub app_id: u32,
    pub name: String,
    pub size_bytes: u64,
    /// False when SteamCMD recorded the dependency but has not fetched it yet
    pub installed: bool,
}

/// A game process the launcher started and is still watching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningGame {
    pub app_name: String,
    pub title: String,
    pub pid: u32,
    pub started_at: String,
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub default_install_dir: String,
    pub max_workers: usize,
    pub throttle_mbps: u32,
    pub language: String,
    pub auto_update: bool,
    pub skip_update_check: bool,
    pub platform: String,
    #[serde(default)]
    pub steam_account: Option<String>,
    #[serde(default)]
    pub steam_id: Option<String>,
    #[serde(default)]
    pub steam_token: Option<String>,
    /// DPAPI-encrypted Steam password (base64). Never stored in plaintext.
    #[serde(default)]
    pub steam_password_enc: Option<String>,
    /// Opt-in: read this PC's Steam client files (library folders, app
    /// manifests, `userdata`) to list installed and owned Steam games. Off by
    /// default.
    #[serde(default)]
    pub steam_library_scan: bool,
}

/// Automatically detects a suitable root gaming drive (D:\Games, E:\Games, or C:\Games)
pub fn detect_default_install_dir() -> String {
    for drive in &["D", "E", "C"] {
        let root = format!("{}:\\", drive);
        if std::path::Path::new(&root).exists() {
            return format!("{}:\\Games", drive);
        }
    }
    "C:\\Games".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_install_dir: detect_default_install_dir(),
            max_workers: 8,
            throttle_mbps: 0,
            language: "en".to_string(),
            auto_update: true,
            skip_update_check: false,
            platform: "Windows".to_string(),
            steam_account: None,
            steam_id: None,
            steam_token: None,
            steam_password_enc: None,
            steam_library_scan: false,
        }
    }
}

/// System diagnostics info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDiagnostics {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub total_memory_mb: u64,
    pub free_memory_mb: u64,
    pub default_install_dir: String,
    pub free_disk_space_gb: f64,
    pub app_version: String,
    pub rust_version: String,
}

/// Verification result for installed files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub app_name: String,
    pub total_files: usize,
    pub valid_files: usize,
    pub corrupt_files: Vec<String>,
    pub missing_files: Vec<String>,
    pub is_valid: bool,
}

/// Cloud save sync state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSaveState {
    pub app_name: String,
    pub local_path: Option<String>,
    pub local_files_count: usize,
    pub local_updated_at: Option<String>,
    pub cloud_files_count: usize,
    pub cloud_updated_at: Option<String>,
    pub in_sync: bool,
    pub status_message: String,
}

/// Detailed manifest inspection info for UI preview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestInfo {
    pub app_name: String,
    pub build_version: String,
    pub launch_exe: String,
    pub launch_command: String,
    pub total_size: u64,
    pub file_count: usize,
    pub chunk_count: usize,
    pub suggested_base_url: Option<String>,
    pub matched_game_title: Option<String>,
}

/// Parameters for custom manifest installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomInstallRequest {
    pub manifest_path_or_url: String,
    pub app_name: Option<String>,
    pub title: Option<String>,
    pub base_url: Option<String>,
    pub install_dir: Option<String>,
}
