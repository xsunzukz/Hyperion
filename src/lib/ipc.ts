import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import {
  AppSettings,
  BootstrapProgress,
  BootstrapReport,
  CloudSaveState,
  CustomInstallRequest,
  DownloadProgress,
  GameItem,
  ManifestInfo,
  SystemDiagnostics,
  UserProfile,
  VerificationResult,
  SteamStoreItem,
  SteamStoreDetails,
  SteamAuthStatus,
  SteamCredentialStatus,
  SteamExtraContent,
  RunningGame,
} from '../types';

export const isTauri = (): boolean => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

// ==========================================
// Mock Data for Pure Browser Development
// ==========================================
let mockUserProfile: UserProfile | null = {
  account_id: 'epic_dev_user_9921',
  display_name: 'CommanderShepard',
  email: 'commander@normandy.alliance',
  country: 'US',
  logged_in: true,
};

let mockGames: GameItem[] = [
  {
    app_name: 'Cyberpunk2077',
    title: 'Cyberpunk 2077',
    namespace: 'cdpr_cp2077',
    catalog_item_id: 'cp2077_catalog_id',
    description: 'An open-world, action-adventure RPG set in the megalopolis of Night City, where you play as a cyberpunk mercenary.',
    developer: 'CD PROJEKT RED',
    publisher: 'CD PROJEKT RED',
    icon_url: 'https://images.unsplash.com/photo-1542751371-adc38448a05e?auto=format&fit=crop&w=600&q=80',
    banner_url: 'https://images.unsplash.com/photo-1578632767115-351597cf2477?auto=format&fit=crop&w=1200&q=80',
    logo_url: undefined,
    is_installed: true,
    is_dlc: false,
    build_version: '2.12-Release',
    install_path: 'C:\\Games\\Cyberpunk2077',
    install_size: 74200000000,
    executable: 'bin\\x64\\Cyberpunk2077.exe',
    can_run_offline: true,
    supports_cloud_saves: true,
  },
  {
    app_name: 'AlanWake2',
    title: 'Alan Wake 2',
    namespace: 'remedy_aw2',
    catalog_item_id: 'aw2_catalog_id',
    description: 'A string of ritualistic murders threatens Bright Falls. Saga Anderson and Alan Wake fight darkness in two realities.',
    developer: 'Remedy Entertainment',
    publisher: 'Epic Games Publishing',
    icon_url: 'https://images.unsplash.com/photo-1518709268805-4e9042af9f23?auto=format&fit=crop&w=600&q=80',
    banner_url: 'https://images.unsplash.com/photo-1509198397868-475647b2a1e5?auto=format&fit=crop&w=1200&q=80',
    logo_url: undefined,
    is_installed: false,
    is_dlc: false,
    build_version: '1.0.16',
    install_path: undefined,
    install_size: 85000000000,
    executable: undefined,
    can_run_offline: true,
    supports_cloud_saves: true,
  },
  {
    app_name: 'DeathStranding',
    title: 'DEATH STRANDING DIRECTOR\'S CUT',
    namespace: '505games_ds',
    catalog_item_id: 'ds_catalog_id',
    description: 'From acclaimed game creator Hideo Kojima comes a genre-defying experience, now expanded in this definitive version.',
    developer: 'KOJIMA PRODUCTIONS',
    publisher: '505 Games',
    icon_url: 'https://images.unsplash.com/photo-1550745165-9bc0b252726f?auto=format&fit=crop&w=600&q=80',
    banner_url: 'https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?auto=format&fit=crop&w=1200&q=80',
    logo_url: undefined,
    is_installed: true,
    is_dlc: false,
    build_version: '1.05',
    install_path: 'C:\\Games\\DeathStranding',
    install_size: 68500000000,
    executable: 'ds.exe',
    can_run_offline: true,
    supports_cloud_saves: true,
  },
  {
    app_name: 'Hades2',
    title: 'Hades II',
    namespace: 'supergiant_hades2',
    catalog_item_id: 'hades2_catalog_id',
    description: 'Battle beyond the Underworld using dark sorcery to take on the Titan of Time in this bewitching rogue-like dungeon crawler.',
    developer: 'Supergiant Games',
    publisher: 'Supergiant Games',
    icon_url: 'https://images.unsplash.com/photo-1563089145-599997674d42?auto=format&fit=crop&w=600&q=80',
    banner_url: 'https://images.unsplash.com/photo-1579783902614-a3fb3927b675?auto=format&fit=crop&w=1200&q=80',
    logo_url: undefined,
    is_installed: false,
    is_dlc: false,
    build_version: 'EA-0.9.1',
    install_path: undefined,
    install_size: 10400000000,
    executable: undefined,
    can_run_offline: true,
    supports_cloud_saves: true,
  },
  {
    app_name: 'GrandTheftAutoV',
    title: 'Grand Theft Auto V: Premium Edition',
    namespace: 'rockstar_gtav',
    catalog_item_id: 'gtav_catalog_id',
    description: 'When a young street hustler, a retired bank robber and a terrifying psychopath find themselves entangled with the criminal underworld.',
    developer: 'Rockstar Games',
    publisher: 'Rockstar Games',
    icon_url: 'https://images.unsplash.com/photo-1511512578047-dfb367046420?auto=format&fit=crop&w=600&q=80',
    banner_url: 'https://images.unsplash.com/photo-1568605117036-5fe5e7bab0b7?auto=format&fit=crop&w=1200&q=80',
    logo_url: undefined,
    is_installed: true,
    is_dlc: false,
    build_version: 'v3095',
    install_path: 'C:\\Games\\GTA5',
    install_size: 110000000000,
    executable: 'GTA5.exe',
    can_run_offline: false,
    supports_cloud_saves: true,
  },
  {
    app_name: 'FalloutNewVegas',
    title: 'Fallout: New Vegas Ultimate Edition',
    namespace: 'bethesda_fnv',
    catalog_item_id: 'fnv_catalog_id',
    description: 'Welcome to Vegas. New Vegas. It\'s the kind of town where you dig your own grave prior to being shot in the head.',
    developer: 'Obsidian Entertainment',
    publisher: 'Bethesda Softworks',
    icon_url: 'https://images.unsplash.com/photo-1579373903781-fd5c0c30c4cd?auto=format&fit=crop&w=600&q=80',
    banner_url: 'https://images.unsplash.com/photo-1518709268805-4e9042af9f23?auto=format&fit=crop&w=1200&q=80',
    logo_url: undefined,
    is_installed: false,
    is_dlc: false,
    build_version: '1.4.0.525',
    install_path: undefined,
    install_size: 9800000000,
    executable: undefined,
    can_run_offline: true,
    supports_cloud_saves: false,
  }
];

