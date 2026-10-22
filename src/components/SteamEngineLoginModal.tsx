import React, { useState } from 'react';
import { AlertCircle, Eye, EyeOff, Loader2, Lock, ShieldCheck, X } from 'lucide-react';

interface SteamEngineLoginModalProps {
  isOpen: boolean;
  /** 'full' asks for account + password, 'guard' only asks for a fresh Steam Guard code */
  mode: 'full' | 'guard';
  defaultUsername?: string;
  gameTitle?: string;
  onClose: () => void;
  onSubmit: (data: {
    username?: string;
    password?: string;
    guardCode?: string;
    remember?: boolean;
  }) => Promise<void>;
}

const SteamIcon: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => (
  <svg
    role="img"
    viewBox="0 0 24 24"
    fill="currentColor"
    className={className}
    xmlns="http://www.w3.org/2000/svg"
  >
    <path d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.005.105.005.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 12-5.373 12-12s-5.373-12-12-12zM7.556 16.41c-.347.834-1.218 1.34-2.158 1.185-.94-.155-1.635-.909-1.635-1.864 0-.154.019-.304.055-.447l3.96 1.638c-.066-.17-.145-.337-.222-.512zm4.364-7.5c0-1.748 1.422-3.17 3.17-3.17 1.747 0 3.17 1.422 3.17 3.17 0 1.748-1.423 3.17-3.17 3.17-1.748 0-3.17-1.422-3.17-3.17zm3.17-1.811c-.997 0-1.811.814-1.811 1.811s.814 1.811 1.811 1.811 1.811-.814 1.811-1.811-.814-1.811-1.811-1.811z" />
  </svg>
);

