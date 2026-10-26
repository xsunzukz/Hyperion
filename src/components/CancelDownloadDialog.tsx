import React, { useEffect } from 'react';
import { AlertTriangle, Loader2 } from 'lucide-react';

interface CancelDownloadDialogProps {
  title: string;
  downloadedBytes?: number;
  isCancelling?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}

const formatBytes = (bytes: number) => {
  const gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 1) return `${gb.toFixed(2)} GB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

export const CancelDownloadDialog: React.FC<CancelDownloadDialogProps> = ({
  title,
  downloadedBytes = 0,
  isCancelling = false,
  onConfirm,
  onClose,
}) => {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isCancelling) {
        onClose();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [isCancelling, onClose]);

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-fade-in"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="cancel-download-title"
      aria-describedby="cancel-download-description"
    >
      <div className="w-full max-w-md overflow-hidden rounded-3xl border border-rose-500/25 bg-zinc-950 shadow-2xl shadow-black/60">
        <div className="space-y-4 p-6">
          <div className="flex items-start gap-4">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-rose-500/30 bg-rose-500/10">
              <AlertTriangle className="h-5 w-5 text-rose-400" />
            </div>
            <div className="min-w-0">
              <h2
                id="cancel-download-title"
                className="text-lg font-extrabold tracking-tight text-white"
              >
                Cancel this download?
              </h2>
              <p className="mt-1 text-xs leading-relaxed text-zinc-400">
                <span className="font-semibold text-zinc-200">{title}</span> is still downloading. If
                you continue, it will be stopped.
              </p>
            </div>
          </div>

          <div className="rounded-xl border border-rose-500/25 bg-rose-500/10 p-3.5">
            <div className="flex items-start gap-2 text-xs leading-relaxed text-rose-200">
              <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-rose-400" />
              <span>
                All partial files for this download will be removed from disk. This cannot be undone.
              </span>
            </div>
            {downloadedBytes > 0 && (
              <div className="mt-2 pl-6 font-mono text-[11px] text-rose-300/70">
                {formatBytes(downloadedBytes)} downloaded
              </div>
            )}
          </div>
        </div>

        <div className="flex flex-wrap items-center justify-end gap-2 border-t border-zinc-800/80 bg-black/30 px-5 py-4">
          <button
            type="button"
            onClick={onClose}
            disabled={isCancelling}
            autoFocus
            className="rounded-xl border border-zinc-800 bg-zinc-900 px-4 py-2.5 text-xs font-semibold text-zinc-200 transition hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-orange-500/50"
          >
            Keep Downloading
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={isCancelling}
            className="flex items-center gap-2 rounded-xl border border-rose-500/40 bg-rose-600 px-4 py-2.5 text-xs font-bold text-white shadow-lg shadow-rose-950/40 transition hover:bg-rose-500 disabled:cursor-not-allowed disabled:opacity-60 focus:outline-none focus-visible:ring-2 focus-visible:ring-rose-400/60"
          >
            {isCancelling ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <AlertTriangle className="h-4 w-4" />
            )}
            <span>{isCancelling ? 'Cancelling...' : 'Yes, Cancel Download'}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