let mockSettings: AppSettings = {
  default_install_dir: 'D:\\Games',
  max_workers: 32,
  throttle_mbps: 0,
  language: 'en',
  auto_update: true,
  skip_update_check: false,
  platform: 'Windows',
  steam_library_scan: false,
  close_to_tray: true,
  bootstrap_done: false,
};

let mockDownloads: DownloadProgress[] = [];

// Games "running" in the browser preview, so the Stop UI can be exercised
// without the Tauri backend.
let mockRunningGames: RunningGame[] = [];

// ==========================================
// Typed IPC API Methods
// ==========================================

export const authGetLoginUrl = async (): Promise<string> => {
  if (isTauri()) {
    return invoke<string>('auth_get_login_url');
  }
  return 'https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode';
};

export const authStartAutomaticLogin = async (): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('auth_start_automatic_login');
  }
  // Pure browser preview mode simulation
  return new Promise((resolve) => {
    setTimeout(() => {
      mockUserProfile = {
        account_id: 'epic_acc_' + Math.floor(Math.random() * 900000 + 100000),
        display_name: 'EpicPioneer',
        email: 'gamer@epicgames.com',
        country: 'US',
        logged_in: true,
      };
      window.dispatchEvent(
        new CustomEvent('hyperion:auth-login-success', { detail: mockUserProfile })
      );
      resolve();
    }, 2000);
  });
};

