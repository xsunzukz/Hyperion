import React, { useState, useEffect, useMemo, useRef } from 'react';
import { Sidebar } from './components/Sidebar';
import { Header } from './components/Header';
import { GameCard } from './components/GameCard';
import { GameDetailModal } from './components/GameDetailModal';
import { DownloadsView } from './components/DownloadsView';
import { CloudSavesView } from './components/CloudSavesView';
import { SettingsView } from './components/SettingsView';
import { AccountView } from './components/AccountView';
import { AboutView } from './components/AboutView';
import { LoginModal } from './components/LoginModal';
import { LoginGate } from './components/LoginGate';
import { CustomManifestModal } from './components/CustomManifestModal';
import { SteamEngineLoginModal } from './components/SteamEngineLoginModal';
import { CancelDownloadDialog } from './components/CancelDownloadDialog';
import { InstallLocationDialog } from './components/InstallLocationDialog';
import {
  AppSettings,
  DownloadProgress,
  GameItem,
  NavigationTab,
  RunningGame,
  UserProfile,
  ViewMode,
} from './types';
import {
  appBootstrap,
  authLogout,
  authResumeSession,
  catalogGetGames,
  gameAdoptRunning,
  gameCancelDownload,
  gameGetAllDownloads,
  gameGetRunning,
  gameInstall,
  gameLaunch,
  gamePauseDownload,
  gameResumeDownload,
  gameStop,
  gameUninstall,
  launchMysticLauncher,
  listenToDownloadProgress,
  mysticLauncherStatus,
  openPath,
  settingsGet,
  settingsSave,
  steamClearCredentials,
  steamGetAuthStatus,
  steamSetCredentials,
  steamSetGuardCode,
} from './lib/ipc';
import { Download, Gamepad2, Github, Loader2, Play } from 'lucide-react';
import sponsorBanner from './assets/sponsor.webp';

/** Featured community build published as a bare Epic manifest */
const SPONSOR_MANIFEST_URL = 'https://api.mysticfox.dev/v2/Dauntless_1.14.7.647472.manifest';
/** The app name inside that manifest: Epic's codename for Dauntless */
const SPONSOR_APP_NAME = 'Jackal';
const SPONSOR_TITLE = 'Dauntless';
const MYSTIC_PARADOX_RELEASES_URL = 'https://github.com/pranav158/Mystic-Paradox/releases';

/** Statuses that mean a queue entry is already in flight. */
const LIVE_DOWNLOAD_STATUSES: DownloadProgress['status'][] = [
  'Queued',
  'Analyzing',
  'Downloading',
  'Paused',
  'Verifying',
  'Installing',
];

const isLiveDownload = (status: DownloadProgress['status']) =>
  LIVE_DOWNLOAD_STATUSES.includes(status);

/** Without an Epic catalog entry the featured build would show up as a bare
 *  "Jackal" card with stock art. */
