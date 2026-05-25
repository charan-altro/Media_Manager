import React, { useState, useEffect } from 'react';
import { X, Star, Play, Monitor, CheckCircle2, RefreshCw, Loader2 } from 'lucide-react';
import { getImageUrl, api, type Movie, type TVShow } from '../api/adapter';
import toast from 'react-hot-toast';
import VidstackPlayer from './VidstackPlayer';

import { useMediaStore } from '../context/MediaStoreContext';

interface DetailModalProps {
  item: Movie | TVShow;
  onClose: () => void;
  onRefresh: (id: number) => void;
  onAdvanced: (id: number, type: 'movie' | 'tv') => void;
  onDownload: (id: number, type: 'movie' | 'tv') => void;
  refreshingIds: Record<number, boolean>;
  loadData: () => void;
}

const DetailModal: React.FC<DetailModalProps> = ({ 
  item, onClose, onRefresh, onAdvanced, onDownload, refreshingIds, loadData 
}) => {
  const { libraries } = useMediaStore();
  const library = libraries.find(l => l.id === item.library_id);
  const isShow = library ? library.media_type === 'tv' : false;
  
  const [isEditing, setIsEditing] = useState(false);
  const [editForm, setEditForm] = useState<Partial<Movie & TVShow>>({});
  const [seasons, setSeasons] = useState<any[]>([]);
  const [selectedSeasonId, setSelectedSeasonId] = useState<number | null>(null);
  const [episodes, setEpisodes] = useState<Record<number, any[]>>({});
  const [playbackStatus, setPlaybackStatus] = useState<any>(null);
  
  const [isStartingStream, setIsStartingStream] = useState(false);
  const [streamingUrl, setStreamingUrl] = useState<string | null>(null);
  const [activeMediaId, setActiveMediaId] = useState<number | null>(null);
  const [activeMediaType, setActiveMediaType] = useState<'movie' | 'episode' | null>(null);
  const [activeTitle, setActiveTitle] = useState<string>('');
  const [activePosterUrl, setActivePosterUrl] = useState<string | undefined>(undefined);
  const [activeHash, setActiveHash] = useState<string | undefined>(undefined);
  const [resumePosition, setResumePosition] = useState(0);
  const [showResumeDialog, setShowResumeDialog] = useState(false);
  const [pendingUrl, setPendingUrl] = useState<string | null>(null);

  const currentSeason = seasons.find(s => s.id === selectedSeasonId);
  const seasonNumber = currentSeason ? currentSeason.season_number : 1;
  const paddedSeason = seasonNumber.toString().padStart(2, '0');

  const handlePlayMedia = async (mediaId: number, mediaType: 'movie' | 'episode', metadata?: { title: string, posterUrl?: string, videoCodec?: string, audioCodec?: string, hash?: string }) => {
    if (isStartingStream) return;
    setIsStartingStream(true);
    try {
      const status = await api.getPlaybackStatus(mediaType, mediaId);
      const url = await api.startStreaming(mediaId, mediaType);

      setActiveMediaId(mediaId);
      setActiveMediaType(mediaType);
      
      if (metadata) {
        setActiveTitle(metadata.title);
        setActivePosterUrl(metadata.posterUrl);
        setActiveHash(metadata.hash);
      } else {
        setActiveTitle(item.title);
        setActivePosterUrl(item.poster_url || item.backdrop_url);
        setActiveHash(item.hash);
      }

      if (status && status.position_ms > 5000 && !status.is_finished) {
        setResumePosition(status.position_ms);
        setPendingUrl(url);
        setShowResumeDialog(true);
      } else {
        setResumePosition(0);
        setStreamingUrl(url);
      }
    } catch (err: any) {
      console.warn("Streaming start failed, attempting preview fallback:", err);
      try {
        const url = await api.generatePreview(mediaId, mediaType);
        setActiveMediaId(mediaId);
        setActiveMediaType(mediaType);
        setResumePosition(0);
        setStreamingUrl(url);
      } catch (innerErr: any) {
        toast.error('Playback failed. Forcing UI refresh...');
        setTimeout(() => window.location.reload(), 2000);
      }
    } finally {
      setIsStartingStream(false);
    }
  };

  const getStreamingUrlWithSeek = (url: string, positionMs: number) => {
    if (!url) return url;
    if (url.includes('/direct/') && url.includes('start=')) {
      const seconds = Math.floor(positionMs / 1000);
      return url.replace(/start=\d+(\.\d+)?/, `start=${seconds}`);
    }
    return url;
  };

  const genres = React.useMemo(() => {
    try {
      const g = typeof item.genres === 'string' ? JSON.parse(item.genres) : item.genres;
      return Array.isArray(g) ? g : [];
    } catch (e) {
      return [];
    }
  }, [item.genres]);

  const castList = React.useMemo(() => {
    try {
      const c = typeof item.cast_list === 'string' ? JSON.parse(item.cast_list) : item.cast_list;
      return Array.isArray(c) ? c : [];
    } catch (e) {
      return [];
    }
  }, [item.cast_list]);

  const loadSeasons = async (showId: number) => {
    try {
      setSeasons([]);
      setSelectedSeasonId(null);
      setEpisodes({});
      const data = await api.getSeasons(showId);
      data.sort((a: any, b: any) => a.season_number - b.season_number);
      setSeasons(data);
      if (data.length > 0) {
        setSelectedSeasonId(data[0].id);
      }
      for (const season of data) {
        loadEpisodes(season.id);
      }
    } catch (err) {
      console.error('Failed to load seasons', err);
    }
  };

  const loadEpisodes = async (seasonId: number) => {
    try {
      const data = await api.getEpisodes(seasonId);
      setEpisodes(prev => ({ ...prev, [seasonId]: data }));
    } catch (err) {
      console.error('Failed to load episodes', err);
    }
  };

  useEffect(() => {
    if (isShow) {
      loadSeasons(item.id);
    }
    const type = isShow ? 'tv' : 'movie';
    api.getPlaybackStatus(type, item.id).then(setPlaybackStatus).catch(() => setPlaybackStatus(null));
  }, [item, isShow]);

  const startEditing = () => {
    setEditForm({
      title: item.title,
      year: item.year || undefined,
      plot: item.plot || '',
      rating: item.rating || 0,
      genres: genres as any,
      tagline: item.tagline || '',
      runtime: item.runtime || undefined,
      language: item.language || '',
      trailer_url: item.trailer_url || '',
    });
    setIsEditing(true);
  };

  const handleSaveMetadata = async () => {
    if (!editForm) return;
    try {
      if (!isShow) {
        await api.updateMovie(item.id, editForm);
      } else {
        await api.updateTvShow(item.id, editForm);
      }
      setIsEditing(false);
      loadData();
    } catch (err) {
      console.error('Failed to save metadata', err);
      toast.error('Failed to save: ' + (err as any).message);
    }
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 md:p-8 animate-in fade-in duration-300">
      <div className="absolute inset-0 bg-black/90 backdrop-blur-xl" onClick={onClose} />
      
      <div className="relative w-full max-w-6xl max-h-full overflow-y-auto bg-[#111] rounded-2xl shadow-[0_0_100px_rgba(0,0,0,0.9)] border border-zinc-850 scrollbar-hide animate-in zoom-in-95 duration-500">
        <button 
          className="absolute top-6 right-6 z-[110] bg-black/60 hover:bg-red-600 backdrop-blur-md rounded-full p-2.5 text-white transition duration-300 active:scale-90 border border-white/5"
          onClick={onClose}
        >
          <X className="w-5 h-5" />
        </button>

        {/* Immersive Header Banner (Netflix Style) */}
        <div className="relative h-[45vh] md:h-[65vh] w-full">
          <img 
            src={getImageUrl(item.backdrop_url || item.poster_url)} 
            className="w-full h-full object-cover brightness-[0.45]"
            alt="Backdrop"
          />
          <div className="absolute inset-0 bg-gradient-to-t from-[#111] via-[#111]/30 to-transparent" />
          <div className="absolute inset-0 bg-gradient-to-r from-black/80 via-black/20 to-transparent" />
          
          <div className="absolute bottom-8 left-6 md:left-12 right-6 md:right-12 space-y-5">
            {isEditing ? (
              <input 
                value={editForm.title}
                onChange={(e) => setEditForm({...editForm, title: e.target.value})}
                className="text-3xl md:text-6xl font-black bg-transparent border-b border-red-650 text-white italic tracking-tighter uppercase outline-none w-full"
              />
            ) : (
              <h2 className="text-4xl md:text-6xl font-black text-white italic tracking-tighter uppercase drop-shadow-2xl leading-none">{item.title}</h2>
            )}
            
            <div className="flex flex-wrap items-center gap-4 text-xs font-black uppercase tracking-widest text-zinc-400">
              <span className="flex items-center gap-1.5 text-green-500 font-extrabold">
                <Star className="w-4 h-4 fill-current" />
                {isEditing ? (
                  <input
                    type="number" step="0.1" max="10" min="0"
                    value={editForm.rating ?? ''}
                    onChange={(e) => setEditForm({...editForm, rating: parseFloat(e.target.value)})}
                    className="bg-zinc-900 border border-zinc-700 rounded px-2 w-16 outline-none text-white"
                  />
                ) : (
                  `${Math.round((item.rating || 0) * 10)}% Match`
                )}
              </span>
              
              {!isShow && (
                <span className="bg-zinc-900/80 px-2 py-0.5 rounded border border-zinc-800 text-zinc-300">
                  {isEditing ? (
                    <input 
                      type="number"
                      value={editForm.year || ''}
                      onChange={(e) => setEditForm({...editForm, year: parseInt(e.target.value) || undefined})}
                      className="bg-zinc-900 border border-zinc-700 rounded px-2 w-20 outline-none text-xs text-white"
                    />
                  ) : (
                    item.year
                  )}
                </span>
              )}

              {!isShow && item.runtime && (
                <span className="bg-zinc-900/80 px-2 py-0.5 rounded border border-zinc-800 text-zinc-300">
                  {Math.floor(item.runtime! / 60)}h {item.runtime! % 60}m
                </span>
              )}
              {item.aspect_ratio && <span className="bg-zinc-900/80 px-2 py-0.5 rounded border border-zinc-800 text-[10px] text-zinc-500">{item.aspect_ratio}</span>}
              <span className="bg-zinc-900/80 px-2 py-0.5 rounded border border-zinc-800 text-[10px] text-zinc-400">{item.status}</span>
            </div>

            {playbackStatus && !playbackStatus.is_finished && (
              <div className="flex items-center gap-3 w-full max-w-xs animate-in slide-in-from-left-4 duration-500">
                <div className="flex-1 h-1.5 bg-zinc-800/80 rounded-full overflow-hidden">
                  <div 
                    className="h-full bg-red-650 shadow-[0_0_8px_#dc2626]" 
                    style={{ width: `${(playbackStatus.position_ms / playbackStatus.duration_ms) * 100}%` }}
                  />
                </div>
                <span className="text-[10px] text-zinc-400 font-extrabold uppercase tracking-wider">
                  {Math.floor((playbackStatus.duration_ms - playbackStatus.position_ms) / 60000)}m left
                </span>
              </div>
            )}

            {/* Play Control Action Buttons (Netflix Styled Primary Row) */}
            {!isEditing && (
              <div className="flex flex-wrap items-center gap-3 pt-2">
                {!isShow && (
                  <button 
                    onClick={() => handlePlayMedia(item.id, 'movie', { 
                      title: item.title, 
                      posterUrl: item.poster_url || item.backdrop_url,
                      videoCodec: item.video_codec,
                      audioCodec: item.audio_codec,
                      hash: item.hash
                    })}
                    disabled={isStartingStream}
                    className={`px-8 py-3.5 rounded-lg font-black uppercase tracking-widest text-xs transition-all active:scale-95 flex items-center gap-2 shadow-2xl ${
                      isStartingStream 
                        ? 'bg-zinc-800 text-zinc-500 cursor-wait' 
                        : 'bg-white hover:bg-zinc-200 text-zinc-950 shadow-white/5'
                    }`}
                  >
                    {isStartingStream ? (
                      <Loader2 className="w-4 h-4 animate-spin text-red-600" />
                    ) : (
                      <Play className="w-4 h-4 fill-current" />
                    )}
                    {isStartingStream ? 'Starting Engine...' : 'Play / Stream'}
                  </button>
                )}

                <button 
                  onClick={() => {
                    if (isShow) {
                      toast.error("Please select an episode below to play locally");
                    } else {
                      api.playMovie(item.id);
                      toast.success("Opening in local player...");
                    }
                  }}
                  className="px-6 py-3.5 rounded-lg font-black uppercase tracking-widest text-xs transition bg-zinc-850 text-zinc-200 hover:bg-zinc-700/80 hover:text-white border border-zinc-750 flex items-center gap-2 shadow-xl active:scale-95"
                >
                  <Monitor className="w-4 h-4" /> Play Locally (VLC)
                </button>

                {item.trailer_url && (
                  <button 
                    onClick={() => window.open(item.trailer_url!, '_blank')}
                    className="px-6 py-3.5 rounded-lg font-black uppercase tracking-widest text-xs transition bg-zinc-900/60 hover:bg-zinc-850/80 text-zinc-400 hover:text-white border border-zinc-800/80 flex items-center gap-2 active:scale-95"
                  >
                    Watch Trailer
                  </button>
                )}
              </div>
            )}
            
            {isShow && (
              <div className="bg-red-650/10 text-red-500 border border-red-900/20 px-5 py-2.5 rounded-lg font-black uppercase tracking-widest text-[10px] w-fit">
                Select Episode Below to Play
              </div>
            )}
          </div>
        </div>

        {/* 2-Column Detail Layout Grid */}
        <div className="p-6 md:p-12 grid grid-cols-1 lg:grid-cols-3 gap-12">
          
          {/* Left Main Area (70%): Details and Episodes */}
          <div className="lg:col-span-2 space-y-8">
            {!isShow && item.tagline && <p className="text-xl font-medium text-zinc-400 italic">"{item.tagline}"</p>}
            
            {isEditing ? (
              <div className="space-y-4 bg-zinc-900/30 p-6 rounded-2xl border border-zinc-800/80">
                <div>
                  <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">Tagline</label>
                  <input 
                    value={editForm.tagline}
                    onChange={(e) => setEditForm({...editForm, tagline: e.target.value})}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2 text-zinc-300 outline-none focus:border-red-600 transition text-xs"
                    placeholder="Epic tagline here..."
                  />
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">Runtime (mins)</label>
                    <input 
                      type="number"
                      value={editForm.runtime || ''}
                      onChange={(e) => setEditForm({...editForm, runtime: parseInt(e.target.value) || undefined})}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2 text-zinc-300 outline-none focus:border-red-600 transition text-xs"
                    />
                  </div>
                  <div>
                    <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">Language</label>
                    <input 
                      value={editForm.language}
                      onChange={(e) => setEditForm({...editForm, language: e.target.value})}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2 text-zinc-300 outline-none focus:border-red-600 transition text-xs"
                    />
                  </div>
                </div>
                <div>
                  <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">Plot / Overview</label>
                  <textarea 
                    value={editForm.plot}
                    onChange={(e) => setEditForm({...editForm, plot: e.target.value})}
                    rows={5}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-xl p-4 text-zinc-300 outline-none focus:border-red-600 transition text-xs"
                  />
                </div>
                <div>
                  <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">YouTube Trailer URL</label>
                  <input 
                    value={editForm.trailer_url}
                    onChange={(e) => setEditForm({...editForm, trailer_url: e.target.value})}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2 text-zinc-300 outline-none focus:border-red-600 transition text-xs"
                    placeholder="https://www.youtube.com/watch?v=..."
                  />
                </div>
              </div>
            ) : (
              <p className="text-base leading-relaxed text-zinc-300 font-medium whitespace-pre-line">
                {item.plot || "No description available for this title."}
              </p>
            )}

            {/* Seasons & Episodes Grid Section (TV Shows Only) */}
            {isShow && !isEditing && (
              <div className="space-y-6 pt-4">
                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-zinc-850 pb-4">
                  <h3 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Episodes</h3>
                  
                  {seasons.length > 0 && (
                    <div className="relative">
                      <select
                        value={selectedSeasonId ?? ''}
                        onChange={(e) => setSelectedSeasonId(Number(e.target.value))}
                        className="appearance-none bg-zinc-900 border border-zinc-800 rounded-lg px-4 py-2.5 pr-10 text-xs font-black uppercase tracking-wider text-zinc-200 focus:outline-none focus:border-red-650 transition cursor-pointer"
                      >
                        {seasons.map((s) => (
                          <option key={s.id} value={s.id} className="bg-zinc-950 text-zinc-200">
                            Season {s.season_number}
                          </option>
                        ))}
                      </select>
                      <div className="pointer-events-none absolute inset-y-0 right-0 flex items-center px-3 text-zinc-400">
                        <svg className="fill-current h-4 w-4" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path d="M9.293 12.95l.707.707L15.657 8l-1.414-1.414L10 10.828 5.757 6.586 4.343 8z"/></svg>
                      </div>
                    </div>
                  )}
                </div>

                {selectedSeasonId && (
                  <div className="space-y-4 animate-in fade-in duration-300">
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                      {episodes[selectedSeasonId]?.map(ep => (
                        <div 
                          key={ep.id} 
                          onClick={() => handlePlayMedia(ep.id, 'episode', { 
                            title: `${item.title} - S${paddedSeason}E${ep.episode_number.toString().padStart(2, '0')} - ${ep.title || 'Episode ' + ep.episode_number}`,
                            posterUrl: ep.thumbnail_path || item.poster_url || item.backdrop_url,
                            videoCodec: ep.video_codec || ep.codec,
                            audioCodec: ep.audio_codec,
                            hash: ep.hash
                          })} 
                          className="flex flex-col bg-zinc-900/35 rounded-xl border border-zinc-850/80 hover:border-zinc-700/80 hover:bg-zinc-850/30 transition-all duration-300 group cursor-pointer overflow-hidden shadow-md"
                        >
                          {/* Episode Card Thumbnail */}
                          <div className="relative aspect-video bg-zinc-950 overflow-hidden shrink-0">
                            {ep.thumbnail_path ? (
                              <img src={getImageUrl(ep.thumbnail_path)} className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500" alt={ep.title} />
                            ) : (
                              <div className="w-full h-full flex items-center justify-center text-zinc-700 bg-zinc-900/60">
                                <Monitor className="w-7 h-7" />
                              </div>
                            )}
                            <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center z-10">
                              <div className="w-10 h-10 rounded-full bg-white/20 hover:bg-white text-white hover:text-black flex items-center justify-center border border-white/30 transition duration-300 transform scale-90 group-hover:scale-100 shadow-2xl">
                                <Play className="w-4 h-4 fill-current translate-x-0.5" />
                              </div>
                            </div>
                            <div className="absolute bottom-2 right-2 bg-black/75 px-1.5 py-0.5 rounded text-[9px] font-bold font-mono text-zinc-400">
                              {ep.runtime ? `${ep.runtime}m` : 'HD'}
                            </div>
                          </div>
                          
                          {/* Episode Card Info */}
                          <div className="p-4 flex-1 flex flex-col justify-between space-y-2">
                            <div className="space-y-1">
                              <div className="flex items-center gap-2">
                                <span className="text-xs font-black text-red-500 tracking-wider">E{ep.episode_number}</span>
                                <span className="text-zinc-200 font-bold text-sm line-clamp-1 group-hover:text-white transition">{ep.title || `Episode ${ep.episode_number}`}</span>
                              </div>
                              {ep.plot && <p className="text-[11px] text-zinc-500 line-clamp-2 leading-relaxed font-medium">{ep.plot}</p>}
                            </div>
                            
                            <div className="flex items-center justify-between pt-2 border-t border-zinc-900/50">
                              <div className="flex items-center gap-2 text-[9px] text-zinc-500 font-mono uppercase tracking-tighter">
                                <span>{ep.resolution || '1080p'}</span>
                                <span>•</span>
                                <span>{ep.codec || 'AVC'}</span>
                              </div>
                              <div className="flex items-center gap-1">
                                <button 
                                  onClick={(e) => { 
                                    e.stopPropagation(); 
                                    api.playEpisode(ep.id); 
                                    toast.success("Opening in local player...");
                                  }} 
                                  className="p-1.5 hover:bg-zinc-800 rounded-lg text-zinc-500 hover:text-white transition"
                                  title="Play Locally"
                                >
                                  <Monitor className="w-3.5 h-3.5" />
                                </button>
                                <button 
                                  onClick={(e) => { e.stopPropagation(); onDownload(ep.id, 'tv'); }} 
                                  className="p-1.5 hover:bg-zinc-800 rounded-lg text-zinc-500 hover:text-white transition"
                                  title="Download"
                                >
                                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/></svg>
                                </button>
                              </div>
                            </div>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* Utility Rows: Action buttons inside Left Column details area */}
            <div className="space-y-4 pt-4">
              <h3 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Actions & Utilities</h3>
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                {isEditing ? (
                  <>
                    <button 
                      onClick={handleSaveMetadata}
                      className="bg-green-600 hover:bg-green-700 py-3 rounded-lg font-black uppercase tracking-widest text-[10px] text-white transition active:scale-95 flex items-center justify-center gap-2 shadow-lg"
                    >
                      <CheckCircle2 className="w-4 h-4" /> Save Changes
                    </button>
                    <button 
                      onClick={() => setIsEditing(false)}
                      className="bg-zinc-800 hover:bg-zinc-700 py-3 rounded-lg font-black uppercase tracking-widest text-[10px] text-zinc-300 transition border border-zinc-700 flex items-center justify-center gap-2"
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <>
                    <button 
                      onClick={startEditing}
                      className="bg-zinc-900/60 hover:bg-zinc-850/80 py-3 rounded-lg font-black uppercase tracking-widest text-[10px] text-zinc-300 transition border border-zinc-800/80 flex items-center justify-center gap-2"
                    >
                      Edit Metadata
                    </button>
                    <button 
                      onClick={() => onRefresh(item.id)}
                      disabled={refreshingIds[item.id]}
                      className={`py-3 rounded-lg font-black uppercase tracking-widest text-[10px] transition border flex items-center justify-center gap-2 ${
                        refreshingIds[item.id] 
                          ? 'bg-zinc-950 text-zinc-600 border-zinc-900 cursor-not-allowed' 
                          : 'bg-zinc-900/60 hover:bg-zinc-850/80 text-zinc-300 border-zinc-800/80'
                      }`}
                    >
                      <RefreshCw className={`w-3.5 h-3.5 ${refreshingIds[item.id] ? 'animate-spin' : ''}`} /> 
                      {refreshingIds[item.id] ? 'Refreshing...' : 'Refresh Info'}
                    </button>
                    <button 
                      onClick={() => onAdvanced(item.id, isShow ? 'tv' : 'movie')}
                      disabled={refreshingIds[item.id]}
                      className="bg-zinc-900/60 hover:bg-zinc-850/80 py-3 rounded-lg font-black uppercase tracking-widest text-[10px] text-zinc-300 transition border border-zinc-800/80 flex items-center justify-center gap-2"
                    >
                      Analyze Specs
                    </button>
                    {!isShow && (
                      <button 
                        onClick={async () => {
                          try {
                            await api.renameMovie(item.id);
                            toast.success('Rename started!');
                            loadData();
                          } catch (err: any) {
                            toast.error('Rename failed: ' + err.message);
                          }
                        }}
                        className="bg-zinc-900/60 hover:bg-zinc-850/80 py-3 rounded-lg font-black uppercase tracking-widest text-[10px] text-zinc-300 transition border border-zinc-800/80 flex items-center justify-center gap-2"
                      >
                        Rename File
                      </button>
                    )}
                    {!isShow && (
                      <button 
                        onClick={async () => {
                          try {
                            await api.searchSubtitles(item.id);
                            toast.success('Subtitle search started in background.');
                          } catch (err: any) {
                            toast.error('Subtitle search failed: ' + err.message);
                          }
                        }}
                        className="bg-zinc-900/60 hover:bg-zinc-850/80 py-3 rounded-lg font-black uppercase tracking-widest text-[10px] text-zinc-300 transition border border-zinc-800/80 flex items-center justify-center gap-2"
                      >
                        Find Subtitles
                      </button>
                    )}
                    {!isShow && (
                      <button 
                        onClick={() => onDownload(item.id, 'movie')}
                        className="bg-zinc-900/60 hover:bg-zinc-850/80 py-3 rounded-lg font-black uppercase tracking-widest text-[10px] text-zinc-300 transition border border-zinc-800/80 flex items-center justify-center gap-2"
                      >
                        Download File
                      </button>
                    )}
                  </>
                )}
              </div>
            </div>
          </div>

          {/* Right Column (30%): Cast, Genres, and specs list */}
          <div className="space-y-10 border-l border-zinc-900/80 pl-0 lg:pl-10">
            {/* Cast profiles */}
            {!isEditing && castList.length > 0 && (
              <div className="space-y-4">
                <h3 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Top Cast</h3>
                <div className="flex flex-col gap-4">
                  {castList.slice(0, 5).map((actor: any) => (
                    <div key={actor.name} className="flex items-center gap-3.5 group/actor">
                      <div className="w-10 h-10 rounded-full bg-zinc-800 flex items-center justify-center text-xs font-black text-red-650 uppercase border border-zinc-700/80 overflow-hidden shadow-inner shrink-0 transition-transform duration-300 group-hover/actor:scale-105">
                        {actor.image ? (
                          <img src={getImageUrl(actor.image)} className="w-full h-full object-cover" alt={actor.name} />
                        ) : (
                          <span>{actor.name.split(' ').map((n: string) => n[0]).join('')}</span>
                        )}
                      </div>
                      <div className="space-y-0.5">
                        <div className="text-xs font-bold text-zinc-200 group-hover/actor:text-white transition duration-300">{actor.name}</div>
                        {actor.role && <div className="text-[10px] font-medium text-zinc-500 italic">{actor.role}</div>}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Genres Outlined pills */}
            <div className="space-y-4">
              <h3 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Genres</h3>
              <div className="flex flex-wrap gap-2">
                {isEditing ? (
                  <input 
                    value={Array.isArray(editForm.genres) ? editForm.genres.join(', ') : ''}
                    onChange={(e) => setEditForm({...editForm, genres: e.target.value.split(',').map((s: string) => s.trim()).filter((s: string) => s) as any})}
                    className="bg-zinc-900 border border-zinc-800 rounded-lg px-4 py-2 text-xs text-zinc-300 outline-none w-full"
                    placeholder="Comedy, Drama, Sci-Fi"
                  />
                ) : (
                  genres.length > 0 ? genres.map((genre: string) => (
                    <span key={genre} className="px-3.5 py-1.5 bg-zinc-900/60 rounded-full text-xs font-semibold text-zinc-300 border border-zinc-800/80">{genre}</span>
                  )) : <span className="text-zinc-600 text-xs font-bold uppercase italic">None</span>
                )}
              </div>
            </div>

            {/* Technical Specifications details cards */}
            <div className="space-y-4 bg-zinc-900/20 p-5 rounded-2xl border border-zinc-850/60">
              <h3 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500 mb-3">Specifications</h3>
              <div className="space-y-3.5 text-xs font-medium">
                <div className="flex justify-between items-start gap-4">
                  <span className="text-zinc-500 font-bold uppercase tracking-wider text-[10px]">Database ID</span>
                  <span className="text-zinc-300 font-mono text-[10px]">#{item.id}</span>
                </div>
                {item.video_codec && (
                  <div className="flex justify-between items-start gap-4">
                    <span className="text-zinc-500 font-bold uppercase tracking-wider text-[10px]">Video Codec</span>
                    <span className="text-zinc-300 font-mono text-[10px]">{item.video_codec}</span>
                  </div>
                )}
                {item.audio_codec && (
                  <div className="flex justify-between items-start gap-4">
                    <span className="text-zinc-500 font-bold uppercase tracking-wider text-[10px]">Audio Codec</span>
                    <span className="text-zinc-300 font-mono text-[10px]">{item.audio_codec}</span>
                  </div>
                )}
                {item.resolution && (
                  <div className="flex justify-between items-start gap-4">
                    <span className="text-zinc-500 font-bold uppercase tracking-wider text-[10px]">Resolution</span>
                    <span className="text-zinc-300 font-mono text-[10px]">{item.resolution}</span>
                  </div>
                )}
                {item.file_path && (
                  <div className="space-y-1 pt-1.5 border-t border-zinc-900/40">
                    <span className="text-zinc-500 font-bold uppercase tracking-wider text-[10px] block">File Location</span>
                    <span className="text-zinc-400 font-mono text-[10px] leading-relaxed break-all block">{item.file_path}</span>
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Playback Resume Alert Dialog */}
      {showResumeDialog && (
        <div className="fixed inset-0 z-[150] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-black/80 backdrop-blur-md" onClick={() => setShowResumeDialog(false)} />
          <div className="relative bg-zinc-950 border border-zinc-800 p-8 rounded-2xl max-w-sm w-full shadow-[0_0_50px_rgba(0,0,0,0.8)] space-y-6 animate-in zoom-in-95 duration-300 z-10">
            <div className="space-y-2 text-center">
              <h4 className="text-xl font-black text-white uppercase italic tracking-tighter">Resume Playback?</h4>
              <p className="text-zinc-500 text-xs font-medium">You have a saved position at {Math.floor(resumePosition / 60000)} minutes.</p>
            </div>
            <div className="flex flex-col gap-2">
              <button 
                onClick={() => {
                  setStreamingUrl(pendingUrl ? getStreamingUrlWithSeek(pendingUrl, resumePosition) : null);
                  setShowResumeDialog(false);
                }}
                className="w-full bg-white text-zinc-950 hover:bg-zinc-200 py-3 rounded-lg font-black uppercase text-xs tracking-widest transition active:scale-95 shadow-lg"
              >
                Resume last spot
              </button>
              <button 
                onClick={() => {
                  setResumePosition(0);
                  setStreamingUrl(pendingUrl);
                  setShowResumeDialog(false);
                }}
                className="w-full bg-zinc-900 hover:bg-zinc-800 text-zinc-300 py-3 rounded-lg font-black uppercase text-xs tracking-widest transition border border-zinc-800 active:scale-95"
              >
                Start fresh
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Vidstack Video Player Overlay */}
      {streamingUrl && (
        <VidstackPlayer 
          mediaId={activeMediaId!}
          mediaType={activeMediaType!}
          title={activeTitle}
          posterUrl={activePosterUrl}
          hash={activeHash}
          duration={(item as any).duration_secs || ((item as any).runtime ? (item as any).runtime * 60 : 0)}
          initialPosition={resumePosition}
          onClose={async () => {
            setStreamingUrl(null);
            const type = isShow ? 'tv' : 'movie';
            try {
              const status = await api.getPlaybackStatus(type, item.id);
              setPlaybackStatus(status);
            } catch (e) {
              console.error("Failed to refresh playback status", e);
            }
            loadData();
          }} 
        />
      )}
    </div>
  );
};

export default React.memo(DetailModal);
