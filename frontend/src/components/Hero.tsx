import React from 'react';
import { Play, Info, Star } from 'lucide-react';
import { getImageUrl } from '../api/adapter';

interface HeroProps {
  item: any;
  onPlay: (item: any) => void;
  onDetails: (item: any) => void;
}

const Hero: React.FC<HeroProps> = ({ item, onPlay, onDetails }) => {
  if (!item) return null;
  const isShow = 'library_id' in item && !('runtime' in item);

  return (
    <div className="relative h-[85vh] w-full overflow-hidden">
      <img 
        src={getImageUrl(item.backdrop_url)} 
        className="absolute inset-0 w-full h-full object-cover brightness-[0.4]"
        alt="Hero"
      />
      <div className="absolute inset-0 bg-gradient-to-r from-black via-black/40 to-transparent" />
      <div className="absolute inset-0 bg-gradient-to-t from-zinc-950 via-transparent to-transparent" />
      
      <div className="absolute bottom-[15%] left-4 md:left-12 max-w-2xl space-y-6 animate-in fade-in slide-in-from-bottom-10 duration-1000">
        <div className="flex items-center gap-3">
          <div className="px-2 py-1 bg-white/10 backdrop-blur-md rounded text-[10px] font-black text-white uppercase tracking-widest border border-white/20">Featured {isShow ? 'Series' : 'Selection'}</div>
          <span className="text-red-500 font-black tracking-widest text-xs uppercase italic flex items-center gap-1">
            <Star className="w-3 h-3 fill-current" /> Match {Math.round((item.rating || 0) * 10)}%
          </span>
        </div>
        <h2 className="text-5xl md:text-7xl font-black text-white leading-none tracking-tighter drop-shadow-2xl uppercase italic">{item.title}</h2>
        <p className="text-lg text-zinc-300 line-clamp-3 leading-relaxed font-medium drop-shadow">
          {item.plot || "Begin your cinematic journey with this stunning local selection."}
        </p>
        <div className="flex items-center gap-4 pt-4">
          <button 
            onClick={() => onPlay(item)}
            className="bg-white text-black px-8 py-3 rounded flex items-center gap-2 font-black uppercase tracking-widest text-sm hover:bg-zinc-200 transition active:scale-95 shadow-xl"
          >
            <Play className="w-5 h-5 fill-current" /> Play
          </button>
          <button 
            onClick={() => onDetails(item)}
            className="bg-zinc-600/40 hover:bg-zinc-600/60 backdrop-blur-md text-white px-8 py-3 rounded flex items-center gap-2 font-black uppercase tracking-widest text-sm transition border border-white/10"
          >
            <Info className="w-5 h-5" /> Details
          </button>
        </div>
      </div>
    </div>
  );
};

export default Hero;
