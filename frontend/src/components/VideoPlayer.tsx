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
  duration?: number; // in seconds
  initialPosition?: number; // in ms
  onClose: () => void;
}

const VideoPlayer: React.FC<VideoPlayerProps> = ({ url, mediaId, mediaType, duration = 0, initialPosition = 0, onClose }) => {
  const videoRef = useRef<HTMLDivElement>(null);
  const playerRef = useRef<Player | null>(null);
  const [isPreparing, setIsPreparing] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const heartbeatInterval = useRef<any>(null);

  const sendHeartbeat = () => {
    const player = playerRef.current;
    if (!player || player.isDisposed()) return;
    
    const currentTime = player.currentTime() || 0;
    const totalDuration = player.duration() || duration || 0;
    const isFinished = player.ended() || (totalDuration > 0 && currentTime / totalDuration > 0.95);

    api.updatePlaybackProgress({
      media_id: mediaId,
      media_type: mediaType,
      position_ms: Math.round(currentTime * 1000),
      duration_ms: Math.round(totalDuration * 1000),
      is_finished: isFinished
    }).catch(err => console.error("Heartbeat failed:", err));
  };

  useEffect(() => {
    if (!videoRef.current) return;

    // Cleanup existing player
    if (playerRef.current) {
      playerRef.current.dispose();
      playerRef.current = null;
    }

    const videoElement = document.createElement("video-js");
    videoElement.classList.add('vjs-big-play-centered', 'vjs-fluid');
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
          fastQualityChange: true,
          bufferLow: 5,
          bufferHigh: 10,
          useBandwidthFromLocalStorage: true
        }
      },
      sources: [{
        src: url,
        type: url.includes('/direct/') 
          ? (url.toLowerCase().endsWith('.mkv') ? 'video/webm' : 'video/mp4') 
          : 'application/x-mpegURL'
      }]
    });

    player.ready(() => {
      console.log('Video.js player is ready');
      
      if (duration > 0) {
        player.duration(duration);
      }

      if (initialPosition > 0) {
         player.currentTime(initialPosition / 1000);
      }

      if (heartbeatInterval.current) clearInterval(heartbeatInterval.current);
      heartbeatInterval.current = setInterval(sendHeartbeat, 30000);
    });

    player.on('loadedmetadata', () => {
      if (duration > 0) {
        player.duration(duration);
      }
    });

    player.on('playing', () => setIsPreparing(false));
    
    player.on('error', () => {
      const playerError = player.error();
      console.error('Video.js Error:', playerError);
      if (playerError) {
        setError(`Streaming error: ${playerError.message || 'Unknown error'} (Code: ${playerError.code})`);
      }
    });

    return () => {
      if (player && !player.isDisposed()) {
        player.dispose();
        playerRef.current = null;
      }
    };
  }, [url]);

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
          <p className="text-zinc-600 text-[10px] uppercase">Direct Play Engine</p>
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
      </div>
    </div>
  );
};

export default React.memo(VideoPlayer);
