// frontend/src/api/adapter.ts
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

export const IS_TAURI = (window as any).__TAURI_INTERNALS__ !== undefined || (window as any).__TAURI__ !== undefined || (window as any).__TAURI_IPC__ !== undefined;
export const API_BASE = import.meta.env.VITE_API_URL || (IS_TAURI ? 'http://localhost:7878/api' : '/api');

export interface Library {
  id: number;
  name: string;
  path: string;
  media_type: string;
  created_at: string;
}

export interface Movie {
  id: number;
  library_id: number;
  title: string;
  year?: number | null;
  tmdb_id?: number;
  imdb_id?: string;
  status: string;
  plot?: string;
  rating?: number | null;
  tagline?: string;
  runtime?: number | null;
  trailer_url?: string;
  aspect_ratio?: string;
  duration_secs?: number;
  poster_url?: string;
  backdrop_url?: string;
  genres?: string | string[];
  language?: string;
  cast_list?: string;
  created_at: string;
  file_path?: string;
  resolution?: string;
  video_codec?: string;
  audio_codec?: string;
  hash?: string;
}

export interface TVShow {
  id: number;
  library_id: number;
  title: string;
  year?: number | null;
  tmdb_id?: number;
  imdb_id?: string;
  tvdb_id?: string;
  status: string;
  plot?: string;
  rating?: number | null;
  tagline?: string;
  runtime?: number | null;
  trailer_url?: string;
  aspect_ratio?: string;
  duration_secs?: number;
  poster_url?: string;
  backdrop_url?: string;
  genres?: string | string[];
  language?: string;
  cast_list?: string;
  created_at: string;
  file_path?: string;
  resolution?: string;
  video_codec?: string;
  audio_codec?: string;
  hash?: string;
}

export interface MovieFile {
  id: number;
  movie_id: number;
  file_path: string;
  original_name: string;
  size_bytes: number;
  resolution?: string | null;
  codec?: string | null;
  audio_codec?: string | null;
  duration_secs?: number | null;
  hash?: string | null;
  fingerprint?: string | null;
  is_missing: number;
  last_scanned: string;
  created_at: string;
  updated_at: string;
}

export interface Season {
  id: number;
  show_id: number;
  season_number: number;
  name?: string;
  plot?: string;
  poster_url?: string;
}

export interface Episode {
  id: number;
  season_id: number;
  episode_number: number;
  title?: string;
  original_name: string;
  plot?: string;
  rating?: number;
  runtime?: number;
  thumbnail_path?: string;
  file_path: string;
  video_codec?: string;
  audio_codec?: string;
  hash?: string;
  size_bytes?: number;
  resolution?: string;
  codec?: string;
}

export interface UpdateCheckResult {
  latest_version: string;
  current_version: string;
}

export interface DiskSpaceInfo {
  movies: {
    id: number;
    title: string;
    year: number | null;
    size_bytes: number;
  }[];
  tv_shows: {
    id: number;
    title: string;
    size_bytes: number;
  }[];
}

export interface PlaybackStatus {
  position_ms: number;
  duration_ms: number;
  is_finished: boolean;
}

export interface PlaybackProgressPayload {
  media_id: number;
  media_type: string;
  position_ms: number;
  duration_ms: number;
  is_finished: boolean;
}

export function getImageUrl(path: any): string {
  if (!path || typeof path !== 'string') return 'https://images.unsplash.com/photo-1485846234645-a62644f84728?auto=format&fit=crop&q=80&w=1000';
  if (path.startsWith('http')) return path;
  
  if (IS_TAURI) {
    // In Tauri, we expect absolute paths to be returned by the backend for local files
    if (path.startsWith('/') || path.includes(':')) {
       return convertFileSrc(path);
    }
  }
  
  // If it starts with / and it's short, it's likely a TMDB path (e.g. /863920.jpg)
  if (path.startsWith('/') && path.length < 50 && !path.includes('/') && (path.endsWith('.jpg') || path.endsWith('.png'))) {
     return `https://image.tmdb.org/t/p/original${path}`;
  }

  // If it's a relative path (local artwork)
  if (!path.startsWith('/') && !path.startsWith('http')) {
    return `${API_BASE}/artwork/local?path=${encodeURIComponent(path)}`;
  }

  // Fallback for absolute paths on server
  if (path.startsWith('/') || path.includes(':')) {
    return `${API_BASE}/artwork/local?path=${encodeURIComponent(path)}`;
  }
  
  return `https://image.tmdb.org/t/p/original${path}`;
}

