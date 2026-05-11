// apps/desktop/src/main.rs
use std::sync::Arc;
use std::path::PathBuf;
use tauri::{State, Emitter, Manager};
use media_core::db;
use media_core::task_manager::TaskManager;
use media_core::models::{Library, Movie, MediaType, TVShow, Season, Episode, TaskUpdate};
use media_core::cleanup::CleanupService;
use media_core::exporter::Exporter;
use sqlx::SqlitePool;

use tauri::path::BaseDirectory;

struct AppState {
    pool: SqlitePool,
    task_manager: Arc<TaskManager>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tauri::command]
async fn get_libraries(state: State<'_, AppState>) -> Result<Vec<Library>, String> {
    db::queries::get_all_libraries(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_library(
    name: String,
    path: String,
    media_type: MediaType,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let id = db::queries::insert_library(&state.pool, &name, &path, media_type)
        .await
        .map_err(|e| e.to_string())?;
    
    // Auto-scan
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let library = Library { id, name, path, media_type, created_at: "".to_string() };
    
    tauri::async_runtime::spawn(async move {
        let _ = media_core::scanner::worker::scan_library(&pool, &library, task_id, &task_manager).await;
    });

    Ok(id.into())
}

#[tauri::command]
async fn delete_library(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    db::queries::delete_library(&state.pool, media_core::models::LibraryId(id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_movies(
    library_id: Option<i64>, 
    genre: Option<String>, 
    language: Option<String>, 
    state: State<'_, AppState>
) -> Result<Vec<Movie>, String> {
    let mut movies = db::queries::get_all_movies(&state.pool, library_id.map(media_core::models::LibraryId), genre, language)
        .await.map_err(|e| e.to_string())?;
    
    let libraries = db::queries::get_all_libraries(&state.pool).await.unwrap_or_default();
    
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
    let mut shows = db::queries::get_all_tv_shows(&state.pool, library_id.map(media_core::models::LibraryId), genre, language)
        .await.map_err(|e| e.to_string())?;
    
    let libraries = db::queries::get_all_libraries(&state.pool).await.unwrap_or_default();

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
    db::queries::get_seasons_by_show_id(&state.pool, media_core::models::TvShowId(show_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_episodes(season_id: i64, state: State<'_, AppState>) -> Result<Vec<Episode>, String> {
    db::queries::get_episodes_by_season_id(&state.pool, media_core::models::SeasonId(season_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_genres(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    db::queries::get_unique_genres(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_languages(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    db::queries::get_unique_languages(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_scan(library_id: i64, state: State<'_, AppState>) -> Result<String, String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    let libraries = db::queries::get_all_libraries(&pool).await.map_err(|e| e.to_string())?;
    if let Some(lib) = libraries.into_iter().find(|l| l.id == media_core::models::LibraryId(library_id)) {
        tauri::async_runtime::spawn(async move {
            let _ = media_core::scanner::worker::scan_library(&pool, &lib, task_id, &task_manager).await;
        });
        Ok("Scan started".to_string())
    } else {
        Err("Library not found".to_string())
    }
}

#[tauri::command]
async fn scrape_batch(ids: Vec<i64>, media_type: String, state: State<'_, AppState>) -> Result<String, String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        let clients = std::sync::Arc::new(media_core::scraper::ScraperClients::from_settings(&pool).await);
        
        let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
        let script_path = settings.get("post_processing_script").cloned();

        let mut all_tasks = Vec::new();
        if media_type == "movie" {
            if let Ok(movies) = db::queries::get_movies_by_ids(&pool, &ids.iter().map(|&x| media_core::models::MovieId(x)).collect::<Vec<_>>()).await {
                all_tasks.extend(movies.into_iter().map(|m| (m.id.0, m.title, m.year, "movie")));
            }
        } else {
            if let Ok(shows) = db::queries::get_tv_shows_by_ids(&pool, &ids.iter().map(|&x| media_core::models::TvShowId(x)).collect::<Vec<_>>()).await {
                all_tasks.extend(shows.into_iter().map(|s| (s.id.0, s.title, None, "tv")));
            }
        }

        let total = all_tasks.len() as i32;
        let pool = Arc::new(pool);
        let task_manager_clone = task_manager.clone();
        let task_id_clone = task_id.clone();

        use futures::StreamExt;
        let stream = futures::stream::iter(all_tasks.into_iter().enumerate());
        
        stream.for_each_concurrent(5, |(i, (id, title, year, m_type))| {
            let clients = clients.clone();
            let pool = pool.clone();
            let task_manager = task_manager_clone.clone();
            let task_id = task_id_clone.clone();
            let title_clone = title.clone();
            let script_path_clone = script_path.clone();
            
            async move {
                if m_type == "movie" {
                    let _ = media_core::scraper::scrape_movie(media_core::models::MovieId(id), &title_clone, year, &clients, &pool, script_path_clone.as_deref()).await;
                } else {
                    let _ = media_core::scraper::scrape_tv_show(media_core::models::TvShowId(id), &title_clone, &clients, &pool, script_path_clone.as_deref()).await;
                }
                
                task_manager.broadcast(TaskUpdate {
                    task_id,
                    status: "running".to_string(),
                    progress: (i + 1) as i32,
                    total,
                    message: format!("Processed: {}", title_clone),
                    started_at: Some(start_ms),
                    debug_info: None,
                });
            }
        }).await;

        task_manager.broadcast(TaskUpdate {
            task_id,
            status: "completed".to_string(),
            progress: total,
            total,
            message: "Batch scrape completed".to_string(),
            started_at: Some(start_ms),
            debug_info: None,
        });
    });

    Ok("Batch scrape started".to_string())
}

#[tauri::command]
async fn cleanup_batch(ids: Vec<i64>, media_type: String, state: State<'_, AppState>) -> Result<String, String> {
    let pool = state.pool.clone();
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
                if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, media_core::models::MovieId(id)).await {
                    let libraries = db::queries::get_all_libraries(&pool).await.unwrap_or_default();
                    if let Some(lib) = libraries.into_iter().find(|l| l.id == movie.library_id) {
                        let lib_root = PathBuf::from(&lib.path);
                        let file_info: Option<media_core::models::MovieFile> = sqlx::query_as("SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1")
                            .bind(movie.id).fetch_optional(&pool).await.unwrap_or_default();
                        
                        if let Some(file) = file_info {
                            let old_path = PathBuf::from(&file.file_path);
                            let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
                            let script_path = settings.get("post_processing_script").map(|s| s.as_str());

                            if let Ok(new_path) = renamer.rename_movie(&movie, &old_path, &lib_root, file.resolution, file.codec.as_deref(), script_path) {
                                let new_path_str = new_path.to_string_lossy().to_string();
                                let _ = sqlx::query("UPDATE movie_files SET file_path = ? WHERE id = ?")
                                    .bind(&new_path_str).bind(file.id).execute(&pool).await;

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
                    });
                }
            }
        }
        task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: total, total, message: "Batch cleanup completed".to_string(), started_at: Some(start_ms), debug_info: None });
    });

    Ok("Batch cleanup started".to_string())
}

#[tauri::command]
async fn update_movie(id: i64, title: String, year: Option<i32>, plot: Option<String>, rating: Option<f32>, genres: Option<Vec<String>>, state: State<'_, AppState>) -> Result<(), String> {
    let genres_json = genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    db::queries::update_movie(&state.pool, media_core::models::MovieId(id), &title, year, plot.as_deref(), rating, genres_json.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_tv_show(id: i64, title: String, plot: Option<String>, rating: Option<f32>, genres: Option<Vec<String>>, state: State<'_, AppState>) -> Result<(), String> {
    let genres_json = genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    db::queries::update_tv_show(&state.pool, media_core::models::TvShowId(id), &title, plot.as_deref(), rating, genres_json.as_deref(), None, None, None, None).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<std::collections::HashMap<String, String>, String> {
    db::queries::get_settings(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_settings(settings: std::collections::HashMap<String, String>, state: State<'_, AppState>) -> Result<(), String> {
    for (key, value) in settings {
        db::queries::set_setting(&state.pool, &key, &value).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn refresh_metadata(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let start_ms = now_ms();
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let clients = media_core::scraper::ScraperClients::from_settings(&pool).await;
    let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
    let script_path = settings.get("post_processing_script").map(|s| s.as_str());

    if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, media_core::models::MovieId(id)).await {
        let _ = media_core::scraper::scrape_movie(movie.id, &movie.title, movie.year, &clients, &pool, script_path).await;
    } else {
        let shows = db::queries::get_all_tv_shows(&pool, None, None, None).await.unwrap_or_default();
        if let Some(show) = shows.into_iter().find(|s| s.id == media_core::models::TvShowId(id)) {
            let _ = media_core::scraper::scrape_tv_show(show.id, &show.title, &clients, &pool, script_path).await;
        }
    }
    task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "Metadata refresh complete".to_string(), started_at: Some(start_ms), debug_info: None });

    Ok(())
}

#[tauri::command]
async fn play_movie(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(Some(path)) = db::queries::get_movie_full_path(&state.pool, media_core::models::MovieId(id)).await {
        opener::open(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn play_episode(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(Some(path)) = db::queries::get_episode_full_path(&state.pool, media_core::models::EpisodeId(id)).await {
        opener::open(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn export_csv(state: State<'_, AppState>) -> Result<String, String> {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    let tv_shows = db::queries::get_all_tv_shows(&state.pool, None, None, None).await.unwrap_or_default();
    Ok(Exporter::to_csv(&movies, &tv_shows))
}

#[tauri::command]
async fn export_html(state: State<'_, AppState>) -> Result<String, String> {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    let tv_shows = db::queries::get_all_tv_shows(&state.pool, None, None, None).await.unwrap_or_default();
    Ok(Exporter::to_html(&movies, &tv_shows))
}

#[tauri::command]
async fn export_json(state: State<'_, AppState>) -> Result<String, String> {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    let tv_shows = db::queries::get_all_tv_shows(&state.pool, None, None, None).await.unwrap_or_default();
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
    let _ = media_core::maintenance::MaintenanceEngine::export_all_nfos(&state.pool).await;
    media_core::maintenance::MaintenanceEngine::create_backup(&state.pool, backup_dir).await
        .map(|p| format!("{:?}", p))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn bulk_scrape(id: i64, state: State<'_, AppState>) -> Result<String, String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    let libraries = db::queries::get_all_libraries(&pool).await.map_err(|e| e.to_string())?;
    if let Some(_lib) = libraries.into_iter().find(|l| l.id == media_core::models::LibraryId(id)) {
        let pool_clone = pool.clone();
        let task_manager_clone = task_manager.clone();
        
        tauri::async_runtime::spawn(async move {
            let mut all_ids_movies: Vec<i64> = Vec::new();
            let mut all_ids_tv: Vec<i64> = Vec::new();

            if let Ok(movies) = db::queries::get_all_movies(&pool_clone, Some(media_core::models::LibraryId(id)), None, None).await {
                all_ids_movies = movies.into_iter()
                    .filter(|m| m.status == media_core::models::MediaStatus::Unmatched)
                    .map(|m| m.id.0)
                    .collect();
            }
            if let Ok(shows) = db::queries::get_all_tv_shows(&pool_clone, Some(media_core::models::LibraryId(id)), None, None).await {
                all_ids_tv = shows.into_iter()
                    .filter(|s| s.status == media_core::models::MediaStatus::Unmatched)
                    .map(|s| s.id.0)
                    .collect();
            }

            let start_ms = now_ms();
            let clients = std::sync::Arc::new(media_core::scraper::ScraperClients::from_settings(&pool_clone).await);
            let settings = db::queries::get_settings(&pool_clone).await.unwrap_or_default();
            let script_path = settings.get("post_processing_script").cloned();

            let mut all_tasks = Vec::new();
            if let Ok(movies) = db::queries::get_movies_by_ids(&pool_clone, &all_ids_movies.iter().map(|&x| media_core::models::MovieId(x)).collect::<Vec<_>>()).await {
                all_tasks.extend(movies.into_iter().map(|m| (m.id.0, m.title, m.year, "movie")));
            }
            if let Ok(shows) = db::queries::get_tv_shows_by_ids(&pool_clone, &all_ids_tv.iter().map(|&x| media_core::models::TvShowId(x)).collect::<Vec<_>>()).await {
                all_tasks.extend(shows.into_iter().map(|s| (s.id.0, s.title, None, "tv")));
            }

            let total = all_tasks.len() as i32;
            let pool_clone_inner = Arc::new(pool_clone);

            use futures::StreamExt;
            let stream = futures::stream::iter(all_tasks.into_iter().enumerate());

            stream.for_each_concurrent(5, |(i, (id, title, year, m_type))| {
                let clients = clients.clone();
                let pool = pool_clone_inner.clone();
                let task_manager = task_manager_clone.clone();
                let task_id = task_id.clone();
                let title_clone = title.clone();
                let script_path_clone = script_path.clone();

                async move {
                    if m_type == "movie" {
                        let _ = media_core::scraper::scrape_movie(id.into(), &title_clone, year, &clients, &pool, script_path_clone.as_deref()).await;
                    } else {
                        let _ = media_core::scraper::scrape_tv_show(id.into(), &title_clone, &clients, &pool, script_path_clone.as_deref()).await;
                    }

                    task_manager.broadcast(TaskUpdate {
                        task_id,
                        status: "running".to_string(),
                        progress: (i + 1) as i32,
                        total,
                        message: format!("Processed: {}", title_clone),
                        started_at: Some(start_ms),
                        debug_info: None,
                    });
                }
            }).await;

            task_manager_clone.broadcast(TaskUpdate {
                task_id,
                status: "completed".to_string(),
                progress: total,
                total,
                message: "Bulk scrape completed".to_string(),
                started_at: Some(start_ms),
                debug_info: None,
            });
        });
        Ok("Bulk scrape started".to_string())
    } else {
        Err("Library not found".to_string())
    }
}

#[tauri::command]
async fn rename_movie(id: i64, state: State<'_, AppState>) -> Result<String, String> {
    let pool = state.pool.clone();
    match db::queries::get_movie_by_id(&pool, media_core::models::MovieId(id)).await {
        Ok(Some(movie)) => {
            let movie_id = movie.id;
            let libraries = db::queries::get_all_libraries(&pool).await.unwrap_or_default();
            if let Some(lib) = libraries.into_iter().find(|l| l.id == movie.library_id) {
                let file_info: Option<media_core::models::MovieFile> = sqlx::query_as("SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1")
                    .bind(movie_id).fetch_optional(&pool).await.unwrap_or_default();
                if let Some(file) = file_info {
                    let pool_clone = pool.clone();
                    let lib_path = lib.path.clone();
                    let old_path_str = file.file_path.clone();
                    let renamer = media_core::renamer::Renamer::new(None, None);
                    let old_path = std::path::PathBuf::from(&old_path_str);
                    let lib_root = std::path::PathBuf::from(&lib_path);
                    let settings = db::queries::get_settings(&pool_clone).await.unwrap_or_default();
                    let script_path = settings.get("post_processing_script").cloned();

                    match renamer.rename_movie(&movie, &old_path, &lib_root, file.resolution, file.codec.as_deref(), script_path.as_deref()) {
                        Ok(new_path) => {
                            let new_path_str = new_path.to_string_lossy().to_string();
                            let _ = sqlx::query("UPDATE movie_files SET file_path = ? WHERE id = ?")
                                .bind(new_path_str).bind(file.id).execute(&pool_clone).await;
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
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    // Check environment first, then database settings
    let mut api_key = std::env::var("OPENSUBTITLES_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        if let Ok(settings) = db::queries::get_settings(&pool).await {
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
        if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, media_core::models::MovieId(id)).await {
            let file_info: Option<(String,)> = sqlx::query_as("SELECT file_path FROM movie_files WHERE movie_id = ? LIMIT 1")
                .bind(id).fetch_optional(&pool).await.unwrap_or_default();

            if let Some((path_str,)) = file_info {
                let dest_path = std::path::PathBuf::from(path_str);
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
        let _ = task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "Subtitle search finished".to_string(), started_at: Some(start_ms), debug_info: None });
    });
    Ok(())
}

#[tauri::command]
async fn process_movie_advanced(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, media_core::models::MovieId(id)).await {
            let file_info: Option<media_core::models::MovieFile> = sqlx::query_as("SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1")
                .bind(id).fetch_optional(&pool).await.unwrap_or_default();
            if let Some(file) = file_info {
                task_manager.broadcast(TaskUpdate {
                    task_id: task_id.clone(),
                    status: "running".to_string(),
                    progress: 0,
                    total: 1,
                    message: format!("Analyzing: {}", movie.title),
                    started_at: Some(start_ms),
                    debug_info: None,
                });
                let input_path = std::path::PathBuf::from(&file.file_path);
                if input_path.exists() {
                    let folder = input_path.parent().unwrap();
                    let thumb_dest = folder.join(format!("{}.thumb.jpg", input_path.file_stem().unwrap().to_str().unwrap()));
                    let ratio = media_core::scanner::ffmpeg::FfmpegEngine::detect_aspect_ratio(&input_path).ok();
                    let thumb = media_core::scanner::ffmpeg::FfmpegEngine::extract_thumbnail(&input_path, &thumb_dest, "00:05:00").ok();
                    let _ = sqlx::query("UPDATE movie_files SET aspect_ratio = ?, thumbnail_path = ? WHERE id = ?")
                        .bind(ratio).bind(thumb.map(|p| p.to_string_lossy().to_string())).bind(file.id).execute(&pool).await;
                }
            }
        }
        let _ = task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "Advanced analysis complete".to_string(), started_at: Some(start_ms), debug_info: None });
    });
    Ok(())
}

#[tauri::command]
async fn process_tv_show_advanced(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        if let Ok(Some(show)) = db::queries::get_tv_show_by_id(&pool, media_core::models::TvShowId(id)).await {
            let seasons = db::queries::get_seasons_by_show_id(&pool, show.id).await.unwrap_or_default();
            for s in seasons {
                let eps = db::queries::get_episodes_by_season_id(&pool, s.id).await.unwrap_or_default();
                for ep in eps {
                    let input_path = std::path::PathBuf::from(&ep.file_path);
                    if input_path.exists() {
                        let folder = input_path.parent().unwrap();
                        let thumb_dest = folder.join(format!("{}.thumb.jpg", input_path.file_stem().unwrap().to_str().unwrap()));
                        let ratio = media_core::scanner::ffmpeg::FfmpegEngine::detect_aspect_ratio(&input_path).ok();
                        let thumb = media_core::scanner::ffmpeg::FfmpegEngine::extract_thumbnail(&input_path, &thumb_dest, "00:05:00").ok();
                        let _ = sqlx::query("UPDATE episodes SET aspect_ratio = ?, thumbnail_path = ? WHERE id = ?")
                            .bind(ratio).bind(thumb.map(|p| p.to_string_lossy().to_string())).bind(ep.id).execute(&pool).await;
                    }
                }
            }
        }
        let _ = task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "TV analysis complete".to_string(), started_at: Some(start_ms), debug_info: None });
    });
    Ok(())
}

#[tauri::command]
async fn process_library_advanced(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn(async move {
        let start_ms = now_ms();
        if let Ok(movies) = db::queries::get_all_movies(&pool, Some(media_core::models::LibraryId(id)), None, None).await {
            for movie in movies {
                let file_info: Option<media_core::models::MovieFile> = sqlx::query_as("SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1")
                    .bind(movie.id).fetch_optional(&pool).await.unwrap_or_default();
                if let Some(file) = file_info {
                    let input_path = std::path::PathBuf::from(&file.file_path);
                    if input_path.exists() {
                        let folder = input_path.parent().unwrap();
                        let thumb_dest = folder.join(format!("{}.thumb.jpg", input_path.file_stem().unwrap().to_str().unwrap()));
                        let ratio = media_core::scanner::ffmpeg::FfmpegEngine::detect_aspect_ratio(&input_path).ok();
                        let thumb = media_core::scanner::ffmpeg::FfmpegEngine::extract_thumbnail(&input_path, &thumb_dest, "00:05:00").ok();
                        let _ = sqlx::query("UPDATE movie_files SET aspect_ratio = ?, thumbnail_path = ? WHERE id = ?")
                            .bind(ratio).bind(thumb.map(|p| p.to_string_lossy().to_string())).bind(file.id).execute(&pool).await;
                    }
                }
            }
        }
        let _ = task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "Library analysis complete".to_string(), started_at: Some(start_ms), debug_info: None });
    });
    Ok(())
}

#[tauri::command]
async fn sync_trakt(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let settings_map = db::queries::get_settings(&state.pool).await.unwrap_or_default();
    let access_token = settings_map.get("trakt_access_token").cloned().unwrap_or_default();
    if access_token.is_empty() { return Err("Trakt not authenticated".to_string()); }
    let scraper_clients = media_core::scraper::ScraperClients::from_settings(&state.pool).await;
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
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
    let libraries = db::queries::get_all_libraries(&state.pool).await.map_err(|e| e.to_string())?;
    if let Some(lib) = libraries.into_iter().find(|l| l.id == media_core::models::LibraryId(id)) {
        let cleanup = CleanupService::new(PathBuf::from(lib.path));
        cleanup.remove_duplicate_artwork().map_err(|e| e.to_string())
    } else {
        Err("Library not found".to_string())
    }
}

#[tauri::command]
async fn cleanup_empty_folders(id: i64, state: State<'_, AppState>) -> Result<Vec<PathBuf>, String> {
    let libraries = db::queries::get_all_libraries(&state.pool).await.map_err(|e| e.to_string())?;
    if let Some(lib) = libraries.into_iter().find(|l| l.id == media_core::models::LibraryId(id)) {
        let cleanup = CleanupService::new(PathBuf::from(lib.path));
        cleanup.remove_empty_folders().map_err(|e| e.to_string())
    } else {
        Err("Library not found".to_string())
    }
}

#[tauri::command]
async fn start_streaming(id: i64, media_type: String, state: State<'_, AppState>) -> Result<String, String> {
    let pool = state.pool.clone();

    let path = if media_type == "movie" {
        db::queries::get_movie_full_path(&pool, media_core::models::MovieId(id)).await
    } else {
        db::queries::get_episode_full_path(&pool, media_core::models::EpisodeId(id)).await
    }.map_err(|e| e.to_string())?;

    if let Some(input_path) = path {
        let output_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join("transcodes").join(id.to_string());
        if !output_dir.exists() {
            std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
        }

        media_core::scanner::ffmpeg::FfmpegEngine::create_hls_stream(&input_path, &output_dir)
            .map_err(|e| e.to_string())?;

        let playlist = output_dir.join("playlist.m3u8");
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
                debug_info: None,
            });
            last_report = std::time::Instant::now();
        }
    }
    
    writer.flush().await?;
    Ok(())
}

#[tauri::command]
async fn download_to_local(id: i64, media_type: String, dest_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    let path = if media_type == "movie" {
        db::queries::get_movie_full_path(&pool, media_core::models::MovieId(id)).await
    } else {
        db::queries::get_episode_full_path(&pool, media_core::models::EpisodeId(id)).await
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
                debug_info: None,
            });

            match copy_with_progress(&src_clone, &dest_clone, task_manager_clone.clone(), task_id_clone.clone(), start_ms).await {
                Ok(_) => {
                    task_manager_clone.broadcast(TaskUpdate {
                        task_id: task_id_clone,
                        status: "completed".to_string(),
                        progress: 100,
                        total: 100,
                        message: format!("Download completed: {}", src_clone.file_name().unwrap_or_default().to_string_lossy()),
                        started_at: Some(start_ms),
                        debug_info: None,
                    });
                }
                Err(e) => {
                    task_manager_clone.broadcast(TaskUpdate {
                        task_id: task_id_clone,
                        status: "error".to_string(),
                        progress: 0,
                        total: 100,
                        message: format!("Download failed: {}", e),
                        started_at: Some(start_ms),
                        debug_info: None,
                    });
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
    let row: Option<(i64, i64, bool)> = sqlx::query_as("SELECT position_ms, duration_ms, is_finished FROM playback_state WHERE media_id = ? AND media_type = ?")
        .bind(id).bind(media_type).fetch_optional(&state.pool).await.map_err(|e| e.to_string())?;

    if let Some((pos, dur, fin)) = row {
        Ok(serde_json::json!({ "position_ms": pos, "duration_ms": dur, "is_finished": fin }))
    } else {
        Ok(serde_json::json!({ "position_ms": 0, "duration_ms": 0, "is_finished": false }))
    }
}

#[tauri::command]
async fn update_playback_progress(
    media_id: i64,
    media_type: String,
    position_ms: i64,
    duration_ms: i64,
    is_finished: bool,
    state: State<'_, AppState>
) -> Result<(), String> {
    sqlx::query("INSERT INTO playback_state (media_id, media_type, position_ms, duration_ms, is_finished, updated_at)
                VALUES (?, ?, ?, ?, ?, datetime('now'))
                ON CONFLICT(media_id, media_type) DO UPDATE SET
                position_ms = excluded.position_ms,
                duration_ms = excluded.duration_ms,
                is_finished = excluded.is_finished,
                updated_at = datetime('now')")
        .bind(media_id).bind(media_type).bind(position_ms).bind(duration_ms).bind(is_finished)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    Ok(())
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
    let task_manager = Arc::new(TaskManager::new());
    let task_manager_clone = task_manager.clone();
    let pool_for_watchdog = pool.clone();
    let task_manager_for_watchdog = task_manager.clone();
    let pool_for_notifications = pool.clone();
    let task_manager_for_notifications = task_manager.clone();
    tauri::Builder::default()
        .manage(AppState { pool, task_manager })
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
            
            // Sidecars are named with the target triple, e.g., ffmpeg-x86_64-pc-windows-msvc.exe
            // Tauri's sidecar() helper handles this, but we need the absolute path for Command::new
            
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
                        if let Ok(settings) = db::queries::get_settings(&pool_for_notifications).await {
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
                let watchdog = media_core::scanner::watchdog::Watchdog::new(pool_for_watchdog, task_manager_for_watchdog);
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
