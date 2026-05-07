// frontend/src/api/adapter.ts
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

export const IS_TAURI = (window as any).__TAURI_INTERNALS__ !== undefined || (window as any).__TAURI__ !== undefined || (window as any).__TAURI_IPC__ !== undefined;
export const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:7878/api';

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
  year?: number;
  tmdb_id?: number;
  imdb_id?: string;
  status: string;
  plot?: string;
  rating?: number;
  tagline?: string;
  runtime?: number;
  poster_url?: string;
  backdrop_url?: string;
  genres?: string;
  language?: string;
  cast_list?: string;
  created_at: string;
  file_path?: string;
  resolution?: string;
  video_codec?: string;
}

export interface TVShow {
  id: number;
  library_id: number;
  title: string;
  year?: number;
  tmdb_id?: number;
  imdb_id?: string;
  tvdb_id?: string;
  status: string;
  plot?: string;
  rating?: number;
  poster_url?: string;
  backdrop_url?: string;
  genres?: string;
  language?: string;
  cast_list?: string;
  created_at: string;
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
}

export interface UpdateCheckResult {
  latest_version: string;
  current_version: string;
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
    return convertFileSrc(path);
  }
  
  if (path.startsWith('/') || path.includes(':')) {
    return `${API_BASE}/artwork/local?path=${encodeURIComponent(path)}`;
  }
  return `https://image.tmdb.org/t/p/original${path}`;
}

export async function request<T>(command: string, path: string, args: any = {}): Promise<T> {
  if (IS_TAURI) {
    try {
      return await invoke<T>(command, args);
    } catch (err: any) {
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

export const api = {
  getLibraries: () => request<Library[]>('get_libraries', '/libraries'),
  createLibrary: (name: string, path: string, mediaType: string) => 
    request<number>('create_library', '/libraries', { name, path, mediaType }),
  deleteLibrary: (libraryId: number) =>
    request<void>('delete_library', `/libraries/${libraryId}`, { method: 'DELETE', id: libraryId }),
  getMovies: (libraryId?: number, genre?: string, language?: string) => {
    let q = [];
    if (libraryId) q.push(`library_id=${libraryId}`);
    if (genre) q.push(`genre=${encodeURIComponent(genre)}`);
    if (language) q.push(`language=${encodeURIComponent(language)}`);
    const qs = q.length > 0 ? `?${q.join('&')}` : '';
    return request<Movie[]>('get_movies', `/movies${qs}`, { method: 'GET', libraryId, genre, language });
  },
  getTvShows: (libraryId?: number, genre?: string, language?: string) => {
    let q = [];
    if (libraryId) q.push(`library_id=${libraryId}`);
    if (genre) q.push(`genre=${encodeURIComponent(genre)}`);
    if (language) q.push(`language=${encodeURIComponent(language)}`);
    const qs = q.length > 0 ? `?${q.join('&')}` : '';
    return request<TVShow[]>('get_tv_shows', `/tvshows${qs}`, { method: 'GET', libraryId, genre, language });
  },
  getSeasons: (showId: number) =>
    request<Season[]>('get_seasons', `/tvshows/${showId}/seasons`, { method: 'GET', showId }),
  getEpisodes: (seasonId: number) =>
    request<Episode[]>('get_episodes', `/seasons/${seasonId}/episodes`, { method: 'GET', seasonId }),
  startScan: (libraryId: number) => 
    request<string>('start_scan', `/libraries/${libraryId}/scan`, { method: 'POST', libraryId }),
  cleanupDuplicates: (libraryId: number) =>
    request<string[]>('cleanup_duplicates', `/libraries/${libraryId}/cleanup/duplicates`, { method: 'POST' }),
  cleanupEmptyFolders: (libraryId: number) =>
    request<string[]>('cleanup_empty_folders', `/libraries/${libraryId}/cleanup/empty-folders`, { method: 'POST' }),
  renameMovie: (movieId: number) =>
    request<string>('rename_movie', `/movies/${movieId}/rename`, { method: 'POST' }),
  playMovie: (movieId: number) =>
    request<string>('play_movie', `/movies/${movieId}/play`, { method: 'POST', id: movieId }),
  playEpisode: (episodeId: number) =>
    request<string>('play_episode', `/episodes/${episodeId}/play`, { method: 'POST', id: episodeId }),
  scrapeBatch: (ids: number[], mediaType: string) =>
    request<string>('scrape_batch', `/scrape/batch`, { method: 'POST', ids, mediaType }),
  cleanupBatch: (ids: number[], mediaType: string) =>
    request<string>('cleanup_batch', `/cleanup/batch`, { method: 'POST', ids, mediaType }),
  downloadToLocal: (id: number, mediaType: string, destPath: string) =>
    request<string>('download_to_local', '', { id, mediaType, destPath }),
  getGenres: () => request<string[]>('get_genres', '/genres', { method: 'GET' }),
  getLanguages: () => request<string[]>('get_languages', '/languages', { method: 'GET' }),
  refreshMetadata: (movieId: number) =>
    request<string>('refresh_metadata', `/movies/${movieId}/refresh`, { method: 'POST', id: movieId }),
  
  processMovieAdvanced: (movieId: number) =>
    request<void>('process_movie_advanced', `/movies/${movieId}/process-advanced`, { method: 'POST' }),

  processTvShowAdvanced: (showId: number) =>
    request<void>('process_tv_show_advanced', `/tvshows/${showId}/process-advanced`, { method: 'POST' }),
    
  processLibraryAdvanced: (libraryId: number) =>
    request<void>('process_library_advanced', `/libraries/${libraryId}/process-advanced`, { method: 'POST' }),

  updateMovie: (id: number, data: Partial<Movie>) =>
    request<void>('update_movie', `/movies/${id}`, { method: 'PUT', ...data }),
    
  updateTvShow: (id: number, data: Partial<TVShow>) =>
    request<void>('update_tvshow', `/tvshows/${id}`, { method: 'PUT', ...data }),
    
  getSettings: () =>
    request<Record<string, string>>('get_settings', '/settings'),
    
  setSettings: (settings: Record<string, string>) =>
    request<void>('set_settings', '/settings', { method: 'POST', ...settings }),

  createBackup: () =>
    request<string>('create_backup', '/maintenance/backup', { method: 'POST' }),
    
  checkUpdates: () =>
    request<UpdateCheckResult>('check_updates', '/system/update-check', { method: 'GET' }),

  startStreaming: (id: number, type: 'movie' | 'episode' = 'movie') =>
    request<string>('start_streaming', `/stream/${id}/start?type=${type}`, { method: 'POST' }),

  searchSubtitles: (id: number) =>
    request<void>('search_subtitles', `/movies/${id}/subtitles/search`, { method: 'GET' }),

  getPlaybackStatus: (type: string, id: number) =>
    request<PlaybackStatus>('get_playback_status', `/playback/status/${type}/${id}`, { method: 'GET' }),

  updatePlaybackProgress: (data: PlaybackProgressPayload) =>
    request<any>('update_playback_progress', `/playback/heartbeat`, { method: 'POST', ...data }),

  exportCsv: () => window.open(`${API_BASE}/export/csv`),
  exportHtml: () => window.open(`${API_BASE}/export/html`),
  exportXlsx: () => window.open(`${API_BASE}/export/xlsx`),
  syncTrakt: () => request<any>('sync_trakt', '/sync/trakt', { method: 'POST' }),
  request,
};
