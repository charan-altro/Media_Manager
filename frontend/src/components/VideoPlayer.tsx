import React, { useEffect, useRef } from 'react';
import Hls from 'hls.js';
import { X } from 'lucide-react';

interface VideoPlayerProps {
  url: string;
  onClose: () => void;
}

const VideoPlayer: React.FC<VideoPlayerProps> = ({ url, onClose }) => {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    let hls: Hls;

    if (videoRef.current) {
      const video = videoRef.current;

      if (Hls.isSupported()) {
        hls = new Hls({
          enableWorker: true,
          lowLatencyMode: true,
          backBufferLength: 90
        });
        hls.loadSource(url);
        hls.attachMedia(video);
        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          video.play().catch(e => console.error("Auto-play failed:", e));
        });
        
        hls.on(Hls.Events.ERROR, (event, data) => {
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
                console.error("Fatal error, cannot recover");
                hls.destroy();
                break;
            }
          }
        });
      } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
        // For Safari/iOS native HLS support
        video.src = url;
        video.addEventListener('loadedmetadata', () => {
          video.play().catch(e => console.error("Auto-play failed:", e));
        });
      }
    }

    return () => {
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