export async function request<T>(command: string, path: string, args: any = {}): Promise<T> {
  if (IS_TAURI) {
    try {
      // Map keys to camelCase for Tauri
      const tauriArgs: any = {};
      for (const key in args) {
        if (key === 'method') continue; // Don't pass 'method' to Tauri commands
        
        // Convert snake_case to camelCase
        const camelKey = key.replace(/_([a-z])/g, (g) => g[1].toUpperCase());
        tauriArgs[camelKey] = args[key];
      }
      
      console.log(`Invoking Tauri command: ${command}`, tauriArgs);
      return await invoke<T>(command, tauriArgs);
    } catch (err: any) {
      console.error(`Tauri command ${command} failed:`, err);
      // If Tauri returns a string, wrap it in a proper Error object
      throw new Error(typeof err === 'string' ? err : (err.message || 'Unknown error'));
    }
  } else {
    const { method: rawMethod, ...payload } = args;
    const method = rawMethod || (Object.keys(payload).length > 0 ? 'POST' : 'GET');
    const hasBody = method !== 'GET' && method !== 'HEAD';
    
    const response = await fetch(`${API_BASE}${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
      },
      body: hasBody ? JSON.stringify(payload) : undefined,
    });
    
    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`API Error (${response.status}): ${errorText}`);
    }
    
    const text = await response.text();
    return text ? JSON.parse(text) : null as unknown as T;
  }
}

export interface TaskUpdate {
  taskId: string;
  status: string;
  progress: number;
  total: number;
  message: string;
  startedAt?: number;
  finishedAt?: number;
  debugInfo?: string;
  filesNew?: number;      // Added
  filesHealed?: number;   // Added
  filesMissing?: number;  // Added
}

export const api = {
  getLibraries: () => request<Library[]>('get_libraries', '/libraries'),
  getTasks: () => request<TaskUpdate[]>('get_tasks', '/tasks'),
  createLibrary: (name: string, path: string, media_type: string) => 
    request<number>('create_library', '/libraries', { name, path, media_type }),
  deleteLibrary: (library_id: number) =>
    request<void>('delete_library', `/libraries/${library_id}`, { method: 'DELETE', id: library_id }),
  getMovieFiles: (movieId: number) =>
    request<MovieFile[]>('get_movie_files', `/movies/${movieId}/files`, { method: 'GET', movieId }),
  deleteMovie: (id: number) =>
    request<void>('delete_movie', `/movies/${id}`, { method: 'DELETE', id }),
  deleteMovieFile: (fileId: number) =>
    request<void>('delete_movie_file', `/movies/files/${fileId}`, { method: 'DELETE', fileId }),
  playMovieFile: (fileId: number) =>
    request<void>('play_movie_file', `/movies/files/${fileId}/play`, { method: 'POST', fileId }),
  deleteTvShow: (id: number) =>
    request<void>('delete_tv_show', `/tvshows/${id}`, { method: 'DELETE', id }),
  deleteEpisode: (id: number) =>
    request<void>('delete_episode', `/episodes/${id}`, { method: 'DELETE', id }),
  getMovies: (library_id?: number, genre?: string, language?: string) => {
    let q = [];
    if (library_id) q.push(`library_id=${library_id}`);
    if (genre) q.push(`genre=${encodeURIComponent(genre)}`);
    if (language) q.push(`language=${encodeURIComponent(language)}`);
    const qs = q.length > 0 ? `?${q.join('&')}` : '';
    return request<Movie[]>('get_movies', `/movies${qs}`, { method: 'GET', library_id, genre, language });
  },
  getTvShows: (library_id?: number, genre?: string, language?: string) => {
    let q = [];
    if (library_id) q.push(`library_id=${library_id}`);
    if (genre) q.push(`genre=${encodeURIComponent(genre)}`);
    if (language) q.push(`language=${encodeURIComponent(language)}`);
    const qs = q.length > 0 ? `?${q.join('&')}` : '';
    return request<TVShow[]>('get_tv_shows', `/tvshows${qs}`, { method: 'GET', library_id, genre, language });
  },
  getSeasons: (show_id: number) =>
    request<Season[]>('get_seasons', `/tvshows/${show_id}/seasons`, { method: 'GET', show_id }),
  getEpisodes: (season_id: number) =>
    request<Episode[]>('get_episodes', `/seasons/${season_id}/episodes`, { method: 'GET', season_id }),
  startScan: (library_id: number) => 
    request<string>('start_scan', `/libraries/${library_id}/scan`, { method: 'POST', library_id }),
  cleanupDuplicates: (library_id: number) =>
    request<string[]>('cleanup_duplicates', `/libraries/${library_id}/cleanup/duplicates`, { method: 'POST', id: library_id }),
  cleanupEmptyFolders: (library_id: number) =>
    request<string[]>('cleanup_empty_folders', `/libraries/${library_id}/cleanup/empty-folders`, { method: 'POST', id: library_id }),
  renameMovie: (movie_id: number) =>
    request<string>('rename_movie', `/movies/${movie_id}/rename`, { method: 'POST', id: movie_id }),
  playMovie: (movie_id: number) =>
    request<string>('play_movie', `/movies/${movie_id}/play`, { method: 'POST', id: movie_id }),
  playEpisode: (episode_id: number) =>
    request<string>('play_episode', `/episodes/${episode_id}/play`, { method: 'POST', id: episode_id }),
  scrapeBatch: (ids: number[], media_type: string) =>
    request<string>('scrape_batch', `/scrape/batch`, { method: 'POST', ids, media_type }),
  cleanupBatch: (ids: number[], media_type: string) =>
    request<string>('cleanup_batch', `/cleanup/batch`, { method: 'POST', ids, media_type }),
  downloadToLocal: (id: number, media_type: string, dest_path: string) => {
    if (IS_TAURI) {
      return request<string>('download_to_local', '', { id, media_type, dest_path });
    } else {
      throw new Error("Local download is only supported in the desktop app.");
    }
  },
  getGenres: () => request<string[]>('get_genres', '/genres', { method: 'GET' }),
  getLanguages: () => request<string[]>('get_languages', '/languages', { method: 'GET' }),
  refreshMetadata: (movie_id: number) =>
    request<string>('refresh_metadata', `/movies/${movie_id}/refresh`, { method: 'POST', id: movie_id }),

  processMovieAdvanced: (movie_id: number) =>
    request<void>('process_movie_advanced', `/movies/${movie_id}/process-advanced`, { method: 'POST', id: movie_id }),

  processTvShowAdvanced: (show_id: number) =>
    request<void>('process_tv_show_advanced', `/tvshows/${show_id}/process-advanced`, { method: 'POST', id: show_id }),

  processLibraryAdvanced: (library_id: number) =>
    request<void>('process_library_advanced', `/libraries/${library_id}/process-advanced`, { method: 'POST', id: library_id }),

  updateMovie: (id: number, data: Partial<Movie>) => {
    const { title, year, plot, rating, genres } = data;
    const genres_vec = typeof genres === 'string' ? genres.split(',').map(g => g.trim()) : genres;
    return request<void>('update_movie', `/movies/${id}`, { method: 'PUT', id, title, year, plot, rating, genres: genres_vec });
  },

  updateTvShow: (id: number, data: Partial<TVShow>) => {
    const { title, plot, rating, genres } = data;
    const genres_vec = typeof genres === 'string' ? genres.split(',').map(g => g.trim()) : genres;
    return request<void>('update_tv_show', `/tvshows/${id}`, { method: 'PUT', id, title, plot, rating, genres: genres_vec });
  },

  getSettings: () =>
    request<Record<string, string>>('get_settings', '/settings'),

  setSettings: (settings: Record<string, string>) => {
    if (IS_TAURI) {
      return request<void>('set_settings', '/settings', { method: 'POST', settings });
    } else {
      return request<void>('set_settings', '/settings', { method: 'POST', ...settings });
    }
  },

  createBackup: () =>
    request<string>('create_backup', '/maintenance/backup', { method: 'POST' }),

  checkUpdates: () =>
    request<UpdateCheckResult>('check_updates', '/system/update-check', { method: 'GET' }),

  getDiskSpace: () =>
    request<DiskSpaceInfo>('get_disk_space', '/system/disk-space', { method: 'GET' }),

  startStreaming: async (id: number, type: 'movie' | 'episode' | 'movie_file' = 'movie', protocol?: 'hls' | 'dash' | 'direct') => {
    const path = type === 'movie' 
      ? `/stream/movie/${id}${protocol ? `?protocol=${protocol}` : ''}` 
      : type === 'movie_file'
      ? `/stream/movie_file/${id}${protocol ? `?protocol=${protocol}` : ''}`
      : `/stream/episode/${id}${protocol ? `?protocol=${protocol}` : ''}`;
    const playlistUrl = await api.request<string>('start_streaming', path, { method: 'POST', id, media_type: type });
    
    // Add cache buster to prevent stale manifest loading
    const buster = playlistUrl.includes('?') ? `&t=${Date.now()}` : `?t=${Date.now()}`;
    const urlWithBuster = playlistUrl + buster;

    if (IS_TAURI && !playlistUrl.startsWith('http')) {
       // If it's an absolute path (from Tauri), convert it to asset protocol
       return convertFileSrc(playlistUrl) + buster;
    }
    // For server, return with API_BASE prefix if it's a relative path
    if (!playlistUrl.startsWith('http')) {
       // Safely join API_BASE and playlistUrl to prevent double '/api'
       const base = API_BASE.endsWith('/') ? API_BASE.slice(0, -1) : API_BASE;
       // If playlistUrl already includes /api prefix from the backend response, don't duplicate the /api path.
       // We'll strip the path from the base if it's there.
       let finalBase = base;
       if (playlistUrl.startsWith('/api') && base.endsWith('/api')) {
          finalBase = base.slice(0, -4);
       }
       
       const url = playlistUrl.startsWith('/') ? playlistUrl : `/${playlistUrl}`;
       return `${finalBase}${url}${buster}`;
    }
    return urlWithBuster;
  },

  generatePreview: async (id: number, type: 'movie' | 'episode' | 'movie_file' = 'movie') => {
    const path = type === 'movie' 
      ? `/stream/movie/${id}` 
      : type === 'movie_file'
      ? `/stream/movie_file/${id}`
      : `/stream/episode/${id}`;
    const playlistUrl = await request<string>('start_streaming', path, { method: 'POST', id, media_type: type });
    
    if (IS_TAURI && !playlistUrl.startsWith('http')) {
       return convertFileSrc(playlistUrl);
    }
    if (!playlistUrl.startsWith('http')) {
       const base = API_BASE.endsWith('/') ? API_BASE.slice(0, -1) : API_BASE;
       let finalBase = base;
       if (playlistUrl.startsWith('/api') && base.endsWith('/api')) {
          finalBase = base.slice(0, -4);
       }
       const url = playlistUrl.startsWith('/') ? playlistUrl : `/${playlistUrl}`;
       return `${finalBase}${url}`;
    }
    return playlistUrl;
  },

  searchSubtitles: (id: number) =>
    request<void>('search_subtitles', `/movies/${id}/subtitles/search`, { method: 'GET', id }),

  getSceneMarkers: (mediaId: number, mediaType: string) =>
    request<any[]>('get_scene_markers', `/media/${mediaType}/${mediaId}/markers`, { method: 'GET', mediaId, mediaType }),

  getSidecarSubtitles: (mediaId: number, mediaType: string) =>
    request<any[]>('get_sidecar_subtitles', `/media/${mediaType}/${mediaId}/subtitles`, { method: 'GET', mediaId, mediaType }),

  createSceneMarker: (mediaId: number, mediaType: string, seconds: number, title: string) =>
    request<any>('create_scene_marker', `/media/${mediaType}/${mediaId}/markers`, { method: 'POST', mediaId, mediaType, seconds, title }),

  deleteSceneMarker: (markerId: number) =>
    request<void>('delete_scene_marker', `/media/markers/${markerId}`, { method: 'DELETE', markerId }),

  getPlaybackStatus: (type: string, id: number) =>
    request<PlaybackStatus>('get_playback_status', `/playback/status/${type}/${id}`, { method: 'GET', id, mediaType: type }),
  updatePlaybackProgress: (data: PlaybackProgressPayload) =>
    request<any>('update_playback_progress', `/playback/heartbeat`, { 
      method: 'POST', 
      media_id: data.media_id,
      media_type: data.media_type,
      position_ms: data.position_ms,
      duration_ms: data.duration_ms,
      is_finished: data.is_finished
    }),

  exportCsv: () => window.open(`${API_BASE}/export/csv`),
  exportHtml: () => window.open(`${API_BASE}/export/html`),
  exportXlsx: () => window.open(`${API_BASE}/export/xlsx`),
  exportJson: () => window.open(`${API_BASE}/export/json`),
  syncTrakt: () => request<any>('sync_trakt', '/sync/trakt', { method: 'POST' }),
  request,
};
