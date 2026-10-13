import React from 'react';
import { Play, Download, HardDrive, CheckCircle2, Square } from 'lucide-react';
import { DownloadProgress, GameItem, ViewMode } from '../types';

interface GameCardProps {
  game: GameItem;
  viewMode: ViewMode;
  onSelect: (game: GameItem) => void;
  onLaunch: (game: GameItem) => void;
  onInstall: (game: GameItem) => void;
  onStop?: (game: GameItem) => void;
  isRunning?: boolean;
  downloadProgress?: DownloadProgress;
}

// Official Epic Games Shield Crest SVG
const EpicGamesIcon: React.FC<{ className?: string }> = ({ className = 'w-3 h-3' }) => (
  <svg role="img" viewBox="0 0 24 24" fill="currentColor" className={className} xmlns="http://www.w3.org/2000/svg">
    <path d="M3.537 0C2.165 0 1.66.506 1.66 1.879V18.44a4.262 4.262 0 00.02.433c.031.3.037.59.316.92.027.033.311.245.311.245.153.075.258.13.43.2l8.335 3.491c.433.199.614.276.928.27h.002c.314.006.495-.071.928-.27l8.335-3.492c.172-.07.277-.124.43-.2 0 0 .284-.211.311-.243.28-.33.285-.621.316-.92a4.261 4.261 0 00.02-.434V1.879c0-1.373-.506-1.88-1.878-1.88zm13.366 3.11h.68c1.138 0 1.688.553 1.688 1.696v1.88h-1.374v-1.8c0-.369-.17-.54-.523-.54h-.235c-.367 0-.537.17-.537.539v5.81c0 .369.17.54.537.54h.262c.353 0 .523-.171.523-.54V8.619h1.373v2.143c0 1.144-.562 1.71-1.7 1.71h-.694c-1.138 0-1.7-.566-1.7-1.71V4.82c0-1.144.562-1.709 1.7-1.709zm-12.186.08h3.114v1.274H6.117v2.603h1.648v1.275H6.117v2.774h1.74v1.275h-3.14zm3.816 0h2.198c1.138 0 1.7.564 1.7 1.708v2.445c0 1.144-.562 1.71-1.7 1.71h-.799v3.338h-1.4zm4.53 0h1.4v9.201h-1.4zm-3.13 1.235v3.392h.575c.354 0 .523-.171.523-.54V4.965c0-.368-.17-.54-.523-.54z" />
  </svg>
);

// Official Steam Crank Logo SVG
const SteamIcon: React.FC<{ className?: string }> = ({ className = 'w-3 h-3' }) => (
  <svg role="img" viewBox="0 0 24 24" fill="currentColor" className={className} xmlns="http://www.w3.org/2000/svg">
    <path d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.005.105.005.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 12-5.373 12-12s-5.373-12-12-12zM8.54 16.903c-.496.486-1.282.492-1.78.012-.501-.482-.519-1.272-.036-1.77.202-.208.471-.328.752-.346l2.021.834c-.114.527-.436.953-.957 1.27zm7.404-5.698c-1.284 0-2.327-1.044-2.327-2.329 0-1.283 1.043-2.328 2.327-2.328 1.285 0 2.33 1.045 2.33 2.328 0 1.285-1.045 2.329-2.33 2.329z" />
  </svg>
);