export const authCancelAutomaticLogin = async (): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('auth_cancel_automatic_login');
  }
};

export const listenAuthLoginSuccess = async (
  cb: (profile: UserProfile) => void
): Promise<UnlistenFn> => {
  if (isTauri()) {
    return listen<UserProfile>('auth-login-success', (event) => {
      cb(event.payload);
    });
  }
  const handler = (e: any) => cb(e.detail);
  window.addEventListener('hyperion:auth-login-success', handler);
  return () => window.removeEventListener('hyperion:auth-login-success', handler);
};

export const listenAuthLoginFailed = async (
  cb: (errorMsg: string) => void
): Promise<UnlistenFn> => {
  if (isTauri()) {
    return listen<string>('auth-login-failed', (event) => {
      cb(event.payload);
    });
  }
  const handler = (e: any) => cb(e.detail);
  window.addEventListener('hyperion:auth-login-failed', handler);
  return () => window.removeEventListener('hyperion:auth-login-failed', handler);
};

export const authLoginCode = async (code: string): Promise<UserProfile> => {
  if (isTauri()) {
    return invoke<UserProfile>('auth_login_code', { code });
  }
  mockUserProfile = {
    account_id: 'epic_acc_' + Math.floor(Math.random() * 900000 + 100000),
    display_name: 'EpicPioneer',
    email: 'gamer@epicgames.com',
    country: 'US',
    logged_in: true,
  };
  return mockUserProfile;
};

export const authLoginExchange = async (code: string): Promise<UserProfile> => {
  if (isTauri()) {
    return invoke<UserProfile>('auth_login_exchange', { code });
  }
  return authLoginCode(code);
};

export const authResumeSession = async (): Promise<UserProfile | null> => {
  if (isTauri()) {
    return invoke<UserProfile | null>('auth_resume_session');
  }
  return mockUserProfile;
};

export const authLogout = async (): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('auth_logout');
  }
  mockUserProfile = null;
};

export const authGetProfile = async (): Promise<UserProfile | null> => {
  if (isTauri()) {
    return invoke<UserProfile | null>('auth_get_profile');
  }
  return mockUserProfile;
};

export const catalogGetGames = async (forceRefresh = false): Promise<GameItem[]> => {
  if (isTauri()) {
    return invoke<GameItem[]>('catalog_get_games', { forceRefresh });
  }
  return [...mockGames];
};

/**
 * First-launch bootstrap: runtime files, SteamCMD, then the library.
 * Progress arrives on `bootstrap-progress`.
 */
export const appBootstrap = async (force = false): Promise<BootstrapReport> => {
  if (isTauri()) {
    return invoke<BootstrapReport>('app_bootstrap', { force });
  }
  // Browser preview has nothing to install; report the mock library instead.
  return {
    epic_games: mockGames.filter((g) => (g.platform || 'epic') !== 'steam').length,
    steam_games: mockGames.filter((g) => g.platform === 'steam').length,
    steamcmd_ready: true,
    first_run: false,
  };
};

export const listenToBootstrapProgress = async (
  callback: (event: { payload: BootstrapProgress }) => void
): Promise<UnlistenFn> => {
  if (isTauri()) {
    return listen<BootstrapProgress>('bootstrap-progress', callback);
  }
  return () => {};
};

export const catalogGetGameDetails = async (appName: string): Promise<GameItem | null> => {
  if (isTauri()) {
    return invoke<GameItem | null>('catalog_get_game_details', { appName });
  }
  return mockGames.find(g => g.app_name === appName) || null;
};

