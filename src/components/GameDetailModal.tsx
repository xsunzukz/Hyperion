import React, { useEffect, useState } from 'react';
import {
  X,
  Play,
  Download,
  FolderOpen,
  CheckCircle,
  AlertTriangle,
  Trash2,
  Cloud,
  FileCheck2,
  HardDrive,
  FileCode2,
  Package,
  Square,
} from 'lucide-react';
import { DownloadProgress, GameItem, SteamExtraContent, VerificationResult } from '../types';
import { openPath, gameVerify, cloudSaveSync, steamExtraContent } from '../lib/ipc';

interface GameDetailModalProps {
  game: GameItem;
  onClose: () => void;
  onLaunch: (game: GameItem, offline: boolean) => void;
  onInstall: (game: GameItem) => void;
  onUninstall: (game: GameItem) => void;
  onStop?: (game: GameItem) => void;
  isRunning?: boolean;
  downloadProgress?: DownloadProgress;
  onOpenCustomManifest?: (game: GameItem) => void;
}

export const GameDetailModal: React.FC<GameDetailModalProps> = ({
  game,
  onClose,
  onLaunch,
  onInstall,
  onUninstall,
  onStop,
  isRunning,
  downloadProgress,
  onOpenCustomManifest,
}) => {
  const [isVerifying, setIsVerifying] = useState(false);
  const [verifyResult, setVerifyResult] = useState<VerificationResult | null>(null);
  const [isSyncingSave, setIsSyncingSave] = useState(false);
  const [saveSyncMessage, setSaveSyncMessage] = useState<string | null>(null);
  const [confirmUninstall, setConfirmUninstall] = useState(false);
  const [offlineMode, setOfflineMode] = useState(false);
  const [extraContent, setExtraContent] = useState<SteamExtraContent[]>([]);

  // Packages SteamCMD pulled in alongside the game (redistributables, runtimes…)
  useEffect(() => {
    let cancelled = false;
    setExtraContent([]);

    if (game.platform !== 'steam' || !game.is_installed) {
      return;
    }

    steamExtraContent(game.app_name)
      .then((extras) => {
        if (!cancelled) setExtraContent(extras);
      })
      .catch(() => {
        /* extras are informational only */
      });

    return () => {
      cancelled = true;
    };
  }, [game.app_name, game.platform, game.is_installed]);

  const isDownloading =
    downloadProgress &&
    (downloadProgress.status === 'Downloading' || downloadProgress.status === 'Analyzing');

  const defaultBanner =
    'https://images.unsplash.com/photo-1542751371-adc38448a05e?auto=format&fit=crop&w=1200&q=80';
  const banner = game.banner_url || game.icon_url || defaultBanner;

  const handleVerify = async () => {
    setIsVerifying(true);
    setVerifyResult(null);
    try {
      const res = await gameVerify(game.app_name);
      setVerifyResult(res);
    } catch (e) {
      console.error('Verify error:', e);
    } finally {
      setIsVerifying(false);
    }
  };

  const handleSyncSave = async () => {
    setIsSyncingSave(true);
    setSaveSyncMessage(null);
    try {
      const msg = await cloudSaveSync(game.app_name);
      setSaveSyncMessage(msg);
    } catch (e) {
      setSaveSyncMessage('Cloud save sync failed');
    } finally {
      setIsSyncingSave(false);
    }
  };

  const handleOpenFolder = () => {
    if (game.install_path) {
      openPath(game.install_path);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md animate-fade-in">
      <div className="relative w-full max-w-3xl max-h-[90vh] overflow-y-auto rounded-3xl glass-panel bg-[#08080b] border border-zinc-800 shadow-2xl">
        {/* Banner with close button */}
        <div className="relative h-64 w-full overflow-hidden bg-black">
          <img src={banner} alt={game.title} className="w-full h-full object-cover" />
          <div className="absolute inset-0 bg-gradient-to-t from-[#08080b] via-[#08080b]/50 to-transparent" />
          <button
            onClick={onClose}
            className="absolute top-4 right-4 p-2 rounded-full bg-black/70 text-zinc-400 hover:text-white hover:bg-black transition backdrop-blur-sm cursor-pointer"
          >
            <X className="w-5 h-5" />
          </button>

          {/* Banner bottom title info */}
          <div className="absolute bottom-6 left-8 right-8">
            <div className="flex items-center gap-2 mb-2">
              {game.platform === 'steam' ? (
                <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-bold bg-sky-950/90 text-sky-400 border border-sky-500/40 backdrop-blur-md shadow-sm">
                  Steam Library
                </span>
              ) : (
                <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-bold bg-orange-950/90 text-orange-400 border border-orange-500/40 backdrop-blur-md shadow-sm">
                  Epic Games
                </span>
              )}
              {game.is_installed && (
                <span className="px-2.5 py-1 rounded-lg text-xs font-bold bg-zinc-900/90 text-white border border-zinc-700/60 backdrop-blur-md shadow-sm">
                  Installed
                </span>
              )}
            </div>
            <h1 className="text-2xl md:text-3xl font-extrabold text-white tracking-tight drop-shadow-md">
              {game.title}
            </h1>
            <p className="text-xs text-zinc-300 font-medium mt-1">
              {[game.developer, game.publisher].filter(Boolean).join(' | ') ||
                (game.platform === 'steam' ? 'Steam Game' : 'Epic Games Store')}
            </p>
          </div>
        </div>

        {/* Content Body */}
        <div className="p-8 space-y-6">
          {/* Main Action Bar */}
          <div className="flex flex-wrap items-center justify-between gap-4 p-4 rounded-2xl bg-zinc-950 border border-zinc-800">
            <div className="flex items-center gap-3">
              {game.is_installed ? (
                <>
                  {isRunning ? (
                    <button
                      onClick={() => onStop?.(game)}
                      className="flex items-center gap-2.5 px-6 py-3 rounded-xl bg-rose-600 hover:bg-rose-500 text-white font-extrabold text-xs shadow-lg shadow-rose-600/30 transition hover:scale-102 cursor-pointer"
                    >
                      <Square className="w-4 h-4 fill-current" />
                      <span>STOP GAME</span>
                    </button>
                  ) : (
                    <button
                      onClick={() => onLaunch(game, offlineMode)}
                      className="flex items-center gap-2.5 px-6 py-3 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 text-white font-extrabold text-xs shadow-lg shadow-orange-600/30 transition hover:scale-102 cursor-pointer glow-orange-sm"
                    >
                      <Play className="w-4 h-4 fill-current" />
                      <span>LAUNCH GAME</span>
                    </button>
                  )}

                  {isRunning ? (
                    <span className="flex items-center gap-1.5 text-xs font-bold text-emerald-400">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                      Running
                    </span>
                  ) : (
                    <label className="flex items-center gap-2 text-xs font-semibold text-zinc-300 cursor-pointer ml-2">
                      <input
                        type="checkbox"
                        checked={offlineMode}
                        onChange={(e) => setOfflineMode(e.target.checked)}
                        className="rounded bg-zinc-900 border-zinc-700 text-orange-600 focus:ring-orange-500 accent-orange-500"
                      />
                      <span>Offline Mode</span>
                    </label>
                  )}
                </>
              ) : isDownloading ? (
                <div className="space-y-1.5 w-64">
                  <div className="flex justify-between text-xs font-semibold text-orange-400">
                    <span>Downloading {downloadProgress.progress_percent.toFixed(1)}%</span>
                    <span>{(downloadProgress.speed_bytes_per_sec / (1024 * 1024)).toFixed(1)} MB/s</span>
                  </div>
                  <div className="w-full h-2 rounded-full bg-zinc-900 overflow-hidden">
                    <div
                      className="h-full bg-gradient-to-r from-orange-600 to-amber-500 rounded-full"
                      style={{ width: `${downloadProgress.progress_percent}%` }}
                    />
                  </div>
                </div>
              ) : (
                <div className="flex items-center gap-3">
                  {game.platform === 'steam' ? (
                    <>
                      <button
                        onClick={() => onInstall(game)}
                        className="flex items-center gap-2 px-5 py-3 rounded-xl bg-gradient-to-r from-sky-500 to-blue-600 hover:from-sky-400 hover:to-blue-500 text-white font-extrabold text-xs shadow-lg shadow-sky-600/30 transition hover:scale-102 cursor-pointer"
                        title="Steam installs the game with your signed-in account; progress mirrors here"
                      >
                        <svg role="img" viewBox="0 0 24 24" fill="currentColor" className="w-4 h-4" xmlns="http://www.w3.org/2000/svg">
                          <path d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.005.105.005.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 12-5.373 12-12s-5.373-12-12-12zM7.556 16.41c-.347.834-1.218 1.34-2.158 1.185-.94-.155-1.635-.909-1.635-1.864 0-.154.019-.304.055-.447l3.96 1.638c-.066-.17-.145-.337-.222-.512zm4.364-7.5c0-1.748 1.422-3.17 3.17-3.17 1.747 0 3.17 1.422 3.17 3.17 0 1.748-1.423 3.17-3.17 3.17-1.748 0-3.17-1.422-3.17-3.17zm3.17-1.811c-.997 0-1.811.814-1.811 1.811s.814 1.811 1.811 1.811 1.811-.814 1.811-1.811-.814-1.811-1.811-1.811z" />
                        </svg>
                        <span>INSTALL GAME</span>
                      </button>

                      <button
                        onClick={() => onOpenCustomManifest?.(game)}
                        className="flex items-center gap-1.5 px-3 py-3 rounded-xl bg-zinc-950 hover:bg-zinc-900 text-zinc-400 hover:text-zinc-200 font-medium text-xs border border-zinc-800 transition cursor-pointer"
                        title="Configure download directory or custom depot manifest"
                      >
                        <FileCode2 className="w-3.5 h-3.5" />
                        <span>Options</span>
                      </button>
                    </>
                  ) : (
                    <>
                      <button
                        onClick={() => onInstall(game)}
                        className="flex items-center gap-2.5 px-6 py-3 rounded-xl bg-white hover:bg-zinc-200 text-black font-extrabold text-xs shadow-lg shadow-white/20 transition hover:scale-102 cursor-pointer"
                      >
                        <Download className="w-4 h-4" />
                        <span>INSTALL NOW</span>
                      </button>

                      <button
                        onClick={() => onOpenCustomManifest?.(game)}
                        className="flex items-center gap-2 px-4 py-3 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-white font-bold text-xs border border-orange-500/40 hover:border-orange-500 transition cursor-pointer shadow-sm hover:shadow-orange-500/20"
                        title="Install any specific game build version using a .manifest"
                      >
                        <FileCode2 className="w-4 h-4 text-orange-400" />
                        <span>Install from .manifest</span>
                      </button>
                    </>
                  )}
                </div>
              )}
            </div>

            {/* Quick stats pill */}
            <div className="flex items-center gap-3 text-xs font-medium text-zinc-400">
              <span className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-zinc-900 border border-zinc-800 font-mono text-zinc-200">
                <HardDrive className="w-3.5 h-3.5 text-orange-400" />
                {game.install_size
                  ? `${(game.install_size / (1024 * 1024 * 1024)).toFixed(1)} GB`
                  : 'N/A'}
              </span>
              <span className="px-3 py-1.5 rounded-lg bg-zinc-900 border border-zinc-800 font-mono text-zinc-300">
                {game.can_run_offline ? 'Offline Ready' : 'Online Only'}
              </span>
            </div>
          </div>

          {/* Description */}
          {game.description && (
            <div>
              <h3 className="text-xs font-bold uppercase tracking-wider text-zinc-400 mb-2">About</h3>
              <p className="text-sm text-zinc-300 leading-relaxed">{game.description}</p>
            </div>
          )}

          {/* Metadata Grid */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 p-4 rounded-2xl bg-zinc-950 border border-zinc-800 text-xs">
            <div>
              <div className="text-zinc-500 font-medium">Platform</div>
              <div className="font-semibold text-white mt-0.5 capitalize">
                {game.platform === 'steam' ? 'Steam' : 'Epic Games'}
              </div>
            </div>
            <div>
              <div className="text-zinc-500 font-medium">Identifier</div>
              <div className="font-mono text-zinc-300 mt-0.5 truncate">
                {game.steam_app_id ? `AppID ${game.steam_app_id}` : game.catalog_item_id || game.app_name}
              </div>
            </div>
            <div>
              <div className="text-zinc-500 font-medium">Status</div>
              <div className="font-mono text-orange-400 font-bold mt-0.5">
                {game.is_installed ? 'Installed' : 'In Library'}
              </div>
            </div>
            <div>
              <div className="text-zinc-500 font-medium">Storage Size</div>
              <div className="font-mono text-zinc-300 mt-0.5 truncate">
                {game.install_size ? `${(game.install_size / (1024 * 1024 * 1024)).toFixed(1)} GB` : 'N/A'}
              </div>
            </div>
          </div>

          {/* Extra packages SteamCMD installed alongside the game */}
          {game.platform === 'steam' && game.is_installed && extraContent.length > 0 && (
            <div className="p-4 rounded-2xl bg-zinc-950 border border-zinc-800 space-y-2.5">
              <div className="flex items-center gap-2">
                <Package className="w-4 h-4 text-sky-400" />
                <h3 className="text-xs font-bold uppercase tracking-wider text-zinc-400">
                  Additional content installed with this game
                </h3>
              </div>
              <ul className="space-y-1.5">
                {extraContent.map((extra) => (
                  <li
                    key={extra.app_id}
                    className="flex items-center justify-between gap-3 text-xs rounded-xl bg-zinc-900/60 border border-zinc-800/80 px-3 py-2"
                  >
                    <span className="flex items-center gap-2 min-w-0">
                      <span className="font-medium text-zinc-200 truncate">{extra.name}</span>
                      <span className="font-mono text-[10px] text-zinc-500 shrink-0">
                        AppID {extra.app_id}
                      </span>
                      {!extra.installed && (
                        <span className="px-1.5 py-0.5 rounded-md bg-amber-500/15 text-amber-400 border border-amber-500/30 text-[10px] font-bold shrink-0">
                          not downloaded
                        </span>
                      )}
                    </span>
                    <span className="font-mono text-zinc-400 shrink-0">
                      {extra.size_bytes > 0
                        ? extra.size_bytes >= 1024 * 1024 * 1024
                          ? `${(extra.size_bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
                          : `${(extra.size_bytes / (1024 * 1024)).toFixed(0)} MB`
                        : '—'}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {/* Installed Management Operations */}
          {game.is_installed && (
            <div className="space-y-4 pt-4 border-t border-zinc-800">
              <h3 className="text-xs font-bold uppercase tracking-wider text-zinc-400">
                Installation & Maintenance
              </h3>
              <div className="flex flex-wrap gap-3">
                <button
                  onClick={handleOpenFolder}
                  className="flex items-center gap-2 px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-white font-semibold text-xs border border-zinc-800 transition cursor-pointer"
                >
                  <FolderOpen className="w-4 h-4 text-orange-400" />
                  <span>Open Folder</span>
                </button>

                <button
                  onClick={handleVerify}
                  disabled={isVerifying}
                  className="flex items-center gap-2 px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-white font-semibold text-xs border border-zinc-800 transition disabled:opacity-50 cursor-pointer"
                >
                  <FileCheck2 className={`w-4 h-4 text-orange-400 ${isVerifying ? 'animate-spin' : ''}`} />
                  <span>{isVerifying ? 'Verifying Files...' : 'Verify Integrity'}</span>
                </button>

                <button
                  onClick={handleSyncSave}
                  disabled={isSyncingSave}
                  className="flex items-center gap-2 px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-white font-semibold text-xs border border-zinc-800 transition disabled:opacity-50 cursor-pointer"
                >
                  <Cloud className={`w-4 h-4 text-orange-400 ${isSyncingSave ? 'animate-pulse' : ''}`} />
                  <span>{isSyncingSave ? 'Syncing...' : 'Sync Cloud Save'}</span>
                </button>

                <button
                  onClick={() => onOpenCustomManifest?.(game)}
                  className="flex items-center gap-2 px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-orange-400 font-semibold text-xs border border-orange-500/40 hover:border-orange-500 transition cursor-pointer shadow-sm"
                  title="Downgrade or switch game version using a .manifest file or URL"
                >
                  <FileCode2 className="w-4 h-4 text-orange-400" />
                  <span>Change Version (.manifest)</span>
                </button>

                {!confirmUninstall ? (
                  <button
                    onClick={() => setConfirmUninstall(true)}
                    className="flex items-center gap-2 px-4 py-2 rounded-xl bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 font-semibold text-xs border border-rose-500/20 transition cursor-pointer"
                  >
                    <Trash2 className="w-4 h-4" />
                    <span>Uninstall</span>
                  </button>
                ) : (
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => onUninstall(game)}
                      className="px-4 py-2 rounded-xl bg-rose-600 hover:bg-rose-500 text-white font-bold text-xs transition cursor-pointer"
                    >
                      Confirm Delete
                    </button>
                    <button
                      onClick={() => setConfirmUninstall(false)}
                      className="px-3 py-2 rounded-xl bg-zinc-800 text-zinc-300 text-xs hover:bg-zinc-700 transition cursor-pointer"
                    >
                      Cancel
                    </button>
                  </div>
                )}
              </div>

              {/* Verify feedback result */}
              {verifyResult && (
                <div className={`p-4 rounded-xl border text-xs ${
                  verifyResult.is_valid
                    ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-300'
                    : 'bg-amber-500/10 border-amber-500/30 text-amber-300'
                }`}>
                  <div className="flex items-center gap-2 font-bold mb-1">
                    {verifyResult.is_valid ? (
                      <CheckCircle className="w-4 h-4 text-emerald-400" />
                    ) : (
                      <AlertTriangle className="w-4 h-4 text-amber-400" />
                    )}
                    <span>{verifyResult.is_valid ? 'All files verified successfully!' : 'Integrity issues detected.'}</span>
                  </div>
                  <div>
                    {verifyResult.valid_files} of {verifyResult.total_files} files passed SHA1 checksum.
                    {verifyResult.missing_files.length > 0 && ` Missing: ${verifyResult.missing_files.length}.`}
                    {verifyResult.corrupt_files.length > 0 && ` Corrupt: ${verifyResult.corrupt_files.length}.`}
                  </div>
                </div>
              )}

              {/* Cloud save sync message */}
              {saveSyncMessage && (
                <div className="p-3 rounded-xl bg-orange-500/10 border border-orange-500/30 text-orange-300 text-xs flex items-center gap-2">
                  <Cloud className="w-4 h-4 text-orange-400" />
                  <span>{saveSyncMessage}</span>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
