import React, { useState, useEffect, useRef } from 'react';
import {
  X,
  KeyRound,
  AlertCircle,
  Shield,
  Loader2,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  ExternalLink,
  LogOut,
} from 'lucide-react';
import {
  authGetLoginUrl,
  authStartAutomaticLogin,
  authCancelAutomaticLogin,
  listenAuthLoginSuccess,
  listenAuthLoginFailed,
  authLoginCode,
  authLoginExchange,
  openPath,
  steamGetAuthStatus,
  steamGetCredentialStatus,
  steamClearCredentials,
} from '../lib/ipc';
import { UserProfile, SteamAuthStatus, SteamCredentialStatus } from '../types';

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

interface LoginModalProps {
  onClose: () => void;
  onSuccess: (user: UserProfile) => void;
  initialTab?: 'epic' | 'steam';
  /** Opens the SteamCMD credential prompt (the sign-in that authorises downloads) */
  onRequestSteamAuth?: () => void;
}

export const LoginModal: React.FC<LoginModalProps> = ({
  onClose,
  onSuccess,
  initialTab = 'epic',
  onRequestSteamAuth,
}) => {
  const [platformTab, setPlatformTab] = useState<'epic' | 'steam'>(initialTab);

  // Epic auth state
  const [isAuthOpen, setIsAuthOpen] = useState(false);
  const [authStatus, setAuthStatus] = useState<'idle' | 'waiting' | 'success'>('idle');
  const [showManual, setShowManual] = useState(false);
  const [authCode, setAuthCode] = useState('');
  const [isExchangeCode, setIsExchangeCode] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Steam state: the account signed in with, and whether the headless
  // engine already holds a session that authorises downloads.
  const [currentSteamStatus, setCurrentSteamStatus] = useState<SteamAuthStatus | null>(null);
  const [steamCredentials, setSteamCredentials] = useState<SteamCredentialStatus | null>(null);
  const [steamErrorMessage, setSteamErrorMessage] = useState<string | null>(null);

  const refreshSteamState = async () => {
    try {
      const status = await steamGetAuthStatus();
      setCurrentSteamStatus(status);
    } catch {
      /* account detection is best effort */
    }
    try {
      const creds = await steamGetCredentialStatus();
      setSteamCredentials(creds);
    } catch {
      /* engine status is best effort */
    }
  };

  // Load Steam status on mount and whenever the tab regains focus
  useEffect(() => {
    refreshSteamState();
  }, []);

  const steamAccount =
    steamCredentials?.username || currentSteamStatus?.account_name || undefined;
  const steamAuthorized = !!steamCredentials?.engine_session_cached;

  // Keep the latest callbacks without re-running the listener effect below.
  // Re-running it would execute its cleanup, which cancels the sign-in and
  // closes the Epic window — that is what made it vanish mid-login on every
  // parent re-render (downloads, notifications, running-game polling…).
  const callbacksRef = useRef({ onSuccess, onClose });
  useEffect(() => {
    callbacksRef.current = { onSuccess, onClose };
  });

  // Listeners for the automatic Epic sign-in window
  useEffect(() => {
    let unlistenSuccess: (() => void) | null = null;
    let unlistenFailed: (() => void) | null = null;

    const setupListeners = async () => {
      unlistenSuccess = await listenAuthLoginSuccess((user) => {
        setAuthStatus('success');
        setTimeout(() => {
          callbacksRef.current.onSuccess(user);
          callbacksRef.current.onClose();
        }, 600);
      });

      unlistenFailed = await listenAuthLoginFailed((err) => {
        setIsAuthOpen(false);
        setAuthStatus('idle');
        setErrorMessage(err || 'Authentication failed. Please try again.');
      });
    };

    setupListeners();

    return () => {
      if (unlistenSuccess) unlistenSuccess();
      if (unlistenFailed) unlistenFailed();
      authCancelAutomaticLogin().catch(() => {});
    };
  }, []);

  // Epic Login handlers
  const handleStartEpicLogin = async () => {
    setIsAuthOpen(true);
    setAuthStatus('waiting');
    setErrorMessage(null);

    try {
      await authStartAutomaticLogin();
    } catch (err: any) {
      console.error('Failed to start sign in:', err);
      setIsAuthOpen(false);
      setAuthStatus('idle');
      setErrorMessage(
        err?.toString() || 'Could not launch sign-in window. You can use manual entry below.'
      );
    }
  };

  const handleCancelEpic = async () => {
    setIsAuthOpen(false);
    setAuthStatus('idle');
    try {
      await authCancelAutomaticLogin();
    } catch (err) {
      console.error('Cancel error:', err);
    }
  };

  const executeManualLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authCode.trim()) {
      setErrorMessage('Please enter an authorization code or exchange code.');
      return;
    }

    setIsLoading(true);
    setErrorMessage(null);

    try {
      let cleaned = authCode.trim();
      if (cleaned.startsWith('{') && cleaned.endsWith('}')) {
        try {
          const parsed = JSON.parse(cleaned);
          if (parsed.authorizationCode) cleaned = parsed.authorizationCode;
          else if (parsed.code) cleaned = parsed.code;
        } catch {}
      }

      const user = isExchangeCode
        ? await authLoginExchange(cleaned)
        : await authLoginCode(cleaned);

      setAuthStatus('success');
      setTimeout(() => {
        onSuccess(user);
        onClose();
      }, 500);
    } catch (err: any) {
      console.error('Login error:', err);
      setErrorMessage(
        err?.toString() || 'Authentication failed. Please check that your code is fresh.'
      );
    } finally {
      setIsLoading(false);
    }
  };

  const handleOpenBrowserFallback = async () => {
    try {
      const url = await authGetLoginUrl();
      await openPath(url);
    } catch (e) {
      console.error('Failed to open browser:', e);
      setErrorMessage('Failed to open browser. Please copy login URL manually.');
    }
  };

  // Steam: the sign-in that matters is the one authorising downloads, so hand
  // off to the SteamCMD credential prompt instead of a web-only token flow.
  const handleSteamAuthorize = () => {
    if (!onRequestSteamAuth) return;
    onClose();
    onRequestSteamAuth();
  };

  // Removes the saved password and the in-memory session. It cannot (and should
  // not) touch SteamCMD's own cached session or the Steam client's files.
  const handleDisconnectSteam = async () => {
    try {
      await steamClearCredentials();
      setSteamErrorMessage(null);
      await refreshSteamState();
    } catch (e) {
      console.error('Failed to clear Steam credentials:', e);
      setSteamErrorMessage('Could not clear the saved Steam credentials.');
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/85 backdrop-blur-md animate-fade-in">
      <div className="relative w-full max-w-lg rounded-3xl glass-panel bg-zinc-950 border border-zinc-800 shadow-2xl overflow-hidden">
        {/* Top Header Bar */}
        <div className="p-6 border-b border-zinc-800 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div
              className={`w-10 h-10 rounded-xl bg-zinc-900 border flex items-center justify-center shadow-lg transition-colors ${
                platformTab === 'steam'
                  ? 'border-sky-500/40 shadow-sky-500/10'
                  : 'border-orange-500/30 shadow-orange-500/10'
              }`}
            >
              {platformTab === 'steam' ? (
                <SteamIcon className="w-5 h-5 text-sky-400 drop-shadow-[0_0_8px_rgba(56,189,248,0.6)]" />
              ) : (
                <img
                  src="/hyperion logo.png"
                  alt="Hyperion Logo"
                  className="w-7 h-7 object-contain drop-shadow-[0_0_6px_rgba(255,102,0,0.6)]"
                />
              )}
            </div>
            <div>
              <h2 className="font-extrabold text-lg text-white">
                {platformTab === 'steam' ? 'Steam Downloads' : 'Sign In to Epic Games'}
              </h2>
              <p className="text-xs text-zinc-400">
                {platformTab === 'steam'
                  ? 'Authorise Valve downloads for this PC'
                  : 'Authenticate natively with Epic Games Store'}
              </p>
            </div>
          </div>

          <button
            onClick={() => {
              authCancelAutomaticLogin().catch(() => {});
              onClose();
            }}
            className="p-2 rounded-xl text-zinc-400 hover:text-white hover:bg-zinc-900 transition cursor-pointer"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Platform Tab Switcher */}
        <div className="px-6 pt-4 pb-2">
          <div className="grid grid-cols-2 p-1 bg-zinc-900/90 border border-zinc-800 rounded-2xl">
            <button
              type="button"
              onClick={() => setPlatformTab('epic')}
              className={`flex items-center justify-center gap-2 py-2 px-3 rounded-xl text-xs font-bold transition cursor-pointer ${
                platformTab === 'epic'
                  ? 'bg-gradient-to-r from-orange-600 to-amber-600 text-white shadow-md shadow-orange-600/20'
                  : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              <EpicGamesIcon className="w-3.5 h-3.5 fill-current" />
              <span>Epic Games</span>
            </button>

            <button
              type="button"
              onClick={() => setPlatformTab('steam')}
              className={`flex items-center justify-center gap-2 py-2 px-3 rounded-xl text-xs font-bold transition cursor-pointer ${
                platformTab === 'steam'
                  ? 'bg-gradient-to-r from-sky-600 to-blue-600 text-white shadow-md shadow-sky-600/20'
                  : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              <SteamIcon className="w-3.5 h-3.5 fill-current" />
              <span>Steam</span>
            </button>
          </div>
        </div>

        {/* Modal Body */}
        <div className="p-6 pt-2 space-y-5">
          {/* ======================================================== */}
          {/* STEAM TAB CONTENT (authorise Valve downloads)            */}
          {/* ======================================================== */}
          {platformTab === 'steam' ? (
            <div className="space-y-4">
              {/* Signed-in account and authorisation state */}
              <div className="p-4 rounded-2xl bg-zinc-900/60 border border-zinc-800 flex items-center justify-between gap-3">
                <div className="flex items-center gap-3 min-w-0">
                  <div
                    className={`w-9 h-9 rounded-xl border flex items-center justify-center shrink-0 ${
                      steamAuthorized
                        ? 'bg-emerald-500/15 border-emerald-500/40'
                        : 'bg-zinc-900 border-zinc-700'
                    }`}
                  >
                    <CheckCircle2
                      className={`w-4 h-4 ${steamAuthorized ? 'text-emerald-400' : 'text-zinc-500'}`}
                    />
                  </div>
                  <div className="min-w-0">
                    <div className="text-xs font-bold text-white truncate">
                      {steamAccount || 'Not signed in'}
                    </div>
                    <div className="text-[11px] text-zinc-400 truncate">
                      {steamAuthorized
                        ? 'Downloads authorised — session cached on this PC'
                        : 'Signed in, downloads not authorised yet'}
                    </div>
                  </div>
                </div>
                {(steamAuthorized || steamCredentials?.remembered) && (
                  <button
                    type="button"
                    onClick={handleDisconnectSteam}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-zinc-900 hover:bg-zinc-800 text-zinc-400 hover:text-rose-400 border border-zinc-800 text-xs font-medium transition cursor-pointer shrink-0"
                  >
                    <LogOut className="w-3.5 h-3.5" />
                    <span>Disconnect</span>
                  </button>
                )}
              </div>

              {/* The real sign-in flow, explained honestly */}
              <div className="p-5 rounded-2xl bg-gradient-to-br from-zinc-900/90 via-zinc-900/40 to-sky-950/25 border border-sky-500/30 space-y-3.5 shadow-xl shadow-sky-950/10">
                <div className="flex items-center gap-2">
                  <span className="px-2 py-0.5 rounded-full text-[10px] font-extrabold uppercase tracking-wider bg-sky-500/20 text-sky-400 border border-sky-500/30">
                    SteamCMD
                  </span>
                  <span className="text-xs font-bold text-zinc-200">Steam sign-in for downloads</span>
                </div>

                <p className="text-xs text-zinc-300 leading-relaxed">
                  Steam only issues download licences to a signed-in client, so Hyperion signs in to Valve's
                  headless engine with <strong className="text-white">your own account</strong>. The
                  password is typed once, kept in memory for this session — or encrypted with Windows
                  DPAPI if you tick &ldquo;Remember on this PC&rdquo; — and sent nowhere except Valve.
                  SteamCMD then caches the session, so later downloads do not ask again.
                </p>

                <button
                  type="button"
                  onClick={handleSteamAuthorize}
                  className="w-full flex items-center justify-center gap-2.5 px-5 py-3 rounded-xl bg-gradient-to-r from-sky-600 to-blue-600 hover:from-sky-500 hover:to-blue-500 text-white font-extrabold text-sm shadow-xl shadow-sky-600/30 transition hover:scale-[1.02] cursor-pointer"
                >
                  <SteamIcon className="w-4 h-4 fill-current text-white shrink-0" />
                  <span>
                    {steamAuthorized ? 'Steam Login' : 'Authorise Steam downloads'}
                  </span>
                </button>
              </div>

              {steamCredentials?.remembered && (
                <p className="text-[11px] text-zinc-500 leading-relaxed">
                  A password is stored for this Windows user (DPAPI). &ldquo;Disconnect&rdquo; deletes it
                  and the in-memory session; SteamCMD's own cached session on this PC is left alone.
                </p>
              )}

              {steamErrorMessage && (
                <div className="p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
                  <AlertCircle className="w-4 h-4 text-rose-400 flex-shrink-0" />
                  <span className="leading-tight">{steamErrorMessage}</span>
                </div>
              )}
            </div>
          ) : (
            /* ======================================================== */
            /* EPIC GAMES TAB CONTENT                                   */
            /* ======================================================== */
            isAuthOpen ? (
              <div className="p-6 rounded-2xl bg-gradient-to-b from-orange-500/10 to-zinc-900/80 border border-orange-500/30 text-center space-y-4 animate-fade-in">
                <div className="relative mx-auto w-16 h-16 flex items-center justify-center">
                  <div className="absolute inset-0 rounded-full border-2 border-orange-500/25 animate-ping" />
                  <div className="w-14 h-14 rounded-2xl bg-zinc-900 border border-orange-500/40 flex items-center justify-center shadow-xl shadow-orange-500/20">
                    {authStatus === 'success' ? (
                      <CheckCircle2 className="w-8 h-8 text-emerald-400" />
                    ) : (
                      <Loader2 className="w-8 h-8 text-orange-500 animate-spin" />
                    )}
                  </div>
                </div>

                <div>
                  <h3 className="text-base font-bold text-white">
                    {authStatus === 'success'
                      ? 'Connected Successfully!'
                      : 'Sign-In Window Active'}
                  </h3>
                  <p className="text-xs text-zinc-300 mt-1 max-w-sm mx-auto leading-relaxed">
                    {authStatus === 'success'
                      ? 'Setting up your library and session...'
                      : 'Log into Epic Games in the authentication window. Hyperion captures your session automatically â€” no copying or pasting required!'}
                  </p>
                </div>

                <div className="pt-2">
                  <button
                    type="button"
                    onClick={handleCancelEpic}
                    className="px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-300 border border-zinc-800 text-xs font-semibold transition cursor-pointer"
                  >
                    Cancel Sign-In
                  </button>
                </div>
              </div>
            ) : (
              <div className="space-y-4">
                <div className="p-5 rounded-2xl bg-gradient-to-br from-zinc-900/90 via-zinc-900/40 to-orange-950/20 border border-orange-500/30 space-y-3.5 shadow-xl shadow-orange-950/10">
                  <div className="flex items-center gap-2">
                    <span className="px-2 py-0.5 rounded-full text-[10px] font-extrabold uppercase tracking-wider bg-orange-500/20 text-orange-400 border border-orange-500/30">
                      Automatic
                    </span>
                    <span className="text-xs font-bold text-zinc-200">Zero-Copy Fast Sign In</span>
                  </div>

                  <p className="text-xs text-zinc-300 leading-relaxed">
                    Opens the official, secure Epic Games sign-in window. Once you authenticate, Hyperion automatically connects your session and closes the window.
                  </p>

                  <button
                    type="button"
                    onClick={handleStartEpicLogin}
                    className="w-full flex items-center justify-center gap-2.5 px-5 py-3 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 text-white font-extrabold text-sm shadow-xl shadow-orange-600/30 transition hover:scale-[1.02] cursor-pointer glow-orange"
                  >
                    <EpicGamesIcon className="w-4 h-4 fill-current text-white shrink-0" />
                    <span>Sign In with Epic Games</span>
                  </button>
                </div>

                {errorMessage && (
                  <div className="p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
                    <AlertCircle className="w-4 h-4 text-rose-400 flex-shrink-0" />
                    <span className="leading-tight">{errorMessage}</span>
                  </div>
                )}

                {/* Collapsible Manual Code Fallback */}
                <div className="border-t border-zinc-900 pt-2">
                  <button
                    type="button"
                    onClick={() => setShowManual(!showManual)}
                    className="w-full flex items-center justify-between text-xs text-zinc-400 hover:text-zinc-200 py-1 transition cursor-pointer"
                  >
                    <span className="font-medium">Manual code entry fallback</span>
                    {showManual ? (
                      <ChevronUp className="w-4 h-4 text-zinc-500" />
                    ) : (
                      <ChevronDown className="w-4 h-4 text-zinc-500" />
                    )}
                  </button>

                  {showManual && (
                    <form onSubmit={executeManualLogin} className="mt-3 space-y-3 animate-fade-in">
                      <div className="p-3 rounded-xl bg-zinc-900/60 border border-zinc-800/80 space-y-2">
                        <div className="text-[11px] font-bold uppercase tracking-wider text-orange-400">
                          Open In Browser
                        </div>
                        <button
                          type="button"
                          onClick={handleOpenBrowserFallback}
                          className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded-lg bg-zinc-900 hover:bg-zinc-800 text-white font-semibold text-xs border border-zinc-700/80 transition cursor-pointer"
                        >
                          <ExternalLink className="w-3.5 h-3.5 text-orange-400" />
                          <span>Open Epic Games Login in Browser</span>
                        </button>
                      </div>

                      <div className="space-y-1.5">
                        <div className="flex items-center justify-between">
                          <label className="text-[11px] font-bold uppercase tracking-wider text-orange-400">
                            Paste Code or JSON
                          </label>
                          <label className="flex items-center gap-1.5 text-[11px] text-zinc-400 cursor-pointer">
                            <input
                              type="checkbox"
                              checked={isExchangeCode}
                              onChange={(e) => setIsExchangeCode(e.target.checked)}
                              className="rounded bg-zinc-900 border-zinc-800 accent-orange-500 text-orange-500"
                            />
                            <span>Exchange Code mode</span>
                          </label>
                        </div>

                        <div className="relative">
                          <KeyRound className="w-4 h-4 absolute left-3.5 top-1/2 -translate-y-1/2 text-zinc-500" />
                          <input
                            type="text"
                            value={authCode}
                            onChange={(e) => setAuthCode(e.target.value)}
                            placeholder="Paste authorization code or JSON here..."
                            className="w-full bg-zinc-900 border border-zinc-800 rounded-xl pl-10 pr-4 py-2.5 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-orange-500 font-mono transition"
                          />
                        </div>
                      </div>

                      <div className="flex justify-end pt-1">
                        <button
                          type="submit"
                          disabled={isLoading || !authCode.trim()}
                          className="flex items-center gap-2 px-5 py-2 rounded-xl bg-orange-600 hover:bg-orange-500 text-white font-bold text-xs shadow-lg shadow-orange-600/20 transition disabled:opacity-50 cursor-pointer"
                        >
                          {isLoading ? (
                            <>
                              <Loader2 className="w-3.5 h-3.5 animate-spin" />
                              <span>Connecting...</span>
                            </>
                          ) : (
                            <span>Connect Code</span>
                          )}
                        </button>
                      </div>
                    </form>
                  )}
                </div>
              </div>
            )
          )}

          {/* Security Notice */}
          <div className="flex items-center gap-2 text-[11px] text-zinc-500 pt-2 border-t border-zinc-900">
            <Shield className="w-4 h-4 text-emerald-500/80 flex-shrink-0" />
            <span>Tokens are protected with Windows DPAPI on this PC. Steam passwords stay in memory unless you tick “Remember on this PC”.</span>
          </div>
        </div>
      </div>
    </div>
  );
};