export const gameInstall = async (appName: string, installDir?: string): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('game_install', { appName, installDir });
  }
  const game = mockGames.find(g => g.app_name === appName);
  if (game) {
    mockDownloads.push({
      app_name: appName,
      title: game.title,
      status: 'Downloading',
      total_bytes: game.install_size || 50000000000,
      downloaded_bytes: 12500000000,
      total_chunks: 1200,
      downloaded_chunks: 300,
      speed_bytes_per_sec: 28500000,
      eta_seconds: 1320,
      progress_percent: 25.0,
      current_file: 'Content/Paks/Pakchunk0-WindowsNoEditor.pak',
    });
  }
};

export const gameCancelDownload = async (appName: string): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('game_cancel_download', { appName });
  }
  mockDownloads = mockDownloads.filter(d => d.app_name !== appName);
};

export const gamePauseDownload = async (appName: string): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('game_pause_download', { appName });
  }
  const d = mockDownloads.find(x => x.app_name === appName);
  if (d) d.status = 'Paused';
};

export const gameResumeDownload = async (appName: string): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('game_resume_download', { appName });
  }
  const d = mockDownloads.find(x => x.app_name === appName);
  if (d) d.status = 'Downloading';
};

export const gameGetDownloadProgress = async (appName: string): Promise<DownloadProgress | null> => {
  if (isTauri()) {
    return invoke<DownloadProgress | null>('game_get_download_progress', { appName });
  }
  return mockDownloads.find(d => d.app_name === appName) || null;
};

export const gameGetAllDownloads = async (): Promise<DownloadProgress[]> => {
  if (isTauri()) {
    return invoke<DownloadProgress[]>('game_get_all_downloads');
  }
  return [...mockDownloads];
};

export const gameVerify = async (appName: string): Promise<VerificationResult> => {
  if (isTauri()) {
    return invoke<VerificationResult>('game_verify', { appName });
  }
  return {
    app_name: appName,
    total_files: 1420,
    valid_files: 1420,
    corrupt_files: [],
    missing_files: [],
    is_valid: true,
  };
};

/** Verifies the install and re-downloads only the files that failed. Returns
 *  the pre-repair verification; when it is not valid a download has started. */
export const gameRepair = async (appName: string): Promise<VerificationResult> => {
  if (isTauri()) {
    return invoke<VerificationResult>('game_repair', { appName });
  }
  return {
    app_name: appName,
    total_files: 1420,
    valid_files: 1418,
    corrupt_files: ['Content/Paks/pakchunk1.pak'],
    missing_files: ['Engine/Binaries/Win64/readme.txt'],
    is_valid: false,
  };
};

export const gameUninstall = async (appName: string): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('game_uninstall', { appName });
  }
  const game = mockGames.find(g => g.app_name === appName);
  if (game) {
    game.is_installed = false;
    game.install_path = undefined;
  }
};

export const gameLaunch = async (appName: string, offline = false, extraArgs: string[] = []): Promise<number> => {
  if (isTauri()) {
    return invoke<number>('game_launch', { appName, offline, extraArgs });
  }
  const pid = Math.floor(Math.random() * 8000 + 1000);
  const game = mockGames.find(g => g.app_name === appName);
  mockRunningGames = mockRunningGames.filter(r => r.app_name !== appName);
  mockRunningGames.push({
    app_name: appName,
    title: game?.title || appName,
    pid,
    started_at: new Date().toISOString(),
  });
  return pid;
};

/** Games the launcher started that are still alive */
export const gameGetRunning = async (): Promise<RunningGame[]> => {
  if (isTauri()) {
    return invoke<RunningGame[]>('game_get_running');
  }
  return [...mockRunningGames];
};

/** Stops a game started from the launcher */
export const gameStop = async (appName: string): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('game_stop', { appName });
  }
  if (!mockRunningGames.some(r => r.app_name === appName)) {
    throw new Error(`${appName} is not running under Hyperion`);
  }
  mockRunningGames = mockRunningGames.filter(r => r.app_name !== appName);
};

/**
 * Picks up installed games that are already running without the launcher.
 * Walks the process list, so it is called rarely (startup + a slow interval).
 */
