import React from 'react';
import type { AbLoopManager } from '../hooks/useVidstackAbLoop';
import { Repeat } from 'lucide-react';

interface AbLoopControlsProps {
  abLoop: AbLoopManager;
}

export const AbLoopControls: React.FC<AbLoopControlsProps> = ({ abLoop }) => {
  return (
    <div className="absolute top-6 left-1/2 -translate-x-1/2 flex items-center gap-2 z-[210] bg-black/40 backdrop-blur-xl border border-white/10 p-1.5 rounded-2xl shadow-2xl animate-in slide-in-from-top duration-500">
      <div className={`flex items-center gap-1.5 px-3 py-1.5 rounded-xl transition-colors ${abLoop.loopEnabled ? 'bg-red-600/20 text-red-500' : 'text-zinc-400'}`}>
        <Repeat className={`w-4 h-4 ${abLoop.loopEnabled ? 'animate-spin-slow' : ''}`} />
        <span className="text-[10px] font-black uppercase tracking-widest">
          {abLoop.loopEnabled ? 'Loop Active' : 'A-B Loop'}
        </span>
      </div>

      <div className="h-4 w-px bg-white/10 mx-1" />

      <div className="flex items-center gap-1">
        <button
          onClick={abLoop.setStart}
          className={`px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase transition-all active:scale-95 ${
            abLoop.loopStart !== null ? 'bg-white/20 text-white' : 'hover:bg-white/10 text-zinc-500'
          }`}
        >
          {abLoop.loopStart !== null ? `A: ${Math.round(abLoop.loopStart)}s` : 'Set A'}
        </button>

        <button
          onClick={abLoop.setEnd}
          disabled={abLoop.loopStart === null}
          className={`px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase transition-all active:scale-95 disabled:opacity-30 ${
            abLoop.loopEnd !== null ? 'bg-white/20 text-white' : 'hover:bg-white/10 text-zinc-500'
          }`}
        >
          {abLoop.loopEnd !== null ? `B: ${Math.round(abLoop.loopEnd)}s` : 'Set B'}
        </button>

        {(abLoop.loopStart !== null || abLoop.loopEnd !== null) && (
          <button
            onClick={abLoop.clearLoop}
            className="px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase text-red-500 hover:bg-red-500/10 transition-all active:scale-95"
          >
            Clear
          </button>
        )}
      </div>
    </div>
  );
};
