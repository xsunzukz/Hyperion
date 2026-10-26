import React from 'react';
import {
  Compass,
  Gamepad2,
  Download,
  Cloud,
  Settings,
  Info,
  ChevronRight,
  PanelLeftClose,
  PanelLeftOpen,
  Square,
  UserRound,
} from 'lucide-react';
import { NavigationTab, RunningGame } from '../types';

interface SidebarProps {
  activeTab: NavigationTab;
  onTabChange: (tab: NavigationTab) => void;
  activeDownloadsCount: number;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  runningGames?: RunningGame[];
  onStopGame?: (appName: string) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeTab,
  onTabChange,
  activeDownloadsCount,
  collapsed,
  onToggleCollapsed,
  runningGames = [],
  onStopGame,
}) => {
  const navItems = [
    { id: 'home' as NavigationTab, label: 'Discover', icon: Compass },
    { id: 'library' as NavigationTab, label: 'My Library', icon: Gamepad2 },
    {
      id: 'downloads' as NavigationTab,
      label: 'Downloads',
      icon: Download,
      badge: activeDownloadsCount > 0 ? activeDownloadsCount : undefined,
    },
    { id: 'cloud-saves' as NavigationTab, label: 'Cloud Saves', icon: Cloud },
    { id: 'settings' as NavigationTab, label: 'Settings', icon: Settings },
    { id: 'about' as NavigationTab, label: 'About', icon: Info },
  ];

  const visibleRunning = collapsed ? runningGames.slice(0, 3) : runningGames;
  const hiddenRunningCount = runningGames.length - visibleRunning.length;

  return (
    <aside
      className={`flex-shrink-0 flex flex-col justify-between glass-panel border-r border-zinc-800/80 bg-[#07070a]/95 z-20 overflow-hidden transition-[width] duration-300 ease-out ${collapsed ? 'w-[4.5rem]' : 'w-64'
        }`}
    >
      <div className="min-w-0 flex-1 flex flex-col min-h-0">
        {/* Brand Header with the Hyperion logo */}
        <div
          className={`h-18 flex items-center border-b border-zinc-800/80 bg-black/40 shrink-0 ${collapsed ? 'justify-center px-2' : 'gap-3.5 px-5'
            }`}
        >
          <div className="w-10 h-10 rounded-xl bg-black border border-orange-500/40 flex items-center justify-center shadow-lg shadow-orange-500/20 overflow-hidden shrink-0">
            <img
              src="/hyperion logo.png"
              alt="Hyperion Logo"
              className="w-8 h-8 object-contain drop-shadow-[0_0_8px_rgba(255,102,0,0.6)]"
            />
          </div>
          {!collapsed && (
            <div className="min-w-0">
              <div className="font-extrabold text-sm tracking-wide text-white flex items-center gap-1.5 truncate">
                <span>HYPERION</span>
              </div>
              <div className="text-[10px] text-zinc-400 font-medium truncate">
                Native Game Launcher
              </div>
            </div>
          )}
        </div>

        {/* Navigation Items */}
        <nav className={`space-y-1.5 overflow-y-auto flex-1 min-h-0 ${collapsed ? 'p-2' : 'p-3.5'}`}>
          {!collapsed && (
            <div className="px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest text-zinc-400">
              Navigation
            </div>
          )}
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onTabChange(item.id)}
                title={collapsed ? item.label : undefined}
                className={`relative w-full flex items-center rounded-xl font-medium text-xs transition-all duration-200 cursor-pointer ${collapsed ? 'justify-center px-0 py-2.5' : 'justify-between px-3.5 py-2.5'
                  } ${isActive
                    ? 'bg-gradient-to-r from-orange-600 to-amber-600 text-white shadow-lg shadow-orange-600/30 font-bold border border-orange-400/40'
                    : 'text-zinc-400 hover:text-white hover:bg-zinc-900/70 border border-transparent'
                  }`}
              >
                <div className={`flex items-center ${collapsed ? '' : 'gap-3'}`}>
                  <Icon className={`w-4 h-4 shrink-0 ${isActive ? 'text-white' : 'text-zinc-400'}`} />
                  {!collapsed && <span className="truncate">{item.label}</span>}
                </div>
                {item.badge !== undefined &&
                  (collapsed ? (
                    <span className="absolute right-1.5 top-1.5 w-1.5 h-1.5 rounded-full bg-white shadow-sm" />
                  ) : (
                    <span className="px-2 py-0.5 text-[10px] font-extrabold rounded-full bg-white text-black shadow-sm">
                      {item.badge}
                    </span>
                  ))}
              </button>
            );
          })}

          {/* Games running right now — stop them straight from the launcher */}
          {runningGames.length > 0 && (
            <div
              className={`${collapsed ? 'pt-2 mt-1 border-t border-zinc-800/70 space-y-1' : 'pt-3 mt-1 border-t border-zinc-800/70'}`}
            >
              {!collapsed && (
                <div className="px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest text-emerald-400/90 flex items-center gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse inline-block" />
                  Running now
                </div>
              )}
              <div className="space-y-1">
                {visibleRunning.map((game) => (
                  <div
                    key={game.app_name}
                    className={`group flex items-center rounded-lg border border-transparent hover:border-zinc-800 hover:bg-zinc-900/70 transition ${collapsed ? 'justify-center p-1' : 'gap-2 px-2.5 py-1.5'
                      }`}
                  >
                    {!collapsed && (
                      <>
                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse shrink-0" />
                        <span
                          className="flex-1 truncate text-[11px] font-semibold text-zinc-300"
                          title={game.title}
                        >
                          {game.title}
                        </span>
                      </>
                    )}
                    <button
                      onClick={() => onStopGame?.(game.app_name)}
                      title={`Stop ${game.title}`}
                      aria-label={`Stop ${game.title}`}
                      className={`relative flex items-center justify-center rounded-md border border-rose-500/30 bg-rose-600/10 text-rose-300 hover:bg-rose-600 hover:text-white hover:border-rose-500 transition cursor-pointer ${collapsed ? 'w-9 h-9' : 'w-6 h-6'
                        }`}
                    >
                      <Square className={collapsed ? 'w-3 h-3 fill-current' : 'w-2.5 h-2.5 fill-current'} />
                      {collapsed && (
                        <span className="absolute -top-1 -right-1 w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                      )}
                    </button>
                  </div>
                ))}
                {hiddenRunningCount > 0 && (
                  <div
                    className="text-center text-[10px] font-semibold text-zinc-500 py-1"
                    title={runningGames.map((g) => g.title).join(', ')}
                  >
                    +{hiddenRunningCount}
                  </div>
                )}
              </div>
            </div>
          )}
        </nav>
      </div>

      <div className="shrink-0 border-t border-zinc-800/70 bg-black/40 p-2.5">
        {/* Account management replaces the old profile card */}
        <button
          onClick={() => onTabChange('account')}
          title="Account Management"
          aria-label="Account Management"
          aria-current={activeTab === 'account' ? 'page' : undefined}
          className={`group flex w-full items-center rounded-xl border transition focus:outline-none focus-visible:ring-2 focus-visible:ring-orange-500/50 ${collapsed ? 'justify-center px-0 py-2' : 'gap-2.5 px-2 py-2'
            } ${activeTab === 'account'
              ? 'border-orange-500/30 bg-orange-500/10 text-white'
              : 'border-zinc-800/70 bg-zinc-900/40 text-zinc-300 hover:border-zinc-700 hover:bg-zinc-900/80'
            }`}
        >
          <span
            className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border transition ${activeTab === 'account'
                ? 'border-orange-400/50 bg-orange-500/20'
                : 'border-zinc-800 bg-zinc-900 group-hover:border-orange-500/30'
              }`}
          >
            <UserRound
              className={`h-4 w-4 ${activeTab === 'account'
                  ? 'text-orange-300'
                  : 'text-zinc-400 group-hover:text-orange-400'
                }`}
            />
          </span>
          {!collapsed && (
            <>
              <span className="min-w-0 flex-1 text-left">
                <span
                  className={`block truncate text-[11px] font-bold ${activeTab === 'account' ? 'text-white' : 'text-zinc-200'
                    }`}
                >
                  Account Management
                </span>
                <span className="block truncate text-[10px] font-medium text-zinc-500">
                  Manage all your accounts.
                </span>
              </span>
              <ChevronRight
                className={`h-3.5 w-3.5 shrink-0 ${activeTab === 'account'
                    ? 'text-orange-300'
                    : 'text-zinc-600 group-hover:text-zinc-400'
                  }`}
              />
            </>
          )}
        </button>

        {!collapsed ? (
          <div className="mt-1 flex justify-end">
            <button
              onClick={onToggleCollapsed}
              title="Collapse sidebar (Ctrl+B)"
              aria-label="Collapse sidebar"
              className="flex items-center gap-1.5 rounded-md px-1.5 py-1 text-[10px] font-medium text-zinc-600 transition hover:bg-zinc-900/60 hover:text-zinc-300 focus:outline-none focus-visible:ring-1 focus-visible:ring-orange-500/40"
            >
              <PanelLeftClose className="h-3.5 w-3.5" />
              <span>Collapse</span>
            </button>
          </div>
        ) : (
          <button
            onClick={onToggleCollapsed}
            title="Expand sidebar (Ctrl+B)"
            aria-label="Expand sidebar"
            className="mt-1 flex w-full items-center justify-center rounded-lg py-2 text-zinc-500 transition hover:bg-zinc-900 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-orange-500/40"
          >
            <PanelLeftOpen className="h-4 w-4" />
          </button>
        )}
      </div>
    </aside>
  );
};
