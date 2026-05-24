import React, { useState, useRef, useEffect } from 'react';
import { Play, RefreshCw, Star, ChevronDown, Filter, LayoutGrid } from 'lucide-react';
import { getImageUrl, api } from '../api/adapter';
import { useMediaStore } from '../context/MediaStoreContext';

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

// ─── Individual Media Card (With watch progress) ───────────────────────────
const MediaCard: React.FC<{
  item: any;
  onItemClick: (item: any, e: React.MouseEvent) => void;
  onPlayClick: (item: any, e: React.MouseEvent) => void;
  selectedIds: number[];
  hoveredId: number | null;
  setHoveredId: (id: number | null) => void;
  hoverTimeout: React.MutableRefObject<any>;
}> = ({
  item, onItemClick, onPlayClick, selectedIds,
  hoveredId, setHoveredId, hoverTimeout
}) => {
  const [playback, setPlayback] = useState<any>(null);
  const isShow = 'library_id' in item && !('runtime' in item);
  const type = isShow ? 'tv' : 'movie';

  useEffect(() => {
    let isMounted = true;
    if (!isShow) {
      api.getPlaybackStatus(type, item.id)
        .then(status => {
          if (isMounted && status && !status.is_finished && status.position_ms > 5000) {
            setPlayback(status);
          } else if (isMounted) {
            setPlayback(null);
          }
        })
        .catch(() => {});
    }
    return () => { isMounted = false; };
  }, [item.id, type, isShow]);

  const isSelected = selectedIds.includes(item.id);

  return (
    <div 
      onClick={(e) => onItemClick(item, e)}
      onMouseEnter={() => {
        hoverTimeout.current = setTimeout(() => setHoveredId(item.id), 500);
      }}
      onMouseLeave={() => {
        clearTimeout(hoverTimeout.current);
        setHoveredId(null);
      }}
      className="group relative cursor-pointer flex flex-col justify-between"
    >
      <div className={`relative aspect-[2/3] rounded-xl overflow-hidden bg-zinc-900 border transition-all duration-500 group-hover:scale-[1.04] group-hover:-translate-y-2 group-hover:shadow-[0_20px_50px_rgba(0,0,0,0.7)] ${
        isSelected 
          ? 'border-red-500 ring-2 ring-red-500/50' 
          : 'border-zinc-900 group-hover:border-zinc-700/80'
      }`}>
        
        {hoveredId === item.id && item.preview_path ? (
          <div className="absolute inset-0 z-10 bg-black animate-in fade-in duration-500">
            <video 
              src={getImageUrl(item.preview_path)}
              autoPlay
              loop
              muted
              className="w-full h-full object-cover"
            />
          </div>
        ) : (
          <img 
            src={getImageUrl(item.poster_url)} 
            className="w-full h-full object-cover group-hover:scale-[1.02] group-hover:brightness-110 transition duration-700 ease-out"
            alt={item.title}
            loading="lazy"
          />
        )}

        {/* Shadow overlays */}
        <div className="absolute inset-0 bg-gradient-to-t from-black/90 via-black/10 to-transparent opacity-0 group-hover:opacity-100 transition duration-500 z-20" />
        
        {/* Play Action Hover overlay */}
        <div className="absolute bottom-0 left-0 right-0 p-4 translate-y-full group-hover:translate-y-0 transition duration-500 ease-out z-30 flex items-center justify-between">
           <div 
             onClick={(e) => onPlayClick(item, e)}
             className="w-10 h-10 rounded-full bg-white/20 hover:bg-white text-white hover:text-black backdrop-blur-md flex items-center justify-center border border-white/30 transition duration-300 active:scale-90 shadow-2xl group/play"
           >
             <Play className="w-4 h-4 fill-current translate-x-0.5" />
           </div>
           {item.status === 'matched' && (
             <div className="bg-black/50 backdrop-blur-md px-2 py-1 rounded border border-white/5 flex items-center gap-1">
               <span className="text-[9px] font-black text-green-400">MATCH</span>
             </div>
           )}
        </div>

        {/* Netflix/YouTube styled progress bar */}
        {playback && (
          <div className="absolute bottom-0 left-0 w-full h-1 bg-zinc-800/80 z-30">
            <div 
              className="h-full bg-red-600 shadow-[0_0_8px_#dc2626]" 
              style={{ width: `${(playback.position_ms / playback.duration_ms) * 100}%` }}
            />
          </div>
        )}
      </div>

      {/* Title metadata details below poster card */}
      <div className="mt-3.5 space-y-1">
        <h4 className="text-sm font-bold text-zinc-100 line-clamp-1 group-hover:text-red-500 transition duration-300 uppercase italic tracking-wide">
          {item.title}
        </h4>
        <div className="flex items-center gap-2 text-[10px] font-bold text-zinc-500">
          <span className="bg-zinc-900/80 px-1.5 py-0.5 rounded border border-zinc-800">{item.year}</span>
          <span>•</span>
          <span className={item.status === 'matched' ? 'text-green-500 font-extrabold' : 'text-zinc-600'}>
            {item.status === 'matched' ? `${Math.round((item.rating || 0) * 10)}% Match` : 'Unmatched'}
          </span>
        </div>
      </div>
    </div>
  );
};