export const gameAdoptRunning = async (): Promise<RunningGame[]> => {
  if (isTauri()) {
    return invoke<RunningGame[]>('game_adopt_running');
  }
  return [...mockRunningGames];
};

export const cloudSaveGetStatus = async (appName: string): Promise<CloudSaveState> => {
  if (isTauri()) {
    return invoke<CloudSaveState>('cloud_save_get_status', { appName });
  }
  return {
    app_name: appName,
    local_path: `C:\\Users\\User\\AppData\\Local\\${appName}\\Saved\\SaveGames`,
    local_files_count: 8,
    local_updated_at: '2026-10-19T21:40:00Z',
    cloud_files_count: 8,
    cloud_updated_at: '2026-10-19T21:40:00Z',
    in_sync: true,
    status_message: 'Local and cloud saves in sync',
  };
};

export const cloudSaveSync = async (appName: string): Promise<string> => {
  if (isTauri()) {
    return invoke<string>('cloud_save_sync', { appName });
  }
  return 'Cloud and local saves synced successfully';
};

export const settingsGet = async (): Promise<AppSettings> => {
  if (isTauri()) {
    return invoke<AppSettings>('settings_get');
  }
  return { ...mockSettings };
};

export const settingsSave = async (settings: AppSettings): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('settings_save', { settings });
  }
  mockSettings = { ...settings };
};

export const diagnosticsGet = async (): Promise<SystemDiagnostics> => {
  if (isTauri()) {
    return invoke<SystemDiagnostics>('diagnostics_get');
  }
  return {
    os_name: 'Windows 11 Pro',
    os_version: '10.0.26100',
    arch: 'x86_64',
    total_memory_mb: 32678,
    free_memory_mb: 18432,
    default_install_dir: mockSettings.default_install_dir,
    free_disk_space_gb: 420.5,
    app_version: '1.0.1',
    rust_version: '1.97.1',
  };
};

export const openPath = async (path: string): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('open_path', { path });
  }
  console.log('Opening folder:', path);
};

export const listenToDownloadProgress = async (
  callback: (event: { payload: DownloadProgress }) => void
): Promise<UnlistenFn> => {
  if (isTauri()) {
    return listen<DownloadProgress>('download-progress', callback);
  }
  // Browser stub timer for simulation
  const timer = setInterval(() => {
    if (mockDownloads.length > 0 && mockDownloads[0].status === 'Downloading') {
      const d = mockDownloads[0];
      d.downloaded_bytes = Math.min(d.total_bytes, d.downloaded_bytes + 28500000);
      d.downloaded_chunks = Math.min(d.total_chunks, d.downloaded_chunks + 3);
      d.progress_percent = (d.downloaded_bytes / d.total_bytes) * 100.0;
      d.eta_seconds = Math.max(0, Math.round((d.total_bytes - d.downloaded_bytes) / d.speed_bytes_per_sec));
      if (d.progress_percent >= 100.0) {
        d.status = 'Completed';
        const g = mockGames.find(x => x.app_name === d.app_name);
        if (g) {
          g.is_installed = true;
          g.install_path = 'D:\\Games\\' + g.title;
        }
      }
      callback({ payload: { ...d } });
    }
  }, 1000);

  return () => clearInterval(timer);
};

export const selectManifestFile = async (): Promise<string | null> => {
  if (isTauri()) {
    return invoke<string | null>('select_manifest_file');
  }
  return 'C:\\Downloads\\Fortnite_1.8.manifest';
};

export const selectDirectory = async (defaultPath?: string): Promise<string | null> => {
  if (isTauri()) {
    return invoke<string | null>('select_directory', { defaultPath });
  }
  return defaultPath || 'D:\\Games';
};

