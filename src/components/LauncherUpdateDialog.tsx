import React, { useEffect } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  Loader2,
  RefreshCw,
} from 'lucide-react';

interface LauncherUpdateDialogProps {
  version: string;
  currentVersion: string;
  notes?: string;
  date?: string;
  busy: boolean;
  downloadedBytes: number;
  totalBytes: number;
  error: string | null;
  hasActiveWork: boolean;
  onInstall: () => void;
  onClose: () => void;
}

const formatBytes = (bytes: number) => {
  if (!bytes) return '0 MB';
  const gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 1) return `${gb.toFixed(2)} GB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

export const LauncherUpdateDialog: React.FC<LauncherUpdateDialogProps> = ({
  version,
  currentVersion,
  notes,
  date,
  busy,
  downloadedBytes,
  totalBytes,
  error,
  hasActiveWork,
  onInstall,
  onClose,
}) => {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy) onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [busy, onClose]);

  const percent =
    totalBytes > 0 ? Math.min(100, Math.max(0, (downloadedBytes / totalBytes) * 100)) : 0;

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-fade-in"
      role="dialog"
      aria-modal="true"
      aria-labelledby="launcher-update-title"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) onClose();
      }}
    >
      <div className="w-full max-w-lg overflow-hidden rounded-3xl border border-orange-500/25 bg-zinc-950 shadow-2xl shadow-black/60">
        <div className="space-y-5 p-6">
          <div className="flex items-start gap-4">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-orange-500/35 bg-orange-500/10">
              <RefreshCw className="h-5 w-5 text-orange-400" />
            </div>
            <div className="min-w-0">
              <h2
                id="launcher-update-title"
                className="text-lg font-extrabold tracking-tight text-white"
              >
                Launcher update available
              </h2>
              <p className="mt-1 text-xs leading-relaxed text-zinc-400">
                Hyperion <span className="font-semibold text-zinc-200">{version}</span> is
                available. You’re on {currentVersion}.
              </p>
              {date && (
                <p className="mt-1 font-mono text-[10px] text-zinc-600">
                  {new Date(date).toLocaleDateString()}
                </p>
              )}
            </div>
          </div>

          {notes && (
            <div className="max-h-52 overflow-y-auto whitespace-pre-wrap rounded-xl border border-zinc-800 bg-zinc-900/60 p-3.5 text-[11px] leading-relaxed text-zinc-300">
              {notes}
            </div>
          )}

          {hasActiveWork && !busy && (
            <div className="flex items-start gap-2 rounded-xl border border-amber-500/30 bg-amber-500/10 p-3 text-[11px] leading-relaxed text-amber-200">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-400" />
              <span>
                Downloads or games are running. Installing the update restarts Hyperion and stops
                them.
              </span>
            </div>
          )}

          {busy && (
            <div className="space-y-2 rounded-xl border border-zinc-800/80 bg-zinc-900/60 p-3.5">
              <div className="flex items-center justify-between font-mono text-[11px] text-zinc-400">
                <span className="flex items-center gap-1.5 text-orange-300">
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  {percent > 0 ? 'Downloading update...' : 'Preparing update...'}
                </span>
                <span>
                  {totalBytes > 0
                    ? `${formatBytes(downloadedBytes)} / ${formatBytes(totalBytes)}`
                    : ''}
                </span>
              </div>
              <div className="h-2 overflow-hidden rounded-full bg-zinc-950">
                <div
                  className="h-full rounded-full bg-gradient-to-r from-orange-600 to-amber-400 transition-all duration-300"
                  style={{ width: `${percent}%` }}
                />
              </div>
            </div>
          )}

          {error && (
            <div className="flex items-start gap-2 rounded-xl border border-rose-500/30 bg-rose-500/10 p-3 text-[11px] leading-relaxed text-rose-300">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
              <span>{error}</span>
            </div>
          )}

          <div className="flex flex-wrap items-center justify-between gap-2 border-t border-zinc-800/80 pt-4">
            <span className="flex items-center gap-1.5 text-[10px] text-zinc-600">
              <Download className="h-3.5 w-3.5" />
              Signed update from GitHub Releases
            </span>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onClose}
                disabled={busy}
                className="rounded-xl border border-zinc-800 bg-zinc-900 px-4 py-2.5 text-xs font-semibold text-zinc-200 transition hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Later
              </button>
              <button
                type="button"
                onClick={onInstall}
                disabled={busy}
                className="flex items-center gap-2 rounded-xl bg-gradient-to-r from-orange-600 to-amber-500 px-4 py-2.5 text-xs font-extrabold text-white shadow-lg shadow-orange-950/40 transition hover:from-orange-500 hover:to-amber-400 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {busy ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : error ? (
                  <RefreshCw className="h-4 w-4" />
                ) : (
                  <CheckCircle2 className="h-4 w-4" />
                )}
                <span>{busy ? 'Updating...' : error ? 'Try Again' : 'Install and Restart'}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
