// frontend/src/api/adapter.ts
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

export const IS_TAURI = (window as any).__TAURI_INTERNALS__ !== undefined;
export const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:7878/api';

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
    return await invoke<T>(command, args);
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
    return text ? JSON.parse(text) : null;
  }
}

export const api = {
  getLibraries: () => request<any[]>('get_libraries', '/libraries'),
  createLibrary: (name: string, path: string, mediaType: string) => 
    request<number>('create_library', '/libraries', { name, path, media_type: mediaType }),
  deleteLibrary: (libraryId: number) =>
    request<void>('delete_library', `/libraries/${libraryId}`, { method: 'DELETE' }),
  getMovies: (libraryId?: number, genre?: string, language?: string) => {
    let q = [];
    if (libraryId) q.push(`library_id=${libraryId}`);
    if (genre) q.push(`genre=${encodeURIComponent(genre)}`);
    if (language) q.push(`language=${encodeURIComponent(language)}`);
    const qs = q.length > 0 ? `?${q.join('&')}` : '';
    return request<any[]>('get_movies', `/movies${qs}`, { method: 'GET' });
  },
  getTvShows: (libraryId?: number, genre?: string, language?: string) => {
    let q = [];
    if (libraryId) q.push(`library_id=${libraryId}`);
    if (genre) q.push(`genre=${encodeURIComponent(genre)}`);
    if (language) q.push(`language=${encodeURIComponent(language)}`);
    const qs = q.length > 0 ? `?${q.join('&')}` : '';
    return request<any[]>('get_tvshows', `/tvshows${qs}`, { method: 'GET' });
  },
  getSeasons: (showId: number) =>
    request<any[]>('get_seasons', `/tvshows/${showId}/seasons`, { method: 'GET' }),
  getEpisodes: (seasonId: number) =>
    request<any[]>('get_episodes', `/seasons/${seasonId}/episodes`, { method: 'GET' }),
  startScan: (libraryId: number) => 
    request<string>('start_scan', `/libraries/${libraryId}/scan`, { method: 'POST' }),
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
    request<string>('scrape_batch', `/scrape/batch`, { method: 'POST', ids, media_type: mediaType }),
  cleanupBatch: (ids: number[], mediaType: string) =>
    request<string>('cleanup_batch', `/cleanup/batch`, { method: 'POST', ids, media_type: mediaType }),
  downloadToLocal: (id: number, mediaType: string, destPath: string) =>
    request<string>('download_to_local', '', { id, media_type: mediaType, dest_path: destPath }),
  getGenres: () => request<string[]>('get_genres', '/genres', { method: 'GET' }),
  getLanguages: () => request<string[]>('get_languages', '/languages', { method: 'GET' }),
  refreshMetadata: (movieId: number) =>
    request<string>('refresh_metadata', `/movies/${movieId}/refresh`, { method: 'POST' }),
  
  processMovieAdvanced: (movieId: number) =>
    request<void>('process_movie_advanced', `/movies/${movieId}/process-advanced`, { method: 'POST' }),

  processTvShowAdvanced: (showId: number) =>
    request<void>('process_tv_show_advanced', `/tvshows/${showId}/process-advanced`, { method: 'POST' }),
    
  processLibraryAdvanced: (libraryId: number) =>
    request<void>('process_library_advanced', `/libraries/${libraryId}/process-advanced`, { method: 'POST' }),

  updateMovie: (id: number, data: any) =>
    request<void>('update_movie', `/movies/${id}`, { method: 'PUT', ...data }),
    
  updateTvShow: (id: number, data: any) =>
    request<void>('update_tvshow', `/tvshows/${id}`, { method: 'PUT', ...data }),
    
  getSettings: () =>
    request<Record<string, string>>('get_settings', '/settings'),
    
  setSettings: (settings: Record<string, string>) =>
    request<void>('set_settings', '/settings', { method: 'POST', ...settings }),

  createBackup: () =>
    request<string>('create_backup', '/maintenance/backup', { method: 'POST' }),
    
  checkUpdates: () =>
    request<any>('check_updates', '/system/update-check', { method: 'GET' }),

  startStreaming: (id: number, type: 'movie' | 'episode' = 'movie') =>
    request<string>('start_streaming', `/stream/${id}/start?type=${type}`, { method: 'POST' }),

  searchSubtitles: (id: number) =>
    request<void>('search_subtitles', `/movies/${id}/subtitles/search`, { method: 'GET' }),

  getPlaybackStatus: (type: string, id: number) =>
    request<any>('get_playback_status', `/playback/status/${type}/${id}`, { method: 'GET' }),

  updatePlaybackProgress: (data: any) =>
    request<any>('update_playback_progress', `/playback/heartbeat`, { method: 'POST', ...data }),

  exportCsv: () => window.open(`${API_BASE}/export/csv`),
  exportHtml: () => window.open(`${API_BASE}/export/html`),
  request,
};
