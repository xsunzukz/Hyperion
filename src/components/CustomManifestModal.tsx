import React, { useState, useEffect, useRef } from 'react';
import {
  X,
  FileCode2,
  FolderOpen,
  Search,
  Download,
  AlertCircle,
  HardDrive,
  Layers,
  Cpu,
  Globe,
  CheckCircle2,
  Loader2,
  Sparkles,
  Gamepad2,
  Info,
  KeyRound,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import { GameItem, ManifestInfo, SteamStoreItem, SteamStoreDetails, SteamAuthStatus } from '../types';
import {
  gameInstallCustomManifest,
  manifestInspect,
  selectDirectory,
  selectManifestFile,
  settingsGet,
  steamSearchGames,
  steamGetDetails,
  steamInstallGame,
  steamGetAuthStatus,
} from '../lib/ipc';

interface CustomManifestModalProps {
  isOpen: boolean;
  onClose: () => void;
  presetGame?: GameItem | null;
  /** Pre-fills the manifest field, e.g. when retrying a featured download */
  presetManifestUrl?: string | null;
  /** Display name for a pre-filled manifest whose app name is only a codename
   *  (the featured build's manifest says "Jackal") */
  presetTitle?: string | null;
  onInstallStarted?: (appName: string) => void;
  /** Opens the in-app Steam sign-in modal (no web/Valve window) */
  onRequestSteamLogin?: () => void;
  /** Bumped by the shell after a successful sign-in so the panel refreshes */
  steamAuthVersion?: number;
}

const POPULAR_STEAM_GAMES = [
  { id: 730, name: 'CS2' },
  { id: 480, name: 'Spacewar' },
  { id: 440, name: 'TF2' },
  { id: 570, name: 'Dota 2' },
  { id: 1172470, name: 'Apex Legends' },
  { id: 620, name: 'Portal 2' },
  { id: 550, name: 'L4D2' },
  { id: 252490, name: 'Rust' },
  { id: 105600, name: 'Terraria' },
  { id: 1091500, name: 'Cyberpunk' },
];

export const CustomManifestModal: React.FC<CustomManifestModalProps> = ({
  isOpen,
  onClose,
  presetGame,
  presetManifestUrl,
  presetTitle,
  onInstallStarted,
  onRequestSteamLogin,
  steamAuthVersion = 0,
}) => {
  const [manifestPathOrUrl, setManifestPathOrUrl] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [installDir, setInstallDir] = useState('');
  const [customTitle, setCustomTitle] = useState('');
  const [customAppName, setCustomAppName] = useState('');

  const [installerTab, setInstallerTab] = useState<'epic' | 'steam'>(
    presetGame?.platform === 'steam' ? 'steam' : 'epic'
  );
  const [steamAppId, setSteamAppId] = useState(presetGame?.steam_app_id?.toString() || '');
  const [steamTitle, setSteamTitle] = useState(presetGame?.title || '');
  const [steamSearchQuery, setSteamSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<SteamStoreItem[]>([]);
  const [isSearchingSteam, setIsSearchingSteam] = useState(false);
  const [steamDetails, setSteamDetails] = useState<SteamStoreDetails | null>(null);
  const [isLoadingDetails, setIsLoadingDetails] = useState(false);
  const searchTimeoutRef = useRef<any>(null);
  // Library root from settings; installs go into <root>\<game> unless overridden
  const defaultDirRef = useRef('');

  const [steamDepotId, setSteamDepotId] = useState('');
  const [steamManifestId, setSteamManifestId] = useState('');
  const [showAdvancedSteam, setShowAdvancedSteam] = useState(false);
  const [steamAuth, setSteamAuth] = useState<SteamAuthStatus | null>(null);

  const [isInspecting, setIsInspecting] = useState(false);
  const [manifestInfo, setManifestInfo] = useState<ManifestInfo | null>(null);
  const [inspectError, setInspectError] = useState<string | null>(null);

  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  const fetchSteamGameDetails = async (id: number) => {
    setIsLoadingDetails(true);
    try {
      const details = await steamGetDetails(id);
      if (details) {
        setSteamDetails(details);
        setSteamTitle(details.name);
      }
    } catch (e) {
      console.warn('Failed to fetch Steam details:', e);
    } finally {
      setIsLoadingDetails(false);
    }
  };

  const handleSteamSearchChange = (val: string) => {
    setSteamSearchQuery(val);
    if (searchTimeoutRef.current) {
      clearTimeout(searchTimeoutRef.current);
    }

    const numericId = parseInt(val.trim(), 10);
    if (!isNaN(numericId) && numericId > 0 && String(numericId) === val.trim()) {
      setSteamAppId(val.trim());
      fetchSteamGameDetails(numericId);
      setSearchResults([]);
      return;
    }

    if (val.trim().length >= 2) {
      setIsSearchingSteam(true);
      searchTimeoutRef.current = setTimeout(async () => {
        try {
          const results = await steamSearchGames(val.trim());
          setSearchResults(results || []);
        } catch (e) {
          console.warn('Steam search failed:', e);
        } finally {
          setIsSearchingSteam(false);
        }
      }, 300);
    } else {
      setSearchResults([]);
      setIsSearchingSteam(false);
    }
  };

  const handleSelectGame = (item: SteamStoreItem) => {
    setSteamAppId(item.id.toString());
    setSteamTitle(item.name);
    setSteamSearchQuery(item.name);
    setSearchResults([]);
    fetchSteamGameDetails(item.id);
  };

  // Initialize defaults on open
  useEffect(() => {
    if (isOpen) {
      setInspectError(null);
      setSubmitError(null);
      if (presetManifestUrl) {
        setManifestPathOrUrl(presetManifestUrl);
      }
      if (presetGame) {
        setCustomAppName(presetGame.app_name);
        setCustomTitle(presetGame.title);
        if (presetGame.platform === 'steam') {
          setInstallerTab('steam');
          setSteamAppId(presetGame.steam_app_id?.toString() || '');
          setSteamTitle(presetGame.title);
          if (presetGame.steam_app_id) {
            fetchSteamGameDetails(presetGame.steam_app_id);
          }
        } else {
          setInstallerTab('epic');
        }
      } else {
        setCustomAppName('');
        setCustomTitle(presetTitle || '');
        setSteamAppId('');
        setSteamTitle('');
        setSteamSearchQuery('');
        setSteamDetails(null);
        setSearchResults([]);
      }

      // Load Steam auth status
      steamGetAuthStatus().then((s) => setSteamAuth(s)).catch(() => {});

      // Load default install directory from settings
      settingsGet()
        .then((s) => {
          if (s?.default_install_dir) {
            defaultDirRef.current = s.default_install_dir;
            const folder = presetGame?.title ? presetGame.title.replace(/[^a-zA-Z0-9 -]/g, '_') : '';
            setInstallDir(folder ? `${s.default_install_dir}\\${folder}` : s.default_install_dir);
          }
        })
        .catch(() => {});

      return () => {};
    }
  }, [isOpen, presetGame, presetManifestUrl]);

  // Refresh the Steam session panel after the shell's sign-in modal succeeds
  useEffect(() => {
    if (!isOpen) return;
    steamGetAuthStatus().then((s) => setSteamAuth(s)).catch(() => {});
  }, [isOpen, steamAuthVersion]);

  const handleBrowseFile = async () => {
    try {
      const selected = await selectManifestFile();
      if (selected) {
        setManifestPathOrUrl(selected);
        triggerInspect(selected);
      }
    } catch (e: any) {
      setInspectError(e?.message || 'Failed to open file picker');
    }
  };

  const handleBrowseInstallDir = async () => {
    try {
      const selected = await selectDirectory(installDir || 'D:\\Games');
      if (selected) {
        const folder = (customTitle || steamTitle) ? (customTitle || steamTitle).replace(/[^a-zA-Z0-9 -]/g, '_') : '';
        setInstallDir(folder ? `${selected}\\${folder}` : selected);
      }
    } catch (e: any) {
      setInspectError(e?.message || 'Failed to open directory picker');
    }
  };

  const triggerInspect = async (pathOrUrl?: string) => {
    const target = (pathOrUrl ?? manifestPathOrUrl).trim();
    if (!target) {
      setInspectError('Please provide a valid manifest path or URL');
      return;
    }

    setIsInspecting(true);
    setInspectError(null);
    setSubmitError(null);

    try {
      const info = await manifestInspect(target);
      setManifestInfo(info);

      if (info.app_name && !customAppName) {
        setCustomAppName(info.app_name);
      }
      if (presetTitle && !presetGame) {
        setCustomTitle(presetTitle);
      } else if (info.matched_game_title && !customTitle) {
        setCustomTitle(info.matched_game_title);
      } else if (!customTitle && info.app_name) {
        setCustomTitle(info.app_name);
      }

      if (info.suggested_base_url && !baseUrl) {
        setBaseUrl(info.suggested_base_url);
      }

      // Give the game its own folder under the library root instead of
      // installing straight into <root> itself.
      const gameTitle = (
        (!presetGame && presetTitle) || customTitle || info.matched_game_title || info.app_name || ''
      ).trim();
      const folder = gameTitle.replace(/[^a-zA-Z0-9 -]/g, '_').replace(/\s+/g, ' ').trim();
      const root = defaultDirRef.current.replace(/[\\/]+$/, '');
      if (folder && root) {
        setInstallDir((prev) => {
          const trimmed = (prev || '').replace(/[\\/]+$/, '');
          const untouched = !trimmed || trimmed.toLowerCase() === root.toLowerCase();
          return untouched ? `${root}\\${folder}` : prev;
        });
      }
    } catch (e: any) {
      setInspectError(e?.message || String(e));
      setManifestInfo(null);
    } finally {
      setIsInspecting(false);
    }
  };

  // Opened with a manifest already filled in (e.g. the featured build's INSTALL
  // button): verify it straight away so the dialog is ready to install.
  useEffect(() => {
    if (!isOpen || !presetManifestUrl) return;
    triggerInspect(presetManifestUrl);
    // Re-running only matters when a different manifest is pre-filled
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, presetManifestUrl]);

  if (!isOpen) return null;

  const handleInstall = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!manifestPathOrUrl.trim()) {
      setSubmitError('Please specify a manifest file path or direct URL');
      return;
    }

    setIsSubmitting(true);
    setSubmitError(null);

    try {
      await gameInstallCustomManifest({
        manifest_path_or_url: manifestPathOrUrl.trim(),
        app_name: customAppName.trim() || undefined,
        title: customTitle.trim() || undefined,
        base_url: baseUrl.trim() || undefined,
        install_dir: installDir.trim() || undefined,
      });

      const effectiveAppName = customAppName.trim() || manifestInfo?.app_name || 'custom_game';
      if (onInstallStarted) {
        onInstallStarted(effectiveAppName);
      }
      onClose();
    } catch (e: any) {
      setSubmitError(e?.message || String(e));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleSteamInstall = async (e: React.FormEvent) => {
    e.preventDefault();
    const idNum = parseInt(steamAppId.trim(), 10);
    if (isNaN(idNum) || idNum <= 0) {
      setSubmitError('Please provide a valid numeric Steam App ID (e.g. 730, 367520, 1091500)');
      return;
    }

    setIsSubmitting(true);
    setSubmitError(null);

    try {
      const depotNum = steamDepotId.trim() ? parseInt(steamDepotId.trim(), 10) : undefined;
      const manifestStr = steamManifestId.trim() || undefined;

      await steamInstallGame(
        idNum,
        steamTitle.trim() || undefined,
        installDir.trim() || undefined,
        depotNum,
        manifestStr
      );
      if (onInstallStarted) {
        onInstallStarted(`steam_${idNum}`);
      }
      onClose();
    } catch (err: any) {
      setSubmitError(err?.message || String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md animate-fade-in">
      <div className="relative w-full max-w-2xl max-h-[92vh] overflow-y-auto rounded-3xl glass-panel bg-zinc-950 border border-zinc-800 shadow-2xl p-6 sm:p-8 space-y-6">
        {/* Header */}
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="p-3 rounded-2xl bg-orange-500/15 text-orange-400 border border-orange-500/30">
              <Download className="w-6 h-6" />
            </div>
            <div>
              <h2 className="text-xl font-extrabold text-white tracking-tight">
                Direct Game Downloader & Installer
              </h2>
              <p className="text-xs text-zinc-400 mt-0.5">
                Download and install games from <span className="font-semibold text-orange-400">Epic Games</span> or directly from <span className="font-semibold text-sky-400">Steam CDN</span>.
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-xl text-zinc-400 hover:text-white hover:bg-zinc-900 transition cursor-pointer"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tab Switcher: Epic Manifest vs Steam App ID */}
        <div className="flex items-center p-1 bg-zinc-900/90 rounded-2xl border border-zinc-800">
          <button
            type="button"
            onClick={() => setInstallerTab('epic')}
            className={`flex-1 py-2.5 rounded-xl text-xs font-bold transition flex items-center justify-center gap-2 cursor-pointer ${
              installerTab === 'epic'
                ? 'bg-gradient-to-r from-orange-600 to-amber-600 text-white shadow-md shadow-orange-600/30'
                : 'text-zinc-400 hover:text-white'
            }`}
          >
            <FileCode2 className="w-4 h-4" />
            <span>Epic Games (.manifest)</span>
          </button>
          <button
            type="button"
            onClick={() => setInstallerTab('steam')}
            className={`flex-1 py-2.5 rounded-xl text-xs font-bold transition flex items-center justify-center gap-2 cursor-pointer ${
              installerTab === 'steam'
                ? 'bg-gradient-to-r from-sky-600 to-blue-600 text-white shadow-md shadow-sky-600/30'
                : 'text-zinc-400 hover:text-white'
            }`}
          >
            <Download className="w-4 h-4" />
            <span>Steam Game (Search & App ID)</span>
          </button>
        </div>

        {presetGame && (
          <div className="p-3.5 rounded-xl bg-zinc-900/90 border border-orange-500/30 flex items-center justify-between text-xs">
            <span className="text-zinc-300">
              Target Game: <strong className="text-white">{presetGame.title}</strong>{' '}
              <span className="font-mono text-orange-400">({presetGame.app_name})</span>
            </span>
            <span className="text-orange-400 font-semibold flex items-center gap-1.5">
              <CheckCircle2 className="w-4 h-4 text-orange-400" /> Ready for Install
            </span>
          </div>
        )}

        {installerTab === 'steam' ? (
          <form onSubmit={handleSteamInstall} className="space-y-5">
            {/* Search or Enter App ID */}
            <div className="space-y-2">
              <label className="block text-xs font-bold uppercase tracking-wider text-zinc-300">
                Search Steam Store or Enter App ID <span className="text-sky-400">*</span>
              </label>
              <div className="relative">
                <div className="absolute inset-y-0 left-0 pl-3.5 flex items-center pointer-events-none text-zinc-500">
                  {isSearchingSteam ? (
                    <Loader2 className="w-4 h-4 animate-spin text-sky-400" />
                  ) : (
                    <Search className="w-4 h-4" />
                  )}
                </div>
                <input
                  type="text"
                  placeholder="Search by game name (e.g. Portal 2, Cyberpunk) or enter App ID (e.g. 730)..."
                  value={steamSearchQuery}
                  onChange={(e) => handleSteamSearchChange(e.target.value)}
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-xl pl-10 pr-4 py-2.5 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-sky-500 transition shadow-inner"
                />

                {/* Instant Search Dropdown Results */}
                {searchResults.length > 0 && (
                  <div className="absolute left-0 right-0 top-full mt-1.5 max-h-60 overflow-y-auto bg-zinc-900 border border-zinc-800 rounded-2xl shadow-2xl z-50 p-1.5 space-y-1 backdrop-blur-xl">
                    {searchResults.map((item) => (
                      <button
                        key={item.id}
                        type="button"
                        onClick={() => handleSelectGame(item)}
                        className="w-full flex items-center justify-between p-2 rounded-xl hover:bg-zinc-800 text-left transition cursor-pointer group"
                      >
                        <div className="flex items-center gap-3">
                          {item.tiny_image ? (
                            <img
                              src={item.tiny_image}
                              alt={item.name}
                              className="w-16 h-7 object-cover rounded-lg border border-zinc-700/60"
                            />
                          ) : (
                            <div className="w-16 h-7 rounded-lg bg-zinc-800 flex items-center justify-center">
                              <Gamepad2 className="w-4 h-4 text-zinc-500" />
                            </div>
                          )}
                          <div>
                            <div className="text-xs font-bold text-white group-hover:text-sky-400 transition">
                              {item.name}
                            </div>
                            <div className="text-[10px] text-zinc-400 font-mono">
                              AppID: {item.id}
                            </div>
                          </div>
                        </div>
                        {item.price && (
                          <span className="text-[11px] font-semibold text-emerald-400 bg-emerald-950/60 border border-emerald-800/40 px-2 py-0.5 rounded-lg">
                            {item.price}
                          </span>
                        )}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Popular Games Quick Picks */}
              <div className="flex flex-wrap items-center gap-1.5 pt-1">
                <span className="text-[11px] font-semibold text-zinc-400 flex items-center gap-1">
                  <Sparkles className="w-3 h-3 text-sky-400" /> Quick Picks:
                </span>
                {POPULAR_STEAM_GAMES.map((game) => (
                  <button
                    key={game.id}
                    type="button"
                    onClick={() => {
                      setSteamAppId(game.id.toString());
                      setSteamTitle(game.name);
                      setSteamSearchQuery(game.name);
                      fetchSteamGameDetails(game.id);
                    }}
                    className={`px-2.5 py-1 rounded-lg text-[10px] font-semibold transition cursor-pointer border ${
                      steamAppId === game.id.toString()
                        ? 'bg-sky-500/20 text-sky-300 border-sky-500/50 shadow-sm'
                        : 'bg-zinc-900/90 text-zinc-400 border-zinc-800 hover:text-white hover:border-zinc-700'
                    }`}
                  >
                    {game.name}
                  </button>
                ))}
              </div>
            </div>

            {/* Selected Game Preview Card */}
            {(steamAppId || steamDetails) && (
              <div className="p-4 rounded-2xl bg-zinc-900/80 border border-sky-500/30 flex gap-4 items-center">
                <img
                  src={
                    steamDetails?.header_image ||
                    `https://cdn.cloudflare.steamstatic.com/steam/apps/${steamAppId}/header.jpg`
                  }
                  alt={steamTitle || 'Steam Game'}
                  className="w-36 h-20 object-cover rounded-xl border border-zinc-800 shadow-md shrink-0 bg-zinc-950"
                  onError={(e) => {
                    (e.target as HTMLImageElement).src =
                      'https://cdn.cloudflare.steamstatic.com/steam/apps/' + steamAppId + '/capsule_231x87.jpg';
                  }}
                />
                <div className="space-y-1 min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="px-2 py-0.5 rounded-full text-[10px] font-bold bg-sky-950/80 text-sky-400 border border-sky-500/40">
                      Steam AppID: {steamAppId}
                    </span>
                    {isLoadingDetails && (
                      <Loader2 className="w-3 h-3 animate-spin text-zinc-400" />
                    )}
                  </div>
                  <h3 className="font-extrabold text-sm text-white truncate">
                    {steamTitle || steamDetails?.name || `Steam Game ${steamAppId}`}
                  </h3>
                  <p className="text-[11px] text-zinc-400 line-clamp-2 leading-relaxed">
                    {steamDetails?.short_description ||
                      'SteamCMD will download and verify game files directly from Valve CDN into your specified installation folder.'}
                  </p>
                </div>
              </div>
            )}

            {/* Target Install Folder */}
            <div className="space-y-2">
              <label className="block text-xs font-bold uppercase tracking-wider text-zinc-300">
                Installation Directory
              </label>
              <div className="flex gap-2">
                <input
                  type="text"
                  placeholder="D:\Games\SteamApps"
                  value={installDir}
                  onChange={(e) => setInstallDir(e.target.value)}
                  className="flex-1 bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2 text-xs text-white placeholder-zinc-500 font-mono focus:outline-none focus:border-sky-500 transition"
                />
                <button
                  type="button"
                  onClick={handleBrowseInstallDir}
                  className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-200 text-xs font-semibold border border-zinc-800 transition cursor-pointer"
                >
                  <FolderOpen className="w-4 h-4 text-sky-400" />
                  <span>Browse...</span>
                </button>
              </div>

              {/* Quick Drive Presets */}
              <div className="flex items-center gap-2 pt-0.5">
                <span className="text-[10px] text-zinc-500">Root Drive:</span>
                {['C:\\Games', 'D:\\Games', 'E:\\Games'].map((drive) => {
                  const folder = steamTitle ? steamTitle.replace(/[^a-zA-Z0-9 -]/g, '_') : '';
                  const target = folder ? `${drive}\\${folder}` : drive;
                  return (
                    <button
                      key={drive}
                      type="button"
                      onClick={() => setInstallDir(target)}
                      className="px-2.5 py-0.5 rounded-lg text-[10px] font-mono font-bold bg-zinc-900 hover:bg-zinc-800 text-zinc-300 border border-zinc-800 hover:border-sky-500/40 transition cursor-pointer"
                    >
                      {drive}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Steam session status */}
            {steamAuth?.logged_in ? (
              <div className="p-4 rounded-2xl bg-zinc-900/60 border border-sky-500/30 space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2.5">
                    <div className="w-8 h-8 rounded-lg bg-sky-500/20 border border-sky-500/40 flex items-center justify-center">
                      <CheckCircle2 className="w-4 h-4 text-sky-400" />
                    </div>
                    <div>
                      <div className="text-xs font-bold text-white flex items-center gap-2">
                        <span>Steam Account: {steamAuth.account_name || 'Active User'}</span>
                        <span className="px-1.5 py-0.5 rounded text-[9px] font-mono bg-sky-500/20 text-sky-300 border border-sky-500/30">
                          SIGNED IN
                        </span>
                      </div>
                      <div className="text-[10px] text-zinc-400 font-mono mt-0.5">
                        {steamAuth.steam_id ? `SteamID: ${steamAuth.steam_id} • ` : ''}
                        Downloads authenticate automatically
                      </div>
                    </div>
                  </div>

                  <button
                    type="button"
                    onClick={() => onRequestSteamLogin?.()}
                    className="px-2.5 py-1 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-[11px] font-medium text-zinc-300 border border-zinc-700 transition cursor-pointer"
                    title="Sign in to Steam with a different account"
                  >
                    Steam Login
                  </button>
                </div>
              </div>
            ) : (
              <div className="p-4 rounded-2xl bg-zinc-900/60 border border-zinc-800 space-y-2.5">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="text-xs font-bold text-zinc-300 flex items-center gap-1.5">
                      <Info className="w-3.5 h-3.5 text-zinc-400" />
                      <span>Steam Session: Anonymous Mode</span>
                    </div>
                    <div className="text-[11px] text-zinc-500 mt-0.5">
                      Free games download anonymously. For paid/owned games, sign in with Steam.
                    </div>
                  </div>

                  <button
                    type="button"
                    onClick={() => onRequestSteamLogin?.()}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-gradient-to-r from-sky-600 to-blue-600 hover:from-sky-500 hover:to-blue-500 text-white font-bold text-xs shadow-md shadow-sky-600/20 transition cursor-pointer"
                  >
                    <KeyRound className="w-3.5 h-3.5" />
                    <span>Sign In with Steam</span>
                  </button>
                </div>
              </div>
            )}

            {/* Advanced Depot & Manifest Options */}
            <div className="border border-zinc-800/80 rounded-2xl p-3.5 bg-zinc-900/40">
              <button
                type="button"
                onClick={() => setShowAdvancedSteam(!showAdvancedSteam)}
                className="w-full flex items-center justify-between text-xs font-bold text-zinc-300 hover:text-white cursor-pointer"
              >
                <div className="flex items-center gap-2">
                  <FileCode2 className="w-4 h-4 text-sky-400" />
                  <span>Depot & Manifest Options (Optional)</span>
                </div>
                {showAdvancedSteam ? <ChevronUp className="w-4 h-4 text-zinc-500" /> : <ChevronDown className="w-4 h-4 text-zinc-500" />}
              </button>

              {showAdvancedSteam && (
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-3 mt-3 border-t border-zinc-800/60">
                  <div className="space-y-1">
                    <label className="text-[11px] font-semibold text-zinc-400">Depot ID</label>
                    <input
                      type="text"
                      placeholder="e.g. 367521"
                      value={steamDepotId}
                      onChange={(e) => setSteamDepotId(e.target.value)}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-xl px-3.5 py-1.5 text-xs text-white placeholder-zinc-600 font-mono focus:outline-none focus:border-sky-500 transition"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-[11px] font-semibold text-zinc-400">Manifest ID</label>
                    <input
                      type="text"
                      placeholder="e.g. 73652819401..."
                      value={steamManifestId}
                      onChange={(e) => setSteamManifestId(e.target.value)}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-xl px-3.5 py-1.5 text-xs text-white placeholder-zinc-600 font-mono focus:outline-none focus:border-sky-500 transition"
                    />
                  </div>
                </div>
              )}
            </div>

            {/* Guidance banner */}
            <div className="p-3.5 rounded-2xl bg-sky-950/40 border border-sky-500/25 text-xs text-sky-200/90 flex items-start gap-2.5">
              <Sparkles className="w-4 h-4 text-sky-400 shrink-0 mt-0.5" />
              <div className="leading-relaxed">
                <strong>In-Launcher Steam Engine:</strong> Game manifests and chunk files are downloaded directly by Hyperion into your chosen folder without launching the external Steam client app.
              </div>
            </div>

            {/* Submit Error */}
            {submitError && (
              <div className="p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
                <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
                <span>{submitError}</span>
              </div>
            )}

            {/* Action Buttons */}
            <div className="flex items-center justify-end gap-3 pt-4 border-t border-zinc-800/80">
              <button
                type="button"
                onClick={onClose}
                className="px-4 py-2.5 rounded-xl text-xs font-bold text-zinc-400 hover:text-white hover:bg-zinc-900 transition cursor-pointer"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={isSubmitting || !steamAppId.trim()}
                className="flex items-center gap-2 px-6 py-2.5 rounded-xl bg-gradient-to-r from-sky-600 to-blue-600 hover:from-sky-500 hover:to-blue-500 disabled:opacity-50 text-white font-extrabold text-xs shadow-lg shadow-sky-600/30 transition hover:scale-102 cursor-pointer"
                title="Download directly in Hyperion"
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    <span>Preparing Download...</span>
                  </>
                ) : (
                  <>
                    <Download className="w-4 h-4" />
                    <span>Download in Hyperion</span>
                  </>
                )}
              </button>
            </div>
          </form>
        ) : (

          <form onSubmit={handleInstall} className="space-y-5">
            {/* Manifest Path or URL */}
          <div className="space-y-2">
            <label className="block text-xs font-bold uppercase tracking-wider text-zinc-300">
              Manifest File Path or URL <span className="text-orange-500">*</span>
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                placeholder="C:\Downloads\version.manifest or https://.../...manifest"
                value={manifestPathOrUrl}
                onChange={(e) => setManifestPathOrUrl(e.target.value)}
                className="flex-1 bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-xs text-white placeholder-zinc-500 font-mono focus:outline-none focus:border-orange-500 transition"
                required
              />
              <button
                type="button"
                onClick={handleBrowseFile}
                className="flex items-center gap-1.5 px-3.5 py-2.5 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-200 text-xs font-semibold border border-zinc-800 transition cursor-pointer"
                title="Browse local .manifest file"
              >
                <FolderOpen className="w-4 h-4 text-orange-400" />
                <span>Browse...</span>
              </button>
              <button
                type="button"
                onClick={() => triggerInspect()}
                disabled={isInspecting || !manifestPathOrUrl.trim()}
                className="flex items-center gap-1.5 px-3.5 py-2.5 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 disabled:opacity-50 text-white text-xs font-bold transition cursor-pointer shadow-md shadow-orange-600/20 glow-orange"
                title="Inspect manifest binary"
              >
                {isInspecting ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Search className="w-4 h-4" />
                )}
                <span>Inspect</span>
              </button>
            </div>
            <p className="text-[11px] text-zinc-400">
              You can paste a direct download URL or pick a local <span className="font-mono text-zinc-300">.manifest</span> file extracted from Epic archives or previous versions.
            </p>
          </div>

          {/* Inspect Error */}
          {inspectError && (
            <div className="p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
              <span>{inspectError}</span>
            </div>
          )}

          {/* Manifest Inspection Preview Card */}
          {manifestInfo && (
            <div className="p-4 rounded-2xl bg-gradient-to-br from-orange-950/20 via-zinc-900/60 to-zinc-950/90 border border-orange-500/30 space-y-3">
              <div className="flex items-center justify-between border-b border-orange-500/20 pb-2.5">
                <div className="flex items-center gap-2">
                  <span className="w-2 h-2 rounded-full bg-orange-400 animate-pulse" />
                  <span className="text-xs font-bold uppercase tracking-wider text-orange-400">
                    Manifest Verified
                  </span>
                </div>
                <span className="text-xs font-mono text-zinc-200 bg-zinc-900 px-2.5 py-0.5 rounded-md border border-zinc-800">
                  {manifestInfo.build_version || 'Unknown Version'}
                </span>
              </div>

              <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs">
                <div className="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800/80">
                  <div className="text-zinc-400 flex items-center gap-1.5 mb-1">
                    <Cpu className="w-3.5 h-3.5 text-orange-400" />
                    <span>App Name</span>
                  </div>
                  <div className="font-mono text-white font-bold truncate">
                    {manifestInfo.app_name || 'N/A'}
                  </div>
                </div>

                <div className="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800/80">
                  <div className="text-zinc-400 flex items-center gap-1.5 mb-1">
                    <HardDrive className="w-3.5 h-3.5 text-orange-400" />
                    <span>Install Size</span>
                  </div>
                  <div className="font-mono text-white font-bold">
                    {formatBytes(manifestInfo.total_size)}
                  </div>
                </div>

                <div className="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800/80">
                  <div className="text-zinc-400 flex items-center gap-1.5 mb-1">
                    <Layers className="w-3.5 h-3.5 text-orange-400" />
                    <span>Total Files</span>
                  </div>
                  <div className="font-mono text-white font-bold">
                    {manifestInfo.file_count.toLocaleString()}
                  </div>
                </div>

                <div className="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800/80">
                  <div className="text-zinc-400 flex items-center gap-1.5 mb-1">
                    <FileCode2 className="w-3.5 h-3.5 text-orange-400" />
                    <span>Chunks</span>
                  </div>
                  <div className="font-mono text-white font-bold">
                    {manifestInfo.chunk_count.toLocaleString()}
                  </div>
                </div>
              </div>

              {manifestInfo.launch_exe && (
                <div className="text-[11px] text-zinc-300 font-mono bg-black/60 p-2.5 rounded-xl border border-zinc-800 flex items-center gap-2">
                  <span className="text-zinc-500 font-sans">Launch Executable:</span>
                  <span className="text-orange-400">{manifestInfo.launch_exe}</span>
                </div>
              )}
            </div>
          )}

          {/* Advanced / Overrides */}
          <div className="space-y-4 pt-2 border-t border-zinc-800/80">
            {/* CDN Base URL */}
            <div className="space-y-1.5">
              <label className="flex items-center gap-2 text-xs font-semibold text-zinc-300">
                <Globe className="w-3.5 h-3.5 text-orange-400" />
                <span>CDN Chunk Base URL</span>
                <span className="text-zinc-500 text-[10px] lowercase font-normal">
                  (auto-detected or override)
                </span>
              </label>
              <input
                type="text"
                placeholder="https://egdownload.fastly-edge.com/Builds/Org/.../default"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2 text-xs text-white placeholder-zinc-500 font-mono focus:outline-none focus:border-orange-500 transition"
              />
              <p className="text-[10px] text-zinc-400">
                The CDN root directory hosting chunk data. If blank, it will be automatically fetched from your owned game catalog.
              </p>
            </div>

            {/* Install Directory */}
            <div className="space-y-2">
              <label className="flex items-center gap-2 text-xs font-semibold text-zinc-300">
                <FolderOpen className="w-3.5 h-3.5 text-orange-400" />
                <span>Installation Directory</span>
              </label>
              <div className="flex gap-2">
                <input
                  type="text"
                  placeholder="D:\Games\CustomGame"
                  value={installDir}
                  onChange={(e) => setInstallDir(e.target.value)}
                  className="flex-1 bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2 text-xs text-white placeholder-zinc-500 font-mono focus:outline-none focus:border-orange-500 transition"
                />
                <button
                  type="button"
                  onClick={handleBrowseInstallDir}
                  className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-200 text-xs font-semibold border border-zinc-800 transition cursor-pointer"
                  title="Browse target folder"
                >
                  <FolderOpen className="w-4 h-4 text-orange-400" />
                  <span>Browse...</span>
                </button>
              </div>

              {/* Quick Drive Presets */}
              <div className="flex items-center gap-2 pt-0.5">
                <span className="text-[10px] text-zinc-500">Root Drive:</span>
                {['C:\\Games', 'D:\\Games', 'E:\\Games'].map((drive) => {
                  const folder = customTitle ? customTitle.replace(/[^a-zA-Z0-9 -]/g, '_') : '';
                  const target = folder ? `${drive}\\${folder}` : drive;
                  return (
                    <button
                      key={drive}
                      type="button"
                      onClick={() => setInstallDir(target)}
                      className="px-2.5 py-0.5 rounded-lg text-[10px] font-mono font-bold bg-zinc-900 hover:bg-zinc-800 text-zinc-300 border border-zinc-800 hover:border-orange-500/40 transition cursor-pointer"
                    >
                      {drive}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Name / Title */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div className="space-y-1">
                <label className="text-[11px] font-semibold text-zinc-300">
                  Game Title (Display)
                </label>
                <input
                  type="text"
                  placeholder="e.g. Fortnite (Version 1.8)"
                  value={customTitle}
                  onChange={(e) => setCustomTitle(e.target.value)}
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-3.5 py-1.5 text-xs text-white focus:outline-none focus:border-orange-500 transition"
                />
              </div>

              <div className="space-y-1">
                <label className="text-[11px] font-semibold text-zinc-300">
                  App Name (Internal)
                </label>
                <input
                  type="text"
                  placeholder="e.g. Fortnite, Jackal, etc."
                  value={customAppName}
                  onChange={(e) => setCustomAppName(e.target.value)}
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-3.5 py-1.5 text-xs text-white font-mono focus:outline-none focus:border-orange-500 transition"
                />
              </div>
            </div>
          </div>

          {/* Submit Error */}
          {submitError && (
            <div className="p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
              <span>{submitError}</span>
            </div>
          )}

          {/* Action Footer */}
          <div className="flex items-center justify-end gap-3 pt-3 border-t border-zinc-800">
            <button
              type="button"
              onClick={onClose}
              disabled={isSubmitting}
              className="px-5 py-2.5 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-300 border border-zinc-800 text-xs font-semibold transition cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting || !manifestPathOrUrl.trim()}
              className="flex items-center gap-2 px-6 py-2.5 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 text-white text-xs font-extrabold transition hover:scale-105 shadow-xl shadow-orange-600/30 disabled:opacity-50 disabled:scale-100 cursor-pointer glow-orange"
            >
              {isSubmitting ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  <span>Preparing Download...</span>
                </>
              ) : (
                <>
                  <Download className="w-4 h-4" />
                  <span>Download & Install Version</span>
                </>
              )}
            </button>
          </div>
        </form>
      )}
      </div>
    </div>
  );
};
