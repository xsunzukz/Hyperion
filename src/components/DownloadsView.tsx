import React from 'react';
import { Download, Pause, Play, X, AlertCircle, Loader2 } from 'lucide-react';
import { DownloadProgress } from '../types';

interface DownloadsViewProps {
  downloads: DownloadProgress[];
  onPause: (appName: string) => void;
  onResume: (appName: string) => void;
  onCancel: (appName: string) => void;
  onRetry?: (appName: string) => void;
  onConfigure?: (appName: string) => void;
}

export const DownloadsView: React.FC<DownloadsViewProps> = ({
  downloads,
  onPause,
  onResume,
  onCancel,
  onRetry,
  onConfigure,
}) => {
  const formatBytes = (bytes: number) => {
    const gb = bytes / (1024 * 1024 * 1024);
    if (gb >= 1) return `${gb.toFixed(2)} GB`;
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(1)} MB`;
  };

  const formatSpeed = (bytesPerSec: number) => {
    const mb = bytesPerSec / (1024 * 1024);
    return `${mb.toFixed(1)} MB/s`;
  };

  const formatEta = (seconds: number) => {
    if (seconds <= 0) return 'Calculating...';
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    if (mins >= 60) {
      const hrs = Math.floor(mins / 60);
      return `${hrs}h ${mins % 60}m`;
    }
    return `${mins}m ${secs}s`;
  };

  return (
    <div className="p-8 space-y-6">
      <div>
        <h2 className="text-2xl font-extrabold text-white tracking-tight">Downloads Queue</h2>
        <p className="text-sm text-zinc-400 mt-1">
          Monitor native chunk streams, resumable downloads, and write speeds.
        </p>
      </div>

      {downloads.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-16 rounded-3xl glass-panel text-center border border-zinc-800/80 bg-zinc-950/40">
          <div className="w-16 h-16 rounded-2xl bg-zinc-900 border border-zinc-800 flex items-center justify-center text-zinc-500 mb-4">
            <Download className="w-8 h-8 text-orange-500/70" />
          </div>
          <h3 className="font-bold text-white text-lg">No Active Downloads</h3>
          <p className="text-sm text-zinc-400 mt-1 max-w-sm">
            Games you download or update will appear here with live speed and chunk verification.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {downloads.map((d) => (
            <div
              key={d.app_name}
              className="p-6 rounded-2xl glass-card border border-zinc-800/80 bg-zinc-950/80 space-y-4 shadow-xl hover:border-orange-500/30 transition-all duration-300"
            >
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="font-bold text-base text-white">{d.title}</h3>
                  <div className="flex items-center gap-2 mt-1.5 font-mono text-xs">
                    <span
                      className={`px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider ${
                        d.status === 'Downloading'
                          ? 'bg-orange-500/20 text-orange-400 border border-orange-500/40'
                          : d.status === 'Paused'
                          ? 'bg-amber-500/20 text-amber-400 border border-amber-500/40'
                          : d.status === 'Queued'
                          ? 'bg-sky-500/15 text-sky-300 border border-sky-500/30'
                          : 'bg-zinc-800 text-zinc-300 border border-zinc-700'
                      }`}
                    >
                      {d.status}
                    </span>
                    <span className="text-zinc-500">•</span>
                    <span className={d.status === 'Queued' ? 'text-sky-400/80' : 'text-zinc-400'}>
                      {d.status === 'Queued'
                        ? 'Preparing download...'
                        : d.app_name.startsWith('steam_')
                        ? 'Steam engine'
                        : `${d.downloaded_chunks} / ${d.total_chunks} chunks`}
                    </span>
                  </div>
                </div>

                {/* Control Actions */}
                <div className="flex items-center gap-2">
                  {/* Steam transfers are owned by the Steam client, so pausing here is not possible */}
                  {d.status === 'Downloading' && !d.app_name.startsWith('steam_') ? (
                    <button
                      onClick={() => onPause(d.app_name)}
                      className="p-2.5 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-200 border border-zinc-800 transition cursor-pointer"
                      title="Pause Download"
                    >
                      <Pause className="w-4 h-4" />
                    </button>
                  ) : d.status === 'Paused' ? (
                    <button
                      onClick={() => onResume(d.app_name)}
                      className="p-2.5 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 text-white font-bold transition shadow-lg shadow-orange-600/30 cursor-pointer glow-orange"
                      title="Resume Download"
                    >
                      <Play className="w-4 h-4 fill-current" />
                    </button>
                  ) : null}

                  {d.status !== 'Queued' && (
                    <button
                      onClick={() => onCancel(d.app_name)}
                      className="p-2.5 rounded-xl bg-zinc-900 hover:bg-rose-500/20 hover:text-rose-400 hover:border-rose-500/40 text-zinc-400 border border-zinc-800 transition cursor-pointer"
                      title={
                        d.status === 'Completed' || d.status === 'Cancelled' || d.status === 'Failed'
                          ? 'Dismiss from queue'
                          : 'Cancel download and delete partial files'
                      }
                    >
                      <X className="w-4 h-4" />
                    </button>
                  )}
                </div>
              </div>

              {/* Progress Bar */}
              {d.status === 'Queued' ? (
                <div className="space-y-2">
                  <div className="w-full h-2.5 rounded-full bg-zinc-900 border border-zinc-800/80 overflow-hidden">
                    <div className="queue-loading-slide h-full w-1/3 rounded-full bg-sky-500/60" />
                  </div>
                  <div className="flex items-center justify-between text-xs font-mono text-zinc-400">
                    <span className="flex items-center gap-1.5 text-sky-300/90">
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                      Resolving manifest...
                    </span>
                    <span className="text-zinc-600">Starting soon</span>
                  </div>
                </div>
              ) : (
                <div className="space-y-2">
                  <div className="w-full h-2.5 rounded-full bg-zinc-900 border border-zinc-800/80 overflow-hidden relative">
                    <div
                      className="h-full gradient-primary rounded-full transition-all duration-300"
                      style={{ width: `${d.progress_percent}%` }}
                    />
                  </div>
                  <div className="flex items-center justify-between text-xs font-mono text-zinc-400">
                    <span>
                      {formatBytes(d.downloaded_bytes)} /{' '}
                      {d.total_bytes > 0 ? formatBytes(d.total_bytes) : '—'} (
                      {Math.min(100, Math.max(0, d.progress_percent)).toFixed(1)}%)
                    </span>
                    <div className="flex items-center gap-2">
                      {d.status === 'Downloading' ? (
                        <>
                          <span className="text-orange-400 font-semibold">
                            {formatSpeed(d.speed_bytes_per_sec)}
                          </span>
                          <span className="text-zinc-600">|</span>
                          <span>ETA: {formatEta(d.eta_seconds)}</span>
                        </>
                      ) : (
                        <span>{d.status}</span>
                      )}
                    </div>
                  </div>
                </div>
              )}

              {/* Error Message if Failed */}
              {d.status === 'Failed' && d.error_message && (
                <div className="p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                  <div className="flex items-center gap-2 min-w-0">
                    <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
                    <span className="leading-relaxed">{d.error_message}</span>
                  </div>
                  <div className="flex flex-wrap items-center gap-2 shrink-0">
                    {onRetry && (
                      <button
                        onClick={() => onRetry(d.app_name)}
                        className="px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 text-[11px] font-bold flex items-center gap-1.5 transition cursor-pointer"
                        title="Retry download in launcher"
                      >
                        <Download className="w-3.5 h-3.5" />
                        <span>Retry</span>
                      </button>
                    )}
                    {onConfigure && (
                      <button
                        onClick={() => onConfigure(d.app_name)}
                        className="px-3 py-1.5 rounded-lg bg-sky-500/20 hover:bg-sky-500/30 text-sky-300 border border-sky-500/40 text-[11px] font-bold flex items-center gap-1.5 transition cursor-pointer"
                        title="Configure credentials or custom manifest options"
                      >
                        <span>Options & Manifest</span>
                      </button>
                    )}
                  </div>
                </div>
              )}

              {/* Current File Activity */}
              {d.current_file && d.status !== 'Failed' && (
                <div className="text-[11px] text-zinc-400 font-mono truncate bg-zinc-900/60 px-3 py-2 rounded-xl border border-zinc-800/80">
                  <span className="text-zinc-500">Writing: </span>{d.current_file}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
