import React, { useState, useEffect } from 'react';
import {
  Save,
  Folder,
  FolderOpen,
  HardDrive,
  Cpu,
  Globe,
  CheckCircle2,
  Minimize2,
} from 'lucide-react';
import { AppSettings } from '../types';
import { selectDirectory, settingsGet, settingsSave } from '../lib/ipc';

interface SettingsViewProps {
  /** Lets the shell reload the library after preferences change */
  onSettingsSaved?: (settings: AppSettings) => void;
}

export const SettingsView: React.FC<SettingsViewProps> = ({ onSettingsSaved }) => {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetch = async () => {
      try {
        const s = await settingsGet();
        setSettings(s);
      } catch (e) {
        console.error('Failed to load settings:', e);
      } finally {
        setLoading(false);
      }
    };
    fetch();
  }, []);

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
              max="64"
              value={settings.max_workers}
              onChange={(e) => setSettings({ ...settings, max_workers: parseInt(e.target.value) })}
              className="w-full accent-orange-500 cursor-pointer"
            />
            <p className="text-xs text-zinc-400">
              Controls maximum concurrent HTTP requests to game CDN nodes. More workers hide slow CDN
              chunks and usually mean more throughput; recommended 32, drop to 8–16 on a slow or
              shared connection.
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

        {/* Window & tray behavior */}
        <div className="p-6 rounded-2xl glass-panel border border-zinc-800/80 bg-zinc-950/80 space-y-4">
          <label className="flex items-center gap-2 text-sm font-bold text-white">
            <Minimize2 className="w-4 h-4 text-orange-400" />
            <span>Window &amp; Tray</span>
          </label>

          <div className="flex items-start justify-between gap-5 pt-1">
            <div className="min-w-0">
              <div className="text-xs font-semibold text-zinc-200">
                Close app to minimize to system tray
              </div>
              <p className="mt-1 max-w-lg text-xs leading-relaxed text-zinc-400">
                Closing the window keeps Hyperion running in the system tray, so active downloads
                and running games carry on. Turn this off to quit Hyperion completely when the
                window is closed.
              </p>
              <p className="mt-2 text-[11px] text-zinc-500">
                {settings.close_to_tray
                  ? 'Hyperion will keep running in the background when closed.'
                  : 'Closing the window will exit Hyperion and stop active downloads.'}
              </p>
            </div>

            <button
              type="button"
              role="switch"
              aria-checked={settings.close_to_tray}
              aria-label="Close app to minimize to system tray"
              onClick={() =>
                setSettings({ ...settings, close_to_tray: !settings.close_to_tray })
              }
              className={`relative mt-0.5 inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-orange-500/60 ${
                settings.close_to_tray
                  ? 'border-orange-400/50 bg-orange-500/80'
                  : 'border-zinc-700 bg-zinc-800'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 rounded-full bg-white shadow-md transition-transform ${
                  settings.close_to_tray ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
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
