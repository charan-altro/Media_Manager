import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { 
  MediaPlayer, 
  MediaProvider, 
  Poster, 
  Gesture,
  type MediaSrc,
  type MediaPlayerInstance
} from '@vidstack/react';
import { 
  DefaultVideoLayout, 
  defaultLayoutIcons 
} from '@vidstack/react/player/layouts/default';

import '@vidstack/react/player/styles/default/theme.css';
import '@vidstack/react/player/styles/default/layouts/video.css';

import { X, Loader2, AlertCircle } from 'lucide-react';
import { getImageUrl, api, API_BASE } from '../api/adapter';
import { useVidstackAbLoop, type AbLoopManager } from '../hooks/useVidstackAbLoop';
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

// Inner component that HAS access to MediaPlayer context
const InnerPlayer: React.FC<VidstackPlayerProps & { sources: any[], isBuffering: boolean, abLoop: AbLoopManager }> = ({
  title,
  hash,
  isBuffering,
  abLoop
}) => {
  // Handle keyboard hotkeys for A-B Loop
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) return;

      switch (e.key.toLowerCase()) {
        case 'a': abLoop.setStart(); break;
        case 'b': abLoop.setEnd(); break;
        case 'l': abLoop.toggleLoop(); break;
        case 'c': abLoop.clearLoop(); break;
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
    <>
      <MediaProvider>
        <Poster className="vds-poster" />
        <Gesture className="vds-gesture" event="pointerup" action="toggle:paused" />
        <Gesture className="vds-gesture" event="dblpointerup" action="toggle:fullscreen" />
        <Gesture className="vds-gesture" event="dblpointerup" action="seek:-10" />
        <Gesture className="vds-gesture" event="dblpointerup" action="seek:10" />
      </MediaProvider>
      
      <DefaultVideoLayout 
        icons={defaultLayoutIcons}
        thumbnails={vttUrl}
        slots={{
          beforeTitle: (
            <div className="flex flex-col mb-4">
              <span className="text-white/60 text-[10px] font-black uppercase tracking-[0.3em]">Now Playing</span>
              <h2 className="text-white text-xl font-black uppercase tracking-tight leading-none">{title}</h2>
            </div>
          )
        }}
      />

      <AbLoopControls abLoop={abLoop} />
      
      {isBuffering && (
        <div className="absolute inset-0 flex items-center justify-center z-[50] pointer-events-none">
          <div className="relative">
            <div className="w-20 h-20 border-4 border-red-600/20 border-t-red-600 rounded-full animate-spin" />
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="w-12 h-12 bg-red-600/10 rounded-full blur-xl animate-pulse" />
            </div>
          </div>
        </div>
      )}
    </>
  );
};

