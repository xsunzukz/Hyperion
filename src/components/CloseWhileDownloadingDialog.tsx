import React, { useEffect, useState } from 'react';
import { AlertTriangle, Download, Loader2, LogOut, MonitorDown } from 'lucide-react';
import { DownloadProgress } from '../types';

interface CloseWhileDownloadingDialogProps {
  /** Downloads that would stop if Hyperion quit now. */
  downloads: DownloadProgress[];
  /** Downloads the frontend has no row for (e.g. a Steam job it never saw). */
  extraCount?: number;
  onKeepInTray: () => Promise<void> | void;
  onQuit: () => Promise<void> | void;
  onClose: () => void;
}

const statusLabel = (d: DownloadProgress) => {
  switch (d.status) {
    case 'Downloading':
      return `${d.progress_percent.toFixed(0)}%`;
    case 'Verifying':
      return 'checking files';
    case 'Paused':
      return 'paused';
    default:
      return 'preparing';
  }
};

/**
 * Shown when the window's close button is pressed while a download runs.
 * Closing normally quits Hyperion; here the user picks between keeping the
 * download going in the tray and quitting anyway.
 */
export const CloseWhileDownloadingDialog: React.FC<CloseWhileDownloadingDialogProps> = ({
  downloads,
  extraCount = 0,
  onKeepInTray,
  onQuit,
  onClose,
}) => {
  const [busy, setBusy] = useState<'tray' | 'quit' | null>(null);
  const total = downloads.length + extraCount;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy) onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [busy, onClose]);

  const run = async (kind: 'tray' | 'quit') => {
    setBusy(kind);
    try {
      await (kind === 'tray' ? onKeepInTray() : onQuit());
    } finally {
      setBusy(null);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[80] flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-fade-in"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="close-downloading-title"
      aria-describedby="close-downloading-description"
    >
      <div className="w-full max-w-lg overflow-hidden rounded-3xl border border-amber-500/25 bg-zinc-950 shadow-2xl shadow-black/60">
        <div className="space-y-4 p-6">
          <div className="flex items-start gap-4">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-amber-500/30 bg-amber-500/10">
              <Download className="h-5 w-5 text-amber-400" />
            </div>
            <div className="min-w-0">
              <h2
                id="close-downloading-title"
                className="text-lg font-extrabold tracking-tight text-white"
              >
                {total === 1 ? 'A download is still running' : `${total} downloads are still running`}
              </h2>
              <p
                id="close-downloading-description"
                className="mt-1 text-xs leading-relaxed text-zinc-400"
              >
                Keep Hyperion in the system tray to let {total === 1 ? 'it' : 'them'} finish, or quit
                now.
              </p>
            </div>
          </div>

          {downloads.length > 0 && (
            <ul className="space-y-1.5">
              {downloads.map((d) => (
                <li
                  key={d.app_name}
                  className="flex items-center justify-between gap-3 rounded-xl border border-zinc-800 bg-zinc-900/60 px-3 py-2 text-xs"
                >
                  <span className="truncate font-semibold text-zinc-200">{d.title}</span>
                  <span className="shrink-0 font-mono text-[11px] text-amber-300">
                    {statusLabel(d)}
                  </span>
                </li>
              ))}
            </ul>
          )}

          <div className="flex items-start gap-2 rounded-xl border border-zinc-800 bg-zinc-900/40 p-3 text-[11px] leading-relaxed text-zinc-400">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-zinc-500" />
            <span>
              Quitting stops the download. Files that already finished are kept: start the same
              download again later and it continues from there.
            </span>
          </div>
        </div>

        <div className="flex flex-wrap items-center justify-end gap-2 border-t border-zinc-800/80 bg-black/30 px-5 py-4">
          <button
            type="button"
            onClick={onClose}
            disabled={busy !== null}
            className="rounded-xl border border-zinc-800 bg-zinc-900 px-4 py-2.5 text-xs font-semibold text-zinc-200 transition hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-orange-500/50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void run('quit')}
            disabled={busy !== null}
            className="flex items-center gap-2 rounded-xl border border-rose-500/40 bg-rose-600/90 px-4 py-2.5 text-xs font-bold text-white transition hover:bg-rose-500 disabled:cursor-not-allowed disabled:opacity-60 focus:outline-none focus-visible:ring-2 focus-visible:ring-rose-400/60"
          >
            {busy === 'quit' ? <Loader2 className="h-4 w-4 animate-spin" /> : <LogOut className="h-4 w-4" />}
            <span>Quit Anyway</span>
          </button>
          <button
            type="button"
            onClick={() => void run('tray')}
            disabled={busy !== null}
            autoFocus
            className="flex items-center gap-2 rounded-xl bg-gradient-to-r from-orange-600 to-amber-500 px-4 py-2.5 text-xs font-extrabold text-white shadow-lg shadow-orange-950/40 transition hover:from-orange-500 hover:to-amber-400 disabled:cursor-not-allowed disabled:opacity-60 focus:outline-none focus-visible:ring-2 focus-visible:ring-orange-400/60"
          >
            {busy === 'tray' ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <MonitorDown className="h-4 w-4" />
            )}
            <span>Keep Downloading in Tray</span>
          </button>
        </div>
      </div>
    </div>
  );
};