export const SteamEngineLoginModal: React.FC<SteamEngineLoginModalProps> = ({
  isOpen,
  mode,
  defaultUsername,
  gameTitle,
  onClose,
  onSubmit,
}) => {
  const [username, setUsername] = useState(defaultUsername || '');
  const [password, setPassword] = useState('');
  const [guardCode, setGuardCode] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [remember, setRemember] = useState(true);

  // The modal is unmounted when it closes, so initial state is already correct.
  // Deliberately no effect here: resetting on a mode switch would wipe whatever
  // the user is typing if the engine sends another event while they type.

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (mode === 'full') {
      if (!username.trim() || !password) {
        setError('Steam account name and password are both required.');
        return;
      }
    } else if (!guardCode.trim()) {
      setError('Enter the current Steam Guard code.');
      return;
    }

    setIsSubmitting(true);
    try {
      await onSubmit({
        username: mode === 'full' ? username.trim() : undefined,
        password: mode === 'full' ? password : undefined,
        guardCode: guardCode.trim() || undefined,
        remember: mode === 'full' ? remember : undefined,
      });
    } catch (err: any) {
      setError(err?.toString() || 'Steam sign-in failed.');
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md animate-fade-in">
      <div className="relative w-full max-w-md rounded-3xl glass-panel bg-zinc-950 border border-zinc-800 shadow-2xl overflow-hidden">
        <div className="p-6 border-b border-zinc-800 flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-zinc-900 border border-sky-500/40 flex items-center justify-center shadow-lg shadow-sky-500/10">
              <SteamIcon className="w-5 h-5 text-sky-400" />
            </div>
            <div>
              <h2 className="font-extrabold text-lg text-white">
                {mode === 'guard' ? 'Steam Guard Code' : 'Steam Sign-In'}
              </h2>
              <p className="text-xs text-zinc-400">
                {mode === 'guard'
                  ? 'Confirm the download with a fresh authenticator code'
                  : 'In-app download engine — no Steam client needed'}
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

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {gameTitle && (
            <div className="px-3 py-2 rounded-xl bg-sky-500/10 border border-sky-500/30 text-[11px] text-sky-200 font-mono truncate">
              Downloading: {gameTitle}
            </div>
          )}

          {mode === 'full' ? (
            <>
              <div className="space-y-1.5">
                <label className="text-[11px] font-bold uppercase tracking-wider text-zinc-400">
                  Steam account name
                </label>
                <input
                  type="text"
                  autoFocus
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="your_steam_login"
                  spellCheck={false}
                  autoComplete="off"
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-sky-500 font-mono transition"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-[11px] font-bold uppercase tracking-wider text-zinc-400">
                  Password
                </label>
                <div className="relative">
                  <input
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="••••••••••"
                    autoComplete="off"
                    className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 pr-11 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-sky-500 font-mono transition"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-200 transition cursor-pointer"
                    title={showPassword ? 'Hide password' : 'Show password'}
                  >
                    {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                  </button>
                </div>
              </div>
            </>
          ) : (
            <div className="space-y-1.5">
              <label className="text-[11px] font-bold uppercase tracking-wider text-zinc-400">
                Steam Guard code
              </label>
              <input
                type="text"
                autoFocus
                value={guardCode}
                onChange={(e) => setGuardCode(e.target.value)}
                placeholder="ABCDE"
                maxLength={10}
                spellCheck={false}
                autoComplete="off"
                className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-sm tracking-[0.3em] text-center text-white placeholder-zinc-600 focus:outline-none focus:border-sky-500 font-mono uppercase transition"
              />
              <p className="text-[11px] text-zinc-500">
                Open the Steam Mobile app → Steam Guard. Codes expire after ~30 seconds.
              </p>
            </div>
          )}

          {mode === 'full' && (
            <div className="space-y-1.5">
              <label className="text-[11px] font-bold uppercase tracking-wider text-zinc-400">
                Steam Guard code <span className="text-zinc-600 normal-case font-medium">(optional)</span>
              </label>
              <input
                type="text"
                value={guardCode}
                onChange={(e) => setGuardCode(e.target.value)}
                placeholder="Leave blank if Steam Guard is off"
                maxLength={10}
                spellCheck={false}
                autoComplete="off"
                className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-sky-500 font-mono uppercase transition"
              />
            </div>
          )}

          {mode === 'full' && (
            <label className="flex items-start gap-2.5 p-3 rounded-xl bg-zinc-900/60 border border-zinc-800 cursor-pointer hover:border-zinc-700 transition">
              <input
                type="checkbox"
                checked={remember}
                onChange={(e) => setRemember(e.target.checked)}
                className="mt-0.5 rounded bg-zinc-950 border-zinc-700 accent-sky-500 text-sky-500"
              />
              <span className="text-[11px] leading-relaxed text-zinc-300">
                <strong className="text-white">Remember on this PC</strong>
                <br />
                Stored in Hyperion's local database, encrypted with Windows DPAPI for your Windows
                user — a copied database can't be decrypted on another machine or account. Uncheck
                to keep it in memory for this run only.
              </span>
            </label>
          )}

          {error && (
            <div className="p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-[11px] flex items-start gap-2">
              <AlertCircle className="w-4 h-4 text-rose-400 flex-shrink-0 mt-0.5" />
              <span className="leading-relaxed">{error}</span>
            </div>
          )}

          <div className="flex items-start gap-2 text-[11px] text-zinc-500 pt-1">
            <Lock className="w-3.5 h-3.5 text-emerald-500/80 flex-shrink-0 mt-0.5" />
            <span className="leading-relaxed">
              {mode === 'full' && remember
                ? "Encrypted with Windows DPAPI and written only to Hyperion's local database. Sent nowhere except Valve's download service; signing out forgets it."
                : 'Held in memory for this app run only — never written to disk, never sent anywhere except Valve’s download service.'}
            </span>
          </div>

          <div className="flex items-center justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-300 border border-zinc-800 text-xs font-semibold transition cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="flex items-center gap-2 px-5 py-2 rounded-xl bg-gradient-to-r from-sky-600 to-blue-600 hover:from-sky-500 hover:to-blue-500 text-white font-bold text-xs shadow-lg shadow-sky-600/25 transition disabled:opacity-50 cursor-pointer"
            >
              {isSubmitting ? (
                <>
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  <span>{mode === 'guard' ? 'Checking code...' : 'Verifying with Steam...'}</span>
                </>
              ) : (
                <>
                  <ShieldCheck className="w-3.5 h-3.5" />
                  <span>{mode === 'guard' ? 'Confirm & retry' : 'Sign in & download'}</span>
                </>
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
