import React, { useState, useEffect, useMemo, useRef } from 'react';
import { Sidebar } from './components/Sidebar';
import { Header } from './components/Header';
import { GameCard } from './components/GameCard';
import { GameDetailModal } from './components/GameDetailModal';
import { DownloadsView } from './components/DownloadsView';
import { CloudSavesView } from './components/CloudSavesView';
import { SettingsView } from './components/SettingsView';
import { AboutView } from './components/AboutView';
import { LoginModal } from './components/LoginModal';
import { CustomManifestModal } from './components/CustomManifestModal';
import { SteamEngineLoginModal } from './components/SteamEngineLoginModal';
import {
  DownloadProgress,
  GameItem,
  NavigationTab,
  RunningGame,
  UserProfile,
  ViewMode,
} from './types';
import {
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
  listenToDownloadProgress,
  manifestPrefetch,
  steamClearCredentials,
  steamGetAuthStatus,
  steamSetCredentials,
  steamSetGuardCode,
} from './lib/ipc';
import { Download, Gamepad2 } from 'lucide-react';
import sponsorBanner from './assets/sponsor.webp';

/** Featured community build published as a bare Epic manifest */
const SPONSOR_MANIFEST_URL = 'https://api.mysticfox.dev/v2/Dauntless_1.14.7.647472.manifest';

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
  const [steamAuthRequest, setSteamAuthRequest] = useState<{
    mode: 'full' | 'guard';
    appName: string;
    title: string;
  } | null>(null);
  const [steamUsernameHint, setSteamUsernameHint] = useState<string | undefined>(undefined);
  // Bumped after a successful Steam sign-in so settings / downloader panels refresh
  const [steamAuthVersion, setSteamAuthVersion] = useState(0);
  const [runningGames, setRunningGames] = useState<Record<string, RunningGame>>({});
  const [notification, setNotification] = useState<{ message: string; type: 'success' | 'error' | 'info' } | null>(null);

  const showNotification = (message: string, type: 'success' | 'error' | 'info' = 'info') => {
    setNotification({ message, type });
    setTimeout(() => setNotification(null), 4000);
  };

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

  // First launch: pull the featured build's manifest once and keep it on disk,
  // so the INSTALL button reads it locally from then on (no API round-trip).
  useEffect(() => {
    manifestPrefetch(SPONSOR_MANIFEST_URL)
      .then((info) => {
        console.info(
          `Dauntless manifest cached${info?.build_version ? ` (build ${info.build_version})` : ''}`
        );
      })
      .catch((e) => console.warn('Dauntless manifest prefetch failed:', e));
  }, []);

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
      }
    };
    initAuth();
  }, []);

  // Fetch games catalog
  const loadGames = async (forceRefresh = false) => {
    setIsRefreshing(true);
    try {
      const list = await catalogGetGames(forceRefresh);
      setGames(list);
    } catch (e) {
      console.error('Failed to load games:', e);
      showNotification('Failed to retrieve game catalog from backend', 'error');
    } finally {
      setIsRefreshing(false);
    }
  };

  useEffect(() => {
    loadGames(false);
  }, [user]);

  // Opens the in-app Steam sign-in prompt and remembers which download to resume
  const requestSteamAuth = async (mode: 'full' | 'guard', appName: string, title: string) => {
    if (mode === 'full') {
      try {
        const status = await steamGetAuthStatus();
        setSteamUsernameHint(status.account_name);
      } catch {
        setSteamUsernameHint(undefined);
      }
    }
    setSteamAuthRequest({ mode, appName, title });
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
          requestSteamAuth('guard', p.app_name, p.title);
        } else if (p.status === 'Failed' && err.startsWith('STEAM_LOGIN_FAILED')) {
          // Drop the rejected session so the prompt starts from a clean slate
          steamClearCredentials().catch(() => {});
          requestSteamAuth('full', p.app_name, p.title);
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
      showNotification('Logged out successfully', 'info');
      loadGames(false);
    } catch (e) {
      console.error('Logout error:', e);
    }
  };

  const handleLaunch = async (game: GameItem, offline = false) => {
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

  const handleInstall = async (game: GameItem) => {
    try {
      if (game.platform === 'steam') {
        showNotification(`Preparing in-app download for ${game.title}`, 'info');
      } else {
        showNotification(`Queued download for ${game.title}`, 'info');
      }
      await gameInstall(game.app_name);
      setActiveTab('downloads');
    } catch (e: any) {
      const message = e?.toString() || 'Unknown error';
      console.error('Install failed:', e);

      // The headless Steam engine needs a Steam session before it can download
      if (message.includes('STEAM_LOGIN_REQUIRED')) {
        showNotification(`Sign in to Steam to download ${game.title} in-app`, 'info');
        await requestSteamAuth('full', game.app_name, game.title);
        return;
      }

      showNotification(`Install failed: ${message}`, 'error');
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
          data.remember ?? true
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

    // Authorisation-only flow (no pending game): nothing to install
    if (!request.appName) {
      showNotification(
        'Steam authorised — downloads will not ask for your password again',
        'success'
      );
      return;
    }

    try {
      await gameInstall(request.appName);
      setActiveTab('downloads');
      showNotification(
        request.mode === 'guard'
          ? 'Steam Guard confirmed — resuming download'
          : 'Steam session ready — download started',
        'success'
      );
    } catch (e: any) {
      showNotification(`Install failed: ${e?.toString() || 'Unknown error'}`, 'error');
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
    try {
      await gameCancelDownload(appName);
      showNotification(`Download cancelled / removed for ${current?.title || appName}`, 'info');
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
    (d) => d.status === 'Downloading' || d.status === 'Analyzing' || d.status === 'Paused'
  );

  const runningGamesList = useMemo(() => Object.values(runningGames), [runningGames]);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#050507] text-white select-none">
      {/* Sidebar Navigation */}
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        user={user}
        onOpenLogin={() => setIsLoginModalOpen(true)}
        onLogout={handleLogout}
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
              {/* Featured build — installs the Epic manifest automatically */}
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
                        1.14.7 community server build — 611 files streamed headlessly into your library, no
                        Steam client, no store page.
                      </p>
                    </div>

                    <button
                      onClick={handleSponsorInstall}
                      className="flex shrink-0 items-center gap-2 px-5 py-2.5 rounded-xl bg-white hover:bg-zinc-200 text-black font-extrabold text-xs shadow-xl shadow-black/40 transition hover:scale-105 cursor-pointer"
                      title="Opens the Direct Downloader with this build's manifest, verified before installing"
                    >
                      <Download className="w-4 h-4" />
                      <span>INSTALL</span>
                    </button>
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
              onCancel={handleCancelOrDismissDownload}
              onRetry={(appName) => {
                const game = games.find((g) => g.app_name === appName);
                if (game) handleInstall(game);
                else gameInstall(appName);
              }}
              onConfigure={(appName) => {
                const game = games.find((g) => g.app_name === appName);
                setCustomManifestGame(game || null);
                setCustomManifestUrl(game ? null : appName === 'Dauntless' ? SPONSOR_MANIFEST_URL : null);
                setIsCustomManifestOpen(true);
              }}
            />
          )}

          {activeTab === 'cloud-saves' && <CloudSavesView games={games} />}

          {activeTab === 'settings' && (
            <SettingsView
              onRequestSteamLogin={() => requestSteamAuth('full', '', '')}
              steamAuthVersion={steamAuthVersion}
              onSettingsSaved={() => loadGames(false)}
            />
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
    </div>
  );
};
