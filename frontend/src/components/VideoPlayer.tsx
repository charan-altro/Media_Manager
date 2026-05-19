import React, { useEffect, useRef, useState, useCallback } from 'react';
import { createPlayer, videoFeatures, usePlayer } from '@videojs/react';
import { VideoSkin, Video } from '@videojs/react/video';
import '@videojs/react/video/skin.css';

import { X, Loader2, AlertCircle, Settings } from 'lucide-react';
import { getImageUrl, api } from '../api/adapter';
import { useVideoPlayer } from '../hooks/useVideoPlayer';
import { useAbLoop } from '../hooks/useAbLoop';
import VttThumbnails from './VttThumbnails';

type Protocol = 'direct' | 'hls' | 'dash';
const PROTOCOL_PRIORITY: Protocol[] = ['direct', 'hls', 'dash'];

interface VideoPlayerProps {
  url: string;
  mediaId: number;
  mediaType: 'movie' | 'episode';
  title: string;
  posterUrl?: string;
  duration?: number; // in seconds
  initialPosition?: number; // in ms
  videoCodec?: string;
  audioCodec?: string;
  onClose: () => void;
}

const SUPPORTED_AUDIO_CODECS = ['aac', 'mp3', 'opus', 'vorbis', 'flac'];

const Player = createPlayer({ features: videoFeatures });