const PlayerContent: React.FC<VidstackPlayerProps & { 
  sources: any[], 
  onSeek: (time: number) => void, 
  duration: number,
  startOffset: number,
  isPiped: boolean
}> = (props) => {
  const { title, sources, posterUrl, mediaId, mediaType, initialPosition = 0, onSeek, duration, startOffset, isPiped } = props;
  const [isBuffering, setIsBuffering] = useState(false);
  const playerRef = useRef<MediaPlayerInstance>(null);
  const abLoop = useVidstackAbLoop();
  
  const lastHeartbeatTime = useRef(0);
  const durationRef = useRef(0);
  const hasSeeked = useRef(false);
  const seekTimeoutRef = useRef<any>(null);
  const isRewritingSourceRef = useRef(false);
  const loadedSourceRef = useRef<string>('');

  const seek = useCallback((time: number) => {
    if (playerRef.current) {
      if (isPiped) {
        onSeek(time);
      } else {
        playerRef.current.currentTime = time;
      }
    }
  }, [isPiped, onSeek]);

  const handleTimeUpdate = useCallback((event: any) => {
    const currentTime = event?.detail?.currentTime ?? playerRef.current?.currentTime ?? 0;
    const durationVal = durationRef.current || playerRef.current?.duration || 0;
    
    // Manage A-B Loop
    abLoop.checkLoop(currentTime, seek);
    
    // Heartbeat logic
    const now = Date.now();
    if (now - lastHeartbeatTime.current > 30000) { // Every 30 seconds
      lastHeartbeatTime.current = now;
      const isFinished = durationVal > 0 && currentTime / durationVal > 0.95;

      api.updatePlaybackProgress({
        media_id: mediaId,
        media_type: mediaType,
        position_ms: Math.round(currentTime * 1000),
        duration_ms: Math.round(durationVal * 1000),
        is_finished: isFinished
      }).catch(err => console.error("[VidstackPlayer] Heartbeat failed:", err));
    }
  }, [mediaId, mediaType, abLoop, seek]);

  const handleEnded = useCallback(() => {
    api.updatePlaybackProgress({
      media_id: mediaId,
      media_type: mediaType,
      position_ms: 0,
      duration_ms: Math.round((durationRef.current || playerRef.current?.duration || 0) * 1000),
      is_finished: true
    }).catch(err => console.error("[VidstackPlayer] Final heartbeat failed:", err));
  }, [mediaId, mediaType]);

  // Handle initial seek / source reloads
  const handleCanPlay = useCallback((e: any) => {
    setIsBuffering(false);
    const playerDuration = e?.detail?.duration ?? playerRef.current?.duration ?? 0;
    durationRef.current = duration || playerDuration;
    
    const player = playerRef.current;
    if (!player) return;

    // Use current source URL or path to identify source changes
    const currentSrc = (sources[0]?.src as string) ?? '';
    if (currentSrc !== loadedSourceRef.current) {
      loadedSourceRef.current = currentSrc;
      if (isPiped) {
        // Piped stream is seeked by backend starting at startOffset,
        // so we must seek the video element to startOffset to align with the stream
        player.currentTime = startOffset;
      } else if (!hasSeeked.current && initialPosition > 0) {
        player.currentTime = initialPosition / 1000;
        hasSeeked.current = true;
      }
    }
  }, [initialPosition, duration, isPiped, startOffset, sources]);

  const handleSeeking = useCallback(() => {
    const player = playerRef.current;
    if (!player) return;

    // For static files (native range seeks), browser handles seeks natively without reloading source URL
    if (!isPiped) {
      return;
    }

    const targetTime = player.currentTime;

    // If we're reloading the source, ignore the temporary seeked events caused by browser source resets
    if (isRewritingSourceRef.current) {
      isRewritingSourceRef.current = false;
      return;
    }

    // Ignore seek triggers that are close to our target offset to prevent loops
    if (Math.abs(targetTime - startOffset) < 2.0) {
      return;
    }

    // Debounce stream restarts to prevent backend hammering during scrubbing
    if (seekTimeoutRef.current) {
      clearTimeout(seekTimeoutRef.current);
    }

    seekTimeoutRef.current = setTimeout(() => {
      isRewritingSourceRef.current = true;
      onSeek(targetTime);
    }, 250);
  }, [onSeek, startOffset, isPiped]);

  useEffect(() => {
    return () => {
      if (seekTimeoutRef.current) {
        clearTimeout(seekTimeoutRef.current);
      }
    };
  }, []);

  // Netflix/YouTube style styling
  const playerStyles: any = {
    '--video-brand': '#E50914', // Netflix Red
    '--video-loader-size': '80px',
    '--video-volume-slider-orientation': 'vertical',
  };

  return (
    <div className="w-full h-full relative group">
      <MediaPlayer
        ref={playerRef}
        title={title}
        src={sources}
        poster={getImageUrl(posterUrl)}
        className="w-full h-full bg-black overflow-hidden"
        style={playerStyles}
        crossOrigin
        playsInline
        onWaiting={() => setIsBuffering(true)}
        onPlaying={() => setIsBuffering(false)}
        onCanPlay={handleCanPlay}
        onTimeUpdate={handleTimeUpdate}
        onEnded={handleEnded}
        onSeeking={handleSeeking}
        streamType="on-demand"
        duration={duration}
      >
        <InnerPlayer {...props} isBuffering={isBuffering} abLoop={abLoop} />
      </MediaPlayer>
    </div>
  );
};

const VidstackPlayer: React.FC<VidstackPlayerProps> = (props) => {
  const { mediaId, mediaType, onClose, initialPosition = 0 } = props;
  const [directUrl, setDirectUrl] = useState<string | null>(null);
  const [startOffset, setStartOffset] = useState<number>(initialPosition / 1000);
  const [duration, setDuration] = useState<number>(props.duration || 0);
  const [isPreparing, setIsPreparing] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const isFirstLoad = useRef(true);

  // Fetch direct playback/remux URL once
  useEffect(() => {
    const loadSources = async () => {
      try {
        if (isFirstLoad.current) {
          setIsPreparing(true);
        }

        // Fetch playback status to get accurate duration
        if (isFirstLoad.current) {
          const status = await api.getPlaybackStatus(mediaType, mediaId).catch(() => null);
          if (status && status.duration_ms > 0) {
            setDuration(status.duration_ms / 1000);
          }
        }

        const url = await api.startStreaming(mediaId, mediaType, 'direct');
        setDirectUrl(url || null);
        isFirstLoad.current = false;
        setIsPreparing(false);
      } catch (err: any) {
        setError(err.message || "Failed to prepare stream.");
        setIsPreparing(false);
      }
    };

    loadSources();
  }, [mediaId, mediaType]);

  const isPiped = useMemo(() => {
    if (!directUrl) return false;
    return (
      directUrl.includes('/stream.mp4') ||
      directUrl.includes('/stream.webm') ||
      directUrl.includes('/stream.mkv')
    );
  }, [directUrl]);

  const sources = useMemo<any[]>(() => {
    if (!directUrl) return [];

    const url = new URL(directUrl, window.location.origin);
    // Only append start parameter for piped streams
    if (isPiped && startOffset > 0) {
      url.searchParams.set("start", startOffset.toString());
    }

    const finalSrc = url.pathname + url.search;
    const type = directUrl.includes('.mkv') ? 'video/x-matroska' : 'video/mp4';
    return [{ src: finalSrc, type }];
  }, [directUrl, isPiped, isPiped ? startOffset : undefined]);

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
        <PlayerContent 
          {...props} 
          sources={sources} 
          onSeek={setStartOffset} 
          duration={duration} 
          startOffset={startOffset}
          isPiped={isPiped}
        />
      )}
    </div>
  );
};

export default React.memo(VidstackPlayer);
