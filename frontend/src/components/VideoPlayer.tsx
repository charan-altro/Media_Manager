import React, { useEffect, useRef, useState, useCallback } from 'react';
import videojs from 'video.js';
import 'video.js/dist/video-js.css';
// @ts-ignore - videojs-abloop does not have types
import abLoopPlugin from 'videojs-abloop';
import type Player from 'video.js/dist/types/player';

// Register the plugin manually if it hasn't been already
if (typeof videojs.getPlugin('abLoopPlugin') === 'undefined') {
  abLoopPlugin(window, videojs);
}

import '../plugins/sourceSelector'; // Import the custom plugin
import { X, Loader2, AlertCircle } from 'lucide-react';
import { getImageUrl, api } from '../api/adapter';
import { useVideoPlayer } from '../hooks/useVideoPlayer';
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
  onClose: () => void;
}

const VideoPlayer: React.FC<VideoPlayerProps> = ({ 
  url: initialUrl, 
  mediaId, 
  mediaType, 
  title,
  posterUrl,
  duration = 0, 
  initialPosition = 0, 
  onClose 
}) => {
  const videoRef = useRef<HTMLDivElement>(null);
  const playerRef = useRef<Player | null>(null);
  const [player, setPlayer] = useState<Player | null>(null);
  const [currentUrl, setCurrentUrl] = useState<string>(initialUrl);
  const [currentProtocol, setCurrentProtocol] = useState<Protocol>(() => {
    if (initialUrl.includes('protocol=hls')) return 'hls';
    if (initialUrl.includes('protocol=dash')) return 'dash';
    return 'direct';
  });
  const [isPreparing, setIsPreparing] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const heartbeatInterval = useRef<ReturnType<typeof setInterval> | null>(null);
  const wakeLock = useRef<any>(null);

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
      return url.toLowerCase().endsWith('.mkv') ? 'video/webm' : 'video/mp4';
    }
    if (protocol === 'dash') {
      return 'application/dash+xml';
    }
    return 'application/x-mpegURL'; // HLS
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

  // Surgical update of player source when currentUrl or currentProtocol changes
  useEffect(() => {
    const p = playerRef.current;
    if (!p || !currentUrl) return;

    // Avoid redundant updates
    if (p.currentSrc() === currentUrl) return;

    const currentTime = p.currentTime() || 0;
    const isPaused = p.paused();

    p.src({
      src: currentUrl,
      type: getSourceType(currentUrl, currentProtocol)
    });

    p.ready(() => {
      p.currentTime(currentTime);
      if (!isPaused) {
        const playPromise = p.play();
        if (playPromise !== undefined) {
          playPromise.catch(() => {});
        }
      }
      p.trigger('protocolChanged', { protocol: currentProtocol });
      setIsPreparing(false);
    });
  }, [currentUrl, currentProtocol, getSourceType]);

  const requestWakeLock = useCallback(async () => {
    if ('wakeLock' in navigator) {
      try {
        wakeLock.current = await (navigator as any).wakeLock.request('screen');
        console.log('Wake Lock is active');
      } catch (err: unknown) {
        if (err instanceof Error) {
          console.error(`Wake Lock error: ${err.name}, ${err.message}`);
        }
      }
    }
  }, []);

  const releaseWakeLock = useCallback(async () => {
    if (wakeLock.current !== null) {
      try {
        await wakeLock.current.release();
        wakeLock.current = null;
        console.log('Wake Lock released');
      } catch (err: unknown) {
        if (err instanceof Error) {
          console.error(`Wake Lock release error: ${err.name}, ${err.message}`);
        }
      }
    }
  }, []);

  const sendHeartbeat = useCallback(() => {
    const p = playerRef.current;
    if (!p || p.isDisposed()) return;
    
    const currentTime = p.currentTime() || 0;
    const totalDuration = p.duration() || duration || 0;
    const isFinished = p.ended() || (totalDuration > 0 && currentTime / totalDuration > 0.95);

    api.updatePlaybackProgress({
      media_id: mediaId,
      media_type: mediaType,
      position_ms: Math.round(currentTime * 1000),
      duration_ms: Math.round(totalDuration * 1000),
      is_finished: isFinished
    }).catch(err => console.error("Heartbeat failed:", err));
  }, [mediaId, mediaType, duration]);

  // Initialize hotkeys and core logic
  const { seekStep } = useVideoPlayer(player);

  // MediaSession integration
  useEffect(() => {
    if (!player || !('mediaSession' in navigator)) return;

    navigator.mediaSession.metadata = new MediaMetadata({
      title: title,
      artist: 'Media Manager',
      artwork: [
        { src: getImageUrl(posterUrl), sizes: '512x512', type: 'image/jpeg' }
      ]
    });

    const handlers: [MediaSessionAction, MediaSessionActionHandler][] = [
      ['play', () => player.play()],
      ['pause', () => player.pause()],
      ['seekbackward', (details) => {
        const skipTime = details.seekOffset || 10;
        seekStep(-skipTime);
      }],
      ['seekforward', (details) => {
        const skipTime = details.seekOffset || 10;
        seekStep(skipTime);
      }],
      ['previoustrack', () => {
        player.currentTime(0);
      }],
      ['nexttrack', () => {
        // No-op for single video player
      }]
    ];

    for (const [action, handler] of handlers) {
      try {
        navigator.mediaSession.setActionHandler(action, handler);
      } catch (error) {
        console.warn(`Media session action "${action}" not supported.`);
      }
    }

    return () => {
      for (const [action] of handlers) {
        try {
          navigator.mediaSession.setActionHandler(action, null);
        } catch (error) {}
      }
    };
  }, [player, title, posterUrl, seekStep]);

  useEffect(() => {
    if (!videoRef.current) return;

    // Cleanup existing player
    if (playerRef.current) {
      playerRef.current.dispose();
      playerRef.current = null;
      setPlayer(null);
    }

    const videoElement = document.createElement("video-js");
    videoElement.classList.add('vjs-big-play-centered', 'vjs-fluid');
    videoRef.current.appendChild(videoElement);

    console.log('Initializing Video.js with URL:', initialUrl);
    const p = playerRef.current = videojs(videoElement, {
      autoplay: true,
      controls: true,
      responsive: true,
      fluid: true,
      preload: 'auto',
      liveui: true,
      controlBar: {
        volumePanel: {
          inline: false,
          vertical: true
        }
      },
      plugins: {
        abLoopPlugin: {},
        sourceSelector: {
          sources: [
            { label: 'Direct Play', protocol: 'direct' },
            { label: 'HLS', protocol: 'hls' },
            { label: 'DASH', protocol: 'dash' }
          ],
          selectedProtocol: currentProtocol,
          onSelect: (protocol: Protocol) => {
            switchProtocol(protocol);
          }
        }
      },
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
        src: initialUrl,
        type: getSourceType(initialUrl, currentProtocol)
      }]
    });

    setPlayer(p);

    p.ready(() => {
      console.log('Video.js player is ready');
      
      if (duration > 0) {
        p.duration(duration);
      }

      if (initialPosition > 0) {
         p.currentTime(initialPosition / 1000);
      }

      if (heartbeatInterval.current) clearInterval(heartbeatInterval.current);
      heartbeatInterval.current = setInterval(sendHeartbeat, 30000);
    });

    p.on('loadedmetadata', () => {
      if (duration > 0) {
        p.duration(duration);
      }
    });

    p.on('playing', () => {
      setIsPreparing(false);
      requestWakeLock();
    });

    p.on('pause', () => {
      releaseWakeLock();
    });

    p.on('waiting', () => {
      // Potentially release wake lock if buffering for too long? 
      // Usually keep it active while "playing" state is intended
    });
    
    p.on('error', () => {
      const playerError = p.error();
      console.error('Video.js Error:', playerError);
      
      // Intelligent Fallback Logic
      if (playerError && playerError.code === 4) { // MEDIA_ERR_SRC_NOT_SUPPORTED
        // Note: fallback logic will trigger currentProtocol update, which triggers the surgical update effect
        const currentIndex = PROTOCOL_PRIORITY.indexOf(currentProtocol);
        if (currentIndex < PROTOCOL_PRIORITY.length - 1) {
          const nextProtocol = PROTOCOL_PRIORITY[currentIndex + 1];
          console.log(`Fallback: Switching to ${nextProtocol}`);
          switchProtocol(nextProtocol);
          return;
        }
      }

      releaseWakeLock();
      if (playerError) {
        setError(`Streaming error: ${playerError.message || 'Unknown error'} (Code: ${playerError.code})`);
      }
    });

    return () => {
      if (heartbeatInterval.current) clearInterval(heartbeatInterval.current);
      releaseWakeLock();
      if (p && !p.isDisposed()) {
        p.dispose();
        playerRef.current = null;
        setPlayer(null);
      }
    };
  }, [initialUrl, duration, initialPosition, sendHeartbeat, requestWakeLock, releaseWakeLock, switchProtocol, getSourceType]);

  // Dispose the player on unmount
  useEffect(() => {
    const player = playerRef.current;

    return () => {
      if (heartbeatInterval.current) clearInterval(heartbeatInterval.current);
      releaseWakeLock();
      // Send one final heartbeat before closing
      sendHeartbeat();

      if (player && !player.isDisposed()) {
        player.dispose();
        playerRef.current = null;
      }
    };
  }, [sendHeartbeat, releaseWakeLock]);

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
      <VttThumbnails player={player} />
    </div>
  );
};

export default React.memo(VideoPlayer);
