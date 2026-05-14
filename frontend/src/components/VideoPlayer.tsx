import React, { useEffect, useRef, useState } from 'react';
import videojs from 'video.js';
import type Player from 'video.js/dist/types/player';
import 'video.js/dist/video-js.css';
import { X, Loader2, AlertCircle } from 'lucide-react';
import { api } from '../api/adapter';

interface VideoPlayerProps {
  url: string;
  mediaId: number;
  mediaType: 'movie' | 'episode';
  initialPosition?: number; // in ms
  onClose: () => void;
}

const VideoPlayer: React.FC<VideoPlayerProps> = ({ url, mediaId, mediaType, initialPosition = 0, onClose }) => {
  const videoRef = useRef<HTMLDivElement>(null);
  const playerRef = useRef<Player | null>(null);
  const [isPreparing, setIsPreparing] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const heartbeatInterval = useRef<any>(null);

  const sendHeartbeat = () => {
    const player = playerRef.current;
    if (!player) return;
    
    const currentTime = player.currentTime() || 0;
    const duration = player.duration() || 0;
    const isFinished = player.ended() || (duration > 0 && currentTime / duration > 0.95);

    api.updatePlaybackProgress({
      media_id: mediaId,
      media_type: mediaType,
      position_ms: Math.round(currentTime * 1000),
      duration_ms: Math.round(duration * 1000) || 0,
      is_finished: isFinished
    }).catch(err => console.error("Heartbeat failed:", err));
  };

  useEffect(() => {
    // Make sure Video.js player is only initialized once
    if (!playerRef.current && videoRef.current) {
      const videoElement = document.createElement("video-js");

      videoElement.classList.add('vjs-big-play-centered');
      videoElement.classList.add('vjs-fluid');
      videoRef.current.appendChild(videoElement);

      console.log('Initializing Video.js with URL:', url);
      const player = playerRef.current = videojs(videoElement, {
        autoplay: true,
        controls: true,
        responsive: true,
        fluid: true,
        preload: 'auto',
        liveui: true,
        html5: {
          vhs: {
            overrideNative: true,
            enableLowInitialPlaylist: true,
            enableWorker: true,
            smoothQualityChange: true,
            fastQualityChange: true
          }
        },
        sources: [{
          src: url,
          type: 'application/x-mpegURL'
        }]
      }, () => {
        console.log('Video.js player is ready');
        
        if (initialPosition > 0) {
          player.currentTime(initialPosition / 1000);
        }

        // Start heartbeat
        if (heartbeatInterval.current) clearInterval(heartbeatInterval.current);
        heartbeatInterval.current = setInterval(sendHeartbeat, 30000);
      });

      player.on('loadstart', () => console.log('Video.js: loadstart'));
      player.on('loadedmetadata', () => console.log('Video.js: loadedmetadata'));
      player.on('canplay', () => console.log('Video.js: canplay'));

      player.on('playing', () => {
        console.log('Video.js: playing');
        setIsPreparing(false);
      });

      player.on('error', () => {
        const playerError = player.error();
        console.error('Video.js Error:', playerError);
        setError(`Streaming error: ${playerError?.message || 'Unknown error'}`);
      });

      player.on('waiting', () => {
        console.log('Video.js: waiting (buffering)');
      });
    }
  }, [videoRef]);

  // Dispose the player on unmount
  useEffect(() => {
    const player = playerRef.current;

    return () => {
      if (heartbeatInterval.current) clearInterval(heartbeatInterval.current);
      // Send one final heartbeat before closing
      sendHeartbeat();

      if (player && !player.isDisposed()) {
        player.dispose();
        playerRef.current = null;
      }
    };
  }, [playerRef]);

  return (
    <div className="fixed inset-0 z-[200] bg-black flex flex-col items-center justify-center animate-in fade-in duration-500">
      <div className="absolute top-6 right-6 z-[210]">
        <button 
          onClick={onClose}
          className="bg-white/10 hover:bg-red-600 backdrop-blur-xl rounded-full p-3 text-white transition active:scale-90"
        >
          <X className="w-8 h-8" />
        </button>
      </div>
      
      {isPreparing && !error && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 z-[205] bg-black/60 backdrop-blur-sm">
          <Loader2 className="w-12 h-12 text-red-600 animate-spin" />
          <p className="text-zinc-400 font-black uppercase tracking-[0.2em] text-xs">Preparing Stream...</p>
          <p className="text-zinc-600 text-[10px] uppercase">Optimized Video.js Engine</p>
        </div>
      )}

      {error && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 z-[205] bg-black">
          <AlertCircle className="w-16 h-16 text-red-600" />
          <p className="text-white font-black uppercase tracking-widest">{error}</p>
          <button 
            onClick={onClose}
            className="mt-4 px-8 py-3 bg-zinc-800 hover:bg-zinc-700 text-white rounded-xl font-bold uppercase text-xs transition"
          >
            Go Back
          </button>
        </div>
      )}
      
      <div className="w-full h-full max-w-7xl aspect-video relative group overflow-hidden" ref={videoRef}>
        <div className="absolute top-4 left-4 pointer-events-none z-10 opacity-0 group-hover:opacity-100 transition-opacity">
           <div className="bg-black/60 backdrop-blur-md px-4 py-2 rounded-lg border border-white/10">
              <span className="text-red-500 font-black italic uppercase text-xs tracking-tighter">HLS Player (vjs)</span>
           </div>
        </div>
      </div>
    </div>
  );
};

export default React.memo(VideoPlayer);