export const GameCard: React.FC<GameCardProps> = ({
  game,
  viewMode,
  onSelect,
  onLaunch,
  onInstall,
  onStop,
  isRunning,
  downloadProgress,
}) => {
  const isDownloading =
    downloadProgress &&
    (downloadProgress.status === 'Downloading' || downloadProgress.status === 'Analyzing');
  const defaultPoster =
    'https://images.unsplash.com/photo-1550745165-9bc0b252726f?auto=format&fit=crop&w=600&q=80';
  const coverUrl = game.icon_url || game.banner_url || defaultPoster;
  const isSteam = game.platform === 'steam';

  const formatSize = (bytes?: number) => {
    if (!bytes) return '';
    const gb = bytes / (1024 * 1024 * 1024);
    return `${gb.toFixed(1)} GB`;
  };

  if (viewMode === 'list') {
    return (
      <div
        onClick={() => onSelect(game)}
        className="group flex items-center justify-between p-3.5 rounded-2xl glass-card cursor-pointer hover:bg-zinc-900/60 border border-zinc-800/80 hover:border-orange-500/50 transition-all duration-200"
      >
        <div className="flex items-center gap-4 overflow-hidden">
          <img
            src={coverUrl}
            alt={game.title}
            className="w-14 h-18 rounded-xl object-cover shadow-md flex-shrink-0 bg-zinc-950"
            onError={(e) => {
              const target = e.target as HTMLImageElement;
              if (game.banner_url && target.src !== game.banner_url) {
                target.src = game.banner_url;
              } else if (target.src !== defaultPoster) {
                target.src = defaultPoster;
              }
            }}
          />
          <div className="overflow-hidden">
            <div className="flex items-center gap-2">
              <span className="font-bold text-white text-sm group-hover:text-orange-400 transition truncate">
                {game.title}
              </span>
              {isSteam ? (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10px] font-semibold bg-sky-950/60 text-sky-400 border border-sky-500/30 shrink-0">
                  <SteamIcon className="w-2.5 h-2.5 fill-current" /> Steam
                </span>
              ) : (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10px] font-semibold bg-orange-950/60 text-orange-400 border border-orange-500/30 shrink-0">
                  <EpicGamesIcon className="w-2.5 h-2.5 fill-current" /> Epic
                </span>
              )}
            </div>
            <div className="text-xs text-zinc-400 truncate mt-0.5">
              {game.developer || game.publisher || (isSteam ? 'Steam' : 'Epic Games Store')}
            </div>
            <div className="flex items-center gap-3 mt-1.5">
              {isRunning && (
                <span className="flex items-center gap-1 text-[11px] font-bold text-emerald-400">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" /> Running
                </span>
              )}
              {game.is_installed && (
                <span className="flex items-center gap-1 text-[11px] font-bold text-orange-400">
                  <CheckCircle2 className="w-3.5 h-3.5" /> Installed
                </span>
              )}
              {game.install_size && (
                <span className="text-[11px] text-zinc-400 flex items-center gap-1 font-mono">
                  <HardDrive className="w-3 h-3" /> {formatSize(game.install_size)}
                </span>
              )}
            </div>
          </div>
        </div>

        {/* Action Button */}
        <div className="flex items-center gap-3">
          {isDownloading ? (
            <div className="w-36">
              <div className="flex justify-between text-[11px] font-mono text-orange-300 mb-1">
                <span>{downloadProgress.progress_percent.toFixed(0)}%</span>
                <span>{(downloadProgress.speed_bytes_per_sec / (1024 * 1024)).toFixed(1)} MB/s</span>
              </div>
              <div className="w-full h-1.5 rounded-full bg-zinc-900 overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-orange-600 to-amber-500 rounded-full transition-all duration-300"
                  style={{ width: `${downloadProgress.progress_percent}%` }}
                />
              </div>
            </div>
          ) : isRunning ? (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onStop?.(game);
              }}
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-rose-600/90 hover:bg-rose-500 text-white font-bold text-xs shadow-md shadow-rose-600/30 transition hover:scale-102 cursor-pointer"
              title="Stop the running game"
            >
              <Square className="w-3.5 h-3.5 fill-current" />
              <span>STOP</span>
            </button>
          ) : game.is_installed ? (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onLaunch(game);
              }}
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 text-white font-bold text-xs shadow-md shadow-orange-600/30 transition hover:scale-102 cursor-pointer"
            >
              <Play className="w-3.5 h-3.5 fill-current" />
              <span>PLAY</span>
            </button>
          ) : (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onInstall(game);
              }}
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-white hover:bg-zinc-200 text-black font-extrabold text-xs shadow-md shadow-white/10 transition hover:scale-102 cursor-pointer"
            >
              <Download className="w-3.5 h-3.5" />
              <span>INSTALL</span>
            </button>
          )}
        </div>
      </div>
    );
  }

  // Grid Mode Card
  return (
    <div
      onClick={() => onSelect(game)}
      className="group relative flex flex-col rounded-2xl glass-card overflow-hidden cursor-pointer border border-zinc-800/80 hover:border-orange-500/60 transition-all duration-300 hover:-translate-y-1"
    >
      {/* Cover Image Container */}
      <div className="relative aspect-[3/4] w-full overflow-hidden bg-zinc-950">
        <img
          src={coverUrl}
          alt={game.title}
          className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
          loading="lazy"
          onError={(e) => {
            const target = e.target as HTMLImageElement;
            if (game.banner_url && target.src !== game.banner_url) {
              target.src = game.banner_url;
            } else if (target.src !== defaultPoster) {
              target.src = defaultPoster;
            }
          }}
        />

        {/* Gradient Overlay */}
        <div className="absolute inset-0 bg-gradient-to-t from-[#060608] via-transparent to-black/30 opacity-90 group-hover:opacity-60 transition-opacity" />

        {/* Top Badges */}
        <div className="absolute top-3 left-3 right-3 flex items-center justify-between pointer-events-none">
          {isRunning ? (
            <span className="flex items-center gap-1 px-2 py-0.5 rounded-md text-[10px] font-extrabold uppercase tracking-wider bg-emerald-500/20 text-emerald-300 border border-emerald-400/50 backdrop-blur-md shadow-sm">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" /> Running
            </span>
          ) : game.is_installed ? (
            <span className="px-2 py-0.5 rounded-md text-[10px] font-extrabold uppercase tracking-wider bg-black/80 text-orange-400 border border-orange-500/40 backdrop-blur-md shadow-sm">
              Installed
            </span>
          ) : (
            <span />
          )}

          {isSteam ? (
            <span className="flex items-center gap-1 px-2 py-0.5 rounded-md text-[10px] font-bold bg-sky-950/80 text-sky-300 border border-sky-500/40 backdrop-blur-md shadow-sm">
              <SteamIcon className="w-2.5 h-2.5 fill-current" /> Steam
            </span>
          ) : (
            <span className="flex items-center gap-1 px-2 py-0.5 rounded-md text-[10px] font-bold bg-orange-950/80 text-orange-300 border border-orange-500/40 backdrop-blur-md shadow-sm">
              <EpicGamesIcon className="w-2.5 h-2.5 fill-current" /> Epic
            </span>
          )}
        </div>

        {/* Hover Quick Action Play / Install Button (running games use the
            always-visible Stop bar below instead) */}
        {!isRunning && (
          <div className="absolute inset-0 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity duration-300 bg-black/50 backdrop-blur-sm">
            {isDownloading ? (
              <div className="p-4 rounded-xl glass-panel w-4/5 text-center border border-orange-500/30">
                <div className="text-xs font-bold text-white mb-1.5">Downloading...</div>
                <div className="w-full h-1.5 rounded-full bg-zinc-900 overflow-hidden">
                  <div
                    className="h-full bg-gradient-to-r from-orange-600 to-amber-500 rounded-full"
                    style={{ width: `${downloadProgress.progress_percent}%` }}
                  />
                </div>
              </div>
            ) : game.is_installed ? (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onLaunch(game);
                }}
                className="w-14 h-14 rounded-full gradient-primary text-white flex items-center justify-center shadow-2xl hover:scale-110 transition-transform glow-orange cursor-pointer"
              >
                <Play className="w-6 h-6 fill-current translate-x-0.5" />
              </button>
            ) : (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onInstall(game);
                }}
                className="w-14 h-14 rounded-full bg-white text-black flex items-center justify-center shadow-2xl hover:scale-110 transition-transform cursor-pointer"
              >
                <Download className="w-6 h-6" />
              </button>
            )}
          </div>
        )}

        {/* Stop control, visible the whole time the game is running */}
        {isRunning && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onStop?.(game);
            }}
            title={`Stop ${game.title}`}
            className="absolute bottom-3 left-3 right-3 z-20 flex items-center justify-center gap-2 py-2 rounded-xl bg-rose-600/95 hover:bg-rose-500 text-white font-extrabold text-[11px] uppercase tracking-wider shadow-lg shadow-rose-900/50 border border-rose-400/40 transition hover:scale-[1.02] cursor-pointer"
          >
            <Square className="w-3.5 h-3.5 fill-current" />
            <span>Stop game</span>
          </button>
        )}

        {/* Active Download Progress Bar on Image Bottom */}
        {isDownloading && (
          <div className="absolute bottom-0 left-0 right-0 h-1.5 bg-zinc-900">
            <div
              className="h-full bg-gradient-to-r from-orange-600 to-amber-500 transition-all duration-300"
              style={{ width: `${downloadProgress.progress_percent}%` }}
            />
          </div>
        )}
      </div>

      {/* Card Info */}
      <div className="p-4 flex-1 flex flex-col justify-between bg-zinc-950/80">
        <div>
          <h3 className="font-bold text-sm text-white line-clamp-1 group-hover:text-orange-400 transition">
            {game.title}
          </h3>
          <p className="text-xs text-zinc-400 truncate mt-0.5">
            {game.developer || game.publisher || (isSteam ? 'Steam Library' : 'Epic Games Store')}
          </p>
        </div>

        <div className="flex items-center justify-between mt-3 pt-2.5 border-t border-zinc-800/80 text-[11px] text-zinc-400 font-medium">
          <span>
            {isRunning ? 'Running' : game.is_installed ? 'Ready to play' : formatSize(game.install_size)}
          </span>
          <span className="font-mono text-zinc-500">{game.build_version || 'Live'}</span>
        </div>
      </div>
    </div>
  );
};
