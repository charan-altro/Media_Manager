import React from 'react';
import { Play, CheckCircle2, RefreshCw, Star, Layers, ChevronDown } from 'lucide-react';
import { getImageUrl, api } from '../api/adapter';

interface MediaGridProps {
  title: string;
  icon: React.ReactNode;
  items: any[];
  itemCount: number;
  onItemClick: (item: any, e: React.MouseEvent) => void;
  onPlayClick: (item: any, e: React.MouseEvent) => void;
  selectedIds: number[];
  selectionMode: boolean;
  setSelectionMode: (mode: boolean) => void;
  setSelectedIds: (ids: number[] | ((prev: number[]) => number[])) => void;
  selectedLibrary: number | null;
  genreFilter: string;
  setGenreFilter: (genre: string) => void;
  languageFilter: string;
  setLanguageFilter: (lang: string) => void;
  allGenres: string[];
  allLanguages: string[];
  showFilterMenu: boolean;
  setShowFilterMenu: (show: boolean) => void;
}

const MediaGrid: React.FC<MediaGridProps> = ({
  title, icon, items, itemCount, onItemClick, onPlayClick,
  selectedIds, selectionMode, setSelectionMode, setSelectedIds,
  selectedLibrary, genreFilter, setGenreFilter, languageFilter, setLanguageFilter,
  allGenres, allLanguages, showFilterMenu, setShowFilterMenu
}) => {
  return (
    <div className="px-4 md:px-12 py-12 space-y-10">
      <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-6">
        <div>
          <h3 className="text-2xl font-black text-white uppercase italic tracking-tighter flex items-center gap-3">
            {icon} {title}
          </h3>
          <p className="text-zinc-500 text-sm font-medium">{itemCount} total titles identified</p>
        </div>
        <div className="flex flex-wrap gap-4">
          <div className="relative">
            <button 
              onClick={() => setShowFilterMenu(!showFilterMenu)}
              className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-zinc-400 hover:text-white transition group bg-zinc-900/50 px-4 py-2 rounded-full border border-zinc-800"
            >
              Filters <ChevronDown className="w-3 h-3" />
            </button>
            {showFilterMenu && (
              <div className="absolute right-0 top-full mt-2 w-64 bg-zinc-900 border border-zinc-800 rounded-lg p-4 z-50 shadow-xl space-y-4">
                <div>
                  <label className="text-xs text-zinc-500 font-bold uppercase mb-2 block">Genre</label>
                  <select 
                    value={genreFilter}
                    onChange={e => setGenreFilter(e.target.value)}
                    className="w-full bg-black/50 border border-zinc-800 rounded px-3 py-1.5 text-sm text-white focus:border-red-500 focus:outline-none"
                  >
                    <option value="">All Genres</option>
                    {allGenres.map(g => <option key={g} value={g}>{g}</option>)}
                  </select>
                </div>
                <div>
                  <label className="text-xs text-zinc-500 font-bold uppercase mb-2 block">Language</label>
                  <select 
                    value={languageFilter}
                    onChange={e => setLanguageFilter(e.target.value)}
                    className="w-full bg-black/50 border border-zinc-800 rounded px-3 py-1.5 text-sm text-white focus:border-red-500 focus:outline-none"
                  >
                    <option value="">All Languages</option>
                    {allLanguages.map(l => <option key={l} value={l}>{l}</option>)}
                  </select>
                </div>
                <button onClick={() => { setGenreFilter(''); setLanguageFilter(''); setShowFilterMenu(false); }} className="w-full text-xs py-1.5 text-zinc-400 hover:text-white transition">Clear Filters</button>
              </div>
            )}
          </div>
          <button 
            onClick={() => {
              setSelectionMode(!selectionMode);
              if (selectionMode) setSelectedIds([]);
            }}
            className={`flex items-center gap-2 text-xs font-black uppercase tracking-widest transition group px-4 py-2 rounded-full border ${selectionMode ? 'bg-red-600 text-white border-red-500' : 'text-zinc-400 hover:text-white bg-zinc-900/50 border-zinc-800'}`}
          >
            Select
          </button>
          {selectionMode && (
            <button 
              onClick={() => {
                const allVisibleIds = items.map(m => m.id);
                if (selectedIds.length === allVisibleIds.length) {
                  setSelectedIds([]);
                } else {
                  setSelectedIds(allVisibleIds);
                }
              }}
              className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-zinc-400 hover:text-white transition group bg-zinc-900/50 px-4 py-2 rounded-full border border-zinc-800"
            >
              {selectedIds.length === items.length && items.length > 0 ? 'Deselect All' : 'Select All'}
            </button>
          )}
          {selectedLibrary && (
            <>
              <button 
                onClick={() => api.startScan(selectedLibrary)}
                className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-zinc-400 hover:text-white transition group bg-zinc-900/50 px-4 py-2 rounded-full border border-zinc-800"
              >
                <RefreshCw className="w-3 h-3 group-hover:rotate-180 transition-transform duration-500" /> Rescan Source
              </button>
              <button 
                onClick={() => api.request('scrape_batch', `/libraries/${selectedLibrary}/scrape`, { method: 'POST' })}
                className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-red-500 hover:text-red-400 transition group bg-red-950/10 px-4 py-2 rounded-full border border-red-900/20"
              >
                <Star className="w-3 h-3" /> Match Unmatched
              </button>
            </>
          )}
        </div>
      </div>
      
      <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7 gap-y-10 gap-x-4 md:gap-x-6">
        {items.map(item => (
          <div 
            key={item.id} 
            onClick={(e) => onItemClick(item, e)}
            className="group relative cursor-pointer space-y-3"
          >
            <div className={`aspect-[2/3] rounded-lg overflow-hidden shadow-lg transition-all duration-500 group-hover:scale-[1.03] group-hover:-translate-y-2 group-hover:shadow-[0_20px_50px_rgba(0,0,0,0.5)] border ${selectedIds.includes(item.id) ? 'border-red-500 ring-2 ring-red-500/50' : 'border-zinc-900 group-hover:border-zinc-700'}`}>
              <img 
                src={getImageUrl(item.poster_url)} 
                className="w-full h-full object-cover group-hover:brightness-110 transition duration-500"
                alt={item.title}
                loading="lazy"
              />
              <div className="absolute inset-0 bg-gradient-to-t from-black via-transparent to-transparent opacity-0 group-hover:opacity-100 transition duration-500" />
              <div className="absolute bottom-0 left-0 right-0 p-4 translate-y-full group-hover:translate-y-0 transition duration-500">
                <div className="flex items-center justify-between">
                   <div 
                     onClick={(e) => onPlayClick(item, e)}
                     className="w-8 h-8 rounded-full bg-white/20 hover:bg-white hover:text-black backdrop-blur-md flex items-center justify-center border border-white/20 transition group/play"
                   >
                     <Play className="w-3 h-3 text-white group-hover/play:text-black fill-current translate-x-0.5 transition" />
                   </div>
                   {item.status === 'matched' && <CheckCircle2 className="w-4 h-4 text-green-500 drop-shadow" />}
                </div>
              </div>
            </div>
            <div>
              <h4 className="text-sm font-black text-zinc-100 line-clamp-1 group-hover:text-red-500 transition uppercase italic tracking-tight">{item.title}</h4>
              <div className="flex items-center gap-2 text-[10px] font-bold text-zinc-500">
                <span>{item.year}</span>
                <span>•</span>
                <span className={item.status === 'matched' ? 'text-green-500/80' : 'text-zinc-600'}>
                  {item.status === 'matched' ? `${Math.round((item.rating || 0) * 10)}% Match` : 'Unmatched'}
                </span>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default MediaGrid;
