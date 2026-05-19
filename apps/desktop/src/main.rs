// apps/desktop/src/main.rs
use std::sync::Arc;
use std::path::PathBuf;
use tauri::{State, Emitter, Manager};
use media_core::db::{self, Repositories, LibraryReader, LibraryWriter, MovieReader, MovieWriter, TvReader, TvWriter, MediaRepository, SettingsRepository};
use media_core::task_manager::{TaskManager, ProgressSink};
use media_core::scanner::service::{ScannerService, DefaultScannerService};
use media_core::scraper::service::{ScraperService, DefaultScraperService};
use media_core::models::{Library, Movie, MediaType, TVShow, Season, Episode, TaskUpdate, MovieId, TvShowId, LibraryId};
use media_core::cleanup::CleanupService;
use media_core::exporter::Exporter;
use sqlx::SqlitePool;

use tauri::path::BaseDirectory;

struct AppState {
    pool: SqlitePool,
    repos: Arc<Repositories>,
    task_manager: Arc<TaskManager>,
    scanner_service: Arc<dyn ScannerService>,
    scraper_service: Arc<dyn ScraperService>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tauri::command]
async fn get_libraries(state: State<'_, AppState>) -> Result<Vec<Library>, String> {
    state.repos.library.find_all().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_library(
    name: String,
    path: String,
    media_type: MediaType,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let id = state.repos.library.insert(&name, &path, media_type)
        .await
        .map_err(|e| e.to_string())?;
    
    // Auto-scan
    let scanner_service = state.scanner_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let library = Library { id, name, path, media_type, created_at: "".to_string() };
    
    tauri::async_runtime::spawn(async move {
        let _ = scanner_service.scan_library(&library, task_id).await;
    });

    Ok(id.0)
}

#[tauri::command]
async fn delete_library(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.repos.library.delete(media_core::models::LibraryId(id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_movies(
    library_id: Option<i64>, 
    genre: Option<String>, 
    language: Option<String>, 
    state: State<'_, AppState>
) -> Result<Vec<Movie>, String> {
    let mut movies = state.repos.movie.find_all(library_id.map(media_core::models::LibraryId), genre, language)
        .await.map_err(|e| e.to_string())?;
    
    let libraries = state.repos.library.find_all().await.unwrap_or_default();
    
    // Convert relative paths to absolute for Tauri
    for movie in &mut movies {
        if let Some(lib) = libraries.iter().find(|l| l.id == movie.library_id) {
            let lib_root = std::path::Path::new(&lib.path);
            if let Some(ref poster) = movie.poster_url {
                if !poster.starts_with("http") && !poster.starts_with('/') && !poster.contains(':') {
                    movie.poster_url = Some(lib_root.join(poster).to_string_lossy().to_string());
                }
            }
            if let Some(ref backdrop) = movie.backdrop_url {
                if !backdrop.starts_with("http") && !backdrop.starts_with('/') && !backdrop.contains(':') {
                    movie.backdrop_url = Some(lib_root.join(backdrop).to_string_lossy().to_string());
                }
            }
        }
    }
    
    Ok(movies)
}

#[tauri::command]
async fn get_tv_shows(
    library_id: Option<i64>, 
    genre: Option<String>, 
    language: Option<String>, 
    state: State<'_, AppState>
) -> Result<Vec<TVShow>, String> {
    let mut shows = state.repos.tv.find_all_shows(library_id.map(media_core::models::LibraryId), genre, language)
        .await.map_err(|e| e.to_string())?;
    
    let libraries = state.repos.library.find_all().await.unwrap_or_default();

    for show in &mut shows {
        if let Some(lib) = libraries.iter().find(|l| l.id == show.library_id) {
            let lib_root = std::path::Path::new(&lib.path);
            if let Some(ref poster) = show.poster_url {
                if !poster.starts_with("http") && !poster.starts_with('/') && !poster.contains(':') {
                    show.poster_url = Some(lib_root.join(poster).to_string_lossy().to_string());
                }
            }
            if let Some(ref backdrop) = show.backdrop_url {
                if !backdrop.starts_with("http") && !backdrop.starts_with('/') && !backdrop.contains(':') {
                    show.backdrop_url = Some(lib_root.join(backdrop).to_string_lossy().to_string());
                }
            }
        }
    }
    
    Ok(shows)
}

#[tauri::command]
async fn get_seasons(show_id: i64, state: State<'_, AppState>) -> Result<Vec<Season>, String> {
    state.repos.tv.find_seasons_by_show_id(media_core::models::TvShowId(show_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_episodes(season_id: i64, state: State<'_, AppState>) -> Result<Vec<Episode>, String> {
    state.repos.tv.find_episodes_by_season_id(media_core::models::SeasonId(season_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_genres(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.repos.media.get_unique_genres().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_languages(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.repos.media.get_unique_languages().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_scan(library_id: i64, state: State<'_, AppState>) -> Result<String, String> {
    let service = state.scanner_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    let libraries = state.repos.library.find_all().await.map_err(|e| e.to_string())?;
    if let Some(lib) = libraries.into_iter().find(|l| l.id == media_core::models::LibraryId(library_id)) {
        tauri::async_runtime::spawn(async move {
            let _ = service.scan_library(&lib, task_id).await;
        });
        Ok("Scan started".to_string())
    } else {
        Err("Library not found".to_string())
    }
}

#[tauri::command]
async fn scrape_batch(ids: Vec<i64>, media_type: String, state: State<'_, AppState>) -> Result<String, String> {
    let service = state.scraper_service.clone();
    
    tauri::async_runtime::spawn(async move {
        for id in ids {
            let task_id = uuid::Uuid::new_v4().to_string();
            if media_type == "movie" {
                let _ = service.scrape_movie(MovieId(id), task_id).await;
            } else {
                let _ = service.scrape_tv_show(TvShowId(id), task_id).await;
            }
        }
    });

    Ok("Batch scrape started".to_string())
}

#[tauri::command]
async fn cleanup_batch(ids: Vec<i64>, media_type: String, state: State<'_, AppState>) -> Result<String, String> {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        let total = ids.len() as i32;
        let mut processed = 0;

        if media_type == "movie" {
            let renamer = media_core::renamer::Renamer::new(None, None);
            for id in ids {
                processed += 1;
                if let Ok(Some(movie)) = repos.movie.find_by_id(media_core::models::MovieId(id)).await {
                    let libraries = repos.library.find_all().await.unwrap_or_default();
                    if let Some(lib) = libraries.into_iter().find(|l| l.id == movie.library_id) {
                        let lib_root = PathBuf::from(&lib.path);
                        let file_info = repos.movie.find_file_by_movie_id(movie.id).await.unwrap_or_default();
                        
                        if let Some(file) = file_info {
                            let old_path = PathBuf::from(&file.file_path);
                            let settings = repos.settings.get_all().await.unwrap_or_default();
                            let script_path = settings.get("post_processing_script").map(|s| s.as_str());

                            if let Ok(new_path) = renamer.rename_movie(&movie, &old_path, &lib_root, file.resolution, file.codec.as_deref(), script_path) {
                                let new_path_str = new_path.to_string_lossy().to_string();
                                let _ = repos.movie.update_file_path(file.id, &new_path_str).await;

                                let parent = new_path.parent().unwrap_or(&lib_root);
                                let cleanup = CleanupService::new(parent.to_path_buf());
                                let standard_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                let _ = cleanup.cleanup_metadata_for_movie(parent, standard_stem);
                                let _ = cleanup.remove_duplicate_artwork();
                            }
                        }
                    }
                    task_manager.broadcast(TaskUpdate {
                        task_id: task_id.clone(),
                        status: "running".to_string(),
                        progress: processed,
                        total,
                        message: format!("Cleaned: {}", movie.title),
                        started_at: Some(start_ms),
                        debug_info: None, 
                        finished_at: None,
                        ..Default::default()
                    });
                }
            }
        }
        task_manager.broadcast(TaskUpdate { 
            task_id, 
            status: "completed".to_string(), 
            progress: total, 
            total, 
            message: "Batch cleanup completed".to_string(), 
            started_at: Some(start_ms), 
            debug_info: None, 
            finished_at: Some(now_ms()),
            ..Default::default()
        });
    });

    Ok("Batch cleanup started".to_string())
}

#[tauri::command]
async fn update_movie(id: i64, title: String, year: Option<i32>, plot: Option<String>, rating: Option<f32>, genres: Option<Vec<String>>, state: State<'_, AppState>) -> Result<(), String> {
    let genres_json = genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    state.repos.movie.update(media_core::models::MovieId(id), &title, year, plot.as_deref(), rating, genres_json.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_tv_show(id: i64, title: String, plot: Option<String>, rating: Option<f32>, genres: Option<Vec<String>>, state: State<'_, AppState>) -> Result<(), String> {
    let genres_json = genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    state.repos.tv.update_show(media_core::models::TvShowId(id), &title, plot.as_deref(), rating, genres_json.as_deref(), None, None, None, None).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<std::collections::HashMap<String, String>, String> {
    state.repos.settings.get_all().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_settings(settings: std::collections::HashMap<String, String>, state: State<'_, AppState>) -> Result<(), String> {
    for (key, value) in settings {
        state.repos.settings.set(&key, &value).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn refresh_metadata(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let service = state.scraper_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tauri::async_runtime::spawn(async move {
        // Try as movie
        let res = service.scrape_movie(MovieId(id), task_id.clone()).await;
        if res.is_err() {
            // Try as TV show
            let _ = service.scrape_tv_show(TvShowId(id), task_id).await;
        }
    });

    Ok(())
}

#[tauri::command]
async fn play_movie(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(Some(path)) = state.repos.movie.get_full_path(media_core::models::MovieId(id)).await {
        opener::open(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn play_episode(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(Some(path)) = state.repos.tv.get_episode_full_path(media_core::models::EpisodeId(id)).await {
        opener::open(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn export_csv(state: State<'_, AppState>) -> Result<String, String> {
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    let tv_shows = state.repos.tv.find_all_shows(None, None, None).await.unwrap_or_default();
    Ok(Exporter::to_csv(&movies, &tv_shows))
}

#[tauri::command]
async fn export_html(state: State<'_, AppState>) -> Result<String, String> {
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    let tv_shows = state.repos.tv.find_all_shows(None, None, None).await.unwrap_or_default();
    Ok(Exporter::to_html(&movies, &tv_shows))
}

#[tauri::command]
async fn export_json(state: State<'_, AppState>) -> Result<String, String> {
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    let tv_shows = state.repos.tv.find_all_shows(None, None, None).await.unwrap_or_default();
    Exporter::to_json(&movies, &tv_shows).map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_updates() -> Result<serde_json::Value, String> {
    media_core::maintenance::MaintenanceEngine::check_for_updates()
        .await
        .map(|v| serde_json::json!({ "latest_version": v, "current_version": "0.1.0" }))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_backup(state: State<'_, AppState>) -> Result<String, String> {
    let backup_dir = std::path::Path::new("backups");
    let _ = media_core::maintenance::MaintenanceEngine::export_all_nfos(&state.repos).await;
    media_core::maintenance::MaintenanceEngine::create_backup(&state.pool, backup_dir).await
        .map(|p| format!("{:?}", p))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn bulk_scrape(id: i64, state: State<'_, AppState>) -> Result<String, String> {
    let service = state.scraper_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    tauri::async_runtime::spawn(async move {
        let _ = service.bulk_scrape_library(LibraryId(id), task_id).await;
    });
    
    Ok("Bulk scrape started".to_string())
}

#[tauri::command]
async fn rename_movie(id: i64, state: State<'_, AppState>) -> Result<String, String> {
    let repos = state.repos.clone();
    match repos.movie.find_by_id(media_core::models::MovieId(id)).await {
        Ok(Some(movie)) => {
            let movie_id = movie.id;
            let libraries = repos.library.find_all().await.unwrap_or_default();
            if let Some(lib) = libraries.into_iter().find(|l| l.id == movie.library_id) {
                let file_info = repos.movie.find_file_by_movie_id(movie_id).await.unwrap_or_default();
                if let Some(file) = file_info {
                    let repos_clone = repos.clone();
                    let lib_path = lib.path.clone();
                    let old_path_str = file.file_path.clone();
                    let renamer = media_core::renamer::Renamer::new(None, None);
                    let old_path = std::path::PathBuf::from(&old_path_str);
                    let lib_root = std::path::PathBuf::from(&lib_path);
                    let settings = repos_clone.settings.get_all().await.unwrap_or_default();
                    let script_path = settings.get("post_processing_script").cloned();

                    match renamer.rename_movie(&movie, &old_path, &lib_root, file.resolution, file.codec.as_deref(), script_path.as_deref()) {
                        Ok(new_path) => {
                            let new_path_str = new_path.to_string_lossy().to_string();
                            let _ = repos_clone.movie.update_file_path(file.id, &new_path_str).await;
                            return Ok("Movie renamed successfully".to_string());
                        }
                        Err(e) => return Err(e.to_string())
                    }
                }
            }
            Err("Library or file not found".to_string())
        }
        _ => Err("Movie not found".to_string())
    }
}

#[tauri::command]
async fn search_subtitles(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    // Check environment first, then database settings
    let mut api_key = std::env::var("OPENSUBTITLES_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        if let Ok(settings) = repos.settings.get_all().await {
            if let Some(key) = settings.get("opensubtitles_api_key") {
                api_key = key.clone();
            }
        }
    }

    if api_key.is_empty() { 
        return Err("OpenSubtitles API Key missing. Please add it in Settings (opensubtitles_api_key).".to_string()); 
    }

    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        if let Ok(Some(movie)) = repos.movie.find_by_id(media_core::models::MovieId(id)).await {
            let file_info = repos.movie.find_file_by_movie_id(movie.id).await.unwrap_or_default();

            if let Some(file) = file_info {
                let dest_path = std::path::PathBuf::from(file.file_path);
                let client = media_core::subtitles::SubtitleClient::new(api_key);
                let mut results = None;
                if let Ok(hash) = media_core::subtitles::compute_opensubtitles_hash(&dest_path) {
                    if let Ok(hash_results) = client.search_by_hash(&hash, "en").await {
                        if !hash_results.is_empty() { results = Some(hash_results); }
                    }
                }
                if results.is_none() {
                    if let Some(imdb_id) = movie.imdb_id {
                        if let Ok(imdb_results) = client.search(&imdb_id, "en").await {
                            if !imdb_results.is_empty() { results = Some(imdb_results); }
                        }
                    }
                }
                if let Some(res) = results {
                    if let Some(best) = res.first() {
                        if let Some(file_id) = best.attributes.files.first().map(|f| f.file_id) {
                            let _ = client.download(file_id, &dest_path, "en").await;
                        }
                    }
                }
            }
        }
        let _ = task_manager.broadcast(TaskUpdate { 
            task_id, 
            status: "completed".to_string(), 
            progress: 1, 
            total: 1, 
            message: "Subtitle search finished".to_string(), 
            started_at: Some(start_ms), 
            debug_info: None, 
            finished_at: Some(now_ms()),
            ..Default::default()
        });
    });
    Ok(())
}

#[tauri::command]
async fn process_movie_advanced(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        if let Ok(Some(movie)) = repos.movie.find_by_id(media_core::models::MovieId(id)).await {
            let file_info = repos.movie.find_file_by_movie_id(movie.id).await.unwrap_or_default();
            if let Some(file) = file_info {
                task_manager.broadcast(TaskUpdate {
                    task_id: task_id.clone(),
                    status: "running".to_string(),
                    progress: 0,
                    total: 1,
                    message: format!("Analyzing: {}", movie.title),
                    started_at: Some(start_ms),
                    debug_info: None, finished_at: None, ..Default::default() });
                if let Ok(Some(input_path)) = repos.movie.get_full_path(movie.id).await {
                    if input_path.exists() {
                        let fingerprint = match file.fingerprint {
                            Some(f) if !f.is_empty() => f,
                            _ => {
                                let f = media_core::scanner::hash::calculate_oshash(&input_path).unwrap_or_default();
                                let _ = repos.movie.update_file_fingerprint(file.id, &f).await;
                                f
                            }
                        };

                        if !fingerprint.is_empty() {
                            // Refresh technical metadata
                            if let Ok(details) = media_core::scanner::mediainfo::get_media_info(&input_path) {
                                let res = Resolution::from_dimensions(details.width, details.height);
                                let _ = repos.movie.upsert_file(
                                    movie.id, 
                                    &file.file_path, 
                                    &file.original_name, 
                                    file.size_bytes, 
                                    file.mtime, 
                                    Some(res), 
                                    Some(&details.video_codec), 
                                    Some(&details.audio_codec), 
                                    Some(details.duration_secs), 
                                    file.hash.as_deref(), 
                                    Some(&fingerprint)
                                ).await;
                            }

                            let duration = file.duration_secs.unwrap_or(0) as f64;
                            let generated_root = std::path::Path::new("data/generated");
                            if let Ok(_) = media_core::scanner::ffmpeg::FfmpegEngine::generate_advanced_assets(&input_path, &fingerprint, generated_root, duration) {
                                let _ = repos.media.upsert_generated_asset(&fingerprint, "thumb", &format!("{}/thumb.jpg", fingerprint)).await;
                                let _ = repos.media.upsert_generated_asset(&fingerprint, "sprite", &format!("{}/sprite.webp", fingerprint)).await;
                                let _ = repos.media.upsert_generated_asset(&fingerprint, "vtt", &format!("{}/sprite.vtt", fingerprint)).await;
                                let _ = repos.media.upsert_generated_asset(&fingerprint, "preview", &format!("{}/preview.mp4", fingerprint)).await;
                            }
                        }
                    }
                }
            }
        }
        let _ = task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "Advanced analysis complete".to_string(), started_at: Some(start_ms), debug_info: None, finished_at: Some(now_ms()), ..Default::default() });
    });
    Ok(())
}

#[tauri::command]
async fn process_tv_show_advanced(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        if let Ok(Some(show)) = repos.tv.find_show_by_id(media_core::models::TvShowId(id)).await {
            let seasons = repos.tv.find_seasons_by_show_id(show.id).await.unwrap_or_default();
            for s in seasons {
                let eps = repos.tv.find_episodes_by_season_id(s.id).await.unwrap_or_default();
                for ep in eps {
                    if let Ok(Some(input_path)) = repos.tv.get_episode_full_path(ep.id).await {
                        if input_path.exists() {
                            // Refresh technical metadata
                            if let Ok(details) = media_core::scanner::mediainfo::get_media_info(&input_path) {
                                let res = Resolution::from_dimensions(details.width, details.height);
                                let _ = repos.tv.upsert_episode(
                                    ep.season_id, 
                                    ep.episode_number, 
                                    &ep.file_path, 
                                    &ep.original_name, 
                                    ep.size_bytes as i64, 
                                    ep.mtime, 
                                    Some(res), 
                                    Some(&details.video_codec), 
                                    Some(&details.audio_codec), 
                                    Some(details.duration_secs), 
                                    ep.hash.as_deref(), 
                                    ep.fingerprint.as_deref()
                                ).await;
                            }

                            let fingerprint = match ep.fingerprint {
                                Some(f) if !f.is_empty() => f,
                                _ => {
                                    let f = media_core::scanner::hash::calculate_oshash(&input_path).unwrap_or_default();
                                    let _ = repos.tv.update_episode_fingerprint(ep.id, &f).await;
                                    f
                                }
                            };

                            if !fingerprint.is_empty() {
                                let duration = ep.duration_secs.unwrap_or(0) as f64;
                                let generated_root = std::path::Path::new("data/generated");
                                if let Ok(_) = media_core::scanner::ffmpeg::FfmpegEngine::generate_advanced_assets(&input_path, &fingerprint, generated_root, duration) {
                                    let _ = repos.media.upsert_generated_asset(&fingerprint, "thumb", &format!("{}/thumb.jpg", fingerprint)).await;
                                    let _ = repos.media.upsert_generated_asset(&fingerprint, "sprite", &format!("{}/sprite.webp", fingerprint)).await;
                                    let _ = repos.media.upsert_generated_asset(&fingerprint, "vtt", &format!("{}/sprite.vtt", fingerprint)).await;
                                    let _ = repos.media.upsert_generated_asset(&fingerprint, "preview", &format!("{}/preview.mp4", fingerprint)).await;
                                }
                            }
                        }
                    }
                }
            }
        }
        let _ = task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "TV analysis complete".to_string(), started_at: Some(start_ms), debug_info: None, finished_at: Some(now_ms()), ..Default::default() });
    });
    Ok(())
}

#[tauri::command]
async fn process_library_advanced(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        if let Ok(movies) = repos.movie.find_all(Some(media_core::models::LibraryId(id)), None, None).await {
            for movie in movies {
                let file_info = repos.movie.find_file_by_movie_id(movie.id).await.unwrap_or_default();
                if let Some(file) = file_info {
                    if let Ok(Some(input_path)) = repos.movie.get_full_path(movie.id).await {
                        if input_path.exists() {
                            let fingerprint = match file.fingerprint {
                                Some(f) if !f.is_empty() => f,
                                _ => {
                                    let f = media_core::scanner::hash::calculate_oshash(&input_path).unwrap_or_default();
                                    let _ = repos.movie.update_file_fingerprint(file.id, &f).await;
                                    f
                                }
                            };

                            if !fingerprint.is_empty() {
                                let duration = file.duration_secs.unwrap_or(0) as f64;
                                let generated_root = std::path::Path::new("data/generated");
                                if let Ok(_) = media_core::scanner::ffmpeg::FfmpegEngine::generate_advanced_assets(&input_path, &fingerprint, generated_root, duration) {
                                    let _ = repos.media.upsert_generated_asset(&fingerprint, "thumb", &format!("{}/thumb.jpg", fingerprint)).await;
                                    let _ = repos.media.upsert_generated_asset(&fingerprint, "sprite", &format!("{}/sprite.webp", fingerprint)).await;
                                    let _ = repos.media.upsert_generated_asset(&fingerprint, "vtt", &format!("{}/sprite.vtt", fingerprint)).await;
                                    let _ = repos.media.upsert_generated_asset(&fingerprint, "preview", &format!("{}/preview.mp4", fingerprint)).await;
                                }
                            }
                        }
                    }
                }
            }
        }
        let _ = task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "Library analysis complete".to_string(), started_at: Some(start_ms), debug_info: None, finished_at: Some(now_ms()), ..Default::default() });
    });
    Ok(())
}

#[tauri::command]
async fn sync_trakt(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let settings_map = state.repos.settings.get_all().await.unwrap_or_default();
    let access_token = settings_map.get("trakt_access_token").cloned().unwrap_or_default();
    if access_token.is_empty() { return Err("Trakt not authenticated".to_string()); }
    let scraper_clients = media_core::scraper::ScraperClients::from_settings(&state.repos).await;
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    let mut trakt_movies = Vec::new();
    for m in movies {
        if let Some(tmdb) = m.tmdb_id {
            trakt_movies.push(serde_json::json!({ "ids": { "tmdb": tmdb, "imdb": m.imdb_id } }));
        }
    }
    if !trakt_movies.is_empty() {
        scraper_clients.trakt.add_to_collection(&access_token, trakt_movies).await.map_err(|e| e.to_string())
    } else {
        Ok(serde_json::json!({"added": 0}))
    }
}

#[tauri::command]
async fn cleanup_duplicates(id: i64, state: State<'_, AppState>) -> Result<Vec<PathBuf>, String> {
    let libraries = state.repos.library.find_all().await.map_err(|e| e.to_string())?;
    if let Some(lib) = libraries.into_iter().find(|l| l.id == media_core::models::LibraryId(id)) {
        let cleanup = CleanupService::new(PathBuf::from(lib.path));
        cleanup.remove_duplicate_artwork().map_err(|e| e.to_string())
    } else {
        Err("Library not found".to_string())
    }
}

#[tauri::command]
async fn cleanup_empty_folders(id: i64, state: State<'_, AppState>) -> Result<Vec<PathBuf>, String> {
    let libraries = state.repos.library.find_all().await.map_err(|e| e.to_string())?;
    if let Some(lib) = libraries.into_iter().find(|l| l.id == media_core::models::LibraryId(id)) {
        let cleanup = CleanupService::new(PathBuf::from(lib.path));
        cleanup.remove_empty_folders().map_err(|e| e.to_string())
    } else {
        Err("Library not found".to_string())
    }
}

#[tauri::command]
async fn start_streaming(id: i64, media_type: String, state: State<'_, AppState>, app_handle: tauri::AppHandle) -> Result<String, String> {
    let path = if media_type == "movie" {
        state.repos.movie.get_full_path(media_core::models::MovieId(id)).await
    } else {
        state.repos.tv.get_episode_full_path(media_core::models::EpisodeId(id)).await
    }.map_err(|e| e.to_string())?;

    if let Some(input_path) = path {
        let cache_dir = app_handle.path().app_cache_dir().unwrap_or_else(|_| std::env::current_dir().unwrap());
        let output_dir = cache_dir.join("transcodes").join(id.to_string());
        
        // Clean up previous transcodes for this ID
        if output_dir.exists() {
            let _ = std::fs::remove_dir_all(&output_dir);
        }
        std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

        media_core::scanner::ffmpeg::FfmpegEngine::create_hls_stream(&input_path, &output_dir)
            .map_err(|e| e.to_string())?;

        let playlist = output_dir.join("playlist.m3u8");
        
        // Wait for playlist to be created and non-empty
        let mut attempts = 0;
        while attempts < 30 { // Wait up to 15 seconds
            if playlist.exists() {
                if let Ok(meta) = std::fs::metadata(&playlist) {
                    if meta.len() > 0 {
                        // Small extra delay to ensure the first segments are written
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            attempts += 1;
        }

        if !playlist.exists() {
            return Err("Streaming failed to start: playlist not created".to_string());
        }

        Ok(playlist.to_string_lossy().to_string())
    } else {
        Err("Media not found".to_string())
    }
}

async fn copy_with_progress(
    src: &std::path::Path,
    dest: &std::path::Path,
    task_manager: Arc<TaskManager>,
    task_id: String,
    start_ms: u64
) -> std::io::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    let mut reader = tokio::fs::File::open(src).await?;
    let total_size = reader.metadata().await?.len();
    let mut writer = tokio::fs::File::create(dest).await?;
    
    let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer
    let mut copied_size = 0u64;
    let mut last_report = std::time::Instant::now();

    loop {
        let n = reader.read(&mut buffer).await?;
        if n == 0 { break; }
        writer.write_all(&buffer[..n]).await?;
        copied_size += n as u64;
        
        // Report progress every 500ms
        if last_report.elapsed() > std::time::Duration::from_millis(500) {
            let progress = (copied_size as f64 / total_size as f64 * 100.0) as i32;
            task_manager.broadcast(TaskUpdate {
                task_id: task_id.clone(),
                status: "running".to_string(),
                progress,
                total: 100,
                message: format!("Downloading: {} ({}%)", src.file_name().unwrap_or_default().to_string_lossy(), progress),
                started_at: Some(start_ms),
                debug_info: None, finished_at: None, ..Default::default() });
            last_report = std::time::Instant::now();
        }
    }
    
    writer.flush().await?;
    Ok(())
}

#[tauri::command]
async fn download_to_local(id: i64, media_type: String, dest_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    let path = if media_type == "movie" {
        repos.movie.get_full_path(media_core::models::MovieId(id)).await
    } else {
        repos.tv.get_episode_full_path(media_core::models::EpisodeId(id)).await
    }.map_err(|e| e.to_string())?;

    if let Some(src) = path {
        if !src.exists() {
            return Err(format!("Source file not found: {:?}", src));
        }

        let mut dest = std::path::PathBuf::from(&dest_path);

        // If dest is a directory, or ends with a slash, or has no extension, treat it as a directory
        let is_dir = dest.is_dir() || dest_path.ends_with('\\') || dest_path.ends_with('/') || dest.extension().is_none();
        
        if is_dir {
            if !dest.exists() {
                std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
            }
            if let Some(filename) = src.file_name() {
                dest.push(filename);
            }
        } else {
            // Ensure parent directory exists for the file path
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }

        let task_id_clone = task_id.clone();
        let task_manager_clone = task_manager.clone();
        let src_clone = src.clone();
        let dest_clone = dest.clone();
        
        tauri::async_runtime::spawn(async move {
            let start_ms = now_ms();
            task_manager_clone.broadcast(TaskUpdate {
                task_id: task_id_clone.clone(),
                status: "running".to_string(),
                progress: 0,
                total: 100,
                message: format!("Downloading: {}", src_clone.file_name().unwrap_or_default().to_string_lossy()),
                started_at: Some(start_ms),
                debug_info: None, finished_at: None, ..Default::default() });

            match copy_with_progress(&src_clone, &dest_clone, task_manager_clone.clone(), task_id_clone.clone(), start_ms).await {
                Ok(_) => {
                    task_manager_clone.broadcast(TaskUpdate {
                        task_id: task_id_clone,
                        status: "completed".to_string(),
                        progress: 100,
                        total: 100,
                        message: format!("Download completed: {}", src_clone.file_name().unwrap_or_default().to_string_lossy()),
                        started_at: Some(start_ms),
                        debug_info: None, finished_at: Some(now_ms()), ..Default::default() });
                }
                Err(e) => {
                    task_manager_clone.broadcast(TaskUpdate {
                        task_id: task_id_clone,
                        status: "error".to_string(),
                        progress: 0,
                        total: 100,
                        message: format!("Download failed: {}", e),
                        started_at: Some(start_ms),
                        debug_info: None, finished_at: Some(now_ms()), ..Default::default() });
                }
            }
        });

        Ok(format!("Download started to: {}", dest.display()))
    } else {
        Err("Media file not found in database".to_string())
    }
}

#[tauri::command]
async fn get_playback_status(id: i64, media_type: String, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    match state.repos.media.get_playback_status(id, &media_type).await {
        Ok(Some(status)) => Ok(serde_json::json!({
            "position_ms": status.position_ms,
            "duration_ms": status.duration_ms,
            "is_finished": status.is_finished
        })),
        _ => Ok(serde_json::json!({ "position_ms": 0, "duration_ms": 0, "is_finished": false }))
    }
}

#[tauri::command]
async fn update_playback_progress(
    media_id: i64,
    media_type: String,
    position_ms: i32,
    duration_ms: i32,
    is_finished: bool,
    state: State<'_, AppState>
) -> Result<(), String> {
    state.repos.media.update_playback_status(media_id, &media_type, position_ms, duration_ms, is_finished)
        .await
        .map_err(|e| e.to_string())
}

fn main() {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        let db_dir = std::path::PathBuf::from(app_data).join("MediaManager");
        if !db_dir.exists() {
            std::fs::create_dir_all(&db_dir).expect("CRITICAL: Failed to create AppData directory for database.");
        }
        let db_path = db_dir.join("mediavault.db");
        format!("sqlite:{}?mode=rwc", db_path.to_string_lossy())
    });
    let pool = tauri::async_runtime::block_on(async {
        db::init_pool(&database_url).await.expect("Failed to initialize database pool")
    });
    let repos = Arc::new(Repositories::new(pool.clone()));
    let task_manager = Arc::new(TaskManager::new());
    
    let clients = tauri::async_runtime::block_on(async {
        Arc::new(media_core::scraper::ScraperClients::from_settings(&repos).await)
    });
    let scraper_service = Arc::new(DefaultScraperService::new(
        repos.clone(),
        task_manager.clone(),
        clients,
    ));
    let scanner_service = Arc::new(DefaultScannerService::new(
        repos.clone(),
        task_manager.clone(),
    ));

    let task_manager_clone = task_manager.clone();
    let repos_for_watchdog = repos.clone();
    let scanner_service_for_watchdog = scanner_service.clone();
    let repos_for_notifications = repos.clone();
    let task_manager_for_notifications = task_manager.clone();
    
    tauri::Builder::default()
        .manage(AppState { 
            pool: pool.clone(), 
            repos: repos.clone(), 
            task_manager,
            scanner_service,
            scraper_service,
        })
        .invoke_handler(tauri::generate_handler![
            get_libraries, create_library, delete_library,
            get_movies, get_tv_shows, get_seasons, get_episodes,
            get_genres, get_languages, start_scan, scrape_batch,
            cleanup_batch, update_movie, update_tv_show,
            get_settings, set_settings, refresh_metadata,
            play_movie, play_episode, export_csv, export_html, export_json,
            check_updates, create_backup, bulk_scrape,
            rename_movie, search_subtitles, process_movie_advanced,
            process_tv_show_advanced, process_library_advanced, sync_trakt,
            cleanup_duplicates, cleanup_empty_folders, start_streaming,
            download_to_local, get_playback_status, update_playback_progress
        ])
        .setup(|app| {
            // Resolve sidecar paths
            let handle = app.handle();
            
            #[cfg(target_os = "windows")]
            let (ffmpeg_name, ffprobe_name) = ("ffmpeg.exe", "ffprobe.exe");
            #[cfg(not(target_os = "windows"))]
            let (ffmpeg_name, ffprobe_name) = ("ffmpeg", "ffprobe");

            if let Ok(ffmpeg_path) = handle.path().resolve(format!("bin/{}", ffmpeg_name), BaseDirectory::Resource) {
                if ffmpeg_path.exists() {
                    tracing::info!("Found bundled FFmpeg at: {:?}", ffmpeg_path);
                    media_core::config::set_ffmpeg_path(ffmpeg_path.to_string_lossy().to_string());
                }
            }

            if let Ok(ffprobe_path) = handle.path().resolve(format!("bin/{}", ffprobe_name), BaseDirectory::Resource) {
                if ffprobe_path.exists() {
                    tracing::info!("Found bundled ffprobe at: {:?}", ffprobe_path);
                    media_core::config::set_ffprobe_path(ffprobe_path.to_string_lossy().to_string());
                }
            }

            tauri::async_runtime::spawn(async move {
                let mut rx = task_manager_for_notifications.subscribe();
                let notifier = media_core::notifications::Notifier::new();
                while let Ok(update) = rx.recv().await {
                    if update.status == "completed" || update.status == "error" {
                        if let Ok(settings) = repos_for_notifications.settings.get_all().await {
                            if let Some(url) = settings.get("discord_webhook_url") {
                                if !url.is_empty() {
                                    let _ = notifier.send_discord_webhook(url, &update).await;
                                }
                            }
                        }
                    }
                }
            });
            tauri::async_runtime::spawn(async move {
                let watchdog = media_core::scanner::watchdog::Watchdog::new(repos_for_watchdog, scanner_service_for_watchdog);
                let _ = watchdog.start().await;
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = task_manager_clone.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(update) => { let _ = handle.emit("task-update", update); }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