// ─── Main Media Grid Component ──────────────────────────────────────────────
const MediaGrid: React.FC<MediaGridProps> = ({
  title, icon, items, itemCount, onItemClick, onPlayClick,
  selectedIds, selectionMode, setSelectionMode, setSelectedIds,
  selectedLibrary, genreFilter, setGenreFilter, languageFilter, setLanguageFilter,
  allGenres, allLanguages, showFilterMenu, setShowFilterMenu
}) => {
  const { libraries, setSelectedLibrary } = useMediaStore();
  const [hoveredId, setHoveredId] = useState<number | null>(null);
  const hoverTimeout = useRef<any>(null);
  const [gridSize, setGridSize] = useState(6); // Default to 6 columns

  const getGridCols = () => {
    const mapping: Record<number, string> = {
      2: 'grid-cols-2',
      3: 'grid-cols-2 sm:grid-cols-3',
      4: 'grid-cols-2 sm:grid-cols-3 md:grid-cols-4',
      5: 'grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5',
      6: 'grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6',
      7: 'grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7',
      8: 'grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-7 2xl:grid-cols-8',
      9: 'grid-cols-3 sm:grid-cols-4 md:grid-cols-6 lg:grid-cols-7 xl:grid-cols-8 2xl:grid-cols-9',
      10: 'grid-cols-4 sm:grid-cols-5 md:grid-cols-7 lg:grid-cols-8 xl:grid-cols-9 2xl:grid-cols-10',
    };
    return mapping[gridSize] || 'grid-cols-6';
  };

  return (
    <div className="px-4 md:px-12 py-12 space-y-8">
      {/* Immersive Sources / Library Tabs Selector */}
      {libraries && libraries.length > 0 && (
        <div className="flex flex-wrap items-center gap-2 border-b border-zinc-900 pb-5">
          <button
            onClick={() => setSelectedLibrary(null)}
            className={`px-5 py-2.5 rounded-lg text-xs font-black uppercase tracking-wider transition-all duration-300 active:scale-95 border cursor-pointer ${
              selectedLibrary === null
                ? "bg-red-650 text-white border-red-500 shadow-lg shadow-red-600/20"
                : "bg-zinc-900/40 text-zinc-500 hover:text-zinc-350 hover:bg-zinc-800/40 border-zinc-850/50"
            }`}
          >
            All Sources
          </button>
          {libraries.map((lib) => (
            <button
              key={lib.id}
              onClick={() => setSelectedLibrary(lib.id)}
              className={`px-5 py-2.5 rounded-lg text-xs font-black uppercase tracking-wider transition-all duration-300 active:scale-95 border cursor-pointer ${
                selectedLibrary === lib.id
                  ? "bg-zinc-800 text-white border-zinc-700/60 shadow-md"
                  : "bg-zinc-900/40 text-zinc-500 hover:text-zinc-350 hover:bg-zinc-800/40 border-zinc-850/50"
              }`}
            >
              {lib.name}
            </button>
          ))}
          {/* Storage / Identified count badge aligned to the right */}
          <div className="ml-auto bg-zinc-900/50 border border-zinc-800/60 px-4 py-2 rounded-full flex items-center gap-2 shadow-inner">
            <span className="h-2 w-2 rounded-full bg-red-600 animate-pulse shadow-[0_0_8px_#dc2626]" />
            <span className="text-[10px] font-mono font-black text-zinc-400 uppercase tracking-widest">
              {itemCount} Titles Identified
            </span>
          </div>
        </div>
      )}

      {/* Title & Actions Bar */}
      <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-6">
        <div>
          <h3 className="text-2xl font-black text-white uppercase italic tracking-tighter flex items-center gap-3">
            {icon} {title}
          </h3>
          <p className="text-zinc-500 text-sm font-medium">{itemCount} total titles identified</p>
        </div>
        
        <div className="flex flex-wrap items-center gap-4">
          {/* YouTube-style Grid Slider */}
          <div className="flex items-center gap-3 bg-zinc-900/50 px-4 py-2 rounded-full border border-zinc-800/80">
            <LayoutGrid className="w-3.5 h-3.5 text-zinc-500" />
            <input 
              type="range" 
              min="2" 
              max="10" 
              value={gridSize} 
              onChange={(e) => setGridSize(parseInt(e.target.value))}
              className="w-20 h-1 bg-zinc-800 rounded-lg appearance-none cursor-pointer accent-red-600"
            />
          </div>

          <div className="relative">
            <button 
              onClick={() => setShowFilterMenu(!showFilterMenu)}
              className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-zinc-400 hover:text-white transition group bg-zinc-900/50 px-4 py-2 rounded-full border border-zinc-800/80"
            >
              <Filter className="w-3 h-3" /> Language <ChevronDown className="w-3 h-3" />
            </button>
            {showFilterMenu && (
              <div className="absolute right-0 top-full mt-2 w-64 bg-zinc-950 border border-zinc-800 rounded-2xl p-4 z-50 shadow-2xl space-y-4 animate-in fade-in duration-200">
                <div>
                  <label className="text-[10px] text-zinc-500 font-bold uppercase tracking-widest mb-2 block">Filter Language</label>
                  <select 
                    value={languageFilter}
                    onChange={e => setLanguageFilter(e.target.value)}
                    className="w-full bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-xs text-white focus:border-red-500 focus:outline-none"
                  >
                    <option value="">All Languages</option>
                    {allLanguages.map(l => <option key={l} value={l}>{l}</option>)}
                  </select>
                </div>
                <button 
                  onClick={() => { setLanguageFilter(''); setShowFilterMenu(false); }} 
                  className="w-full text-center text-[10px] font-black uppercase tracking-wider py-2 bg-zinc-900 hover:bg-zinc-800 rounded-lg text-zinc-400 hover:text-white transition"
                >
                  Clear Language Filter
                </button>
              </div>
            )}
          </div>
          
          <button 
            onClick={() => {
              setSelectionMode(!selectionMode);
              if (selectionMode) setSelectedIds([]);
            }}
            className={`flex items-center gap-2 text-xs font-black uppercase tracking-widest transition group px-4 py-2 rounded-full border ${
              selectionMode 
                ? 'bg-red-600 text-white border-red-500 shadow-lg shadow-red-900/20' 
                : 'text-zinc-400 hover:text-white bg-zinc-900/50 border-zinc-800/80'
            }`}
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
              className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-zinc-400 hover:text-white transition group bg-zinc-900/50 px-4 py-2 rounded-full border border-zinc-800/80"
            >
              {selectedIds.length === items.length && items.length > 0 ? 'Deselect All' : 'Select All'}
            </button>
          )}
          
          {selectedLibrary && (
            <>
              <button 
                onClick={() => api.startScan(selectedLibrary)}
                className="flex items-center gap-2 text-xs font-black uppercase tracking-widest text-zinc-400 hover:text-white transition group bg-zinc-900/50 px-4 py-2 rounded-full border border-zinc-800/80"
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

      {/* YouTube-style Horizontal Scrollable Genre Tag Pills */}
      {allGenres.length > 0 && (
        <div className="flex items-center gap-2 overflow-x-auto py-2 -mx-4 px-4 scroll-smooth [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none] border-y border-zinc-900/30 bg-zinc-950/10">
          <button
            onClick={() => setGenreFilter("")}
            className={`px-4 py-1.5 rounded-full text-xs font-bold transition-all duration-300 whitespace-nowrap active:scale-95 border ${
              genreFilter === ""
                ? "bg-white text-zinc-950 border-white shadow-lg shadow-white/5"
                : "bg-zinc-900/40 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 border-zinc-800/50"
            }`}
          >
            All Genres
          </button>
          {allGenres.map((genre) => (
            <button
              key={genre}
              onClick={() => setGenreFilter(genre)}
              className={`px-4 py-1.5 rounded-full text-xs font-bold transition-all duration-300 whitespace-nowrap active:scale-95 border ${
                genreFilter === genre
                  ? "bg-white text-zinc-950 border-white shadow-lg shadow-white/5"
                  : "bg-zinc-900/40 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 border-zinc-800/50"
              }`}
            >
              {genre}
            </button>
          ))}
        </div>
      )}
      
      {/* Media Grid Cards */}
      <div className={`grid ${getGridCols()} gap-y-10 gap-x-4 md:gap-x-6`}>
        {items.map(item => (
          <MediaCard
            key={item.id}
            item={item}
            onItemClick={onItemClick}
            onPlayClick={onPlayClick}
            selectedIds={selectedIds}
            hoveredId={hoveredId}
            setHoveredId={setHoveredId}
            hoverTimeout={hoverTimeout}
          />
        ))}
      </div>

      {/* Empty State */}
      {items.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 border border-zinc-900 rounded-3xl bg-zinc-950/20">
           <div className="w-16 h-16 bg-zinc-900/60 rounded-2xl flex items-center justify-center mb-6 border border-zinc-800/60 shadow-inner">
              <Star className="w-8 h-8 text-zinc-700" />
           </div>
           <h4 className="text-lg font-black text-white uppercase italic tracking-tighter mb-2">No Titles Found</h4>
           <p className="text-zinc-500 text-sm max-w-xs text-center font-medium">We couldn't find any media matching your current filters or search query.</p>
           {(genreFilter || languageFilter) && (
             <button 
               onClick={() => { setGenreFilter(''); setLanguageFilter(''); }}
               className="mt-6 text-red-500 text-xs font-black uppercase tracking-widest hover:text-red-400 transition"
             >
               Clear Active Filters
             </button>
           )}
        </div>
      )}
    </div>
  );
};

export default MediaGrid;
