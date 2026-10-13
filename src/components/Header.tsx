import React from 'react';
import { Search, LayoutGrid, List, RefreshCw, X, FileCode2 } from 'lucide-react';
import { ViewMode } from '../types';

interface HeaderProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  selectedFilter: 'all' | 'installed' | 'uninstalled';
  onFilterChange: (filter: 'all' | 'installed' | 'uninstalled') => void;
  platformFilter?: 'all' | 'epic' | 'steam';
  onPlatformFilterChange?: (platform: 'all' | 'epic' | 'steam') => void;
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  onRefresh: () => void;
  isRefreshing: boolean;
  onOpenCustomManifest: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  searchQuery,
  onSearchChange,
  selectedFilter,
  onFilterChange,
  platformFilter = 'all',
  onPlatformFilterChange,
  viewMode,
  onViewModeChange,
  onRefresh,
  isRefreshing,
  onOpenCustomManifest,
}) => {
  // Full labels on wide windows, short ones when the sidebar eats the space
  const platforms = [
    { id: 'all', label: 'All Platforms', shortLabel: 'All' },
    { id: 'epic', label: 'Epic Games', shortLabel: 'Epic' },
    { id: 'steam', label: 'Steam', shortLabel: 'Steam' },
  ] as const;

  return (
    <header className="h-18 px-6 flex flex-nowrap items-center gap-3 border-b border-zinc-800/80 glass-panel bg-[#07070a]/90 z-10 sticky top-0">
      {/* Search Bar — shrinks first so the pills never wrap */}
      <div className="relative w-64 min-w-[7.5rem] max-w-64 shrink">
        <Search className="w-4 h-4 absolute left-3.5 top-1/2 -translate-y-1/2 text-zinc-500" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Search games, developers..."
          className="w-full bg-zinc-950/90 border border-zinc-800 rounded-xl pl-10 pr-9 py-2 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-orange-500 focus:ring-1 focus:ring-orange-500/40 transition"
        />
        {searchQuery && (
          <button
            onClick={() => onSearchChange('')}
            className="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-white"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      {/* Filters and Controls */}
      <div className="flex shrink-0 items-center justify-end gap-2.5">
        {/* Platform Filter Pills */}
        {onPlatformFilterChange && (
          <div className="flex shrink-0 items-center p-1 bg-zinc-950 rounded-xl border border-zinc-800">
            {platforms.map((p) => (
              <button
                key={p.id}
                onClick={() => onPlatformFilterChange(p.id)}
                className={`px-2.5 py-1.5 rounded-lg text-[11px] font-bold whitespace-nowrap transition cursor-pointer ${
                  platformFilter === p.id
                    ? p.id === 'steam'
                      ? 'bg-sky-600 text-white shadow-md shadow-sky-600/30'
                      : 'bg-gradient-to-r from-orange-600 to-amber-600 text-white shadow-md shadow-orange-600/25'
                    : 'text-zinc-400 hover:text-white'
                }`}
              >
                <span className="xl:hidden">{p.shortLabel}</span>
                <span className="hidden xl:inline">{p.label}</span>
              </button>
            ))}
          </div>
        )}

        {/* Status Filter Pills */}
        <div className="flex shrink-0 items-center p-1 bg-zinc-950 rounded-xl border border-zinc-800">
          {(['all', 'installed', 'uninstalled'] as const).map((filter) => (
            <button
              key={filter}
              onClick={() => onFilterChange(filter)}
              className={`px-2.5 py-1.5 rounded-lg text-[11px] font-bold capitalize whitespace-nowrap transition cursor-pointer ${
                selectedFilter === filter
                  ? 'bg-gradient-to-r from-orange-600 to-amber-600 text-white shadow-md shadow-orange-600/25'
                  : 'text-zinc-400 hover:text-white'
              }`}
            >
              {filter}
            </button>
          ))}
        </div>

        {/* View Mode Toggle */}
        <div className="flex shrink-0 items-center p-1 bg-zinc-950 rounded-xl border border-zinc-800">
          <button
            onClick={() => onViewModeChange('grid')}
            className={`p-1.5 rounded-lg transition cursor-pointer ${
              viewMode === 'grid'
                ? 'bg-zinc-800 text-orange-400'
                : 'text-zinc-500 hover:text-white'
            }`}
            title="Grid View"
          >
            <LayoutGrid className="w-4 h-4" />
          </button>
          <button
            onClick={() => onViewModeChange('list')}
            className={`p-1.5 rounded-lg transition cursor-pointer ${
              viewMode === 'list'
                ? 'bg-zinc-800 text-orange-400'
                : 'text-zinc-500 hover:text-white'
            }`}
            title="List View"
          >
            <List className="w-4 h-4" />
          </button>
        </div>

        {/* Install from Custom Manifest or Steam App ID — label only when there is room */}
        <button
          onClick={onOpenCustomManifest}
          className="flex shrink-0 items-center gap-2 px-2.5 xl:px-3 py-1.5 rounded-xl bg-zinc-900/90 hover:bg-zinc-800 border border-orange-500/30 hover:border-orange-500 text-white text-[11px] font-semibold whitespace-nowrap transition cursor-pointer shadow-sm hover:shadow-orange-500/20"
          title="Download any game build by .manifest or Steam App ID"
        >
          <FileCode2 className="w-3.5 h-3.5 text-orange-400" />
          <span className="hidden xl:inline">Direct Downloader</span>
        </button>

        {/* Refresh Library */}
        <button
          onClick={onRefresh}
          disabled={isRefreshing}
          className="p-2 shrink-0 rounded-xl bg-zinc-950 border border-zinc-800 text-zinc-400 hover:text-white hover:border-zinc-700 transition disabled:opacity-50 cursor-pointer"
          title="Refresh Library Catalog"
        >
          <RefreshCw className={`w-4 h-4 ${isRefreshing ? 'animate-spin text-orange-400' : ''}`} />
        </button>
      </div>
    </header>
  );
};
