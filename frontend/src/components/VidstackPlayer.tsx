import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { 
  MediaPlayer, 
  MediaProvider, 
  Poster, 
  Gesture,
  type MediaPlayerInstance
} from '@vidstack/react';
import { 
  DefaultVideoLayout, 
  defaultLayoutIcons 
} from '@vidstack/react/player/layouts/default';

import '@vidstack/react/player/styles/default/theme.css';
import '@vidstack/react/player/styles/default/layouts/video.css';

import { X, Loader2, AlertCircle } from 'lucide-react';
import toast from 'react-hot-toast';
import { getImageUrl, api, API_BASE } from '../api/adapter';
import { useVidstackAbLoop, type AbLoopManager } from '../hooks/useVidstackAbLoop';
import { AbLoopControls } from './AbLoopControls';

const formatTime = (secs: number) => {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  if (h > 0) {
    return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }
  return `${m}:${s.toString().padStart(2, '0')}`;
};

const TimelineMarkers: React.FC<{
  markers: any[];
  duration: number;
  onSeek: (seconds: number) => void;
}> = ({ markers, duration, onSeek }) => {
  const [trackEl, setTrackEl] = useState<Element | null>(null);

  useEffect(() => {
    // Poll for the track element
    const interval = setInterval(() => {
      const el = document.querySelector('.vds-time-slider .vds-slider-track') || 
                 document.querySelector('.vds-time-slider') || 
                 document.querySelector('.vds-slider');
      if (el) {
        setTrackEl(el);
        clearInterval(interval);
      }
    }, 500);

    return () => clearInterval(interval);
  }, []);

  if (!trackEl || duration <= 0) return null;

  return createPortal(
    <div className="absolute inset-y-0 left-0 right-0 pointer-events-none" style={{ zIndex: 100 }}>
      {markers.map((marker) => {
        const pct = (marker.seconds / duration) * 100;
        if (pct < 0 || pct > 100) return null;
        return (
          <div
            key={marker.id}
            className="absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 bg-yellow-400 hover:bg-yellow-300 rounded-full border-2 border-black pointer-events-auto cursor-pointer group transition-transform hover:scale-150"
            style={{ left: `${pct}%` }}
            onClick={(e) => {
              e.stopPropagation();
              e.preventDefault();
              onSeek(marker.seconds);
            }}
          >
            {/* Custom tooltip */}
            <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block bg-zinc-950/95 border border-zinc-800 text-white text-[10px] font-black uppercase tracking-wider py-1 px-2 rounded shadow-2xl whitespace-nowrap z-[999] pointer-events-none">
              {marker.title} <span className="text-yellow-400 ml-1">{formatTime(marker.seconds)}</span>
            </div>
          </div>
        );
      })}
    </div>,
    trackEl
  );
};

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
const InnerPlayer: React.FC<VidstackPlayerProps & { 
  sources: any[], 
  isBuffering: boolean, 
  abLoop: AbLoopManager, 
  hasStarted: boolean,
  sidecarSubs: any[],
  markers: any[],
  onSeek: (seconds: number) => void
}> = ({
  mediaId,
  mediaType,
  title,
  hash,
  isBuffering,
  abLoop,
  hasStarted,
  sidecarSubs,
  markers,
  onSeek,
  duration
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

  // Screen Wake Lock support
  useEffect(() => {
    let wakeLock: any = null;
    const requestWakeLock = async () => {
      try {
        if ('wakeLock' in navigator) {
          wakeLock = await (navigator as any).wakeLock.request('screen');
          console.log("[VidstackPlayer] Wake Lock acquired successfully.");
        }
      } catch (err) {
        console.warn("[VidstackPlayer] Failed to acquire Wake Lock:", err);
      }
    };

    const releaseWakeLock = async () => {
      try {
        if (wakeLock) {
          await wakeLock.release();
          wakeLock = null;
          console.log("[VidstackPlayer] Wake Lock released.");
        }
      } catch (err) {
        console.error(err);
      }
    };

    // Acquire lock if player starts playing
    if (hasStarted) {
      requestWakeLock();
    }

    return () => {
      releaseWakeLock();
    };
  }, [hasStarted]);

  const vttUrl = useMemo(() => {
    if (!hash) return undefined;
    return `${API_BASE}/assets/${hash}/vtt`;
  }, [hash]);

  return (
    <>
      <MediaProvider>
        {!hasStarted && <Poster className="vds-poster" />}
        <Gesture className="vds-gesture" event="pointerup" action="toggle:paused" />
        <Gesture className="vds-gesture" event="dblpointerup" action="toggle:fullscreen" />
        <Gesture className="vds-gesture" event="dblpointerup" action="seek:-10" />
        <Gesture className="vds-gesture" event="dblpointerup" action="seek:10" />

        {sidecarSubs.map(sub => (
          <track
            key={sub.language}
            src={`${API_BASE}/media/${mediaType}/${mediaId}/subtitles/${sub.language}`}
            label={sub.name}
            kind="subtitles"
            srcLang={sub.language}
          />
        ))}
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
      
      <TimelineMarkers markers={markers} duration={duration || 0} onSeek={onSeek} />
      
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
  isPiped: boolean,
  onPlayerError?: (err: any) => void
}> = (props) => {
  const { title, sources, posterUrl, mediaId, mediaType, initialPosition = 0, onSeek, duration, startOffset, isPiped, onPlayerError } = props;
  const [isBuffering, setIsBuffering] = useState(false);
  const playerRef = useRef<MediaPlayerInstance>(null);
  const abLoop = useVidstackAbLoop();

  const seek = useCallback((time: number) => {
    if (playerRef.current) {
      if (isPiped) {
        onSeek(time);
      } else {
        playerRef.current.currentTime = time;
      }
    }
  }, [isPiped, onSeek]);
  
  // Scene Markers State
  const [markers, setMarkers] = useState<any[]>([]);
  const [showMarkersList, setShowMarkersList] = useState(false);
  const [showAddMarkerModal, setShowAddMarkerModal] = useState(false);
  const [newMarkerTitle, setNewMarkerTitle] = useState('');
  const [newMarkerTime, setNewMarkerTime] = useState(0);
  const [newMarkerSaving, setNewMarkerSaving] = useState(false);

  const loadMarkers = useCallback(async () => {
    try {
      const data = await api.getSceneMarkers(mediaId, mediaType);
      setMarkers(data);
    } catch (err) {
      console.error("[VidstackPlayer] Failed to load markers:", err);
    }
  }, [mediaId, mediaType]);

  useEffect(() => {
    loadMarkers();
  }, [loadMarkers]);

  // Sidecar Subtitles State
  const [sidecarSubs, setSidecarSubs] = useState<any[]>([]);

  const loadSidecarSubs = useCallback(async () => {
    try {
      const data = await api.getSidecarSubtitles(mediaId, mediaType);
      setSidecarSubs(data);
    } catch (err) {
      console.error("[VidstackPlayer] Failed to load sidecar subtitles:", err);
    }
  }, [mediaId, mediaType]);

  useEffect(() => {
    loadSidecarSubs();
  }, [loadSidecarSubs]);

  const handleSaveMarker = async () => {
    if (!newMarkerTitle.trim()) return;
    setNewMarkerSaving(true);
    try {
      await api.createSceneMarker(mediaId, mediaType, newMarkerTime, newMarkerTitle.trim());
      toast.success("Marker saved!");
      setShowAddMarkerModal(false);
      loadMarkers();
      playerRef.current?.play();
    } catch (err) {
      console.error("Failed to save marker:", err);
      toast.error("Failed to save marker.");
    } finally {
      setNewMarkerSaving(false);
    }
  };

  // Hotkeys listener for player actions
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) return;

      const player = playerRef.current;
      if (!player) return;

      // Blur focused slider/buttons inside Vidstack so arrow keys don't trigger dual actions
      if (document.activeElement && document.activeElement instanceof HTMLElement) {
        if (document.activeElement.closest('.vds-slider') || document.activeElement.closest('button')) {
          document.activeElement.blur();
        }
      }

      const key = e.key.toLowerCase();

      // 0-9 percentage-based seeks
      if (/^[0-9]$/.test(key)) {
        e.preventDefault();
        const num = parseInt(key);
        const targetTime = (durationRef.current || player.duration || 0) * (num / 10);
        seek(targetTime);
      }

      // Space play/pause toggle
      if (e.key === ' ' || key === 'spacebar') {
        e.preventDefault();
        if (player.paused) {
          player.play().catch(err => console.warn(err));
        } else {
          player.pause();
        }
      }

      // Left/Right arrows skip 5 seconds
      if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
        e.preventDefault();
        const diff = e.key === 'ArrowRight' ? 5 : -5;
        const targetTime = Math.max(0, Math.min(durationRef.current || player.duration || 0, player.currentTime + diff));
        seek(targetTime);
      }

      // Up/Down arrows volume control (by 5%)
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault();
        const diff = e.key === 'ArrowUp' ? 0.05 : -0.05;
        player.volume = Math.max(0, Math.min(1, player.volume + diff));
      }

      // F key fullscreen toggle
      if (key === 'f') {
        e.preventDefault();
        if (player.state.fullscreen) {
          player.exitFullscreen().catch(err => console.warn(err));
        } else {
          player.enterFullscreen().catch(err => console.warn(err));
        }
      }

      // M key for Add Marker dialog
      if (key === 'm') {
        e.preventDefault();
        player.pause();
        setNewMarkerTime(player.currentTime);
        setNewMarkerTitle('');
        setShowAddMarkerModal(true);
      }

      // V key for toggling Scene Bookmarks panel
      if (key === 'v') {
        e.preventDefault();
        setShowMarkersList(prev => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [seek, loadMarkers]);
  
  const lastHeartbeatTime = useRef(0);
  const durationRef = useRef(0);
  const hasSeeked = useRef(false);
  const seekTimeoutRef = useRef<any>(null);
  const isRewritingSourceRef = useRef(false);
  const loadedSourceRef = useRef<string>('');
  const [hasStarted, setHasStarted] = useState(false);
  const wasPlayingRef = useRef(true);

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
        if (wasPlayingRef.current) {
          player.play().catch(err => console.warn("[VidstackPlayer] Failed to resume playback:", err));
        }
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

    // Capture the playing state before rewriting source
    wasPlayingRef.current = !player.paused;

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
        storage="media-manager-player-settings"
        poster={hasStarted ? undefined : getImageUrl(posterUrl)}
        className="w-full h-full bg-black overflow-hidden"
        style={playerStyles}
        crossOrigin
        playsInline
        autoplay
        onPlay={() => setHasStarted(true)}
        onWaiting={() => setIsBuffering(true)}
        onPlaying={() => setIsBuffering(false)}
        onCanPlay={handleCanPlay}
        onTimeUpdate={handleTimeUpdate}
        onEnded={handleEnded}
        onSeeking={handleSeeking}
        onError={(err) => {
          if (onPlayerError) onPlayerError(err);
        }}
        streamType="on-demand"
        duration={duration}
      >
        <InnerPlayer 
          {...props} 
          isBuffering={isBuffering} 
          abLoop={abLoop} 
          hasStarted={hasStarted} 
          sidecarSubs={sidecarSubs} 
          markers={markers}
          onSeek={seek}
        />
      </MediaPlayer>

      {/* Floating button to open Bookmarks drawer */}
      {!showMarkersList && (
        <button
          onClick={() => setShowMarkersList(true)}
          className="absolute top-24 right-6 z-[180] bg-black/60 hover:bg-red-600 text-white p-3 rounded-full border border-zinc-800/80 backdrop-blur-md shadow-2xl transition active:scale-95 group/btn"
          title="Open Scene Bookmarks (Press V)"
        >
          <svg className="w-5 h-5 group-hover/btn:scale-110 transition" fill="none" stroke="currentColor" strokeWidth="2.5" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" d="M17.593 3.322c1.1.128 1.907 1.077 1.907 2.185V21L12 17.25 4.5 21V5.507c0-1.108.806-2.057 1.907-2.185a48.507 48.507 0 0111.186 0z" />
          </svg>
        </button>
      )}

      {/* Bookmarks Drawer Panel */}
      {showMarkersList && (
        <div className="absolute top-0 right-0 bottom-0 w-80 z-[250] bg-black/90 border-l border-zinc-800/80 backdrop-blur-md flex flex-col p-6 space-y-6 animate-in slide-in-from-right duration-300">
          <div className="flex items-center justify-between">
            <h3 className="text-white text-lg font-black uppercase tracking-tight italic">Scene Bookmarks</h3>
            <button 
              onClick={() => setShowMarkersList(false)}
              className="text-zinc-500 hover:text-white p-1 hover:bg-zinc-800/50 rounded-lg transition"
            >
              <X className="w-5 h-5" />
            </button>
          </div>

          <div className="flex-1 overflow-y-auto pr-2 space-y-2 scrollbar-thin">
            {markers.length === 0 ? (
              <div className="h-40 flex flex-col items-center justify-center text-center gap-1">
                <p className="text-zinc-500 text-xs uppercase font-black tracking-widest">No Markers Yet</p>
                <p className="text-zinc-600 text-[10px] font-bold">Press <kbd className="bg-zinc-900 border border-zinc-800 px-1 py-0.5 rounded font-mono font-black text-zinc-500">M</kbd> to add markers at any timestamp.</p>
              </div>
            ) : (
              markers.map((marker) => (
                <div 
                  key={marker.id} 
                  className="flex items-center justify-between p-3 bg-zinc-900/40 border border-zinc-800/50 hover:bg-zinc-800/40 rounded-xl transition group/marker"
                >
                  <button
                    onClick={() => seek(marker.seconds)}
                    className="flex-1 text-left flex flex-col gap-0.5 cursor-pointer"
                  >
                    <span className="text-zinc-200 text-xs font-bold line-clamp-1">{marker.title}</span>
                    <span className="text-zinc-500 text-[10px] font-mono font-black">{formatTime(marker.seconds)}</span>
                  </button>
                  <button
                    onClick={async (e) => {
                      e.stopPropagation();
                      if (window.confirm("Delete this scene marker?")) {
                        try {
                          await api.deleteSceneMarker(marker.id);
                          toast.success("Marker deleted!");
                          loadMarkers();
                        } catch (err) {
                          console.error("Failed to delete marker:", err);
                        }
                      }
                    }}
                    className="p-1.5 hover:bg-red-600/10 text-zinc-500 hover:text-red-500 rounded-lg transition"
                    title="Delete Marker"
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {/* Add Marker Modal */}
      {showAddMarkerModal && (
        <div className="absolute inset-0 z-[300] flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-in fade-in duration-200">
          <div className="bg-[#141414] border border-zinc-800 p-6 rounded-2xl max-w-sm w-full shadow-2xl space-y-4 animate-in zoom-in-95 duration-200">
            <div className="space-y-1 text-center">
              <h4 className="text-lg font-black text-white uppercase italic tracking-tight">Add Scene Marker</h4>
              <p className="text-zinc-500 text-xs font-mono">Position: {formatTime(newMarkerTime)}</p>
            </div>
            <input
              autoFocus
              type="text"
              placeholder="e.g. Action Scene, Intro..."
              value={newMarkerTitle}
              onChange={(e) => setNewMarkerTitle(e.target.value)}
              onKeyDown={async (e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  await handleSaveMarker();
                } else if (e.key === 'Escape') {
                  setShowAddMarkerModal(false);
                  playerRef.current?.play();
                }
              }}
              className="w-full bg-zinc-900 border border-zinc-800 rounded-lg px-4 py-2.5 text-sm text-zinc-200 outline-none focus:border-red-600 transition"
            />
            <div className="flex gap-2">
              <button
                onClick={() => {
                  setShowAddMarkerModal(false);
                  playerRef.current?.play();
                }}
                className="flex-1 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 py-2.5 rounded-lg text-xs font-black uppercase tracking-wider transition"
              >
                Cancel
              </button>
              <button
                onClick={handleSaveMarker}
                disabled={newMarkerSaving || !newMarkerTitle.trim()}
                className="flex-1 bg-red-600 hover:bg-red-700 disabled:bg-zinc-800 disabled:text-zinc-600 text-white py-2.5 rounded-lg text-xs font-black uppercase tracking-wider transition flex items-center justify-center gap-1.5"
              >
                {newMarkerSaving ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : 'Save Marker'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

const VidstackPlayer: React.FC<VidstackPlayerProps> = (props) => {
  const { mediaId, mediaType, onClose, initialPosition = 0 } = props;
  const [directUrl, setDirectUrl] = useState<string | null>(null);
  const [protocol, setProtocol] = useState<'direct' | 'hls'>('direct');
  const [startOffset, setStartOffset] = useState<number>(initialPosition / 1000);
  const [duration, setDuration] = useState<number>(props.duration || 0);
  const [isPreparing, setIsPreparing] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const isFirstLoad = useRef(true);

  const handlePlayerError = useCallback(async (err: any) => {
    console.error("[VidstackPlayer] Player error fired:", err);
    if (protocol === 'direct') {
      console.log("[VidstackPlayer] Attempting fallback to full transcoded HLS...");
      toast.error("Direct play failed. Falling back to transcoded HLS...");
      setIsPreparing(true);
      try {
        const url = await api.startStreaming(mediaId, mediaType, 'hls');
        if (url) {
          setDirectUrl(url);
          setProtocol('hls');
        } else {
          setError("Failed to generate fallback HLS stream.");
        }
      } catch (e: any) {
        setError(e.message || "Failed to start transcoded HLS stream.");
      } finally {
        setIsPreparing(false);
      }
    } else {
      setError("Playback failed on both direct play and HLS transcoded fallback.");
    }
  }, [mediaId, mediaType, protocol]);

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
      directUrl.includes('/playlist.m3u8') ||
      directUrl.includes('/stream.mp4') ||
      directUrl.includes('/stream.webm') ||
      directUrl.includes('/stream.mkv') ||
      directUrl.includes('/stream.ts')
    );
  }, [directUrl]);

  const sources = useMemo<any[]>(() => {
    if (!directUrl) return [];

    let finalUrlStr = directUrl;
    
    // Safari Workaround: Safari does not handle progressive/fragmented stream pipes well.
    // If it is Safari, and it's a piped stream (but not already HLS), redirect/rewrite it to HLS (.m3u8)
    const isSafari = typeof window !== 'undefined' && 
      /Safari/.test(navigator.userAgent) && 
      !/Chrome/.test(navigator.userAgent) && 
      !/Chromium/.test(navigator.userAgent);
      
    if (isSafari && isPiped && !directUrl.includes('.m3u8')) {
      if (directUrl.includes('/stream/direct/')) {
        finalUrlStr = directUrl.replace(/\/stream\.[a-z0-9]+/, '/playlist.m3u8');
      } else if (directUrl.includes('/stream/jit/movie/')) {
        const id = directUrl.split('/').pop();
        finalUrlStr = `/api/stream/direct/movie_${id}/playlist.m3u8`;
      } else if (directUrl.includes('/stream/jit/episode/')) {
        const id = directUrl.split('/').pop();
        finalUrlStr = `/api/stream/direct/episode_${id}/playlist.m3u8`;
      }
    }

    const url = new URL(finalUrlStr, window.location.origin);
    // Only append start parameter for piped streams (but not for HLS playlist itself)
    const finalIsPiped = url.pathname.includes('/stream.ts') || url.pathname.includes('/stream.mp4') || url.pathname.includes('/stream.webm') || url.pathname.includes('/stream.mkv');
    if (finalIsPiped && startOffset > 0) {
      url.searchParams.set("start", startOffset.toString());
    }

    const finalSrc = url.pathname + url.search;
    const type = finalUrlStr.includes('.m3u8') 
      ? 'application/x-mpegURL' 
      : (finalUrlStr.includes('.mkv') || finalUrlStr.includes('ext=mkv')
        ? 'video/webm' 
        : (finalUrlStr.includes('.webm') || finalUrlStr.includes('ext=webm')
          ? 'video/webm'
          : (finalUrlStr.includes('.ts') 
            ? 'video/mp2t' 
            : 'video/mp4')));
    return [{ src: finalSrc, type }];
  }, [directUrl, isPiped, startOffset]);

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
          onPlayerError={handlePlayerError}
        />
      )}
    </div>
  );
};

export default React.memo(VidstackPlayer);
