import React, { useState, useEffect } from 'react';
import { X, Star, Play, Calendar, Clock, Monitor, Cpu, CheckCircle2, RefreshCw, Loader2 } from 'lucide-react';
import { getImageUrl, api } from '../api/adapter';
import toast from 'react-hot-toast';
import VideoPlayer from './VideoPlayer';

interface DetailModalProps {
  item: any;
  onClose: () => void;
  onRefresh: (id: number) => void;
  onAdvanced: (id: number) => void;
  onDownload: (id: number, type: 'movie' | 'tv') => void;
  refreshingIds: Record<number, boolean>;
  loadData: () => void;
}

const DetailModal: React.FC<DetailModalProps> = ({ 
  item, onClose, onRefresh, onAdvanced, onDownload, refreshingIds, loadData 
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [editForm, setEditForm] = useState<any>(null);
  const [seasons, setSeasons] = useState<any[]>([]);
  const [episodes, setEpisodes] = useState<Record<number, any[]>>({});
  const [playbackStatus, setPlaybackStatus] = useState<any>(null);
  
  const [isStartingStream, setIsStartingStream] = useState(false);
  const [streamingUrl, setStreamingUrl] = useState<string | null>(null);
  const [activeMediaId, setActiveMediaId] = useState<number | null>(null);
  const [activeMediaType, setActiveMediaType] = useState<'movie' | 'episode' | null>(null);
  const [resumePosition, setResumePosition] = useState(0);
  const [showResumeDialog, setShowResumeDialog] = useState(false);
  const [pendingUrl, setPendingUrl] = useState<string | null>(null);

  const isShow = 'library_id' in item && !('runtime' in item);

  const handlePlayMedia = async (mediaId: number, mediaType: 'movie' | 'episode') => {
    if (isStartingStream) return;
    setIsStartingStream(true);
    try {
      const status = await api.getPlaybackStatus(mediaType, mediaId);
      const url = await api.startStreaming(mediaId, mediaType);

      setActiveMediaId(mediaId);
      setActiveMediaType(mediaType);
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
    // If it's a JIT stream (contains /direct/), we MUST update the start parameter
    // so FFmpeg performs server-side seeking.
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
      const data = await api.getSeasons(showId);
      setSeasons(data);
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
      year: item.year || null,
      plot: item.plot || '',
      rating: item.rating || 0,
      genres: genres,
      tagline: item.tagline || '',
      runtime: item.runtime || null,
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
      
      <div className="relative w-full max-w-6xl max-h-full overflow-y-auto bg-[#111] rounded-2xl shadow-[0_0_100px_rgba(0,0,0,0.8)] border border-zinc-800 scrollbar-hide animate-in zoom-in-95 duration-500">
        <button 
          className="absolute top-6 right-6 z-[110] bg-black/50 hover:bg-red-600 backdrop-blur-xl rounded-full p-2 text-white transition active:scale-90"
          onClick={onClose}
        >
          <X className="w-6 h-6" />
        </button>

        <div className="relative h-[40vh] md:h-[60vh] w-full">
          <img 
            src={getImageUrl(item.backdrop_url)} 
            className="w-full h-full object-cover brightness-[0.5]"
            alt="Backdrop"
          />
          <div className="absolute inset-0 bg-gradient-to-t from-[#111] via-[#111]/20 to-transparent" />
          <div className="absolute bottom-10 left-10 space-y-4 w-full pr-20">
            {isEditing ? (
              <input 
                value={editForm.title}
                onChange={(e) => setEditForm({...editForm, title: e.target.value})}
                className="text-4xl md:text-6xl font-black bg-transparent border-b border-red-600 text-white italic tracking-tighter uppercase outline-none w-full"
              />
            ) : (
              <h2 className="text-4xl md:text-6xl font-black text-white italic tracking-tighter uppercase">{item.title}</h2>
            )}
            <div className="flex flex-wrap items-center gap-6 text-sm font-black uppercase tracking-widest text-zinc-400">
              <span className="flex items-center gap-1.5 text-green-500">
                <Star className="w-4 h-4 fill-current" />
                {isEditing ? (
                  <input
                    type="number" step="0.1" max="10" min="0"
                    value={editForm.rating}
                    onChange={(e) => setEditForm({...editForm, rating: parseFloat(e.target.value)})}
                    className="bg-zinc-900 border border-zinc-700 rounded px-2 w-16 outline-none"
                  />
                ) : (
                  `${Math.round((item.rating || 0) * 10)}% Match`
                )}
              </span>
              {item.trailer_url && (
                <button 
                  onClick={() => window.open(item.trailer_url!, '_blank')}
                  className="flex items-center gap-2 hover:text-red-500 transition cursor-pointer"
                >
                  <Play className="w-4 h-4" /> Trailer
                </button>
              )}

              {!isShow && (
                <span className="flex items-center gap-1.5">
                  <Calendar className="w-4 h-4" /> 
                  {isEditing ? (
                    <input 
                      type="number"
                      value={editForm.year || ''}
                      onChange={(e) => setEditForm({...editForm, year: parseInt(e.target.value) || null})}
                      className="bg-zinc-900 border border-zinc-700 rounded px-2 w-20 outline-none"
                    />
                  ) : (
                    item.year
                  )}
                </span>
              )}
              {playbackStatus && !playbackStatus.is_finished && (
                <div className="flex items-center gap-3 flex-1 max-w-xs">
                  <div className="flex-1 h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                    <div 
                      className="h-full bg-red-600" 
                      style={{ width: `${(playbackStatus.position_ms / playbackStatus.duration_ms) * 100}%` }}
                    />
                  </div>
                  <span className="text-[10px] text-zinc-500 font-black">
                    {Math.floor(playbackStatus.position_ms / 60000)}m left
                  </span>
                </div>
              )}
              {!isShow && item.runtime && <span className="flex items-center gap-1.5"><Clock className="w-4 h-4" /> {Math.floor(item.runtime! / 60)}h {item.runtime! % 60}m</span>}
              {item.aspect_ratio && <span className="px-2 py-0.5 border border-zinc-700 rounded text-[10px] text-zinc-500">{item.aspect_ratio}</span>}
              <span className="px-2 py-0.5 border border-zinc-700 rounded text-[10px]">{item.status}</span>
            </div>
          </div>
        </div>

        <div className="p-10 grid grid-cols-1 lg:grid-cols-3 gap-12">
          <div className="lg:col-span-2 space-y-8">
            {!isShow && item.tagline && <p className="text-2xl font-medium text-zinc-400 italic">"{item.tagline}"</p>}
            
              {isEditing ? (
                <div className="space-y-4">
                  <div>
                    <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">Tagline</label>
                    <input 
                      value={editForm.tagline}
                      onChange={(e) => setEditForm({...editForm, tagline: e.target.value})}
                      className="w-full bg-zinc-900/50 border border-zinc-700 rounded-lg px-4 py-2 text-zinc-300 outline-none focus:border-red-600 transition"
                      placeholder="Epic tagline here..."
                    />
                  </div>
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">Runtime (mins)</label>
                      <input 
                        type="number"
                        value={editForm.runtime || ''}
                        onChange={(e) => setEditForm({...editForm, runtime: parseInt(e.target.value) || null})}
                        className="w-full bg-zinc-900/50 border border-zinc-700 rounded-lg px-4 py-2 text-zinc-300 outline-none focus:border-red-600 transition"
                      />
                    </div>
                    <div>
                      <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">Language</label>
                      <input 
                        value={editForm.language}
                        onChange={(e) => setEditForm({...editForm, language: e.target.value})}
                        className="w-full bg-zinc-900/50 border border-zinc-700 rounded-lg px-4 py-2 text-zinc-300 outline-none focus:border-red-600 transition"
                      />
                    </div>
                  </div>
                  <div>
                    <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">Plot / Overview</label>
                    <textarea 
                      value={editForm.plot}
                      onChange={(e) => setEditForm({...editForm, plot: e.target.value})}
                      rows={6}
                      className="w-full bg-zinc-900/50 border border-zinc-700 rounded-xl p-4 text-zinc-300 outline-none focus:border-red-600 transition"
                    />
                  </div>
                  <div>
                    <label className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-1 block">YouTube Trailer URL</label>
                    <input 
                      value={editForm.trailer_url}
                      onChange={(e) => setEditForm({...editForm, trailer_url: e.target.value})}
                      className="w-full bg-zinc-900/50 border border-zinc-700 rounded-lg px-4 py-2 text-zinc-300 outline-none focus:border-red-600 transition"
                      placeholder="https://www.youtube.com/watch?v=..."
                    />
                  </div>
                </div>
              ) : (
                <p className="text-xl leading-relaxed text-zinc-300 font-medium">
                  {item.plot || "No description available for this title."}
                </p>
              )}

            {isShow && !isEditing && (
              <div className="space-y-8">
                <h3 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Seasons & Episodes</h3>
                <div className="space-y-6">
                  {seasons.map(season => (
                    <div key={season.id} className="space-y-4">
                      <div className="flex items-center gap-4 text-white font-black uppercase italic tracking-tight border-b border-zinc-800 pb-2">
                         <div className="w-1.5 h-6 bg-red-600 rounded-full" />
                         Season {season.season_number}
                      </div>
                      <div className="grid gap-2">
                        {episodes[season.id]?.map(ep => (
                          <div key={ep.id} onClick={() => handlePlayMedia(ep.id, 'episode')} className="flex items-center justify-between p-4 bg-zinc-900/30 rounded-xl border border-zinc-800/50 hover:bg-zinc-800/50 transition group cursor-pointer">
                            <div className="flex items-center gap-6">
                              <div className="text-2xl font-black text-zinc-700 italic group-hover:text-red-600 transition">
                                {ep.episode_number.toString().padStart(2, '0')}
                              </div>
                              <div>
                                <div className="text-zinc-200 font-bold">{ep.title || `Episode ${ep.episode_number}`}</div>
                                <div className="flex items-center gap-3 text-[10px] text-zinc-500 font-mono mt-0.5 uppercase tracking-tighter">
                                  <span className="flex items-center gap-1"><Monitor className="w-3 h-3" /> {ep.resolution || 'SD'}</span>
                                  <span className="flex items-center gap-1"><Cpu className="w-3 h-3" /> {ep.codec || 'AVC'}</span>
                                  <span>{ep.original_name}</span>
                                </div>
                              </div>
                            </div>
                            <div className="flex items-center gap-2">
                              <button 
                                onClick={(e) => { 
                                  e.stopPropagation(); 
                                  api.playEpisode(ep.id); 
                                  toast.success("Opening in local player...");
                                }} 
                                className="p-2 hover:bg-zinc-800 rounded-lg text-zinc-600 hover:text-white transition"
                                title="Play Locally"
                              >
                                <Monitor className="w-4 h-4" />
                              </button>
                              <button 
                                onClick={(e) => { e.stopPropagation(); onDownload(ep.id, 'tv'); }} 
                                className="p-2 hover:bg-zinc-800 rounded-lg text-zinc-600 hover:text-white transition"
                              >
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/></svg>
                              </button>
                              {isStartingStream ? (
                                <Loader2 className="w-5 h-5 animate-spin text-red-600" />
                              ) : (
                                <Play className="w-5 h-5 text-zinc-600 group-hover:text-white transition" />
                              )}
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div className="space-y-4">
              <h3 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Genres</h3>
              <div className="flex flex-wrap gap-2">
                {isEditing ? (
                  <input 
                    value={Array.isArray(editForm.genres) ? editForm.genres.join(', ') : ''}
                    onChange={(e) => setEditForm({...editForm, genres: e.target.value.split(',').map((s: string) => s.trim()).filter((s: string) => s)})}
                    className="bg-zinc-900 border border-zinc-700 rounded px-4 py-2 text-sm text-zinc-300 outline-none w-full"
                    placeholder="Comedy, Drama, Sci-Fi"
                  />
                ) : (
                  genres.length > 0 ? genres.map((genre: string) => (
                    <span key={genre} className="px-4 py-1.5 bg-zinc-900 rounded-full text-xs font-bold text-zinc-400 border border-zinc-800">{genre}</span>
                  )) : <span className="text-zinc-600 text-xs font-bold uppercase italic">No genres listed</span>
                )}
              </div>
            </div>

            {!isEditing && castList.length > 0 && (
              <div className="space-y-6">
                <h3 className="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Top Cast</h3>
                <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-4">
                  {castList.slice(0, 10).map((actor: any) => (
                    <div key={actor.name} className="bg-zinc-900/50 p-2 pb-4 rounded-xl border border-zinc-800/50 flex flex-col items-center justify-center text-center gap-3 group/actor overflow-hidden">
                      <div className="w-full aspect-square rounded-lg bg-zinc-800 flex items-center justify-center text-xl font-black text-red-600 uppercase border border-zinc-700 overflow-hidden shadow-inner relative">
                        {actor.image ? (
                          <img src={getImageUrl(actor.image)} className="w-full h-full object-cover group-hover/actor:scale-110 transition duration-500" alt={actor.name} />
                        ) : (
                          <span>{actor.name.split(' ').map((n: string) => n[0]).join('')}</span>
                        )}
                      </div>
                      <div className="space-y-1">
                        <div className="text-[11px] font-black text-zinc-100 line-clamp-1">{actor.name}</div>
                        {actor.role && <div className="text-[9px] font-bold text-zinc-500 line-clamp-1 italic">{actor.role}</div>}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>

          <div className="space-y-8">
            <div className="bg-zinc-900/30 p-6 rounded-2xl border border-zinc-800/50 space-y-4">
              {isEditing ? (
                <>
                  <button 
                    onClick={handleSaveMetadata}
                    className="w-full bg-green-600 hover:bg-green-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition active:scale-95 flex items-center justify-center gap-2"
                  >
                    <CheckCircle2 className="w-4 h-4" /> Save Changes
                  </button>
                  <button 
                    onClick={() => setIsEditing(false)}
                    className="w-full bg-zinc-800 hover:bg-zinc-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition border border-zinc-700 flex items-center justify-center gap-2"
                  >
                    Cancel
                  </button>
                </>
              ) : (
                <>
                  <div className="flex flex-col gap-3">
                    {!isShow && (
                      <button 
                        onClick={() => handlePlayMedia(item.id, 'movie')}
                        disabled={isStartingStream}
                        className={`w-full py-4 rounded-xl font-black uppercase tracking-widest text-xs transition active:scale-95 flex items-center justify-center gap-2 shadow-lg ${
                          isStartingStream 
                            ? 'bg-zinc-800 text-zinc-500 border-zinc-700 cursor-wait' 
                            : 'bg-red-600 hover:bg-red-700 text-white shadow-red-900/20'
                        }`}
                      >
                        {isStartingStream ? (
                          <Loader2 className="w-4 h-4 animate-spin text-red-600" />
                        ) : (
                          <Play className="w-4 h-4 fill-current" />
                        )}
                        {isStartingStream ? 'Starting Engine...' : 'Stream (Browser)'}
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
                      className="w-full bg-zinc-800 hover:bg-zinc-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition border border-zinc-700 flex items-center justify-center gap-2"
                    >
                      <Monitor className="w-4 h-4" /> Play Locally (VLC)
                    </button>
                  </div>
                  {isShow && (
                    <button 
                      className="w-full bg-red-600/20 text-red-500 border border-red-900/30 py-4 rounded-xl font-black uppercase tracking-widest text-xs cursor-default flex items-center justify-center gap-2"
                    >
                      Select Episode Below
                    </button>
                  )}
                  <button 
                    onClick={startEditing}
                    className="w-full bg-zinc-800 hover:bg-zinc-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition border border-zinc-700 flex items-center justify-center gap-2"
                  >
                    Edit Metadata
                  </button>
                  <button 
                    onClick={() => onRefresh(item.id)}
                    disabled={refreshingIds[item.id]}
                    className={`w-full py-4 rounded-xl font-black uppercase tracking-widest text-xs transition border flex items-center justify-center gap-2 ${
                      refreshingIds[item.id] 
                        ? 'bg-zinc-900 text-zinc-600 border-zinc-800 cursor-not-allowed' 
                        : 'bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border-zinc-700'
                    }`}
                  >
                    <RefreshCw className={`w-4 h-4 ${refreshingIds[item.id] ? 'animate-spin' : ''}`} /> 
                    {refreshingIds[item.id] ? 'Refreshing...' : 'Refresh Metadata'}
                  </button>
                  {!isShow && (
                    <button 
                      onClick={() => onAdvanced(item.id)}
                      disabled={refreshingIds[item.id]}
                      className="w-full bg-zinc-800 hover:bg-zinc-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition border border-zinc-700 flex items-center justify-center gap-2"
                    >
                      <Monitor className={`w-4 h-4 ${refreshingIds[item.id] ? 'animate-pulse' : ''}`} /> Advanced Analysis
                    </button>
                  )}
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
                      className="w-full bg-zinc-800 hover:bg-zinc-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition border border-zinc-700 flex items-center justify-center gap-2"
                    >
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/></svg> Rename File
                    </button>
                  )}
                  {!isShow && (
                    <button 
                      onClick={() => onDownload(item.id, 'movie')}
                      className="w-full bg-zinc-800 hover:bg-zinc-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition border border-zinc-700 flex items-center justify-center gap-2"
                    >
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/></svg> Download
                    </button>
                  )}
                </>
              )}
            </div>

            <div className="px-2 space-y-4">
              <div className="flex justify-between items-center text-xs">
                <span className="text-zinc-600 font-bold uppercase tracking-widest">Database ID</span>
                <span className="text-zinc-400 font-mono">#{item.id}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {showResumeDialog && (
        <div className="fixed inset-0 z-[150] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={() => setShowResumeDialog(false)} />
          <div className="relative bg-[#181818] border border-zinc-800 p-8 rounded-2xl max-w-sm w-full shadow-2xl space-y-6">
            <div className="space-y-2 text-center">
              <h4 className="text-xl font-black text-white uppercase italic">Resume Playback?</h4>
              <p className="text-zinc-500 text-sm font-medium">You have a saved position at {Math.floor(resumePosition / 60000)} minutes.</p>
            </div>
            <div className="flex flex-col gap-2">
              <button 
                onClick={() => {
                  if (pendingUrl) {
                    setStreamingUrl(getStreamingUrlWithSeek(pendingUrl, resumePosition));
                  }
                  setShowResumeDialog(false);
                }}
                className="w-full bg-red-600 hover:bg-red-700 py-3 rounded-xl font-black uppercase text-xs tracking-widest text-white transition"
              >
                Resume from Last Spot
              </button>
              <button 
                onClick={() => {
                  setResumePosition(0);
                  if (pendingUrl) {
                    setStreamingUrl(getStreamingUrlWithSeek(pendingUrl, 0));
                  }
                  setShowResumeDialog(false);
                }}
                className="w-full bg-zinc-800 hover:bg-zinc-700 py-3 rounded-xl font-black uppercase text-xs tracking-widest text-white transition border border-zinc-700"
              >
                Start from Beginning
              </button>
            </div>
          </div>
        </div>
      )}

      {streamingUrl && (
        <VideoPlayer 
          url={streamingUrl} 
          mediaId={activeMediaId!}
          mediaType={activeMediaType!}
          duration={item.duration_secs || (item.runtime ? item.runtime * 60 : 0)}
          initialPosition={resumePosition}
          onClose={async () => {
            setStreamingUrl(null);
            // Refresh playback status for this item
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
