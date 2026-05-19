import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { 
  MediaPlayer, 
  MediaProvider, 
  Poster, 
  Track, 
  useMediaRemote, 
  useMediaState,
  type MediaSource
} from '@vidstack/react';
import { 
  DefaultVideoLayout, 
  defaultLayoutIcons 
} from '@vidstack/react/player/layouts/default';

import '@vidstack/react/player/styles/default/theme.css';
import '@vidstack/react/player/styles/default/layouts/video.css';

import { X, Loader2, AlertCircle } from 'lucide-react';
import { getImageUrl, api, API_BASE } from '../api/adapter';
import { useVidstackAbLoop } from '../hooks/useVidstackAbLoop';
import { AbLoopControls } from './AbLoopControls';

interface VidstackPlayerProps {
  mediaId: number;
  mediaType: 'movie' | 'episode';
  title: string;
  posterUrl?: string;
  hash?: string;
  duration?: number;
  initialPosition?: number;
  onClose: () => void;
}

const PlayerContent: React.FC<VidstackPlayerProps & { sources: MediaSource[] }> = ({
  mediaId,
  mediaType,
  title,
  posterUrl,
  hash,
  initialPosition = 0,
  sources,
  onClose
}) => {
  const remote = useMediaRemote();
  const abLoop = useVidstackAbLoop();
  const heartbeatInterval = useRef<ReturnType<typeof setInterval> | null>(null);

  // Heartbeat for progress tracking
  const sendHeartbeat = useCallback((currentTime: number, duration: number, ended: boolean) => {
    const isFinished = ended || (duration > 0 && currentTime / duration > 0.95);

    api.updatePlaybackProgress({
      media_id: mediaId,
      media_type: mediaType,
      position_ms: Math.round(currentTime * 1000),
      duration_ms: Math.round(duration * 1000),
      is_finished: isFinished
    }).catch(err => console.error("[VidstackPlayer] Heartbeat failed:", err));
  }, [mediaId, mediaType]);

  // Handle keyboard hotkeys for A-B Loop
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if typing in an input
      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) return;

      switch (e.key.toLowerCase()) {
        case 'a':
          abLoop.setStart();
          break;
        case 'b':
          abLoop.setEnd();
          break;
        case 'l':
          abLoop.toggleLoop();
          break;
        case 'c':
          abLoop.clearLoop();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [abLoop]);

  const vttUrl = useMemo(() => {
    if (!hash) return undefined;
    return `${API_BASE}/assets/${hash}/vtt`;
  }, [hash]);

  return (
    <div className="w-full h-full relative group">
      <MediaPlayer
        title={title}
        src={sources}
        poster={getImageUrl(posterUrl)}
        className="w-full h-full bg-black overflow-hidden"
        style={{ '--video-volume-slider-orientation': 'vertical' } as React.CSSProperties}
        onCanPlay={() => {
          if (initialPosition > 0) {
            remote.seek(initialPosition / 1000);
          }
        }}
        onTimeUpdate={(e) => {
          // We can't easily get the duration from the event here without more hooks, 
          // but we'll use a local interval or simpler approach for heartbeat
        }}
        onEnded={() => {
          sendHeartbeat(0, 0, true); // Final heartbeat
        }}
      >
        <MediaProvider>
          <Poster className="vds-poster" />
        </MediaProvider>
        
        <DefaultVideoLayout 
          icons={defaultLayoutIcons}
          thumbnails={vttUrl}
        />

        <AbLoopControls abLoop={abLoop} />
        
        {/* Custom Heartbeat logic using Media State */}
        <Heartbeat mediaId={mediaId} mediaType={mediaType} />
      </MediaPlayer>
    </div>
  );
};

// Internal component to handle heartbeat without re-rendering the whole player
const Heartbeat: React.FC<{ mediaId: number, mediaType: string }> = ({ mediaId, mediaType }) => {
  const currentTime = useMediaState('currentTime');
  const duration = useMediaState('duration');
  const ended = useMediaState('ended');
  const lastHeartbeatTime = useRef(0);

  useEffect(() => {
    const now = Date.now();
    if (now - lastHeartbeatTime.current > 30000) { // Every 30 seconds
      lastHeartbeatTime.current = now;
      const isFinished = ended || (duration > 0 && currentTime / duration > 0.95);

      api.updatePlaybackProgress({
        media_id: mediaId,
        media_type: mediaType,
        position_ms: Math.round(currentTime * 1000),
        duration_ms: Math.round(duration * 1000),
        is_finished: isFinished
      }).catch(err => console.error("[VidstackPlayer] Heartbeat failed:", err));
    }
  }, [currentTime, duration, ended, mediaId, mediaType]);

  return null;
};

const VidstackPlayer: React.FC<VidstackPlayerProps> = (props) => {
  const { mediaId, mediaType, onClose } = props;
  const [sources, setSources] = useState<MediaSource[]>([]);
  const [isPreparing, setIsPreparing] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const loadSources = async () => {
      try {
        setIsPreparing(true);
        // Fetch both Direct and HLS sources in parallel
        const [directUrl, hlsUrl] = await Promise.all([
          api.startStreaming(mediaId, mediaType, 'direct').catch(() => null),
          api.startStreaming(mediaId, mediaType, 'hls').catch(() => null)
        ]);

        const newSources: MediaSource[] = [];
        if (directUrl) {
          newSources.push({ src: directUrl, type: 'video/mp4' });
        }
        if (hlsUrl) {
          newSources.push({ src: hlsUrl, type: 'application/x-mpegURL' });
        }

        if (newSources.length === 0) {
          throw new Error("No playable sources found.");
        }

        setSources(newSources);
        setIsPreparing(false);
      } catch (err: any) {
        setError(err.message || "Failed to prepare stream.");
        setIsPreparing(false);
      }
    };

    loadSources();
  }, [mediaId, mediaType]);

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
      
      {isPreparing && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 z-[205] bg-black/60 backdrop-blur-sm">
          <Loader2 className="w-12 h-12 text-red-600 animate-spin" />
          <p className="text-zinc-400 font-black uppercase tracking-[0.2em] text-xs">Initializing Vidstack...</p>
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
      
      {!isPreparing && !error && (
        <PlayerContent {...props} sources={sources} />
      )}
    </div>
  );
};

export default React.memo(VidstackPlayer);