export const manifestInspect = async (manifestPathOrUrl: string): Promise<ManifestInfo> => {
  if (isTauri()) {
    return invoke<ManifestInfo>('manifest_inspect', { manifestPathOrUrl });
  }
  if (manifestPathOrUrl.toLowerCase().includes('dauntless')) {
    return {
      app_name: 'Jackal',
      build_version: '1.14.7.647472',
      launch_exe: 'dauntless.exe',
      launch_command: '',
      total_size: 16530000000,
      file_count: 611,
      chunk_count: 15749,
      suggested_base_url:
        'https://egdownload.fastly-edge.com/Builds/Org/a-krlzxj88rtb69fredeuaf887bl5az/Jackal/CloudDir',
      matched_game_title: 'Dauntless',
    };
  }
  return {
    app_name: 'Fortnite',
    build_version: '++Fortnite+Release-1.8-CL-3724489',
    launch_exe: 'FortniteClient-Win64-Shipping.exe',
    launch_command: '',
    total_size: 14850000000,
    file_count: 8420,
    chunk_count: 1240,
    suggested_base_url: 'https://egdownload.fastly-edge.com/Builds/Org/Fortnite/default',
    matched_game_title: 'Fortnite (Custom Version)',
  };
};

/**
 * Downloads a manifest ahead of time and caches it on disk, so installing from
 * it later needs no network round-trip for the manifest itself.
 */
export const manifestPrefetch = async (url: string): Promise<ManifestInfo | null> => {
  if (isTauri()) {
    return invoke<ManifestInfo>('manifest_prefetch', { url });
  }
  // Preview mode: pretend the manifest is already cached locally
  await new Promise((resolve) => setTimeout(resolve, 1200));
  return {
    app_name: 'Jackal',
    build_version: '1.14.7.647472',
    launch_exe: 'DauntlessClient-Win64-Shipping.exe',
    launch_command: '',
    total_size: 32000000000,
    file_count: 611,
    chunk_count: 15749,
    suggested_base_url: 'https://api.mysticfox.dev/v2',
    matched_game_title: 'Dauntless',
  };
};

export const gameInstallCustomManifest = async (request: CustomInstallRequest): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('game_install_custom_manifest', { request });
  }
  console.log('Mock Custom Manifest Install triggered:', request);

  // Preview mode: surface the manifest install as a real download entry
  const appName = request.app_name?.trim() || 'custom_game';
  const title = request.title?.trim() || appName;
  mockDownloads = mockDownloads.filter((d) => d.app_name !== appName);
  mockDownloads.push({
    app_name: appName,
    title,
    status: 'Downloading',
    total_bytes: 32000000000,
    downloaded_bytes: 2100000000,
    total_chunks: 611,
    downloaded_chunks: 40,
    speed_bytes_per_sec: 42500000,
    eta_seconds: 700,
    progress_percent: 6.6,
    current_file: 'Content/Paks/Dauntless-WindowsNoEditor.pak',
  });
};

// ==========================================
// Steam Integration Commands
// ==========================================

export const steamGetInstalledGames = async (): Promise<GameItem[]> => {
  if (isTauri()) {
    return invoke<GameItem[]>('steam_get_installed_games');
  }
  return [];
};

/** Packages SteamCMD installed alongside a game (redistributables, runtimes…) */
export const steamExtraContent = async (appName: string): Promise<SteamExtraContent[]> => {
  if (isTauri()) {
    return invoke<SteamExtraContent[]>('steam_extra_content', { appName });
  }
  return [];
};

export const steamLaunchGame = async (appId: number): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('steam_launch_game', { appId });
  }
  console.log(`Mock Steam launch triggered for App ID: ${appId}`);
};

export const steamInstallGame = async (
  appId: number,
  title?: string,
  installDir?: string,
  depotId?: number,
  manifestId?: string
): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('steam_install_custom', {
      appId,
      title,
      installDir,
      depotId,
      manifestId: manifestId ? parseInt(manifestId, 10) : undefined,
    });
  }
  console.log(`Mock Steam native download triggered for App ID: ${appId}`);
};

/** Sets up the SteamCMD engine before the first Steam sign-in (no-op once installed) */
export const steamPrepareEngine = async (): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('steam_prepare_engine');
  }
};

