import React, { useState, useEffect } from 'react';
import { Cloud, RefreshCw, FolderOpen, CheckCircle2, AlertCircle } from 'lucide-react';
import { CloudSaveState, GameItem } from '../types';
import { cloudSaveGetStatus, cloudSaveSync, openPath } from '../lib/ipc';

interface CloudSavesViewProps {
  games: GameItem[];
}

export const CloudSavesView: React.FC<CloudSavesViewProps> = ({ games }) => {
  const [saveStates, setSaveStates] = useState<Record<string, CloudSaveState>>({});
  const [loading, setLoading] = useState(false);
  const [syncingGame, setSyncingGame] = useState<string | null>(null);

  const supportedGames = games.filter((g) => g.supports_cloud_saves);

  const loadAllStatuses = async () => {
    setLoading(true);
    const map: Record<string, CloudSaveState> = {};
    for (const g of supportedGames) {
      try {
        const state = await cloudSaveGetStatus(g.app_name);
        map[g.app_name] = state;
      } catch (e) {
        console.error('Error fetching save status:', e);
      }
    }
    setSaveStates(map);
    setLoading(false);
  };

  useEffect(() => {
    if (supportedGames.length > 0) {
      loadAllStatuses();
    }
  }, [games]);

  const handleSync = async (appName: string) => {
    setSyncingGame(appName);
    try {
      await cloudSaveSync(appName);
      const updated = await cloudSaveGetStatus(appName);
      setSaveStates((prev) => ({ ...prev, [appName]: updated }));
    } catch (e) {
      console.error('Sync failed:', e);
    } finally {
      setSyncingGame(null);
    }
  };

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-extrabold text-white tracking-tight">Cloud Save Synchronization</h2>
          <p className="text-sm text-zinc-400 mt-1">
            Seamless synchronization between local save paths and Epic datastorage savesync.
          </p>
        </div>

        <button
          onClick={loadAllStatuses}
          disabled={loading}
          className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-zinc-900 hover:bg-zinc-800 text-zinc-200 border border-zinc-800 text-xs font-semibold transition disabled:opacity-50 cursor-pointer"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin text-orange-400' : ''}`} />
          <span>Refresh All</span>
        </button>
      </div>

      {supportedGames.length === 0 ? (
        <div className="p-12 text-center rounded-3xl glass-panel border border-zinc-800/80 bg-zinc-950/40">
          <Cloud className="w-12 h-12 text-zinc-600 mx-auto mb-3" />
          <h3 className="text-base font-bold text-white">No Cloud Save Enabled Games</h3>
          <p className="text-xs text-zinc-500 mt-1">
            Install titles with cloud save support to view sync states.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {supportedGames.map((game) => {
            const state = saveStates[game.app_name];
            const isSyncing = syncingGame === game.app_name;

            return (
              <div
                key={game.app_name}
                className="p-5 rounded-2xl glass-card border border-zinc-800/80 bg-zinc-950/80 hover:border-orange-500/30 transition-all duration-300 space-y-4 shadow-xl"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <img
                      src={game.icon_url || 'https://images.unsplash.com/photo-1550745165-9bc0b252726f?auto=format&fit=crop&w=200&q=80'}
                      alt={game.title}
                      className="w-11 h-11 rounded-xl object-cover shadow border border-zinc-800"
                    />
                    <div>
                      <h3 className="font-bold text-sm text-white">{game.title}</h3>
                      <div className="text-[11px] text-zinc-400 font-mono truncate max-w-[200px]">
                        {game.app_name}
                      </div>
                    </div>
                  </div>

                  <button
                    onClick={() => handleSync(game.app_name)}
                    disabled={isSyncing}
                    className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-gradient-to-r from-orange-600 to-amber-600 hover:from-orange-500 hover:to-amber-500 text-white font-bold text-xs shadow-md shadow-orange-600/20 transition disabled:opacity-50 cursor-pointer glow-orange"
                  >
                    <RefreshCw className={`w-3.5 h-3.5 ${isSyncing ? 'animate-spin' : ''}`} />
                    <span>{isSyncing ? 'Syncing...' : 'Sync'}</span>
                  </button>
                </div>

                {state ? (
                  <div className="space-y-2 text-xs">
                    <div className="p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800/80 flex items-center justify-between">
                      <span className="text-zinc-400 font-medium">Status</span>
                      <span
                        className={`font-semibold flex items-center gap-1.5 ${
                          state.in_sync ? 'text-orange-400' : 'text-amber-400'
                        }`}
                      >
                        {state.in_sync ? (
                          <CheckCircle2 className="w-3.5 h-3.5 text-orange-400" />
                        ) : (
                          <AlertCircle className="w-3.5 h-3.5 text-amber-400" />
                        )}
                        {state.status_message}
                      </span>
                    </div>

                    <div className="grid grid-cols-2 gap-2 text-zinc-300">
                      <div className="p-2.5 rounded-xl bg-zinc-900/40 border border-zinc-800/60">
                        <div className="text-[10px] text-zinc-400 font-medium uppercase tracking-wider">Local Saves</div>
                        <div className="font-bold text-sm text-white mt-0.5">
                          {state.local_files_count} files
                        </div>
                      </div>
                      <div className="p-2.5 rounded-xl bg-zinc-900/40 border border-zinc-800/60">
                        <div className="text-[10px] text-zinc-400 font-medium uppercase tracking-wider">Cloud Saves</div>
                        <div className="font-bold text-sm text-white mt-0.5">
                          {state.cloud_files_count} files
                        </div>
                      </div>
                    </div>

                    {state.local_path && (
                      <div className="flex items-center justify-between text-[11px] text-zinc-400 pt-1">
                        <span className="truncate max-w-[240px] font-mono">{state.local_path}</span>
                        <button
                          onClick={() => openPath(state.local_path!)}
                          className="p-1 text-zinc-400 hover:text-orange-400 transition cursor-pointer"
                          title="Open Save Folder"
                        >
                          <FolderOpen className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="text-xs text-zinc-500 animate-pulse">Inspecting save game path...</div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
