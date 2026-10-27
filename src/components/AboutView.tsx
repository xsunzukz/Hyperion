import React, { useState } from 'react';
import {
  Shield,
  ExternalLink,
  ChevronRight,
  X,
  FileText,
  Code2,
  GitBranch,
  Lock,
} from 'lucide-react';

/** Project repository: shown on the About page and opened by the source card. */
export const MY_PROJECT_REPO_URL = 'https://github.com/xsunzukz/Hyperion';

// 🔗 Original Research & Project Credits
export const LEGENDARY_REPO_URL = 'https://github.com/legendary-gl/legendary';

export const AboutView: React.FC = () => {
  const [activeModal, setActiveModal] = useState<'terms' | 'privacy' | 'license' | 'credits' | null>(null);

  const openExternal = async (url: string) => {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(url);
    } catch {
      window.open(url, '_blank', 'noreferrer');
    }
  };

  const handleOpenMyRepo = () => {
    openExternal(MY_PROJECT_REPO_URL);
  };

  return (
    <div className="flex-1 overflow-y-auto p-8 space-y-6 max-w-4xl mx-auto">
      {/* Top Header Card */}
      <div className="rounded-2xl border border-zinc-800/80 bg-zinc-950/70 p-6 flex flex-col sm:flex-row items-start sm:items-center gap-5 shadow-2xl relative overflow-hidden backdrop-blur-md">
        {/* Subtle orange/amber ambient glow */}
        <div className="absolute top-0 left-0 w-64 h-64 bg-orange-500/10 rounded-full blur-3xl pointer-events-none" />

        {/* Icon Emblem */}
        <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-[#1c1208] to-[#0a0603] border border-orange-500/30 flex items-center justify-center p-3 relative overflow-hidden shadow-[0_0_30px_rgba(255,102,0,0.2)] shrink-0">
          <img
            src="/hyperion logo.png"
            alt="Hyperion"
            className="w-14 h-14 object-contain drop-shadow-[0_0_12px_rgba(255,120,20,0.7)]"
          />
        </div>

        {/* Info */}
        <div className="space-y-2.5 flex-1 relative z-10">
          <h2 className="font-serif text-2xl sm:text-3xl font-extrabold tracking-widest text-white uppercase drop-shadow-sm">
            HYPERION
          </h2>
          <p className="text-zinc-300 text-sm leading-relaxed font-normal">
            A blazing-fast, native desktop game launcher for the Epic Games platform, built from scratch with Rust.
          </p>

          {/* Badges */}
          <div className="flex items-center gap-2.5 pt-1">
            <span className="px-3.5 py-1 rounded-full text-xs font-mono font-medium bg-orange-950/50 border border-orange-500/35 text-orange-400 tracking-wide shadow-sm">
              Launcher v1.0.2
            </span>
          </div>
        </div>
      </div>

      {/* Links & Legal List Section */}
      <div className="rounded-2xl border border-zinc-800/80 bg-zinc-950/60 p-2.5 space-y-2 shadow-2xl backdrop-blur-md">
        {/* 1. Project source code */}
        <button
          onClick={handleOpenMyRepo}
          className="w-full flex items-center justify-between p-4 rounded-xl border border-zinc-800/60 bg-zinc-900/40 hover:bg-zinc-800/50 hover:border-zinc-700/80 transition-all duration-200 group text-left cursor-pointer"
          title={`Open ${MY_PROJECT_REPO_URL} in your browser`}
        >
          <div className="flex items-center gap-4">
            <div className="w-10 h-10 rounded-xl bg-zinc-900/80 border border-zinc-800 flex items-center justify-center text-zinc-400 group-hover:text-orange-400 group-hover:border-orange-500/30 transition">
              <GitBranch className="w-5 h-5" />
            </div>
            <div>
              <div className="text-sm font-semibold text-white group-hover:text-orange-300 transition">
                Hyperion source code
              </div>
              <div className="text-xs font-mono text-zinc-400 group-hover:text-zinc-300 transition">
                {MY_PROJECT_REPO_URL.replace(/^https?:\/\//, '')}
              </div>
            </div>
          </div>
          <ExternalLink className="w-4 h-4 text-zinc-500 group-hover:text-zinc-200 transition mr-2" />
        </button>

        {/* 2. Credits to Legendary */}
        <button
          onClick={() => openExternal(LEGENDARY_REPO_URL)}
          className="w-full flex items-center justify-between p-4 rounded-xl border border-zinc-800/60 bg-zinc-900/40 hover:bg-zinc-800/50 hover:border-zinc-700/80 transition-all duration-200 group text-left cursor-pointer"
        >
          <div className="flex items-center gap-4">
            <div className="w-10 h-10 rounded-xl bg-zinc-900/80 border border-zinc-800 flex items-center justify-center text-zinc-400 group-hover:text-orange-400 group-hover:border-orange-500/30 transition">
              <Code2 className="w-5 h-5 text-orange-400/90" />
            </div>
            <div>
              <div className="text-sm font-semibold text-white group-hover:text-orange-300 transition flex items-center gap-2">
                <span>Credits & Inspiration: Legendary</span>
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-zinc-800 text-zinc-300 border border-zinc-700">
                  derrod
                </span>
              </div>
              <div className="text-xs text-zinc-400">
                Open-source protocol research & reverse engineering (legendary-gl)
              </div>
            </div>
          </div>
          <ExternalLink className="w-4 h-4 text-zinc-500 group-hover:text-zinc-200 transition mr-2" />
        </button>

        {/* 3. License */}
        <button
          onClick={() => setActiveModal('license')}
          className="w-full flex items-center justify-between p-4 rounded-xl border border-zinc-800/60 bg-zinc-900/40 hover:bg-zinc-800/50 hover:border-zinc-700/80 transition-all duration-200 group text-left cursor-pointer"
        >
          <div className="flex items-center gap-4">
            <div className="w-10 h-10 rounded-xl bg-zinc-900/80 border border-zinc-800 flex items-center justify-center text-zinc-400 group-hover:text-orange-400 group-hover:border-orange-500/30 transition">
              <Shield className="w-5 h-5" />
            </div>
            <div>
              <div className="text-sm font-semibold text-white group-hover:text-orange-300 transition">
                GPL-3.0 license
              </div>
              <div className="text-xs text-zinc-400">Your rights to this software</div>
            </div>
          </div>
          <ExternalLink className="w-4 h-4 text-zinc-500 group-hover:text-zinc-200 transition mr-2" />
        </button>

        {/* 4. Terms of Use */}
        <button
          onClick={() => setActiveModal('terms')}
          className="w-full flex items-center justify-between p-4 rounded-xl border border-zinc-800/60 bg-zinc-900/40 hover:bg-zinc-800/50 hover:border-zinc-700/80 transition-all duration-200 group text-left cursor-pointer"
        >
          <div className="flex items-center gap-4">
            <div className="w-10 h-10 rounded-xl bg-zinc-900/80 border border-zinc-800 flex items-center justify-center text-zinc-400 group-hover:text-orange-400 group-hover:border-orange-500/30 transition">
              <FileText className="w-5 h-5" />
            </div>
            <div>
              <div className="text-sm font-semibold text-white group-hover:text-orange-300 transition">
                Terms of Use
              </div>
              <div className="text-xs text-zinc-400">Independent client & legal disclaimer</div>
            </div>
          </div>
          <ChevronRight className="w-4 h-4 text-zinc-500 group-hover:text-zinc-200 transition mr-2" />
        </button>

        {/* 5. Privacy Policy */}
        <button
          onClick={() => setActiveModal('privacy')}
          className="w-full flex items-center justify-between p-4 rounded-xl border border-zinc-800/60 bg-zinc-900/40 hover:bg-zinc-800/50 hover:border-zinc-700/80 transition-all duration-200 group text-left cursor-pointer"
        >
          <div className="flex items-center gap-4">
            <div className="w-10 h-10 rounded-xl bg-zinc-900/80 border border-zinc-800 flex items-center justify-center text-zinc-400 group-hover:text-orange-400 group-hover:border-orange-500/30 transition">
              <Lock className="w-5 h-5" />
            </div>
            <div>
              <div className="text-sm font-semibold text-white group-hover:text-orange-300 transition">
                Privacy Policy
              </div>
              <div className="text-xs text-zinc-400">Zero telemetry & local credential encryption</div>
            </div>
          </div>
          <ChevronRight className="w-4 h-4 text-zinc-500 group-hover:text-zinc-200 transition mr-2" />
        </button>
      </div>

      {/* Modal View for Terms / Privacy / License */}
      {activeModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-in fade-in duration-200">
          <div className="w-full max-w-2xl bg-zinc-950 border border-zinc-800 rounded-2xl shadow-2xl flex flex-col max-h-[85vh] overflow-hidden">
            {/* Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-zinc-800/80 bg-zinc-900/40">
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-xl bg-orange-500/10 border border-orange-500/20 text-orange-400 flex items-center justify-center">
                  <FileText className="w-5 h-5" />
                </div>
                <div>
                  <h3 className="font-bold text-base text-white">
                    {activeModal === 'terms' && 'Terms of Use'}
                    {activeModal === 'privacy' && 'Privacy Policy'}
                    {activeModal === 'license' && 'GNU General Public License v3.0'}
                  </h3>
                  <p className="text-xs text-zinc-400">
                    {activeModal === 'terms' && 'Attribution & Software Terms'}
                    {activeModal === 'privacy' && 'Local Security & Data Protection'}
                    {activeModal === 'license' && 'GPL-3.0 or later'}
                  </p>
                </div>
              </div>
              <button
                onClick={() => setActiveModal(null)}
                className="w-8 h-8 rounded-lg bg-zinc-900 hover:bg-zinc-800 border border-zinc-800 text-zinc-400 hover:text-white flex items-center justify-center transition cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Scrollable Body */}
            <div className="p-6 overflow-y-auto space-y-4 text-xs text-zinc-300 leading-relaxed font-sans">
              {activeModal === 'terms' && (
                <>
                  <div className="p-3.5 rounded-xl bg-orange-500/10 border border-orange-500/20 text-orange-300">
                    <strong>Independent Native Client:</strong> Hyperion is a clean-room, native Rust application for launching games and managing libraries on the Epic Games platform.
                  </div>
                  <h4 className="font-bold text-sm text-white pt-2">
                    1. Research & Open-Source Credits
                  </h4>
                  <p>
                    Protocol details, binary manifest structures, and chunk serialization algorithms are based on reverse-engineering research from the <strong>Legendary project</strong> created by <strong>derrod</strong> (licensed under GNU GPL v3.0 or later). We gratefully acknowledge their open-source research and contributions to gaming interoperability.
                  </p>
                  <h4 className="font-bold text-sm text-white pt-2">2. Independent Implementation</h4>
                  <p>
                    Hyperion was written from scratch in Rust and TypeScript. It does not bundle or invoke any external CLI wrappers or Python runtimes. All network requests, manifest decoders, and file assembly routines execute natively.
                  </p>
                  <h4 className="font-bold text-sm text-white pt-2">3. Intellectual Property</h4>
                  <p>
                    Epic Games, the Epic Games Store, and all affiliated game titles, assets, and trademarks are the registered property of Epic Games, Inc. and their respective copyright holders. Hyperion is an independent client and is not affiliated with or endorsed by Epic Games, Inc.
                  </p>
                  <h4 className="font-bold text-sm text-white pt-2">4. Disclaimer of Warranties</h4>
                  <p>
                    THE SOFTWARE IS PROVIDED &quot;AS IS&quot;, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM OR DAMAGES.
                  </p>
                </>
              )}

              {activeModal === 'privacy' && (
                <>
                  <div className="p-3.5 rounded-xl bg-emerald-950/20 border border-emerald-500/20 text-emerald-300">
                    <strong>Zero-Telemetry Policy:</strong> Hyperion does not collect, record, track, or transmit your private data, games catalog, or usage habits.
                  </div>
                  <h4 className="font-bold text-sm text-white pt-2">1. Local Storage Only</h4>
                  <p>
                    All authentication session tokens, cached games metadata, and user settings are stored locally on your device in a private SQLite database. No external telemetry servers or advertising trackers exist in this codebase.
                  </p>
                  <h4 className="font-bold text-sm text-white pt-2">2. Direct Network Connections</h4>
                  <p>
                    All API queries and chunk downloads communicate directly between your machine and official Epic Games CDN servers using TLS (via Rustls). No intermediate proxies or third-party servers inspect your connections.
                  </p>
                  <h4 className="font-bold text-sm text-white pt-2">3. User Control & Data Clearing</h4>
                  <p>
                    You retain total control over your credentials. Logging out instantly deletes your session token from local storage.
                  </p>
                </>
              )}

              {activeModal === 'license' && (
                <>
                  <div className="p-3.5 rounded-xl bg-zinc-900 border border-zinc-800 text-zinc-300">
                    <strong>GNU General Public License v3.0:</strong> Ensures that users have the freedom to distribute, inspect, modify, and run this software.
                  </div>
                  <h4 className="font-bold text-sm text-white pt-2">Key Rights & Conditions</h4>
                  <ul className="list-disc pl-5 space-y-1">
                    <li>Freedom to run the program for any purpose.</li>
                    <li>Freedom to study how the program works and change it.</li>
                    <li>Freedom to redistribute copies to help others.</li>
                    <li>Modifications and derivative distributions must also be shared under the same GPL-3.0 license.</li>
                  </ul>
                  <div className="pt-3">
                    <button
                      onClick={() => openExternal('https://www.gnu.org/licenses/gpl-3.0.html')}
                      className="px-4 py-2 rounded-xl bg-orange-500 hover:bg-orange-600 text-white font-semibold flex items-center gap-2 transition cursor-pointer"
                    >
                      <span>View Full License Text on GNU.org</span>
                      <ExternalLink className="w-4 h-4" />
                    </button>
                  </div>
                </>
              )}
            </div>

            {/* Footer */}
            <div className="px-6 py-3.5 border-t border-zinc-800/80 bg-zinc-900/40 flex justify-end">
              <button
                onClick={() => setActiveModal(null)}
                className="px-5 py-2 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-white font-medium transition cursor-pointer"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
