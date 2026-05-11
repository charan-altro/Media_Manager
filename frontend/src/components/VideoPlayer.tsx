import React, { useEffect, useRef, useState } from 'react';
import Hls from 'hls.js';
import { X, Loader2, AlertCircle } from 'lucide-react';

interface VideoPlayerProps {
  url: string;
  onClose: () => void;
}

const VideoPlayer: React.FC<VideoPlayerProps> = ({ url, onClose }) => {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [isPreparing, setIsPreparing] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let hls: Hls;

    const initPlayer = () => {
      if (!videoRef.current) return;
      const video = videoRef.current;

      if (Hls.isSupported()) {
        hls = new Hls({
          enableWorker: true,
          lowLatencyMode: true,
          backBufferLength: 90,
          manifestLoadingMaxRetry: 10,
          manifestLoadingRetryDelay: 1000,
          levelLoadingMaxRetry: 10,
        });

        hls.loadSource(url);
        hls.attachMedia(video);
        
        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          setIsPreparing(false);
          video.play().catch(e => console.error("Auto-play failed:", e));
        });
        
        hls.on(Hls.Events.ERROR, (_event, data) => {
          console.error("HLS Error:", data);
          if (data.fatal) {
            switch (data.type) {
              case Hls.ErrorTypes.NETWORK_ERROR:
                console.error("Fatal network error encountered, try to recover");
                hls.startLoad();
                break;
              case Hls.ErrorTypes.MEDIA_ERROR:
                console.error("Fatal media error encountered, try to recover");
                hls.recoverMediaError();
                break;
              default:
                setError(`Fatal streaming error: ${data.details}`);
                hls.destroy();
                break;
            }
          }
        });
      } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
        video.src = url;
        video.addEventListener('loadedmetadata', () => {
          setIsPreparing(false);
          video.play().catch(e => console.error("Auto-play failed:", e));
        });
        video.addEventListener('error', () => {
           setError("Native HLS playback failed");
        });
      }
    };

    const timeout = setTimeout(initPlayer, 500);

    return () => {
      clearTimeout(timeout);
      if (hls) {
        hls.destroy();
      }
    };
  }, [url]);

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
          <p className="text-zinc-400 font-black uppercase tracking-[0.2em] text-xs">Preparing HLS Stream...</p>
          <p className="text-zinc-600 text-[10px] uppercase">FFmpeg is generating segments</p>
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
      
      <div className="w-full h-full max-w-7xl aspect-video relative group">
        <video 
          ref={videoRef}
          className="w-full h-full object-contain"
          controls
          autoPlay
          playsInline
        />
        <div className="absolute top-4 left-4 pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity">
           <div className="bg-black/60 backdrop-blur-md px-4 py-2 rounded-lg border border-white/10">
              <span className="text-red-500 font-black italic uppercase text-xs tracking-tighter">HLS Live Stream</span>
           </div>
        </div>
      </div>
    </div>
  );
};

export default VideoPlayer;
