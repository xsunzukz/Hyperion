import React, { useEffect, useState } from 'react';
import {
  CheckCircle2,
  Info,
  LogIn,
  LogOut,
  Mail,
  RefreshCw,
  Shield,
  ShieldCheck,
  UserRound,
} from 'lucide-react';
import { AppSettings, SteamAuthStatus, UserProfile } from '../types';
import { settingsGet, settingsSave, steamGetAuthStatus, steamLogout } from '../lib/ipc';

interface AccountViewProps {
  /** The signed-in Epic profile, or null while browsing offline */
  user: UserProfile | null;
  /** Opens the Epic sign-in modal */
  onOpenLogin: () => void;
  /** Clears the Epic session and returns the shell to the sign-in gate */
  onLogout: () => void;
  /** Opens the in-app Steam credential prompt */
  onRequestSteamLogin?: () => void;
  /** Bumped by the shell after a successful Steam sign-in */
  steamAuthVersion?: number;
  /** Lets the shell reload the library after the Steam scan toggle changes */
  onSettingsChanged?: (settings: AppSettings) => void;
  /** Lets the shell refresh its Steam banners after connect/disconnect */
  onSteamAuthChanged?: () => void;
}

// Official clean Epic Games shield crest SVG icon
const EpicGamesIcon: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => (
  <svg
    role="img"
    viewBox="0 0 24 24"
    fill="currentColor"
    className={className}
    xmlns="http://www.w3.org/2000/svg"
  >
    <path d="M3.537 0C2.165 0 1.66.506 1.66 1.879V18.44a4.262 4.262 0 00.02.433c.031.3.037.59.316.92.027.033.311.245.311.245.153.075.258.13.43.2l8.335 3.491c.433.199.614.276.928.27h.002c.314.006.495-.071.928-.27l8.335-3.492c.172-.07.277-.124.43-.2 0 0 .284-.211.311-.243.28-.33.285-.621.316-.92a4.261 4.261 0 00.02-.434V1.879c0-1.373-.506-1.88-1.878-1.88zm13.366 3.11h.68c1.138 0 1.688.553 1.688 1.696v1.88h-1.374v-1.8c0-.369-.17-.54-.523-.54h-.235c-.367 0-.537.17-.537.539v5.81c0 .369.17.54.537.54h.262c.353 0 .523-.171.523-.54V8.619h1.373v2.143c0 1.144-.562 1.71-1.7 1.71h-.694c-1.138 0-1.7-.566-1.7-1.71V4.82c0-1.144.562-1.709 1.7-1.709zm-12.186.08h3.114v1.274H6.117v2.603h1.648v1.275H6.117v2.774h1.74v1.275h-3.14zm3.816 0h2.198c1.138 0 1.7.564 1.7 1.708v2.445c0 1.144-.562 1.71-1.7 1.71h-.799v3.338h-1.4zm4.53 0h1.4v9.201h-1.4zm-3.13 1.235v3.392h.575c.354 0 .523-.171.523-.54V4.965c0-.368-.17-.54-.523-.54z" />
  </svg>
);

// Official Valve Steam SVG icon
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

