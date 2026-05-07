// apps/desktop/src/main.rs
use std::sync::Arc;
use std::path::PathBuf;
use tauri::{State, Emitter};
use media_core::db;
use media_core::task_manager::TaskManager;
use media_core::models::{Library, Movie, MediaType, TVShow, Season, Episode, TaskUpdate};
use media_core::cleanup::CleanupService;
use media_core::exporter::Exporter;
use sqlx::SqlitePool;

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
    
    tokio::spawn(async move {
        let _ = media_core::scanner::worker::scan_library(&pool, &library, task_id, &task_manager).await;
    });

    Ok(id)
}

#[tauri::command]
async fn delete_library(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    db::queries::delete_library(&state.pool, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_movies(
    library_id: Option<i64>, 
    genre: Option<String>, 
    language: Option<String>, 
    state: State<'_, AppState>
) -> Result<Vec<Movie>, String> {
    db::queries::get_all_movies(&state.pool, library_id, genre, language).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_tv_shows(
    library_id: Option<i64>, 
    genre: Option<String>, 
    language: Option<String>, 
    state: State<'_, AppState>
) -> Result<Vec<TVShow>, String> {
    db::queries::get_all_tv_shows(&state.pool, library_id, genre, language).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_seasons(show_id: i64, state: State<'_, AppState>) -> Result<Vec<Season>, String> {
    db::queries::get_seasons_by_show_id(&state.pool, show_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_episodes(season_id: i64, state: State<'_, AppState>) -> Result<Vec<Episode>, String> {
    db::queries::get_episodes_by_season_id(&state.pool, season_id).await.map_err(|e| e.to_string())
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
    if let Some(lib) = libraries.into_iter().find(|l| l.id == library_id) {
        tokio::spawn(async move {
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
    let tmdb_key = std::env::var("TMDB_API_KEY").unwrap_or_default();

    if tmdb_key.is_empty() { return Err("TMDB API Key missing".to_string()); }

    tokio::spawn(async move {
        let start_ms = now_ms();
        let tmdb_key = std::env::var("TMDB_API_KEY").unwrap_or_default();
        let omdb_key = std::env::var("OMDB_API_KEY").unwrap_or_default();
        let fanart_key = std::env::var("FANART_API_KEY").unwrap_or_default();
        let trakt_key = std::env::var("TRAKT_API_KEY").unwrap_or_default();
        let tvdb_key = std::env::var("TVDB_API_KEY").unwrap_or_default();
        let clients = Arc::new(media_core::scraper::ScraperClients::new(tmdb_key, omdb_key, fanart_key, trakt_key, tvdb_key));
        
        let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
        let script_path = settings.get("post_processing_script").cloned();

        let mut all_tasks = Vec::new();
        if media_type == "movie" {
            if let Ok(movies) = db::queries::get_movies_by_ids(&pool, &ids).await {
                all_tasks.extend(movies.into_iter().map(|m| (m.id, m.title, m.year, "movie")));
            }
        } else {
            if let Ok(shows) = db::queries::get_tv_shows_by_ids(&pool, &ids).await {
                all_tasks.extend(shows.into_iter().map(|s| (s.id, s.title, None, "tv")));
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
                    let _ = media_core::scraper::scrape_movie(id, &title_clone, year, &clients, &pool, script_path_clone.as_deref()).await;
                } else {
                    let _ = media_core::scraper::scrape_tv_show(id, &title_clone, &clients, &pool, script_path_clone.as_deref()).await;
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

    tokio::spawn(async move {
        let start_ms = now_ms();
        let total = ids.len() as i32;
        let mut processed = 0;

        if media_type == "movie" {
            let renamer = media_core::renamer::Renamer::new(None, None);
            for id in ids {
                processed += 1;
                if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, id).await {
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
    db::queries::update_movie(&state.pool, id, &title, year, plot.as_deref(), rating, genres_json.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_tv_show(id: i64, title: String, plot: Option<String>, rating: Option<f32>, genres: Option<Vec<String>>, state: State<'_, AppState>) -> Result<(), String> {
    let genres_json = genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    db::queries::update_tv_show(&state.pool, id, &title, plot.as_deref(), rating, genres_json.as_deref(), None, None, None, None).await.map_err(|e| e.to_string())
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
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let tmdb_key = std::env::var("TMDB_API_KEY").unwrap_or_default();

    if tmdb_key.is_empty() { return Err("TMDB API Key missing".to_string()); }

    tokio::spawn(async move {
        let start_ms = now_ms();
        let tmdb_key = std::env::var("TMDB_API_KEY").unwrap_or_default();
        let omdb_key = std::env::var("OMDB_API_KEY").unwrap_or_default();
        let fanart_key = std::env::var("FANART_API_KEY").unwrap_or_default();
        let trakt_key = std::env::var("TRAKT_API_KEY").unwrap_or_default();
        let tvdb_key = std::env::var("TVDB_API_KEY").unwrap_or_default();
        let clients = media_core::scraper::ScraperClients::new(tmdb_key, omdb_key, fanart_key, trakt_key, tvdb_key);
        let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
        let script_path = settings.get("post_processing_script").map(|s| s.as_str());

        if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, id).await {
            let _ = media_core::scraper::scrape_movie(movie.id, &movie.title, movie.year, &clients, &pool, script_path).await;
        } else {
            let shows = db::queries::get_all_tv_shows(&pool, None, None, None).await.unwrap_or_default();
            if let Some(show) = shows.into_iter().find(|s| s.id == id) {
                let _ = media_core::scraper::scrape_tv_show(show.id, &show.title, &clients, &pool, script_path).await;
            }
        }
        task_manager.broadcast(TaskUpdate { task_id, status: "completed".to_string(), progress: 1, total: 1, message: "Metadata refresh complete".to_string(), started_at: Some(start_ms), debug_info: None });
    });
    Ok(())
}

#[tauri::command]
async fn play_movie(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let pool = state.pool.clone();
    let movie_files: Vec<(String,)> = sqlx::query_as("SELECT file_path FROM movie_files WHERE movie_id = ?")
        .bind(id).fetch_all(&pool).await.map_err(|e| e.to_string())?;
    if let Some((path,)) = movie_files.first() {
        opener::open(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn play_episode(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let pool = state.pool.clone();
    let episodes: Vec<(String,)> = sqlx::query_as("SELECT file_path FROM episodes WHERE id = ?")
        .bind(id).fetch_all(&pool).await.map_err(|e| e.to_string())?;
    if let Some((path,)) = episodes.first() {
        opener::open(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn export_csv(state: State<'_, AppState>) -> Result<String, String> {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    Ok(Exporter::to_csv(&movies))
}

#[tauri::command]
async fn export_html(state: State<'_, AppState>) -> Result<String, String> {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    Ok(Exporter::to_html(&movies))
}

#[tauri::command]
async fn check_updates() -> Result<serde_json::Value, String> {
    media_core::maintenance::MaintenanceEngine::check_for_updates()
        .map(|v| serde_json::json!({ "latest_version": v, "current_version": "0.1.0" }))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_backup(state: State<'_, AppState>) -> Result<String, String> {
    let db_path = std::path::Path::new("mediavault.db");
    let backup_dir = std::path::Path::new("backups");
    
    let _ = media_core::maintenance::MaintenanceEngine::export_all_nfos(&state.pool).await;

    media_core::maintenance::MaintenanceEngine::create_backup(db_path, backup_dir)
        .map(|p| format!("{:?}", p))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn bulk_scrape(id: i64, state: State<'_, AppState>) -> Result<String, String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    let libraries = db::queries::get_all_libraries(&pool).await.map_err(|e| e.to_string())?;
    if let Some(_lib) = libraries.into_iter().find(|l| l.id == id) {
        let pool_clone = pool.clone();
        let task_manager_clone = task_manager.clone();
        
        tokio::spawn(async move {
            let movies = db::queries::get_all_movies(&pool_clone, Some(id), None, None).await.unwrap_or_default();
            let ids: Vec<i64> = movies.into_iter()
                .filter(|m| m.status == media_core::models::MediaStatus::Unmatched)
                .map(|m| m.id)
                .collect();
            
            // Logic from scrape_batch
            let start_ms = now_ms();
            let tmdb_key = std::env::var("TMDB_API_KEY").unwrap_or_default();
            let omdb_key = std::env::var("OMDB_API_KEY").unwrap_or_default();
            let fanart_key = std::env::var("FANART_API_KEY").unwrap_or_default();
            let trakt_key = std::env::var("TRAKT_API_KEY").unwrap_or_default();
            let tvdb_key = std::env::var("TVDB_API_KEY").unwrap_or_default();
            
            if tmdb_key.is_empty() { return; }
            
            let clients = Arc::new(media_core::scraper::ScraperClients::new(tmdb_key, omdb_key, fanart_key, trakt_key, tvdb_key));
            let settings = db::queries::get_settings(&pool_clone).await.unwrap_or_default();
            let script_path = settings.get("post_processing_script").cloned();
            
            let mut all_tasks = Vec::new();
            if let Ok(movies) = db::queries::get_movies_by_ids(&pool_clone, &ids).await {
                all_tasks.extend(movies.into_iter().map(|m| (m.id, m.title, m.year, "movie")));
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
                        let _ = media_core::scraper::scrape_movie(id, &title_clone, year, &clients, &pool, script_path_clone.as_deref()).await;
                    } else {
                        let _ = media_core::scraper::scrape_tv_show(id, &title_clone, &clients, &pool, script_path_clone.as_deref()).await;
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

fn main() {

    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:mediavault.db?mode=rwc".to_string());
    let pool = tauri::async_runtime::block_on(async {
        db::init_pool(&database_url).await.expect("Failed to initialize database pool")
    });
    let task_manager = Arc::new(TaskManager::new());

    let task_manager_clone = task_manager.clone();
    let pool_for_watchdog = pool.clone();
    let task_manager_for_watchdog = task_manager.clone();

    tauri::Builder::default()
        .manage(AppState { pool, task_manager })
        .invoke_handler(tauri::generate_handler![
            get_libraries, create_library, delete_library,
            get_movies, get_tv_shows, get_seasons, get_episodes,
            get_genres, get_languages, start_scan, scrape_batch,
            cleanup_batch, update_movie, update_tv_show,
            get_settings, set_settings, refresh_metadata,
            play_movie, play_episode, export_csv, export_html,
            check_updates, create_backup, bulk_scrape
        ])
        .setup(|app| {
            // Start Watchdog
            tokio::spawn(async move {
                let watchdog = media_core::scanner::watchdog::Watchdog::new(pool_for_watchdog, task_manager_for_watchdog);
                if let Err(e) = watchdog.start().await {
                    eprintln!("Watchdog failed: {}", e);
                }
            });

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = task_manager_clone.subscribe();
                while let Ok(update) = rx.recv().await {
                    let _ = handle.emit("task-update", update);
                }
            });
            Ok(())
        })

        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


