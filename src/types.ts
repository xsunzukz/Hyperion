export type ViewMode = 'grid' | 'list';

export type NavigationTab = 
  | 'home' 
  | 'library' 
  | 'downloads' 
  | 'cloud-saves' 
  | 'settings' 
  | 'about';

export interface UserProfile {
  account_id: string;
  display_name: string;
  email?: string;
  country?: string;
  logged_in: boolean;
}

export type GamePlatform = 'epic' | 'steam';

export interface GameItem {
  app_name: string;
  title: string;
  namespace: string;
  catalog_item_id: string;
  description?: string;
  developer?: string;
  publisher?: string;
  icon_url?: string;
  banner_url?: string;
  logo_url?: string;
  is_installed: boolean;
  is_dlc: boolean;
  build_version?: string;
  install_path?: string;
  install_size?: number;
  executable?: string;
  can_run_offline: boolean;
  supports_cloud_saves: boolean;
  platform?: GamePlatform;
  steam_app_id?: number;
}

export interface InstalledGame {
  app_name: string;
  title: string;
  version: string;
  install_path: string;
  executable: string;
  launch_parameters: string;
  install_size: number;
  can_run_offline: boolean;
  requires_ot: boolean;
  platform: string;
  is_dlc: boolean;
  base_urls: string[];
  installed_at?: string;
}

export type DownloadStatus = 
  | 'Queued'
  | 'Analyzing'
  | 'Downloading'
  | 'Paused'
  | 'Verifying'
  | 'Installing'
  | 'Completed'
  | 'Failed'
  | 'Cancelled';

export interface DownloadProgress {
  app_name: string;
  title: string;
  status: DownloadStatus;
  total_bytes: number;
  downloaded_bytes: number;
  total_chunks: number;
  downloaded_chunks: number;
  speed_bytes_per_sec: number;
  eta_seconds: number;
  progress_percent: number;
  current_file?: string;
  error_message?: string;
}

export interface VerificationResult {
  app_name: string;
  total_files: number;
  valid_files: number;
  corrupt_files: string[];
  missing_files: string[];
  is_valid: boolean;
}

export interface CloudSaveState {
  app_name: string;
  local_path?: string;
  local_files_count: number;
  local_updated_at?: string;
  cloud_files_count: number;
  cloud_updated_at?: string;
  in_sync: boolean;
  status_message: string;
}

export interface SteamAuthStatus {
  logged_in: boolean;
  account_name?: string;
  steam_id?: string;
}

/** In-memory session handed to the headless Steam download engine */
export interface SteamCredentialStatus {
  has_credentials: boolean;
  username?: string;
  remembered?: boolean;
  /** SteamCMD already holds a signed-in session, which authorises downloads */
  engine_session_cached?: boolean;
}

/** A package SteamCMD installed alongside a game (redistributables, runtimes…) */
export interface SteamExtraContent {
  app_id: number;
  name: string;
  size_bytes: number;
  installed: boolean;
}

/** A game process the launcher started and is still watching */
export interface RunningGame {
  app_name: string;
  title: string;
  pid: number;
  started_at: string;
}

export interface AppSettings {
  default_install_dir: string;
  max_workers: number;
  throttle_mbps: number;
  language: string;
  auto_update: boolean;
  skip_update_check: boolean;
  platform: string;
  steam_account?: string;
  steam_id?: string;
  steam_token?: string;
  /** Opt-in: read this PC's Steam library files to list Steam games */
  steam_library_scan: boolean;
  /** Set by the backend once the first-launch bootstrap has finished */
  bootstrap_done: boolean;
}

/** One stage of the startup bootstrap, streamed while the splash is visible */
export interface BootstrapProgress {
  stage: string;
  label: string;
  detail?: string | null;
  /** null means "working, no meaningful percentage" */
  percent?: number | null;
  finished: boolean;
}

export interface BootstrapReport {
  epic_games: number;
  steam_games: number;
  steamcmd_ready: boolean;
  first_run: boolean;
}

export interface SystemDiagnostics {
  os_name: string;
  os_version: string;
  arch: string;
  total_memory_mb: number;
  free_memory_mb: number;
  default_install_dir: string;
  free_disk_space_gb: number;
  app_version: string;
  rust_version: string;
}

export interface ManifestInfo {
  app_name: string;
  build_version: string;
  launch_exe: string;
  launch_command: string;
  total_size: number;
  file_count: number;
  chunk_count: number;
  suggested_base_url?: string;
  matched_game_title?: string;
}

export interface CustomInstallRequest {
  manifest_path_or_url: string;
  app_name?: string;
  title?: string;
  base_url?: string;
  install_dir?: string;
}

export interface SteamStoreItem {
  id: number;
  name: string;
  tiny_image?: string;
  header_image?: string;
  capsule_image?: string;
  price?: string;
}

export interface SteamStoreDetails {
  id: number;
  name: string;
  short_description?: string;
  header_image?: string;
  capsule_image?: string;
  developers?: string[];
  publishers?: string[];
}