export const AccountView: React.FC<AccountViewProps> = ({
  user,
  onOpenLogin,
  onLogout,
  onRequestSteamLogin,
  steamAuthVersion = 0,
  onSettingsChanged,
  onSteamAuthChanged,
}) => {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [steamAuth, setSteamAuth] = useState<SteamAuthStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [steamBusy, setSteamBusy] = useState(false);
  const [steamError, setSteamError] = useState<string | null>(null);
  const [scanState, setScanState] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');

  const refreshSteamAuth = async () => {
    try {
      const status = await steamGetAuthStatus();
      setSteamAuth(status);
    } catch (e) {
      console.warn('Failed to load Steam account status:', e);
    }
  };

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      try {
        const loadedSettings = await settingsGet();
        if (!cancelled) setSettings(loadedSettings);
        await refreshSteamAuth();
      } catch (e) {
        console.error('Failed to load account details:', e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  // Refresh Steam state when the shell reports a new sign-in
  useEffect(() => {
    if (!loading) refreshSteamAuth();
  }, [steamAuthVersion]);

  const handleSteamDisconnect = async () => {
    setSteamBusy(true);
    setSteamError(null);
    try {
      await steamLogout();
      await refreshSteamAuth();
      onSteamAuthChanged?.();
    } catch (e) {
      console.error('Failed to disconnect Steam:', e);
      setSteamError('Could not clear the Steam session. Please try again.');
    } finally {
      setSteamBusy(false);
    }
  };

  const handleScanToggle = async (checked: boolean) => {
    if (!settings || scanState === 'saving') return;

    const previous = settings;
    const next = { ...settings, steam_library_scan: checked };
    setSettings(next);
    setScanState('saving');

    try {
      await settingsSave(next);
      onSettingsChanged?.(next);
      setScanState('saved');
      window.setTimeout(() => setScanState('idle'), 2500);
    } catch (e) {
      console.error('Failed to save the Steam library scan preference:', e);
      setSettings(previous);
      setScanState('error');
      window.setTimeout(() => setScanState('idle'), 4000);
    }
  };

  const epicConnected = !!user?.logged_in;
  const steamConnected = !!steamAuth?.logged_in;

  return (
    <div className="p-8 max-w-4xl space-y-6">
      <div>
        <h2 className="text-2xl font-extrabold text-white tracking-tight">Account Management</h2>
        <p className="text-sm text-zinc-400 mt-1">
          Manage the Epic Games and Steam accounts Hyperion uses for your library, downloads, and
          cloud saves.
        </p>
      </div>

      {loading ? (
        <div className="p-8 rounded-2xl glass-panel border border-zinc-800/80 bg-zinc-950/80 text-zinc-400 font-mono text-sm">
          Loading account details...
        </div>
      ) : (
        <>
          {/* Epic Games account */}
          <section className="rounded-2xl glass-panel border border-zinc-800/80 bg-zinc-950/80 overflow-hidden shadow-lg shadow-black/20">
            <div className="p-6 space-y-5">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex items-center gap-2.5">
                  <div className="w-9 h-9 rounded-xl bg-orange-500/10 border border-orange-500/30 flex items-center justify-center">
                    <EpicGamesIcon className="w-4 h-4 text-orange-400" />
                  </div>
                  <div>
                    <h3 className="text-sm font-bold text-white">Epic Games Account</h3>
                    <p className="text-[11px] text-zinc-500">Library, purchases, and cloud saves</p>
                  </div>
                </div>

                {epicConnected ? (
                  <span className="flex items-center gap-1.5 px-3 py-1 rounded-full text-[11px] font-bold bg-emerald-950/80 text-emerald-300 border border-emerald-500/40">
                    <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Connected</span>
                  </span>
                ) : (
                  <span className="flex items-center gap-1.5 px-3 py-1 rounded-full text-[11px] font-medium bg-zinc-900 text-zinc-400 border border-zinc-800">
                    <Info className="w-3.5 h-3.5 text-zinc-400" />
                    <span>Not signed in</span>
                  </span>
                )}
              </div>

              {epicConnected && user ? (
                <div className="p-4 rounded-xl bg-zinc-900/70 border border-zinc-800 flex flex-wrap items-center justify-between gap-4">
                  <div className="flex items-center gap-3.5 min-w-0">
                    <div className="w-12 h-12 rounded-full bg-gradient-to-tr from-orange-600 to-amber-500 flex items-center justify-center font-extrabold text-base text-white shadow-lg shadow-orange-600/20 shrink-0">
                      {user.display_name.charAt(0).toUpperCase() || 'E'}
                    </div>
                    <div className="min-w-0">
                      <div className="text-sm font-bold text-white flex flex-wrap items-center gap-2">
                        <span className="truncate">{user.display_name}</span>
                        <span className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-orange-500/15 text-orange-300 border border-orange-500/30">
                          ACTIVE
                        </span>
                      </div>
                      {user.email && (
                        <div className="mt-1 flex items-center gap-1.5 text-[11px] text-zinc-400 truncate">
                          <Mail className="w-3 h-3 text-zinc-500 shrink-0" />
                          <span className="truncate">{user.email}</span>
                        </div>
                      )}
                      <div className="mt-0.5 text-[10px] text-zinc-500 font-mono truncate">
                        Epic ID: {user.account_id}
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-2 shrink-0">
                    <button
                      type="button"
                      onClick={onOpenLogin}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 text-xs font-semibold transition cursor-pointer"
                    >
                      <RefreshCw className="w-3.5 h-3.5 text-orange-400" />
                      <span>Switch Account</span>
                    </button>
                    <button
                      type="button"
                      onClick={onLogout}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-zinc-900 hover:bg-rose-950/40 text-zinc-400 hover:text-rose-400 border border-zinc-800 hover:border-rose-500/30 text-xs font-semibold transition cursor-pointer"
                    >
                      <LogOut className="w-3.5 h-3.5" />
                      <span>Sign Out</span>
                    </button>
                  </div>
                </div>
              ) : (
                <div className="p-4 rounded-xl bg-orange-500/5 border border-orange-500/20 flex flex-wrap items-center justify-between gap-4">
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-10 h-10 rounded-full bg-zinc-900 border border-zinc-800 flex items-center justify-center shrink-0">
                      <UserRound className="w-5 h-5 text-zinc-500" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-xs font-bold text-white">You&rsquo;re browsing offline</div>
                      <p className="mt-0.5 text-[11px] text-zinc-400 leading-relaxed">
                        Sign in to load your Epic library, downloads, and cloud saves.
                      </p>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={onOpenLogin}
                    className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 text-white font-bold text-xs shadow-lg shadow-orange-600/20 transition hover:scale-105 cursor-pointer shrink-0"
                  >
                    <LogIn className="w-4 h-4" />
                    <span>Sign In with Epic</span>
                  </button>
                </div>
              )}

              <div className="flex items-center gap-2 text-[11px] text-zinc-500 pt-1 border-t border-zinc-900">
                <Shield className="w-3.5 h-3.5 text-orange-500/70 shrink-0" />
                <span>
                  Epic&rsquo;s own login page handles your password. Hyperion stores only the encrypted
                  session token on this PC.
                </span>
              </div>
            </div>
          </section>

          {/* Steam account */}
          <section className="p-6 rounded-2xl glass-panel border border-sky-500/30 bg-zinc-950/80 space-y-4 shadow-lg shadow-sky-950/20">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-sky-500/10 border border-sky-500/30 flex items-center justify-center">
                  <SteamIcon className="w-4 h-4 text-sky-400" />
                </div>
                <div>
                  <h3 className="text-sm font-bold text-white">Steam Account</h3>
                  <p className="text-[11px] text-zinc-500">In-app downloads for games you own</p>
                </div>
              </div>

              {steamConnected ? (
                <span className="flex items-center gap-1.5 px-3 py-1 rounded-full text-[11px] font-bold bg-emerald-950/80 text-emerald-300 border border-emerald-500/40">
                  <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Connected</span>
                </span>
              ) : (
                <span className="flex items-center gap-1.5 px-3 py-1 rounded-full text-[11px] font-medium bg-zinc-900 text-zinc-400 border border-zinc-800">
                  <Info className="w-3.5 h-3.5 text-zinc-400" />
                  <span>Anonymous Mode (Free Games Only)</span>
                </span>
              )}
            </div>

            <p className="text-xs text-zinc-400 leading-relaxed">
              Link your Steam account to enable direct 1-click in-launcher downloads for owned games.
              Signing in opens the in-app Steam prompt — your password goes only to Valve&rsquo;s
              download service and is kept in memory for this session unless you tick
              &ldquo;Remember on this PC&rdquo;.
            </p>

            {steamConnected ? (
              <div className="p-4 rounded-xl bg-zinc-900/70 border border-zinc-800 flex flex-wrap items-center justify-between gap-3">
                <div className="flex items-center gap-3 min-w-0">
                  <div className="w-10 h-10 rounded-xl bg-sky-500/20 border border-sky-500/30 flex items-center justify-center shrink-0">
                    <SteamIcon className="w-5 h-5 text-sky-400" />
                  </div>
                  <div className="min-w-0">
                    <div className="text-xs font-bold text-white flex flex-wrap items-center gap-2">
                      <span className="truncate">{steamAuth?.account_name || 'Steam Account'}</span>
                      <span className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-sky-500/20 text-sky-300 border border-sky-500/30">
                        SIGNED IN
                      </span>
                    </div>
                    {steamAuth?.steam_id && (
                      <div className="text-[11px] text-zinc-400 font-mono mt-0.5">
                        SteamID: {steamAuth.steam_id}
                      </div>
                    )}
                  </div>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  <button
                    type="button"
                    onClick={onRequestSteamLogin}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 text-xs font-semibold transition cursor-pointer"
                  >
                    <RefreshCw className="w-3.5 h-3.5 text-sky-400" />
                    <span>Steam Login</span>
                  </button>
                  <button
                    type="button"
                    onClick={handleSteamDisconnect}
                    disabled={steamBusy}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-zinc-900 hover:bg-rose-950/40 text-zinc-400 hover:text-rose-400 border border-zinc-800 hover:border-rose-500/30 text-xs font-semibold transition cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <LogOut className="w-3.5 h-3.5" />
                    <span>{steamBusy ? 'Disconnecting...' : 'Disconnect'}</span>
                  </button>
                </div>
              </div>
            ) : (
              <div className="flex flex-wrap items-center justify-between gap-3">
                <button
                  type="button"
                  onClick={onRequestSteamLogin}
                  className="flex items-center gap-2.5 px-5 py-2.5 rounded-xl bg-gradient-to-r from-sky-600 to-blue-600 hover:from-sky-500 hover:to-blue-500 text-white font-bold text-xs shadow-lg shadow-sky-600/20 transition hover:scale-105 cursor-pointer"
                >
                  <SteamIcon className="w-4 h-4 fill-current text-white" />
                  <span>Steam Login</span>
                </button>
                <span className="flex items-center gap-1.5 text-[11px] text-zinc-500">
                  <CheckCircle2 className="w-3.5 h-3.5 text-zinc-600" />
                  Free-to-play Steam titles work without an account.
                </span>
              </div>
            )}

            {steamError && (
              <div className="p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
                <Info className="w-4 h-4 text-rose-400 flex-shrink-0" />
                <span className="leading-tight">{steamError}</span>
              </div>
            )}

            <div className="pt-3 border-t border-zinc-800/80 space-y-1.5">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <label className="flex items-center gap-2.5 text-xs text-zinc-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings?.steam_library_scan ?? false}
                    onChange={(e) => handleScanToggle(e.target.checked)}
                    disabled={!settings || scanState === 'saving'}
                    className="rounded bg-zinc-900 border-zinc-800 accent-sky-500 text-sky-500 focus:ring-sky-500 disabled:opacity-50"
                  />
                  <span>Scan this PC&rsquo;s Steam library</span>
                </label>
                {scanState !== 'idle' && (
                  <span
                    className={`text-[10px] font-semibold ${
                      scanState === 'error' ? 'text-rose-400' : 'text-sky-400'
                    }`}
                  >
                    {scanState === 'saving' && 'Saving...'}
                    {scanState === 'saved' && 'Saved'}
                    {scanState === 'error' && 'Could not save'}
                  </span>
                )}
              </div>
              <p className="text-[11px] text-zinc-500 leading-relaxed pl-6">
                Lists the games your Steam client has installed or owns by reading Steam&rsquo;s
                library files (libraryfolders.vdf, app manifests, userdata). Off by default, because
                some antivirus products flag apps that read these files.
              </p>
            </div>

            <div className="flex items-center gap-2 text-[11px] text-zinc-500 pt-1 border-t border-zinc-900">
              <Shield className="w-3.5 h-3.5 text-sky-500/70 shrink-0" />
              <span>
                Credentials go only to Valve&rsquo;s download service — never to Hyperion. Encrypted
                with Windows DPAPI when remembered.
              </span>
            </div>
          </section>
        </>
      )}
    </div>
  );
};
