import React, { useState, useEffect } from 'react';
import {
  Save,
  Folder,
  FolderOpen,
  HardDrive,
  Cpu,
  Globe,
  CheckCircle2,
  ShieldCheck,
  Info,
  LogOut,
  RefreshCw,
  Shield,
} from 'lucide-react';
import { AppSettings, SteamAuthStatus } from '../types';
import {
  selectDirectory,
  settingsGet,
  settingsSave,
  steamGetAuthStatus,
  steamLogout,
} from '../lib/ipc';

interface SettingsViewProps {
  /** Opens the in-app Steam sign-in modal (no web/Valve window) */
  onRequestSteamLogin?: () => void;
  /** Bumped by the shell after a successful sign-in so the card refreshes */
  steamAuthVersion?: number;
  /** Lets the shell reload the library (the Steam scan toggle changes it) */
  onSettingsSaved?: (settings: AppSettings) => void;
}

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

export const SettingsView: React.FC<SettingsViewProps> = ({
  onRequestSteamLogin,
  steamAuthVersion = 0,
  onSettingsSaved,
}) => {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(true);

  // Steam Auth state
  const [steamAuth, setSteamAuth] = useState<SteamAuthStatus | null>(null);

  const fetchAuthStatus = async () => {
    try {
      const status = await steamGetAuthStatus();
      setSteamAuth(status);
    } catch (e) {
      console.warn('Failed to load Steam auth status:', e);
    }
  };

  useEffect(() => {
    const fetch = async () => {
      try {
        const s = await settingsGet();
        setSettings(s);
        await fetchAuthStatus();
      } catch (e) {
        console.error('Failed to load settings:', e);
      } finally {
        setLoading(false);
      }
    };
    fetch();
  }, []);

  // Refresh the card whenever the shell reports a new Steam sign-in
  useEffect(() => {
    fetchAuthStatus();
  }, [steamAuthVersion]);

  const handleBrowseDirectory = async () => {
    try {
      const selected = await selectDirectory(settings?.default_install_dir || 'D:\\Games');
      if (selected && settings) {
        setSettings({ ...settings, default_install_dir: selected });
      }
    } catch (e) {
      console.error('Failed to open directory picker:', e);
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!settings) return;
    try {
      await settingsSave(settings);
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
      onSettingsSaved?.(settings);
    } catch (e) {
      console.error('Failed to save settings:', e);
    }
  };

  // Every Steam sign-in in the launcher goes through the in-app credential
  // modal — there is no Valve web window or QR flow.
  const handleStartSteamLogin = () => {
    onRequestSteamLogin?.();
  };

  const handleSteamLogout = async () => {
    try {
      await steamLogout();
      await fetchAuthStatus();
    } catch (e) {
      console.error('Failed to sign out of Steam:', e);
    }
  };

  if (loading || !settings) {
    return <div className="p-8 text-zinc-400 font-mono text-sm">Loading configuration...</div>;
  }

  return (
    <div className="p-8 max-w-4xl space-y-6">
      <div>
        <h2 className="text-2xl font-extrabold text-white tracking-tight">Launcher Settings</h2>
        <p className="text-sm text-zinc-400 mt-1">
          Configure default installation locations, download worker pools, and system integration.
        </p>
      </div>

      <form onSubmit={handleSave} className="space-y-6">
        {/* Installation Path */}
        <div className="p-6 rounded-2xl glass-panel border border-zinc-800/80 bg-zinc-950/80 space-y-4">
          <div className="flex items-center justify-between">
            <label className="flex items-center gap-2 text-sm font-bold text-white">
              <Folder className="w-4 h-4 text-orange-400" />
              <span>Default Installation Directory</span>
            </label>
          </div>

          <div className="flex gap-2.5">
            <input
              type="text"
              value={settings.default_install_dir}
              onChange={(e) => setSettings({ ...settings, default_install_dir: e.target.value })}
              className="flex-1 bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-sm text-white focus:outline-none focus:border-orange-500 font-mono transition"
            />
            <button
              type="button"
              onClick={handleBrowseDirectory}
              className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-200 border border-zinc-800 text-xs font-semibold transition hover:border-orange-500/40 cursor-pointer"
              title="Browse folder on your computer"
            >
              <FolderOpen className="w-4 h-4 text-orange-400" />
              <span>Browse...</span>
            </button>
          </div>

          {/* Quick Drive Presets */}
          <div className="flex flex-wrap items-center gap-2 pt-1">
            <span className="text-[11px] font-semibold text-zinc-400">Quick Drive Presets:</span>
            {['C:\\Games', 'D:\\Games', 'E:\\Games'].map((preset) => {
              const isSelected = settings.default_install_dir === preset;
              return (
                <button
                  key={preset}
                  type="button"
                  onClick={() => setSettings({ ...settings, default_install_dir: preset })}
                  className={`flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-mono font-bold transition cursor-pointer border ${
                    isSelected
                      ? 'bg-orange-500/20 text-orange-400 border-orange-500/50 shadow-sm shadow-orange-500/20'
                      : 'bg-zinc-900 hover:bg-zinc-800 text-zinc-300 border-zinc-800 hover:border-zinc-700'
                  }`}
                >
                  <HardDrive className="w-3.5 h-3.5 text-orange-400" />
                  <span>{preset}</span>
                </button>
              );
            })}
          </div>

          <p className="text-xs text-zinc-400">
            All game titles, DLCs, and custom manifest versions will be installed under this root directory.
          </p>
        </div>

        {/* Performance & Downloader */}
        <div className="p-6 rounded-2xl glass-panel border border-zinc-800/80 bg-zinc-950/80 space-y-5">
          <label className="flex items-center gap-2 text-sm font-bold text-white">
            <Cpu className="w-4 h-4 text-orange-400" />
            <span>Downloader & Worker Threads</span>
          </label>

          <div className="space-y-2">
            <div className="flex justify-between text-xs text-zinc-300">
              <span className="font-semibold">Concurrent Chunk Worker Threads</span>
              <span className="font-mono text-orange-400 font-bold">{settings.max_workers} Workers</span>
            </div>
            <input
              type="range"
              min="1"
              max="16"
              value={settings.max_workers}
              onChange={(e) => setSettings({ ...settings, max_workers: parseInt(e.target.value) })}
              className="w-full accent-orange-500 cursor-pointer"
            />
            <p className="text-xs text-zinc-400">
              Controls maximum concurrent HTTP requests to game CDN nodes. Recommended: 8-12.
            </p>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-3 border-t border-zinc-800/80">
            <div>
              <label className="block text-xs font-semibold text-zinc-300 mb-1.5">
                Bandwidth Limit (MB/s, 0 = Unlimited)
              </label>
              <input
                type="number"
                min="0"
                value={settings.throttle_mbps}
                onChange={(e) => setSettings({ ...settings, throttle_mbps: parseInt(e.target.value) || 0 })}
                className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2 text-sm text-white focus:outline-none focus:border-orange-500 font-mono transition"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-zinc-300 mb-1.5">
                Target Platform
              </label>
              <select
                value={settings.platform}
                onChange={(e) => setSettings({ ...settings, platform: e.target.value })}
                className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2 text-sm text-white focus:outline-none focus:border-orange-500 transition cursor-pointer"
              >
                <option value="Windows">Windows (Win64 / Direct3D)</option>
                <option value="Mac">macOS (Metal / Native)</option>
              </select>
            </div>
          </div>
        </div>

        {/* Language & Updates */}
        <div className="p-6 rounded-2xl glass-panel border border-zinc-800/80 bg-zinc-950/80 space-y-4">
          <label className="flex items-center gap-2 text-sm font-bold text-white">
            <Globe className="w-4 h-4 text-orange-400" />
            <span>Localization & Execution</span>
          </label>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-semibold text-zinc-300 mb-1.5">
                Game Language Parameter
              </label>
              <select
                value={settings.language}
                onChange={(e) => setSettings({ ...settings, language: e.target.value })}
                className="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2 text-sm text-white focus:outline-none focus:border-orange-500 transition cursor-pointer"
              >
                <option value="en">English (en)</option>
                <option value="de">Deutsch (de)</option>
                <option value="fr">Français (fr)</option>
                <option value="es">Español (es)</option>
                <option value="ja">日本語 (ja)</option>
              </select>
            </div>
          </div>

          <div className="pt-3 border-t border-zinc-800/80 space-y-2">
            <label className="flex items-center gap-2.5 text-xs text-zinc-300 cursor-pointer">
              <input
                type="checkbox"
                checked={settings.skip_update_check}
                onChange={(e) => setSettings({ ...settings, skip_update_check: e.target.checked })}
                className="rounded bg-zinc-900 border-zinc-800 accent-orange-500 text-orange-500 focus:ring-orange-500"
              />
              <span>Skip update checks on game launch (faster startup)</span>
            </label>
          </div>
        </div>

        {/* Steam Token Authentication Card - ZERO ID/PASSWORD INPUTS */}
        <div className="p-6 rounded-2xl glass-panel border border-sky-500/30 bg-zinc-950/80 space-y-4 shadow-lg shadow-sky-950/20">
          <div className="flex items-center justify-between">
            <label className="flex items-center gap-2.5 text-sm font-bold text-white">
              <SteamIcon className="w-4 h-4 text-sky-400" />
              <span>Steam Token Authentication</span>
            </label>
            {steamAuth?.logged_in ? (
              <span className="flex items-center gap-1.5 px-3 py-1 rounded-full text-[11px] font-bold bg-emerald-950/80 text-emerald-300 border border-emerald-500/40">
                <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
                <span>Connected via Steam Token</span>
              </span>
            ) : (
              <span className="flex items-center gap-1.5 px-3 py-1 rounded-full text-[11px] font-medium bg-zinc-900 text-zinc-400 border border-zinc-800">
                <Info className="w-3.5 h-3.5 text-zinc-400" />
                <span>Anonymous Mode (Free Games Only)</span>
              </span>
            )}
          </div>

          <p className="text-xs text-zinc-400 leading-relaxed">
            Link your Steam account to enable direct 1-click in-launcher downloads for owned games. Signing in
            opens the in-app Steam prompt — your password goes only to Valve's download service and is kept in
            memory for this session unless you tick &ldquo;Remember on this PC&rdquo;.
          </p>

          {steamAuth?.logged_in ? (
            <div className="p-4 rounded-xl bg-zinc-900/70 border border-zinc-800 flex flex-wrap items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-sky-500/20 border border-sky-500/30 flex items-center justify-center">
                  <SteamIcon className="w-5 h-5 text-sky-400" />
                </div>
                <div>
                  <div className="text-xs font-bold text-white flex items-center gap-2">
                    <span>{steamAuth.account_name || 'Steam Account'}</span>
                    <span className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-sky-500/20 text-sky-300 border border-sky-500/30">
                      TOKEN ACTIVE
                    </span>
                  </div>
                  <div className="text-[11px] text-zinc-400 font-mono mt-0.5">
                    SteamID: {steamAuth.steam_id || 'Auto-Detected'}
                  </div>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={handleStartSteamLogin}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 text-xs font-semibold transition cursor-pointer"
                >
                  <RefreshCw className="w-3.5 h-3.5 text-sky-400" />
                  <span>Steam Login</span>
                </button>
                <button
                  type="button"
                  onClick={handleSteamLogout}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-zinc-900 hover:bg-rose-950/40 text-zinc-400 hover:text-rose-400 border border-zinc-800 hover:border-rose-500/30 text-xs font-semibold transition cursor-pointer"
                >
                  <LogOut className="w-3.5 h-3.5" />
                  <span>Disconnect</span>
                </button>
              </div>
            </div>
          ) : (
            <div>
              <button
                type="button"
                onClick={handleStartSteamLogin}
                className="flex items-center gap-2.5 px-5 py-2.5 rounded-xl bg-gradient-to-r from-sky-600 to-blue-600 hover:from-sky-500 hover:to-blue-500 text-white font-bold text-xs shadow-lg shadow-sky-600/20 transition hover:scale-105 cursor-pointer"
              >
                <SteamIcon className="w-4 h-4 fill-current text-white" />
                <span>Steam Login</span>
              </button>
            </div>
          )}

          <div className="pt-3 border-t border-zinc-800/80 space-y-1.5">
            <label className="flex items-center gap-2.5 text-xs text-zinc-300 cursor-pointer">
              <input
                type="checkbox"
                checked={settings.steam_library_scan}
                onChange={(e) => setSettings({ ...settings, steam_library_scan: e.target.checked })}
                className="rounded bg-zinc-900 border-zinc-800 accent-sky-500 text-sky-500 focus:ring-sky-500"
              />
              <span>Scan this PC&rsquo;s Steam library</span>
            </label>
            <p className="text-[11px] text-zinc-500 leading-relaxed pl-6">
              Lists the games your Steam client has installed or owns by reading Steam&rsquo;s library files
              (libraryfolders.vdf, app manifests, userdata). Off by default, because some antivirus products flag
              apps that read these files.
            </p>
          </div>

          <div className="flex items-center gap-2 text-[11px] text-zinc-500 pt-1 border-t border-zinc-900">
            <Shield className="w-3.5 h-3.5 text-sky-500/70 shrink-0" />
            <span>Credentials go only to Valve's download service — never to Hyperion. Encrypted with Windows DPAPI when remembered.</span>
          </div>
        </div>

        {/* Submit Bar */}
        <div className="flex items-center justify-between pt-2">
          <button
            type="submit"
            className="flex items-center gap-2 px-6 py-3 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 text-white font-extrabold text-sm shadow-xl shadow-orange-600/30 transition hover:scale-105 cursor-pointer glow-orange"
          >
            <Save className="w-4 h-4" />
            <span>Save Preferences</span>
          </button>

          {saved && (
            <div className="flex items-center gap-2 text-xs font-semibold text-orange-400 bg-orange-500/10 border border-orange-500/30 px-4 py-2.5 rounded-xl">
              <CheckCircle2 className="w-4 h-4 text-orange-400" />
              <span>Settings saved successfully!</span>
            </div>
          )}
        </div>
      </form>
    </div>
  );
};