const withFeaturedArt = (game: GameItem): GameItem =>
  game.app_name === SPONSOR_APP_NAME
    ? {
        ...game,
        title: game.title === SPONSOR_APP_NAME ? SPONSOR_TITLE : game.title,
        icon_url: game.icon_url || sponsorBanner,
        banner_url: game.banner_url || sponsorBanner,
      }
    : game;

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<NavigationTab>('home');
  const [user, setUser] = useState<UserProfile | null>(null);
  const [games, setGames] = useState<GameItem[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedFilter, setSelectedFilter] = useState<'all' | 'installed' | 'uninstalled'>('all');
  const [platformFilter, setPlatformFilter] = useState<'all' | 'epic' | 'steam'>('all');
  const [viewMode, setViewMode] = useState<ViewMode>('grid');
  const [isRefreshing, setIsRefreshing] = useState(false);
  // Sidebar can be collapsed to an icon rail; the choice survives restarts
  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(() => {
    try {
      return localStorage.getItem('mh.sidebarCollapsed') === '1';
    } catch {
      return false;
    }
  });
  const [selectedGame, setSelectedGame] = useState<GameItem | null>(null);
  const [isLoginModalOpen, setIsLoginModalOpen] = useState(false);
  const [downloads, setDownloads] = useState<Record<string, DownloadProgress>>({});
  const [isCustomManifestOpen, setIsCustomManifestOpen] = useState(false);
  const [customManifestGame, setCustomManifestGame] = useState<GameItem | null>(null);
  // Manifest URL pre-filled into the Direct Downloader (e.g. when retrying the featured build)
  const [customManifestUrl, setCustomManifestUrl] = useState<string | null>(null);
  // Game waiting for the user to pick an install folder.
  const [installPromptGame, setInstallPromptGame] = useState<GameItem | null>(null);
  const [steamAuthRequest, setSteamAuthRequest] = useState<{
    mode: 'full' | 'guard';
    appName: string;
    title: string;
    installDir?: string;
  } | null>(null);
  const [steamUsernameHint, setSteamUsernameHint] = useState<string | undefined>(undefined);
  // Bumped after a successful Steam sign-in so settings / downloader panels refresh
  const [steamAuthVersion, setSteamAuthVersion] = useState(0);
  const [runningGames, setRunningGames] = useState<Record<string, RunningGame>>({});
  // Steam connected but the local library scan is off -> titles are hidden
  const [steamLibraryHint, setSteamLibraryHint] = useState<AppSettings | null>(null);
  const [notification, setNotification] = useState<{ message: string; type: 'success' | 'error' | 'info' } | null>(null);
  // Active download awaiting destructive-cancel confirmation.
  const [cancelCandidate, setCancelCandidate] = useState<DownloadProgress | null>(null);
  const [isCancellingDownload, setIsCancellingDownload] = useState(false);
  // `null` while the first detection runs; false opens GitHub instead.
  const [mysticLauncherInstalled, setMysticLauncherInstalled] = useState<boolean | null>(null);
  // Wait for the saved session check before loading the library, so the first
  // request already knows whether Epic is signed in.
  const [sessionChecked, setSessionChecked] = useState(false);
  // Set when the user chooses "continue offline" on the sign-in gate.
  const [skipLogin, setSkipLogin] = useState(false);

  const showNotification = (message: string, type: 'success' | 'error' | 'info' = 'info') => {
    setNotification({ message, type });
    setTimeout(() => setNotification(null), 4000);
  };

  // The featured card opens Mystic Paradox's own launcher. Re-check when the
  // window regains focus so installing it while the browser is open updates
  // the button without restarting Hyperion.
  const refreshMysticLauncher = async (): Promise<boolean> => {
    try {
      const status = await mysticLauncherStatus();
      setMysticLauncherInstalled(status.installed);
      return status.installed;
    } catch (e) {
      console.warn('Could not detect the Mystic Paradox Launcher:', e);
      setMysticLauncherInstalled(false);
      return false;
    }
  };

  const openMysticParadoxReleases = async () => {
    try {
      await openPath(MYSTIC_PARADOX_RELEASES_URL);
    } catch (e: any) {
      console.error('Could not open Mystic Paradox releases:', e);
      showNotification('Could not open the Mystic Paradox releases page', 'error');
    }
  };

  /**
   * Every Dauntless play action ends up here after the game itself is
   * installed: open Mystic Paradox's launcher when it exists, otherwise send
   * the player to its releases page.
   */
  const handleMysticParadoxAction = async () => {
    let installed = mysticLauncherInstalled;
    if (installed === null) {
      installed = await refreshMysticLauncher();
    }

    if (installed) {
      try {
        await launchMysticLauncher();
        showNotification('Opening Mystic Paradox Launcher...', 'info');
        return;
      } catch (e: any) {
        console.error('Could not launch Mystic Paradox Launcher:', e);
        showNotification('Could not open the Mystic Paradox Launcher', 'error');
        // The launcher may have been uninstalled since the last check.
        await refreshMysticLauncher();
      }
    }

    await openMysticParadoxReleases();
  };

  useEffect(() => {
    void refreshMysticLauncher();
    const onFocus = () => {
      void refreshMysticLauncher();
    };
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, []);

  // Collapse state is mirrored to localStorage so the sidebar keeps its width
  // between launcher sessions.
  const toggleSidebar = () => {
    setSidebarCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem('mh.sidebarCollapsed', next ? '1' : '0');
      } catch {
        /* storage may be unavailable in some contexts */
      }
      return next;
    });
  };

  // Track which games are still running (polled, so exits are noticed too).
  // Only publish a new state when the set actually changes, so the 3 s poll does
  // not re-render the whole app (and re-mount modal effects) for nothing.
  const runningSignature = useRef('');
  const refreshRunningGames = async () => {
    try {
      const list = await gameGetRunning();
      const signature = list
        .map((game) => `${game.app_name}:${game.pid}`)
        .sort()
        .join('|');

      if (signature === runningSignature.current) {
        return;
      }
      runningSignature.current = signature;

      const map: Record<string, RunningGame> = {};
      for (const game of list) map[game.app_name] = game;
      setRunningGames(map);
    } catch (e) {
      console.error('Failed to read running games:', e);
    }
  };

  useEffect(() => {
    refreshRunningGames();
    const timer = setInterval(refreshRunningGames, 3000);
    return () => clearInterval(timer);
  }, []);

  // Liveness above only checks PIDs we already track. Games started outside the
  // launcher (Steam, Epic client) are discovered by this slower sweep, which
  // walks the process list — so it runs at startup and once a minute, not on
  // every poll.
  useEffect(() => {
    let cancelled = false;

    const adopt = async () => {
      try {
        const list = await gameAdoptRunning();
        if (cancelled || list.length === 0) return;
        const signature = list
          .map((game) => `${game.app_name}:${game.pid}`)
          .sort()
          .join('|');
        if (signature === runningSignature.current) return;
        runningSignature.current = signature;

        const map: Record<string, RunningGame> = {};
        for (const game of list) map[game.app_name] = game;
        setRunningGames(map);
      } catch (e) {
        console.error('Failed to adopt running games:', e);
      }
    };

    adopt();
    const timer = setInterval(adopt, 60_000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  // Surface the opt-in when Steam is connected but its library is not scanned,
  // otherwise the Library looks empty for no visible reason.
  useEffect(() => {
    (async () => {
      try {
        const [settings, steamAuth] = await Promise.all([settingsGet(), steamGetAuthStatus()]);
        setSteamLibraryHint(steamAuth.logged_in && !settings.steam_library_scan ? settings : null);
      } catch {
        /* hint only */
      }
    })();
  }, [steamAuthVersion]);

  // Ctrl/Cmd+B collapses or expands the sidebar, like most launcher shells
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'b') {
        event.preventDefault();
        toggleSidebar();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  // Warm the backend in the background: no splash, the shell is interactive
  // immediately while the catalog and Steam state settle.
  useEffect(() => {
    appBootstrap(false).catch((e) => {
      console.error('Startup bootstrap failed:', e);
    });
  }, []);

  // Resume auth session on mount
  useEffect(() => {
    const initAuth = async () => {
      try {
        const profile = await authResumeSession();
        if (profile) {
          setUser(profile);
        }
      } catch (e) {
        console.error('Session resume error:', e);
      } finally {
        setSessionChecked(true);
      }
    };
    initAuth();
  }, []);

  // Fetch games catalog. Loads overlap (sign-in, finished downloads, the
  // Refresh button), so an older answer must not replace a newer library —
  // except a forced refresh, which carries the freshest data whenever it lands.
  const loadSeq = useRef(0);
  const appliedSeq = useRef(0);
  const loadsInFlight = useRef(0);
  // Remember the chosen folder for Steam Guard prompts that arrive later.
  const installDirsRef = useRef<Record<string, string>>({});
  const loadGames = async (forceRefresh = false) => {
    const seq = ++loadSeq.current;
    loadsInFlight.current += 1;
    setIsRefreshing(true);
    try {
      const list = await catalogGetGames(forceRefresh);
      if (forceRefresh || seq > appliedSeq.current) {
        appliedSeq.current = Math.max(appliedSeq.current, seq);
        setGames(list.map(withFeaturedArt));
      }
    } catch (e) {
      console.error('Failed to load games:', e);
      showNotification('Failed to retrieve game catalog from backend', 'error');
    } finally {
      loadsInFlight.current -= 1;
      if (loadsInFlight.current === 0) setIsRefreshing(false);
    }
  };

  useEffect(() => {
    if (!sessionChecked) return;
    loadGames(false);
  }, [user, sessionChecked]);

  // Opens the in-app Steam sign-in prompt and remembers which download to resume
  const requestSteamAuth = async (
    mode: 'full' | 'guard',
    appName: string,
    title: string,
    installDir?: string
  ) => {
    if (mode === 'full') {
      try {
        const status = await steamGetAuthStatus();
        setSteamUsernameHint(status.account_name);
      } catch {
        setSteamUsernameHint(undefined);
      }
    }
    setSteamAuthRequest({ mode, appName, title, installDir });
  };

  // Listen to live Tauri download progress events
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setupListener = async () => {
      unlisten = await listenToDownloadProgress((event) => {
        const p = event.payload;
        setDownloads((prev) => ({ ...prev, [p.app_name]: p }));

        if (p.status === 'Completed' || p.status === 'Cancelled' || p.status === 'Failed') {
          loadGames(false);
        }

        // The headless engine can ask for a fresh Steam Guard code mid-download,
        // or report that the stored password was rejected.
        const err = p.error_message || '';
        if (p.status === 'Failed' && err.startsWith('STEAM_GUARD_REQUIRED')) {
          requestSteamAuth('guard', p.app_name, p.title, installDirsRef.current[p.app_name]);
        } else if (p.status === 'Failed' && err.startsWith('STEAM_LOGIN_FAILED')) {
          // Drop the rejected session so the prompt starts from a clean slate
          steamClearCredentials().catch(() => {});
          requestSteamAuth('full', p.app_name, p.title, installDirsRef.current[p.app_name]);
        }
      });
    };
    setupListener();

    // Initial check for active downloads
    gameGetAllDownloads().then((list) => {
      const map: Record<string, DownloadProgress> = {};
      for (const d of list) {
        map[d.app_name] = d;
      }
      setDownloads(map);
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleLogout = async () => {
    try {
      await authLogout();
      setUser(null);
      // Signing out puts the gate back: the offline skip was for that session only.
      setSkipLogin(false);
      showNotification('Logged out successfully', 'info');
      loadGames(false);
    } catch (e) {
      console.error('Logout error:', e);
    }
  };

  const handleLaunch = async (game: GameItem, offline = false) => {
    // Dauntless must be started through Mystic Paradox's launcher, never
    // directly — that launcher performs the sign-in and runtime checks.
    if (game.app_name === SPONSOR_APP_NAME) {
      if (!game.is_installed) {
        handleSponsorInstall();
      } else {
        await handleMysticParadoxAction();
      }
      return;
    }

    try {
      showNotification(`Launching ${game.title}...`, 'info');
      const pid = await gameLaunch(game.app_name, offline);
      showNotification(`${game.title} is running (PID ${pid})`, 'success');
      await refreshRunningGames();
    } catch (e: any) {
      console.error('Launch failed:', e);
      showNotification(`Launch failed: ${e?.toString() || 'Unknown error'}`, 'error');
    }
  };

  const handleStop = async (game: GameItem) => {
    await handleStopApp(game.app_name, game.title);
  };

  /** Stops a running game by app id — used by the cards, the detail modal and the sidebar's "Running now" list. */
  const handleStopApp = async (appName: string, knownTitle?: string) => {
    const title =
      knownTitle || runningGames[appName]?.title || games.find((g) => g.app_name === appName)?.title || appName;
    try {
      await gameStop(appName);
      showNotification(`Stopped ${title}`, 'info');
    } catch (e: any) {
      console.error('Stop failed:', e);
      showNotification(`Could not stop ${title}: ${e?.toString() || 'Unknown error'}`, 'error');
    } finally {
      await refreshRunningGames();
    }
  };

  /** Featured build: opens the Direct Downloader with this manifest pre-filled,
   *  so the install runs through the same verified preview as a local file. */
  const handleSponsorInstall = () => {
    setCustomManifestGame(null);
    setCustomManifestUrl(SPONSOR_MANIFEST_URL);
    setIsCustomManifestOpen(true);
  };

  /**
   * Put a row in the Downloads queue before the backend has fetched the
   * manifest. The real `download-progress` event replaces this placeholder as
   * soon as the first chunk pipeline reports in.
   */
  const queueDownload = (appName: string, title: string, totalBytes = 0) => {
    const placeholder: DownloadProgress = {
      app_name: appName,
      title,
      status: 'Queued',
      total_bytes: totalBytes,
      downloaded_bytes: 0,
      total_chunks: 0,
      downloaded_chunks: 0,
      speed_bytes_per_sec: 0,
      eta_seconds: 0,
      progress_percent: 0,
    };

    setDownloads((prev) => {
      const current = prev[appName];
      if (current && isLiveDownload(current.status)) return prev;
      return { ...prev, [appName]: placeholder };
    });
    setActiveTab('downloads');
    // The detail modal would otherwise hide the queue we just opened.
    setSelectedGame(null);
  };

  const requestGameInstall = async (appName: string, title: string, installDir?: string) => {
    if (installDir) installDirsRef.current[appName] = installDir;
    try {
      await gameInstall(appName, installDir);

      // The backend has accepted the job; the first progress event usually
      // lands moments later, but do not leave the row at "Queued" if it is slow.
      setDownloads((prev) => {
        const current = prev[appName];
        if (!current || current.status !== 'Queued') return prev;
        return { ...prev, [appName]: { ...current, status: 'Analyzing' } };
      });
      return true;
    } catch (e: any) {
      const message = e?.toString() || 'Unknown error';
      console.error('Install failed:', e);

      // The headless Steam engine needs a Steam session before it can download
      if (message.includes('STEAM_LOGIN_REQUIRED')) {
        setDownloads((prev) => {
          if (prev[appName]?.status !== 'Queued') return prev;
          const next = { ...prev };
          delete next[appName];
          return next;
        });
        showNotification(`Sign in to Steam to download ${title} in-app`, 'info');
        await requestSteamAuth('full', appName, title, installDir);
        return false;
      }

      setDownloads((prev) => {
        const current = prev[appName];
        if (!current || (current.status !== 'Queued' && current.status !== 'Analyzing')) {
          return prev;
        }
        return {
          ...prev,
          [appName]: { ...current, status: 'Failed', error_message: message },
        };
      });
      showNotification(`Install failed: ${message}`, 'error');
      return false;
    }
  };

  const startGameInstall = async (game: GameItem, installDir: string) => {
    queueDownload(game.app_name, game.title, game.install_size || 0);
    showNotification(
      game.platform === 'steam'
        ? `Preparing in-app download for ${game.title}`
        : `Queued download for ${game.title}`,
      'info'
    );
    await requestGameInstall(game.app_name, game.title, installDir);
  };

  const handleInstall = (game: GameItem) => {
    // Dauntless installs from the community build, never Epic's delisted one
    if (game.app_name === SPONSOR_APP_NAME) {
      handleSponsorInstall();
      return;
    }

    const current = downloads[game.app_name];
    if (current && isLiveDownload(current.status)) {
      setActiveTab('downloads');
      setSelectedGame(null);
      return;
    }

    // Ask where this game should go before creating the queue entry.
    setInstallPromptGame(game);
  };

  const handleInstallLocationConfirm = async (installDir: string) => {
    const game = installPromptGame;
    setInstallPromptGame(null);
    if (game) {
      await startGameInstall(game, installDir);
    }
  };

  const handleSteamAuthSubmit = async (data: {
    username?: string;
    password?: string;
    guardCode?: string;
    remember?: boolean;
  }) => {
    const request = steamAuthRequest;
    if (!request) return;

    if (request.mode === 'full') {
      try {
        await steamSetCredentials(
          data.username || '',
          data.password || '',
          data.guardCode,
          data.remember ?? false
        );
      } catch (e: any) {
        // Surface Valve's reason without the Tauri error prefix
        const message = String(e?.message ?? e ?? '').replace(/^Authentication error:\s*/i, '');
        throw new Error(message || 'Steam sign-in failed');
      }
    } else {
      await steamSetGuardCode(data.guardCode || '');
    }

    setSteamAuthRequest(null);
    setSteamAuthVersion((v) => v + 1);

    // Refresh the catalog: titles installed through the engine show up now.
    // This PC's Steam library is listed only once the user enables that scan
    // (the Library banner offers it).
    loadGames(false);

    // Authorisation-only flow (no pending game): nothing to install
    if (!request.appName) {
      showNotification(
        'Steam connected — Steam downloads now run in Hyperion',
        'success'
      );
      return;
    }

    queueDownload(request.appName, request.title);
    const started = await requestGameInstall(
      request.appName,
      request.title,
      request.installDir
    );
    if (started) {
      showNotification(
        request.mode === 'guard'
          ? 'Steam Guard confirmed — resuming download'
          : 'Steam session ready — download started',
        'success'
      );
    }
  };

  const handleUninstall = async (game: GameItem) => {
    try {
      await gameUninstall(game.app_name);
      setSelectedGame(null);
      showNotification(`Uninstalled ${game.title}`, 'info');
      loadGames(false);
    } catch (e: any) {
      console.error('Uninstall failed:', e);
      showNotification(`Uninstall failed: ${e?.toString() || 'Unknown error'}`, 'error');
    }
  };

  const handlePauseDownload = async (appName: string) => {
    try {
      await gamePauseDownload(appName);
      setDownloads((prev) => {
        if (!prev[appName]) return prev;
        return {
          ...prev,
          [appName]: { ...prev[appName], status: 'Paused', speed_bytes_per_sec: 0 },
        };
      });
    } catch (e) {
      console.error('Pause download error:', e);
    }
  };

  const handleResumeDownload = async (appName: string) => {
    try {
      await gameResumeDownload(appName);
      setDownloads((prev) => {
        if (!prev[appName]) return prev;
        return {
          ...prev,
          [appName]: { ...prev[appName], status: 'Downloading' },
        };
      });
    } catch (e) {
      console.error('Resume download error:', e);
    }
  };

  const handleCancelOrDismissDownload = async (appName: string) => {
    const current = downloads[appName];
    const finished =
      current?.status === 'Completed' || current?.status === 'Cancelled' || current?.status === 'Failed';
    try {
      // A finished entry is only cleared from the queue: there is nothing left
      // to cancel, and the backend must never be asked to touch that install.
      if (!finished) {
        await gameCancelDownload(appName);
        showNotification(`Download cancelled for ${current?.title || appName}`, 'info');
      }
    } catch (e) {
      console.error('Cancel download error:', e);
    } finally {
      setDownloads((prev) => {
        const next = { ...prev };
        delete next[appName];
        return next;
      });
      loadGames(false);
    }
  };

  /** Finished entries are just dismissed; active ones need confirmation. */
  const handleCancelRequest = (appName: string) => {
    const target = downloads[appName];
    if (!target) return;

    const finished =
      target.status === 'Completed' ||
      target.status === 'Cancelled' ||
      target.status === 'Failed';
    if (finished) {
      void handleCancelOrDismissDownload(appName);
      return;
    }

    setCancelCandidate(target);
  };

  const handleConfirmCancelDownload = async () => {
    if (!cancelCandidate || isCancellingDownload) return;

    setIsCancellingDownload(true);
    try {
      await handleCancelOrDismissDownload(cancelCandidate.app_name);
    } finally {
      setIsCancellingDownload(false);
      setCancelCandidate(null);
    }
  };

  // Filtered games computation
  const filteredGames = useMemo(() => {
    return games.filter((game) => {
      // Search text match
      const query = searchQuery.toLowerCase().trim();
      const matchesSearch =
        !query ||
        game.title.toLowerCase().includes(query) ||
        game.app_name.toLowerCase().includes(query) ||
        (game.developer && game.developer.toLowerCase().includes(query));

      // Filter pill match
      if (selectedFilter === 'installed' && !game.is_installed) return false;
      if (selectedFilter === 'uninstalled' && game.is_installed) return false;

      // Platform filter match
      if (platformFilter !== 'all' && (game.platform || 'epic') !== platformFilter) {
        return false;
      }

      return matchesSearch;
    });
  }, [games, searchQuery, selectedFilter, platformFilter]);

  const activeDownloadsList = Object.values(downloads).filter(
    (d) => d.status === 'Queued' || d.status === 'Downloading' || d.status === 'Analyzing' || d.status === 'Paused'
  );

  const runningGamesList = useMemo(() => Object.values(runningGames), [runningGames]);

  // First the game, then its required launcher.
  const featuredGame = games.find((game) => game.app_name === SPONSOR_APP_NAME) ?? null;
  const featuredInstalled = !!featuredGame?.is_installed;
  const featuredDownload = downloads[SPONSOR_APP_NAME];
  const featuredActiveDownload =
    featuredDownload && isLiveDownload(featuredDownload.status) ? featuredDownload : null;

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#050507] text-white select-none">
      {/* Sidebar Navigation */}
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        activeDownloadsCount={activeDownloadsList.length}
        collapsed={sidebarCollapsed}
        onToggleCollapsed={toggleSidebar}
        runningGames={runningGamesList}
        onStopGame={handleStopApp}
      />

      {/* Main App Workspace */}
      <div className="flex-1 flex flex-col overflow-hidden relative">
        {/* Header Bar */}
        {(activeTab === 'home' || activeTab === 'library') && (
          <Header
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            selectedFilter={selectedFilter}
            onFilterChange={setSelectedFilter}
            platformFilter={platformFilter}
            onPlatformFilterChange={setPlatformFilter}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            onRefresh={() => loadGames(true)}
            isRefreshing={isRefreshing}
            onOpenCustomManifest={() => {
              setCustomManifestGame(null);
              setCustomManifestUrl(null);
              setIsCustomManifestOpen(true);
            }}
          />
        )}

        {/* Dynamic Notification Toast */}
        {notification && (
          <div
            className={`absolute top-20 right-8 z-40 px-4 py-3 rounded-2xl glass-panel shadow-2xl border flex items-center gap-2.5 text-xs font-semibold animate-fade-in ${
              notification.type === 'success'
                ? 'bg-zinc-950/95 border-orange-500/60 text-white shadow-orange-500/20 shadow-xl'
                : notification.type === 'error'
                ? 'bg-zinc-950/95 border-rose-500/60 text-rose-300 shadow-xl'
                : 'bg-zinc-950/95 border-zinc-700 text-white shadow-xl'
            }`}
          >
            <span
              className={`w-2 h-2 rounded-full ${
                notification.type === 'success'
                  ? 'bg-orange-500'
                  : notification.type === 'error'
                  ? 'bg-rose-500'
                  : 'bg-zinc-400'
              }`}
            />
            <span>{notification.message}</span>
          </div>
        )}

        {/* Tab Content Panes */}
        <main className="flex-1 overflow-y-auto">
          {activeTab === 'home' && (
            <div className="p-8 space-y-8">
              {/* Featured community build — launch when installed, GitHub otherwise */}
              {!searchQuery && selectedFilter === 'all' && (
                <div className="group relative overflow-hidden rounded-3xl border border-zinc-800 shadow-2xl shadow-black/40">
                  <img
                    src={sponsorBanner}
                    alt="Mystic Paradox — Open-Source Dauntless Server Project"
                    className="h-72 w-full object-cover object-center transition-transform duration-[1200ms] ease-out group-hover:scale-[1.03]"
                  />

                  {/* Scrims: dark enough for copy, art still readable */}
                  <div className="pointer-events-none absolute inset-0 bg-gradient-to-t from-black/90 via-black/25 to-transparent" />
                  <div className="pointer-events-none absolute inset-0 bg-gradient-to-r from-black/70 via-transparent to-transparent" />

                  <div className="absolute inset-x-5 top-5 flex items-start justify-between gap-3">
                    <span className="px-2.5 py-1 rounded-full text-[10px] font-bold uppercase tracking-wider bg-black/60 text-zinc-200 border border-white/15 backdrop-blur-md">
                      Sponsored
                    </span>
                    <div className="flex items-center gap-2">
                      <span className="px-2.5 py-1 rounded-full text-[10px] font-semibold bg-black/55 text-emerald-300 border border-emerald-400/30 backdrop-blur-md">
                        Open source
                      </span>
                      <span className="px-2.5 py-1 rounded-full text-[10px] font-mono bg-black/55 text-zinc-300 border border-white/15 backdrop-blur-md">
                        build 1.14.7.647472
                      </span>
                    </div>
                  </div>

                  <div className="absolute inset-x-8 bottom-6 flex flex-wrap items-end justify-between gap-4">
                    <div className="max-w-lg">
                      <div className="text-[10px] font-bold uppercase tracking-[0.22em] text-orange-300/90">
                        Mystic Paradox presents
                      </div>
                      <h2 className="mt-1 text-4xl font-extrabold leading-none tracking-tight text-white drop-shadow-lg">
                        Dauntless
                      </h2>
                      <p className="mt-2 max-w-md text-xs leading-relaxed text-zinc-300/90">
                        1.14.7 community server build — install Dauntless here, then play it through
                        the Mystic Paradox Launcher.
                      </p>
                    </div>

                    {featuredActiveDownload ? (
                      <button
                        onClick={() => setActiveTab('downloads')}
                        className="flex shrink-0 cursor-pointer items-center gap-2 rounded-xl bg-amber-400 px-5 py-2.5 text-xs font-extrabold text-black shadow-xl shadow-black/40 transition hover:scale-105 hover:bg-amber-300"
                        title="Open the Dauntless download in the queue"
                      >
                        <Download className="h-4 w-4" />
                        <span>
                          {featuredActiveDownload.status === 'Downloading'
                            ? `DOWNLOADING ${featuredActiveDownload.progress_percent.toFixed(0)}%`
                            : featuredActiveDownload.status === 'Paused'
                            ? 'PAUSED'
                            : 'PREPARING'}
                        </span>
                      </button>
                    ) : !featuredInstalled ? (
                      <button
                        onClick={handleSponsorInstall}
                        className="flex shrink-0 cursor-pointer items-center gap-2 rounded-xl bg-white px-5 py-2.5 text-xs font-extrabold text-black shadow-xl shadow-black/40 transition hover:scale-105 hover:bg-zinc-200"
                        title="Install Dauntless 1.14.7"
                      >
                        <Download className="h-4 w-4" />
                        <span>INSTALL</span>
                      </button>
                    ) : mysticLauncherInstalled === null ? (
                      <button
                        disabled
                        className="flex shrink-0 cursor-wait items-center gap-2 rounded-xl border border-white/15 bg-white/10 px-5 py-2.5 text-xs font-extrabold text-zinc-200 shadow-xl shadow-black/40 backdrop-blur-md"
                      >
                        <Loader2 className="h-4 w-4 animate-spin" />
                        <span>CHECKING</span>
                      </button>
                    ) : mysticLauncherInstalled ? (
                      <button
                        onClick={() => void handleMysticParadoxAction()}
                        className="flex shrink-0 cursor-pointer items-center gap-2 rounded-xl bg-gradient-to-r from-orange-600 to-amber-500 px-5 py-2.5 text-xs font-extrabold text-white shadow-xl shadow-orange-950/40 transition hover:scale-105 hover:from-orange-500 hover:to-amber-400 glow-orange"
                        title="Open Mystic Paradox Launcher"
                      >
                        <Play className="h-4 w-4 fill-current" />
                        <span>PLAY</span>
                      </button>
                    ) : (
                      <button
                        onClick={() => void handleMysticParadoxAction()}
                        className="flex shrink-0 cursor-pointer items-center gap-2 rounded-xl bg-white px-5 py-2.5 text-xs font-extrabold text-black shadow-xl shadow-black/40 transition hover:scale-105 hover:bg-zinc-200"
                        title="Download the Mystic Paradox Launcher from GitHub Releases"
                      >
                        <Github className="h-4 w-4" />
                        <span>GET LAUNCHER</span>
                      </button>
                    )}
                  </div>
                </div>
              )}

              {/* Games Grid / List Section */}
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="text-xl font-bold text-white tracking-tight">
                      {searchQuery ? `Search Results (${filteredGames.length})` : 'All Titles'}
                    </h3>
                    <p className="text-xs text-zinc-400 mt-0.5">
                      Showing {filteredGames.length} available titles in your Epic library
                    </p>
                  </div>
                </div>

                {filteredGames.length === 0 ? (
                  <div className="p-16 text-center rounded-3xl glass-panel border border-zinc-800/80 bg-zinc-950/40">
                    <Gamepad2 className="w-12 h-12 text-zinc-600 mx-auto mb-3" />
                    <h4 className="font-bold text-white text-base">No Games Found</h4>
                    <p className="text-xs text-zinc-500 mt-1">
                      Try adjusting your search criteria or sign in to refresh your Epic Games account.
                    </p>
                  </div>
                ) : viewMode === 'grid' ? (
                  <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5">
                    {filteredGames.map((game) => (
                      <GameCard
                        key={game.app_name}
                        game={game}
                        viewMode="grid"
                        onSelect={setSelectedGame}
                        onLaunch={handleLaunch}
                        onInstall={handleInstall}
                        isRunning={!!runningGames[game.app_name]}
                        onStop={handleStop}
                        downloadProgress={downloads[game.app_name]}
                      />
                    ))}
                  </div>
                ) : (
                  <div className="space-y-3">
                    {filteredGames.map((game) => (
                      <GameCard
                        key={game.app_name}
                        game={game}
                        viewMode="list"
                        onSelect={setSelectedGame}
                        onLaunch={handleLaunch}
                        onInstall={handleInstall}
                        isRunning={!!runningGames[game.app_name]}
                        onStop={handleStop}
                        downloadProgress={downloads[game.app_name]}
                      />
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {activeTab === 'library' && (
            <div className="p-8 space-y-6">
              <div>
                <h2 className="text-2xl font-extrabold text-white tracking-tight">Your Game Library</h2>
                <p className="text-sm text-zinc-400 mt-1">
                  Manage installed games, launch arguments, integrity verification, and DLCs.
                </p>
              </div>

              {steamLibraryHint && (
                <div className="flex flex-wrap items-center justify-between gap-3 p-4 rounded-2xl bg-sky-500/10 border border-sky-500/30">
                  <div className="min-w-0">
                    <div className="text-xs font-bold text-sky-200">Steam games are hidden</div>
                    <div className="text-[11px] text-zinc-400 mt-0.5 leading-relaxed">
                      Your Steam account is connected, but scanning this PC&apos;s Steam library is off.
                      Enable it to list the Steam games you own and have installed.
                    </div>
                  </div>
                  <button
                    onClick={async () => {
                      try {
                        await settingsSave({ ...steamLibraryHint, steam_library_scan: true });
                        setSteamLibraryHint(null);
                        await loadGames(false);
                      } catch (e) {
                        console.error('Could not enable the Steam library scan:', e);
                      }
                    }}
                    className="shrink-0 px-4 py-2 rounded-xl bg-sky-600 hover:bg-sky-500 text-white text-xs font-bold transition cursor-pointer"
                  >
                    Show Steam games
                  </button>
                </div>
              )}

              {filteredGames.length === 0 ? (
                <div className="p-16 text-center rounded-3xl glass-panel border border-zinc-800/80 bg-zinc-950/40">
                  <Gamepad2 className="w-12 h-12 text-zinc-600 mx-auto mb-3" />
                  <h4 className="font-bold text-white text-base">No Games Matched</h4>
                  <p className="text-xs text-zinc-500 mt-1">
                    No titles found with current filters.
                  </p>
                </div>
              ) : viewMode === 'grid' ? (
                <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5">
                  {filteredGames.map((game) => (
                    <GameCard
                      key={game.app_name}
                      game={game}
                      viewMode="grid"
                      onSelect={setSelectedGame}
                      onLaunch={handleLaunch}
                      onInstall={handleInstall}
                      isRunning={!!runningGames[game.app_name]}
                      onStop={handleStop}
                      downloadProgress={downloads[game.app_name]}
                    />
                  ))}
                </div>
              ) : (
                <div className="space-y-3">
                  {filteredGames.map((game) => (
                    <GameCard
                      key={game.app_name}
                      game={game}
                      viewMode="list"
                      onSelect={setSelectedGame}
                      onLaunch={handleLaunch}
                      onInstall={handleInstall}
                      isRunning={!!runningGames[game.app_name]}
                      onStop={handleStop}
                      downloadProgress={downloads[game.app_name]}
                    />
                  ))}
                </div>
              )}
            </div>
          )}

          {activeTab === 'downloads' && (
            <DownloadsView
              downloads={Object.values(downloads)}
              onPause={handlePauseDownload}
              onResume={handleResumeDownload}
              onCancel={handleCancelRequest}
              onRetry={(appName) => {
                if (appName === SPONSOR_APP_NAME) {
                  handleSponsorInstall();
                  return;
                }
                const game = games.find((g) => g.app_name === appName);
                if (game) handleInstall(game);
                else gameInstall(appName);
              }}
              onConfigure={(appName) => {
                if (appName === SPONSOR_APP_NAME) {
                  handleSponsorInstall();
                  return;
                }
                const game = games.find((g) => g.app_name === appName);
                setCustomManifestGame(game || null);
                setCustomManifestUrl(null);
                setIsCustomManifestOpen(true);
              }}
            />
          )}

          {activeTab === 'cloud-saves' && <CloudSavesView games={games} />}

          {activeTab === 'account' && (
            <AccountView
              user={user}
              onOpenLogin={() => setIsLoginModalOpen(true)}
              onLogout={handleLogout}
              onRequestSteamLogin={() => requestSteamAuth('full', '', '')}
              steamAuthVersion={steamAuthVersion}
              onSettingsChanged={() => loadGames(false)}
              onSteamAuthChanged={() => setSteamAuthVersion((v) => v + 1)}
            />
          )}

          {activeTab === 'settings' && (
            <SettingsView onSettingsSaved={() => loadGames(false)} />
          )}

          {activeTab === 'about' && <AboutView />}
        </main>
      </div>

      {/* Game Details Modal */}
      {selectedGame && (
        <GameDetailModal
          game={selectedGame}
          onClose={() => setSelectedGame(null)}
          onLaunch={handleLaunch}
          onInstall={handleInstall}
          isRunning={!!runningGames[selectedGame.app_name]}
          onStop={handleStop}
          onUninstall={handleUninstall}
          downloadProgress={downloads[selectedGame.app_name]}
          onOpenCustomManifest={(g) => {
            setCustomManifestGame(g);
            setIsCustomManifestOpen(true);
          }}
        />
      )}

      {/* Custom Manifest Modal */}
      {isCustomManifestOpen && (
        <CustomManifestModal
          isOpen={isCustomManifestOpen}
          onClose={() => {
            setIsCustomManifestOpen(false);
            setCustomManifestGame(null);
            setCustomManifestUrl(null);
          }}
          presetGame={customManifestGame}
          presetManifestUrl={customManifestUrl}
          presetTitle={customManifestUrl === SPONSOR_MANIFEST_URL ? SPONSOR_TITLE : null}
          onRequestSteamLogin={() => requestSteamAuth('full', '', '')}
          steamAuthVersion={steamAuthVersion}
          onInstallStarted={(appName) => {
            showNotification(`Started download from custom manifest for ${appName}`, 'success');
            setActiveTab('downloads');
            loadGames(false);
          }}
        />
      )}

      {/* Login Modal */}
      {isLoginModalOpen && (
        <LoginModal
          onClose={() => setIsLoginModalOpen(false)}
          onSuccess={(u) => {
            setUser(u);
            showNotification(`Welcome back, ${u.display_name}!`, 'success');
            loadGames(true);
          }}
          onRequestSteamAuth={() => {
            // Authorisation-only: no pending game to install afterwards
            requestSteamAuth('full', '', '');
          }}
        />
      )}
      {/* Steam sign-in for the in-app download engine */}
      {steamAuthRequest && (
        <SteamEngineLoginModal
          isOpen={!!steamAuthRequest}
          mode={steamAuthRequest.mode}
          defaultUsername={steamUsernameHint}
          gameTitle={steamAuthRequest.title}
          onClose={() => setSteamAuthRequest(null)}
          onSubmit={handleSteamAuthSubmit}
        />
      )}
      {/* Ask for a folder before starting a normal game download */}
      {installPromptGame && (
        <InstallLocationDialog
          title={installPromptGame.title}
          onConfirm={handleInstallLocationConfirm}
          onClose={() => setInstallPromptGame(null)}
        />
      )}

      {/* Destructive confirmation for stopping an active download */}
      {cancelCandidate && (
        <CancelDownloadDialog
          title={cancelCandidate.title}
          downloadedBytes={cancelCandidate.downloaded_bytes}
          isCancelling={isCancellingDownload}
          onConfirm={handleConfirmCancelDownload}
          onClose={() => {
            if (!isCancellingDownload) setCancelCandidate(null);
          }}
        />
      )}

      {/* Sign-in gate: the shell stays mounted underneath, so the real Epic
          login modal (z-50) opens on top of it. */}
      {sessionChecked && !user && !skipLogin && (
        <LoginGate
          onSignIn={() => setIsLoginModalOpen(true)}
          onSkip={() => setSkipLogin(true)}
        />
      )}
    </div>
  );
};