export const steamGetAuthStatus = async (): Promise<SteamAuthStatus> => {
  if (isTauri()) {
    return invoke<SteamAuthStatus>('steam_get_auth_status');
  }
  return {
    logged_in: false,
    account_name: undefined,
    steam_id: undefined,
  };
};

export const steamLogout = async (): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('steam_logout');
  }
};

// Headless engine session (memory only, backend never persists it)

export const steamGetCredentialStatus = async (): Promise<SteamCredentialStatus> => {
  if (isTauri()) {
    return invoke<SteamCredentialStatus>('steam_get_credential_status');
  }
  return { has_credentials: false, username: undefined };
};

export const steamSetCredentials = async (
  username: string,
  password: string,
  guardCode?: string,
  remember?: boolean
): Promise<SteamCredentialStatus> => {
  if (isTauri()) {
    return invoke<SteamCredentialStatus>('steam_set_credentials', {
      username,
      password,
      guardCode,
      remember,
    });
  }
  return { has_credentials: true, username, remembered: remember };
};

export const steamClearCredentials = async (): Promise<void> => {
  if (isTauri()) {
    return invoke<void>('steam_clear_credentials');
  }
};

export const steamSetGuardCode = async (guardCode: string): Promise<SteamCredentialStatus> => {
  if (isTauri()) {
    return invoke<SteamCredentialStatus>('steam_set_guard_code', { guardCode });
  }
  return { has_credentials: true, username: undefined };
};

export const listenSteamLoginSuccess = async (
  cb: (status: SteamAuthStatus) => void
): Promise<UnlistenFn> => {
  if (isTauri()) {
    return listen<SteamAuthStatus>('steam-login-success', (event) => {
      cb(event.payload);
    });
  }
  const handler = (e: any) => cb(e.detail);
  window.addEventListener('hyperion:steam-login-success', handler);
  return () => window.removeEventListener('hyperion:steam-login-success', handler);
};

export const listenSteamLoginFailed = async (
  cb: (errorMsg: string) => void
): Promise<UnlistenFn> => {
  if (isTauri()) {
    return listen<string>('steam-login-failed', (event) => {
      cb(event.payload);
    });
  }
  const handler = (e: any) => cb(e.detail);
  window.addEventListener('hyperion:steam-login-failed', handler);
  return () => window.removeEventListener('hyperion:steam-login-failed', handler);
};

export const steamSearchGames = async (query: string): Promise<SteamStoreItem[]> => {
  if (isTauri()) {
    return invoke<SteamStoreItem[]>('steam_search_games', { query });
  }
  // Fallback mock items
  return [
    {
      id: 730,
      name: 'Counter-Strike 2',
      tiny_image: 'https://cdn.cloudflare.steamstatic.com/steam/apps/730/capsule_231x87.jpg',
      header_image: 'https://cdn.cloudflare.steamstatic.com/steam/apps/730/header.jpg',
      capsule_image: 'https://cdn.cloudflare.steamstatic.com/steam/apps/730/library_600x900.jpg',
      price: 'Free to Play',
    },
    {
      id: 620,
      name: 'Portal 2',
      tiny_image: 'https://cdn.cloudflare.steamstatic.com/steam/apps/620/capsule_231x87.jpg',
      header_image: 'https://cdn.cloudflare.steamstatic.com/steam/apps/620/header.jpg',
      capsule_image: 'https://cdn.cloudflare.steamstatic.com/steam/apps/620/library_600x900.jpg',
      price: '$9.99',
    },
  ];
};

export const steamGetDetails = async (appId: number): Promise<SteamStoreDetails | null> => {
  if (isTauri()) {
    return invoke<SteamStoreDetails | null>('steam_get_details', { appId });
  }
  return {
    id: appId,
    name: `Steam Game ${appId}`,
    short_description: 'Mock game loaded in browser development environment',
    header_image: `https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/header.jpg`,
    capsule_image: `https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/library_600x900.jpg`,
    developers: ['Valve'],
    publishers: ['Valve'],
  };
};