const SourceSelector = ({ currentProtocol, onSelect }: { currentProtocol: Protocol, onSelect: (protocol: Protocol) => void }) => {
  const [isOpen, setIsOpen] = useState(false);
  const protocols: { label: string, value: Protocol }[] = [
    { label: 'Direct Play', value: 'direct' },
    { label: 'HLS', value: 'hls' },
    { label: 'DASH', value: 'dash' }
  ];

  return (
    <div className="relative">
      <button 
        onClick={() => setIsOpen(!isOpen)}
        className="p-2 hover:bg-white/10 rounded transition text-white"
        title="Change Protocol"
      >
        <Settings className="w-5 h-5" />
      </button>
      {isOpen && (
        <div className="absolute bottom-full right-0 mb-2 bg-zinc-900 border border-white/10 rounded-lg shadow-xl overflow-hidden min-w-[120px]">
          {protocols.map((p) => (
            <button
              key={p.value}
              onClick={() => {
                onSelect(p.value);
                setIsOpen(false);
              }}
              className={`w-full text-left px-4 py-2 text-xs font-bold uppercase tracking-wider transition hover:bg-white/5 ${currentProtocol === p.value ? 'text-red-600' : 'text-zinc-400'}`}
            >
              {p.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
};

const InternalPlayer: React.FC<VideoPlayerProps & { 
  currentUrl: string, 
  currentProtocol: Protocol,
  setIsPreparing: (b: boolean) => void,
  setError: (s: string | null) => void,
  getSourceType: (url: string, protocol: Protocol) => string,
  switchProtocol: (protocol: Protocol) => Promise<void>
}> = ({ 
  currentUrl, 
  currentProtocol, 
  setIsPreparing,
  setError,
  getSourceType,
  switchProtocol,
  mediaId, 
  mediaType, 
  title,
  posterUrl,
  duration = 0, 
  initialPosition = 0
}) => {
  const { player } = usePlayer();
  const [media, setMedia] = useState<HTMLVideoElement | null>(null);
  const heartbeatInterval = useRef<ReturnType<typeof setInterval> | null>(null);
  const wakeLock = useRef<any>(null);

  useEffect(() => {
    if (player) {
      const tech = player.tech(true);
      if (tech && tech.el()) {
        setMedia(tech.el() as HTMLVideoElement);
      }
    }
  }, [player]);

  useEffect(() => {
    console.log('[VideoPlayer] InternalPlayer mounted/updated:', { currentUrl, currentProtocol, mediaReady: !!media });
  }, [currentUrl, currentProtocol, media]);

  const abLoop = useAbLoop(media);
  const { seekStep } = useVideoPlayer(media, abLoop);

  const requestWakeLock = useCallback(async () => {
    if ('wakeLock' in navigator) {
      try {
        wakeLock.current = await (navigator as any).wakeLock.request('screen');
        console.log('[VideoPlayer] Wake Lock acquired');
      } catch (err: unknown) {
        if (err instanceof Error) {
          console.warn('[VideoPlayer] Wake Lock error:', err.message);
        }
      }
    }
  }, []);

  const releaseWakeLock = useCallback(async () => {
    if (wakeLock.current !== null) {
      try {
        await wakeLock.current.release();
        wakeLock.current = null;
        console.log('[VideoPlayer] Wake Lock released');
      } catch (err: unknown) {
        if (err instanceof Error) {
          console.warn('[VideoPlayer] Wake Lock release error:', err.message);
        }
      }
    }
  }, []);

  const sendHeartbeat = useCallback(() => {
    if (!media) return;
    
    const currentTime = media.currentTime || 0;
    const totalDuration = media.duration || duration || 0;
    const isFinished = media.ended || (totalDuration > 0 && currentTime / totalDuration > 0.95);

    api.updatePlaybackProgress({
      media_id: mediaId,
      media_type: mediaType,
      position_ms: Math.round(currentTime * 1000),
      duration_ms: Math.round(totalDuration * 1000),
      is_finished: isFinished
    }).catch(err => console.error("[VideoPlayer] Heartbeat failed:", err));
  }, [media, mediaId, mediaType, duration]);

  useEffect(() => {
    if (!media) return;

    console.log('[VideoPlayer] Media instance available, configuring...');

    const clearPreparing = () => {
      console.log('[VideoPlayer] Clearing preparing state via event');
      setIsPreparing(false);
    };
    
    media.addEventListener('canplay', clearPreparing);
    media.addEventListener('playing', clearPreparing);
    media.addEventListener('loadedmetadata', () => {
      console.log('[VideoPlayer] Metadata loaded, duration:', media.duration);
      clearPreparing();
    });

    if (duration > 0) {
      // media.duration is read-only on HTMLMediaElement, so we skip setting it directly.
    }

    if (initialPosition > 0) {
       console.log('[VideoPlayer] Seeking to initial position:', initialPosition / 1000);
       media.currentTime = initialPosition / 1000;
    }

    heartbeatInterval.current = setInterval(sendHeartbeat, 30000);

    return () => {
      console.log('[VideoPlayer] Detaching listeners and cleaning up heartbeat');
      media.removeEventListener('canplay', clearPreparing);
      media.removeEventListener('playing', clearPreparing);
      media.removeEventListener('loadedmetadata', clearPreparing);
      if (heartbeatInterval.current) clearInterval(heartbeatInterval.current);
      releaseWakeLock();
      sendHeartbeat();
    };
  }, [media, initialPosition, duration, sendHeartbeat, releaseWakeLock, setIsPreparing]);

  // MediaSession integration
  useEffect(() => {
    if (!media || !('mediaSession' in navigator)) return;

    navigator.mediaSession.metadata = new MediaMetadata({
      title: title,
      artist: 'Media Manager',
      artwork: [
        { src: getImageUrl(posterUrl), sizes: '512x512', type: 'image/jpeg' }
      ]
    });

    const handlers: [MediaSessionAction, MediaSessionActionHandler][] = [
      ['play', () => { console.log('[VideoPlayer] MediaSession Play'); media.play(); }],
      ['pause', () => { console.log('[VideoPlayer] MediaSession Pause'); media.pause(); }],
      ['seekbackward', (details) => {
        const skipTime = details.seekOffset || 10;
        seekStep(-skipTime);
      }],
      ['seekforward', (details) => {
        const skipTime = details.seekOffset || 10;
        seekStep(skipTime);
      }],
      ['previoustrack', () => {
        media.currentTime = 0;
      }],
      ['nexttrack', () => {}]
    ];

    for (const [action, handler] of handlers) {
      try {
        navigator.mediaSession.setActionHandler(action, handler);
      } catch (error) {
        console.warn(`[VideoPlayer] Media session action "${action}" not supported.`);
      }
    }

    return () => {
      for (const [action] of handlers) {
        try {
          navigator.mediaSession.setActionHandler(action, null);
        } catch (error) {}
      }
    };
  }, [media, title, posterUrl, seekStep]);

  const handlePlaying = () => {
    console.log('[VideoPlayer] onPlaying event');
    setIsPreparing(false);
    requestWakeLock();
  };

  const handlePause = () => {
    console.log('[VideoPlayer] onPause event');
    releaseWakeLock();
  };

  const handleError = () => {
    if (!media) return;
    const playerError = media.error;
    if (playerError) {
      console.error('[VideoPlayer] Media Error:', playerError);
    }
    
    if (playerError && playerError.code === 4) {
      const currentIndex = PROTOCOL_PRIORITY.indexOf(currentProtocol);
      if (currentIndex < PROTOCOL_PRIORITY.length - 1) {
        const nextProtocol = PROTOCOL_PRIORITY[currentIndex + 1];
        console.log(`[VideoPlayer] Fallback: Switching from ${currentProtocol} to ${nextProtocol}`);
        switchProtocol(nextProtocol);
        return;
      }
    }

    releaseWakeLock();
    if (playerError) {
      setError(`Streaming error: ${playerError.message || 'Unknown error'} (Code: ${playerError.code})`);
    } else {
      setError('Unknown playback error occurred.');
    }
  };

  return (
    <>
      <VideoSkin poster={getImageUrl(posterUrl)}>
        <Video 
          src={currentUrl}
          type={getSourceType(currentUrl, currentProtocol)}
          autoPlay 
          controls 
          onPlaying={handlePlaying}
          onPause={handlePause}
          onLoadedMetadata={() => { console.log('[VideoPlayer] onLoadedMetadata'); setIsPreparing(false); }}
          onCanPlay={() => { console.log('[VideoPlayer] onCanPlay'); setIsPreparing(false); }}
          onError={handleError}
          className="vjs-big-play-centered"
        />

        {/* Custom Overlays */}
        <div className="absolute bottom-16 right-6 flex items-center gap-4 z-[210]">
          {abLoop.loopEnabled && (
            <div className="bg-red-600/80 backdrop-blur-md px-3 py-1 rounded-full text-[10px] font-black text-white uppercase tracking-widest animate-pulse">
              Loop Active: {Math.round(abLoop.loopStart || 0)}s - {Math.round(abLoop.loopEnd || 0)}s
            </div>
          )}
          <SourceSelector 
            currentProtocol={currentProtocol} 
            onSelect={switchProtocol} 
          />
        </div>
      </VideoSkin>
      <VttThumbnails player={media} />
    </>
  );
};

const VideoPlayer: React.FC<VideoPlayerProps> = (props) => {
  const { url: initialUrl, audioCodec, mediaId, mediaType, onClose } = props;
  const [currentUrl, setCurrentUrl] = useState<string>(initialUrl);
  const [currentProtocol, setCurrentProtocol] = useState<Protocol>(() => {
    if (initialUrl.includes('protocol=hls')) return 'hls';
    if (initialUrl.includes('protocol=dash')) return 'dash';
    if (audioCodec) {
      const codec = audioCodec.toLowerCase();
      const isSupported = SUPPORTED_AUDIO_CODECS.some(c => codec.includes(c));
      if (!isSupported) return 'hls';
    }
    return 'direct';
  });

  const [isPreparing, setIsPreparing] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchSourceUrl = useCallback(async (protocol: Protocol) => {
    try {
      const newUrl = await api.startStreaming(mediaId, mediaType, protocol);
      return newUrl;
    } catch (err) {
      console.error(`Failed to fetch URL for protocol ${protocol}:`, err);
      return null;
    }
  }, [mediaId, mediaType]);

  const getSourceType = useCallback((url: string, protocol: Protocol) => {
    if (protocol === 'direct') {
      const path = url.split('?')[0].toLowerCase();
      if (path.endsWith('.mkv')) return 'video/webm';
      if (path.endsWith('.webm')) return 'video/webm';
      return 'video/mp4';
    }
    if (protocol === 'dash') {
      return 'application/dash+xml';
    }
    return 'application/x-mpegURL';
  }, []);

  const switchProtocol = useCallback(async (protocol: Protocol) => {
    setIsPreparing(true);
    const newUrl = await fetchSourceUrl(protocol);
    if (newUrl) {
      setCurrentUrl(newUrl);
      setCurrentProtocol(protocol);
    } else {
      setIsPreparing(false);
      setError(`Failed to switch to ${protocol} protocol.`);
    }
  }, [fetchSourceUrl]);

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
      
      <div className="w-full h-full max-w-7xl aspect-video relative group overflow-hidden">
        <Player.Provider>
          <InternalPlayer 
            {...props} 
            currentUrl={currentUrl} 
            currentProtocol={currentProtocol}
            setIsPreparing={setIsPreparing}
            setError={setError}
            getSourceType={getSourceType}
            switchProtocol={switchProtocol}
          />
        </Player.Provider>
      </div>
    </div>
  );
};

export default React.memo(VideoPlayer);
