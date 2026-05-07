import React from 'react';
import { Film, Tv, Layers } from 'lucide-react';

interface SidebarProps {
  libraries: any[];
  selectedLibrary: number | null;
  setSelectedLibrary: (id: number | null) => void;
  moviesCount: number;
  tvShowsCount: number;
}

const Sidebar: React.FC<SidebarProps> = ({ 
  libraries, 
  selectedLibrary, 
  setSelectedLibrary, 
  moviesCount, 
  tvShowsCount 
}) => {
  return (
    <aside className="w-full md:w-64 flex-shrink-0 space-y-10 pt-4">
      <div className="space-y-4 px-2">
        <h2 className="text-[10px] font-black text-zinc-600 uppercase tracking-[0.2em] mb-4">Your Sources</h2>
        <div 
          onClick={() => setSelectedLibrary(null)} 
          className={`flex items-center gap-3 p-4 rounded-xl cursor-pointer transition-all duration-300 border ${!selectedLibrary ? 'bg-red-600 text-white shadow-xl shadow-red-900/20 border-red-500' : 'hover:bg-zinc-900 border-transparent text-zinc-400'}`}
        >
          <Layers className="w-5 h-5" />
          <span className="text-sm font-black uppercase italic tracking-tight">All Media</span>
        </div>
        {libraries.map(lib => (
          <div 
            key={lib.id} 
            onClick={() => setSelectedLibrary(lib.id)} 
            className={`flex items-center gap-3 p-4 rounded-xl cursor-pointer transition-all duration-300 border ${selectedLibrary === lib.id ? 'bg-zinc-800 text-white shadow-xl border-zinc-700' : 'hover:bg-zinc-900 border-transparent text-zinc-400'}`}
          >
            {lib.media_type === 'movie' ? <Film className="w-5 h-5" /> : <Tv className="w-5 h-5" />}
            <span className="text-sm font-black uppercase italic tracking-tight truncate">{lib.name}</span>
          </div>
        ))}
      </div>

      <div className="px-6 py-6 bg-zinc-900/30 rounded-2xl border border-zinc-800/50 mx-2 space-y-4">
        <h3 className="text-[10px] font-black text-zinc-600 uppercase tracking-widest">Storage Status</h3>
        <div className="flex justify-between items-end">
          <span className="text-2xl font-black text-white italic">{moviesCount + tvShowsCount}</span>
          <span className="text-[10px] font-black text-zinc-500 uppercase mb-1">Titles</span>
        </div>
        <div className="h-1.5 w-full bg-zinc-800 rounded-full overflow-hidden">
          <div className="h-full bg-red-600 w-2/3 shadow-[0_0_10px_rgba(220,38,38,0.5)]" />
        </div>
      </div>
    </aside>
  );
};

export default Sidebar;
