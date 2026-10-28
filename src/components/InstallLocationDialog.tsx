import React, { useEffect, useState } from 'react';
import { AlertCircle, FolderOpen, HardDrive, Loader2 } from 'lucide-react';
import { settingsGet, settingsSave, selectDirectory } from '../lib/ipc';

interface InstallLocationDialogProps {
  title: string;
  /** Starts the download after the user confirms the folder. */
  onConfirm: (installDir: string) => Promise<void> | void;
  onClose: () => void;
}

/**
 * Shown before a normal game download starts. The chosen folder is remembered
 * only when the user opts in; otherwise it is used for this install only.
 */
export const InstallLocationDialog: React.FC<InstallLocationDialogProps> = ({
  title,
  onConfirm,
  onClose,
}) => {
  const [installDir, setInstallDir] = useState('');
  const [remember, setRemember] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    settingsGet()
      .then((settings) => {
        if (!cancelled) setInstallDir(settings.default_install_dir || '');
      })
      .catch(() => {
        if (!cancelled) setInstallDir('D:\\Games');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy) onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [busy, onClose]);

  const handleBrowse = async () => {
    try {
      const selected = await selectDirectory(installDir || 'D:\\Games');
      if (selected) setInstallDir(selected);
    } catch (e: any) {
      setError(e?.message || 'Could not open the folder picker.');
    }
  };

  const handleConfirm = async (event?: React.FormEvent) => {
    event?.preventDefault();
    const path = installDir.trim();
    if (!path) {
      setError('Choose a folder before starting the download.');
      return;
    }

    setBusy(true);
    setError(null);
    try {
      if (remember) {
        const current = await settingsGet();
        await settingsSave({ ...current, default_install_dir: path });
      }
      await onConfirm(path);
    } catch (e: any) {
      console.error('Could not start the download:', e);
      setError(e?.toString() || 'Could not start the download.');
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-fade-in"
      role="dialog"
      aria-modal="true"
      aria-labelledby="install-location-title"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) onClose();
      }}
    >
      <form
        onSubmit={handleConfirm}
        className="w-full max-w-lg overflow-hidden rounded-3xl border border-zinc-800 bg-zinc-950 shadow-2xl shadow-black/60"
      >
        <div className="space-y-5 p-6">
          <div className="flex items-start gap-4">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-orange-500/30 bg-orange-500/10">
              <FolderOpen className="h-5 w-5 text-orange-400" />
            </div>
            <div className="min-w-0">
              <h2
                id="install-location-title"
                className="text-lg font-extrabold tracking-tight text-white"
              >
                Choose install location
              </h2>
              <p className="mt-1 text-xs leading-relaxed text-zinc-400">
                Where should <span className="font-semibold text-zinc-200">{title}</span> be
                downloaded?
              </p>
            </div>
          </div>

          <div className="space-y-2">
            <label
              htmlFor="install-location-path"
              className="block text-[11px] font-bold uppercase tracking-wider text-zinc-400"
            >
              Install folder
            </label>
            <div className="flex gap-2">
              <div className="relative min-w-0 flex-1">
                <HardDrive className="absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-500" />
                <input
                  id="install-location-path"
                  type="text"
                  value={installDir}
                  onChange={(event) => setInstallDir(event.target.value)}
                  placeholder="D:\Games"
                  disabled={loading || busy}
                  className="w-full rounded-xl border border-zinc-800 bg-zinc-900 py-2.5 pl-10 pr-4 font-mono text-xs text-white placeholder-zinc-500 transition focus:border-orange-500 focus:outline-none disabled:opacity-60"
                />
              </div>
              <button
                type="button"
                onClick={handleBrowse}
                disabled={busy}
                className="flex shrink-0 items-center gap-1.5 rounded-xl border border-zinc-800 bg-zinc-900 px-3.5 py-2.5 text-xs font-semibold text-zinc-200 transition hover:bg-zinc-800 disabled:opacity-50"
              >
                <FolderOpen className="h-4 w-4 text-orange-400" />
                <span>Browse...</span>
              </button>
            </div>
            <p className="text-[11px] leading-relaxed text-zinc-500">
              Hyperion will create a{' '}
              <span className="font-mono text-zinc-400">
                {title.replace(/[^a-zA-Z0-9 _-]/g, '_')}
              </span>{' '}
              folder inside this location when needed.
            </p>
          </div>

          <div className="flex flex-wrap gap-1.5">
            {['C:\\Games', 'D:\\Games', 'E:\\Games'].map((drive) => (
              <button
                key={drive}
                type="button"
                onClick={() => setInstallDir(drive)}
                disabled={busy}
                className="rounded-lg border border-zinc-800 bg-zinc-900 px-2.5 py-1 font-mono text-[10px] font-bold text-zinc-400 transition hover:border-orange-500/40 hover:text-orange-300 disabled:opacity-50"
              >
                {drive}
              </button>
            ))}
          </div>

          <label className="flex cursor-pointer items-start gap-2.5 rounded-xl border border-zinc-800/80 bg-zinc-900/50 p-3">
            <input
              type="checkbox"
              checked={remember}
              onChange={(event) => setRemember(event.target.checked)}
              disabled={busy}
              className="mt-0.5 rounded border-zinc-700 bg-zinc-900 text-orange-500 accent-orange-500 focus:ring-orange-500"
            />
            <span>
              <span className="block text-xs font-semibold text-zinc-200">
                Remember this as my default install location
              </span>
              <span className="mt-0.5 block text-[11px] leading-relaxed text-zinc-500">
                New games will start here. You can change it later in Settings.
              </span>
            </span>
          </label>

          {error && (
            <div className="flex items-center gap-2 rounded-xl border border-rose-500/30 bg-rose-500/10 p-3 text-xs text-rose-300">
              <AlertCircle className="h-4 w-4 shrink-0" />
              <span>{error}</span>
            </div>
          )}

          <div className="flex flex-wrap items-center justify-end gap-2 border-t border-zinc-800/80 pt-4">
            <button
              type="button"
              onClick={onClose}
              disabled={busy}
              className="rounded-xl border border-zinc-800 bg-zinc-900 px-4 py-2.5 text-xs font-semibold text-zinc-200 transition hover:bg-zinc-800 disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || busy}
              className="flex items-center gap-2 rounded-xl bg-gradient-to-r from-orange-600 to-amber-500 px-4 py-2.5 text-xs font-extrabold text-white shadow-lg shadow-orange-950/40 transition hover:from-orange-500 hover:to-amber-400 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {busy ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <FolderOpen className="h-4 w-4" />
              )}
              <span>{busy ? 'Starting...' : 'Continue to Download'}</span>
            </button>
          </div>
        </div>
      </form>
    </div>
  );
};
