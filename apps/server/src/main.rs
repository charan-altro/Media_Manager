// apps/server/src/main.rs
use axum::{
    routing::{get, post},
    Router,
    extract::{State, Path, Query},
    Json,
    response::{Sse, IntoResponse},
    http::{header, StatusCode},
};
use axum::response::sse::{Event, KeepAlive};
use std::net::SocketAddr;
use std::sync::Arc;
use std::path::PathBuf;
use media_core::db;
use media_core::models::{MovieId, TvShowId, LibraryId, SeasonId, EpisodeId};
use media_core::task_manager::TaskManager;
use media_core::cleanup::CleanupService;
use media_core::exporter::Exporter;
use opener;
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use futures::stream::Stream;
use std::convert::Infallible;

use media_core::scanner::streaming::StreamManager;

struct AppState {
    pool: SqlitePool,
    task_manager: Arc<TaskManager>,
    stream_manager: Arc<StreamManager>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn generate_hls_manifest(_stream_id: &str, duration_secs: f64) -> String {
    let mut manifest = String::from("#EXTM3U\n");
    manifest.push_str("#EXT-X-VERSION:3\n");
    manifest.push_str("#EXT-X-TARGETDURATION:12\n");
    manifest.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
    manifest.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");

    let segment_duration = 10.0;
    let num_segments = (duration_secs / segment_duration).ceil() as usize;

    for i in 0..num_segments {
        let remaining = duration_secs - (i as f64 * segment_duration);
        let current_seg_dur = if remaining < segment_duration {
            remaining
        } else {
            segment_duration
        };

        manifest.push_str(&format!("#EXTINF:{:.1},\n", current_seg_dur));
        manifest.push_str(&format!("seg_{:03}.ts\n", i));
    }

    manifest.push_str("#EXT-X-ENDLIST\n");
    manifest
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    
    // Fallback log level if RUST_LOG is not set
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,media_core=debug,server=debug");
    }
    tracing_subscriber::fmt::init();

    println!("=========================================");
    println!("  Media Orchestrator Backend Starting... ");
    println!("=========================================");

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:mediavault.db?mode=rwc".to_string());
    let pool = db::init_pool(&database_url).await.expect("Failed to initialize database pool");
    let task_manager = Arc::new(TaskManager::new());
    
    // Configurable Transcode Directory (Default to RAM disk /dev/shm on Linux/Pi to save SD card life)
    let transcode_dir = std::env::var("HLS_TRANSCODE_DIR").unwrap_or_else(|_| {
        if cfg!(target_os = "linux") && std::path::Path::new("/dev/shm").exists() {
            "/dev/shm/media_manager_transcodes".to_string()
        } else {
            std::env::temp_dir().join("media_manager_transcodes").to_string_lossy().to_string()
        }
    });
    media_core::config::set_hls_transcode_dir(transcode_dir.clone());
    let stream_manager = Arc::new(StreamManager::new(std::path::PathBuf::from(&transcode_dir)));

    let app_state = Arc::new(AppState {
        pool: pool.clone(),
        task_manager: task_manager.clone(),
        stream_manager: stream_manager.clone(),
    });

    // Start background stream cleanup
    let sm_for_cleanup = stream_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            sm_for_cleanup.cleanup_stale_streams().await;
        }
    });

    // Start background notification monitor
    let task_manager_for_notifications = task_manager.clone();
    let pool_for_notifications = pool.clone();
    tokio::spawn(async move {
        let mut rx = task_manager_for_notifications.subscribe();
        let notifier = media_core::notifications::Notifier::new();
        
        while let Ok(update) = rx.recv().await {
            // Only notify on completion or error
            if update.status == "completed" || update.status == "error" {
                // Check settings for webhook URL
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

    // Start Real-time Watchdog
    let pool_for_watchdog = pool.clone();
    let task_manager_for_watchdog = task_manager.clone();
    tokio::spawn(async move {
        let watchdog = media_core::scanner::watchdog::Watchdog::new(pool_for_watchdog, task_manager_for_watchdog);
        if let Err(e) = watchdog.start().await {
            tracing::error!("Watchdog failed: {}", e);
        }
    });

    let allowed_origin = std::env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let cors = if allowed_origin == "*" {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
            .allow_origin(allowed_origin.parse::<axum::http::HeaderValue>().unwrap())
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
    };

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/webhooks/:source", post(handle_webhook))
        .route("/api/libraries", get(get_libraries).post(create_library))
        .route("/api/libraries/:id", axum::routing::delete(delete_library))
        .route("/api/libraries/:id/scan", post(scan_library))
        .route("/api/libraries/:id/scrape", post(bulk_scrape))
        .route("/api/movies/:id/scrape", post(scrape_single_movie))
        .route("/api/tvshows/:id/scrape", post(scrape_single_tv_show))
        .route("/api/scrape/batch", post(scrape_batch))
        .route("/api/libraries/:id/cleanup/duplicates", post(cleanup_duplicates))
        .route("/api/libraries/:id/cleanup/empty-folders", post(cleanup_empty_folders))
        .route("/api/cleanup/batch", post(cleanup_batch))
        .route("/api/artwork/local", get(get_local_artwork))
        .route("/api/movies", get(get_movies))
        .route("/api/tvshows", get(get_tv_shows))
        .route("/api/genres", get(get_genres))
        .route("/api/languages", get(get_languages))
        .route("/api/tvshows/:id/seasons", get(get_seasons))
        .route("/api/seasons/:id/episodes", get(get_episodes))
        .route("/api/movies/:id", axum::routing::put(update_movie_metadata))
        .route("/api/tvshows/:id", axum::routing::put(update_tv_show_metadata))
        .route("/api/settings", get(get_settings).post(set_settings))
        .route("/api/movies/:id/rename", post(rename_movie))
        .route("/api/movies/:id/process-advanced", post(process_movie_advanced))
        .route("/api/tvshows/:id/process-advanced", post(process_tv_show_advanced))
        .route("/api/libraries/:id/process-advanced", post(process_library_advanced))
        .route("/api/tasks", get(get_tasks))
        .route("/api/movies/:id/refresh", post(refresh_metadata))
        .route("/api/movies/:id/play", post(play_movie))
        .route("/api/episodes/:id/play", post(play_episode))
        .route("/api/stream/movie/:id", post(start_movie_stream))
        .route("/api/stream/episode/:id", post(start_episode_stream))
        .route("/api/stream/direct/movie/:id", get(serve_direct_movie))
        .route("/api/stream/direct/episode/:id", get(serve_direct_episode))
        .route("/api/stream/hls/:id/:file", get(serve_stream_file))
        .nest_service("/transcodes", tower_http::services::ServeDir::new("transcodes"))
        .route("/api/movies/:id/download", get(download_movie))
        .route("/api/episodes/:id/download", get(download_episode))
        // .route("/api/stream/:id/start", post(start_streaming))
        // .route("/api/stream/:id/hls/:file", get(serve_stream))
        .route("/api/playback/heartbeat", post(update_playback_progress))
        .route("/api/playback/status/:type/:id", get(get_playback_status))
        .route("/api/movies/:id/subtitles/search", get(search_subtitles))
        .route("/api/tasks/stream", get(task_stream))
        .route("/api/export/csv", get(export_csv))
        .route("/api/export/html", get(export_html))
        .route("/api/export/xlsx", get(export_xlsx))
        .route("/api/export/json", get(export_json))
        .route("/api/maintenance/backup", post(create_backup))
        .route("/api/system/update-check", get(check_updates))
        .route("/api/sync/trakt", post(sync_trakt))
        .layer(cors)
        .with_state(app_state)
        .fallback_service(
            tower_http::services::ServeDir::new("frontend/dist")
                .fallback(tower_http::services::ServeFile::new("frontend/dist/index.html"))
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 7878));
    tracing::info!("Server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    Path(source): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tracing::info!("Received webhook from {}: {:?}", source, payload);

    tokio::spawn(async move {
        // Trigger a global scan or specific library scan based on webhook logic
        let libraries = db::queries::get_all_libraries(&pool).await.unwrap_or_default();
        let target_lib = match source.as_str() {
            "radarr" => libraries.into_iter().find(|l| l.media_type == media_core::models::MediaType::Movie),
            "sonarr" => libraries.into_iter().find(|l| l.media_type == media_core::models::MediaType::Tv),
            _ => libraries.into_iter().next(),
        };

        if let Some(lib) = target_lib {
            task_manager.broadcast(media_core::models::TaskUpdate {
                task_id: task_id.clone(),
                status: "running".to_string(),
                progress: 0,
                total: 1,
                message: format!("Webhook trigger: Scanning {}", lib.name),
                started_at: Some(now_ms()),
                finished_at: None,
                debug_info: Some(format!("Source: {}", source)),
                files_new: None,
                files_healed: None,
                files_missing: None,
            });

            let _ = media_core::scanner::worker::scan_library(&pool, &lib, task_id.clone(), &task_manager).await;
        }
    });

    StatusCode::ACCEPTED
}


async fn get_libraries(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::queries::get_all_libraries(&state.pool).await {
        Ok(libs) => Json(libs).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch libraries: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
struct CreateLibraryRequest {
    name: String,
    path: String,
    media_type: media_core::models::MediaType,
}

async fn create_library(State(state): State<Arc<AppState>>, Json(payload): Json<CreateLibraryRequest>) -> impl IntoResponse {
    tracing::info!("Creating library: {} at {}", payload.name, payload.path);
    match db::queries::insert_library(&state.pool, &payload.name, &payload.path, payload.media_type).await {
        Ok(id) => {
            tracing::info!("Library created with ID: {}", id);
            
            // Trigger automatic scan
            let pool = state.pool.clone();
            let task_manager = state.task_manager.clone();
            let task_id = uuid::Uuid::new_v4().to_string();
            let library = media_core::models::Library {
                id,
                name: payload.name,
                path: payload.path,
                media_type: payload.media_type,
                created_at: "".to_string(), // Not used by worker
            };
            
            tokio::spawn(async move {
                let _ = media_core::scanner::worker::scan_library(&pool, &library, task_id, &task_manager).await;
            });

            (StatusCode::CREATED, Json(id)).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to create library: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response()
        }
    }
}

async fn delete_library(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    tracing::info!("Deleting library ID: {}", id);
    match db::queries::delete_library(&state.pool, LibraryId(id)).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to delete library {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
struct MovieQuery { 
    library_id: Option<i64>,
    genre: Option<String>,
    language: Option<String>,
}

async fn get_movies(State(state): State<Arc<AppState>>, Query(query): Query<MovieQuery>) -> impl IntoResponse {
    match db::queries::get_all_movies(&state.pool, query.library_id.map(LibraryId), query.genre, query.language).await {
        Ok(movies) => Json(movies).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch movies: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_tv_shows(State(state): State<Arc<AppState>>, Query(query): Query<MovieQuery>) -> impl IntoResponse {
    match db::queries::get_all_tv_shows(&state.pool, query.library_id.map(LibraryId), query.genre, query.language).await {
        Ok(shows) => Json(shows).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch TV shows: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_seasons(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match db::queries::get_seasons_by_show_id(&state.pool, TvShowId(id)).await {
        Ok(seasons) => Json(seasons).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch seasons: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_episodes(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match db::queries::get_episodes_by_season_id(&state.pool, SeasonId(id)).await {
        Ok(episodes) => Json(episodes).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch episodes: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn scan_library(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    // Check if a scan task is already running for this library
    // (Simple check by searching through active tasks)
    let libraries = db::queries::get_all_libraries(&pool).await.unwrap_or_default();
    if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(id)) {
        tokio::spawn(async move {
            let _ = media_core::scanner::worker::scan_library(&pool, &lib, task_id, &task_manager).await;
        });
        Json("Scan started".to_string())
    } else {
        Json("Library not found".to_string())
    }
}

async fn bulk_scrape(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    // We'll use hardcoded keys or get them from env
    let clients = std::sync::Arc::new(media_core::scraper::ScraperClients::from_settings(&pool).await);

    tokio::spawn(async move {
        let start_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;

        // Fetch settings
        let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
        let script_path = settings.get("post_processing_script").cloned();

        let mut all_tasks = Vec::new();

        if let Ok(movies) = db::queries::get_all_movies(&pool, Some(LibraryId(id)), None, None).await {
            let unmatched: Vec<_> = movies.into_iter().filter(|m| m.status == media_core::models::MediaStatus::Unmatched).collect();
            all_tasks.extend(unmatched.into_iter().map(|m| (m.id.0, m.title, m.year, "movie")));
        }

        if let Ok(shows) = db::queries::get_all_tv_shows(&pool, Some(LibraryId(id)), None, None).await {
            let unmatched: Vec<_> = shows.into_iter().filter(|s| s.status == media_core::models::MediaStatus::Unmatched).collect();
            all_tasks.extend(unmatched.into_iter().map(|s| (s.id.0, s.title, None, "tv")));
        }

        let total = all_tasks.len() as i32;
        let task_manager = task_manager.clone();
        let task_id_clone = task_id.clone();
        let pool = Arc::new(pool);

        use futures::StreamExt;
        let stream = futures::stream::iter(all_tasks.into_iter().enumerate());
        
        stream.for_each_concurrent(10, |(i, (id, title, year, m_type))| {
            let clients = clients.clone();
            let pool = pool.clone();
            let task_manager = task_manager.clone();
            let task_id = task_id_clone.clone();
            let title_clone = title.clone();
            
            let script_path_clone = script_path.clone();
            
            async move {
                if m_type == "movie" {
                    let _ = media_core::scraper::scrape_movie(MovieId(id), &title_clone, year, &clients, &pool, script_path_clone.as_deref()).await;
                } else {
                    let _ = media_core::scraper::scrape_tv_show(TvShowId(id), &title_clone, &clients, &pool, script_path_clone.as_deref()).await;
                }
                
                task_manager.broadcast(media_core::models::TaskUpdate {
                    task_id,
                    status: "running".to_string(),
                    progress: (i + 1) as i32,
                    total,
                    message: format!("Processed: {}", title_clone),
                    started_at: Some(start_ms),
                    finished_at: None,
                    debug_info:
 Some(format!("Scraping {}/{} ({}): {}", i+1, total, m_type, title_clone)),
                    files_new: None,
                    files_healed: None,
                    files_missing: None,
                });
            }
        }).await;

        task_manager.broadcast(media_core::models::TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: total,
            total,
            message: "Enrichment completed".to_string(),
            started_at: Some(start_ms),
            finished_at: Some(now_ms()),
            debug_info: None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    Json("Scrape started".to_string())
}

async fn scrape_single_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    // H3 fix: only handles movies — TV shows have their own route
    let pool = state.pool.clone();
    tokio::spawn(async move {
        let clients = media_core::scraper::ScraperClients::from_settings(&pool).await;

        let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
        let script_path = settings.get("post_processing_script").map(|s| s.as_str());

        if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, MovieId(id)).await {
            let _ = media_core::scraper::scrape_movie(movie.id, &movie.title, movie.year, &clients, &pool, script_path).await;
        }
    });

    Json("Scrape started".to_string())
}

async fn scrape_single_tv_show(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    // H3 fix: dedicated TV show scrape endpoint
    let pool = state.pool.clone();
    tokio::spawn(async move {
        let clients = media_core::scraper::ScraperClients::from_settings(&pool).await;

        let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
        let script_path = settings.get("post_processing_script").map(|s| s.as_str());

        if let Ok(shows) = db::queries::get_all_tv_shows(&pool, None, None, None).await {
            if let Some(show) = shows.into_iter().find(|s| s.id == TvShowId(id)) {
                let _ = media_core::scraper::scrape_tv_show(show.id, &show.title, &clients, &pool, script_path).await;
            }
        }
    });

    Json("TV show scrape started".to_string())
}


#[derive(serde::Deserialize)]
struct BatchRequest {
    ids: Vec<i64>,
    media_type: String,
}

async fn scrape_batch(State(state): State<Arc<AppState>>, Json(payload): Json<BatchRequest>) -> Json<String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let start_ms = now_ms();
        let clients = std::sync::Arc::new(media_core::scraper::ScraperClients::from_settings(&pool).await);
        
        let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
        let script_path = settings.get("post_processing_script").cloned();

        let mut all_tasks = Vec::new();

        if payload.media_type == "movie" {
            let movie_ids: Vec<MovieId> = payload.ids.iter().map(|&id| MovieId(id)).collect();
            if let Ok(movies) = db::queries::get_movies_by_ids(&pool, &movie_ids).await {
                all_tasks.extend(movies.into_iter().map(|m| (m.id.0, m.title, m.year, "movie")));
            }
        } else {
            let show_ids: Vec<TvShowId> = payload.ids.iter().map(|&id| TvShowId(id)).collect();
            if let Ok(shows) = db::queries::get_tv_shows_by_ids(&pool, &show_ids).await {
                all_tasks.extend(shows.into_iter().map(|s| (s.id.0, s.title, None, "tv")));
            }
        }

        let total = all_tasks.len() as i32;
        let pool = Arc::new(pool);
        let task_manager = task_manager.clone();
        let task_id_clone = task_id.clone();

        use futures::StreamExt;
        let stream = futures::stream::iter(all_tasks.into_iter().enumerate());
        
        stream.for_each_concurrent(5, |(i, (id, title, year, m_type))| {
            let clients = clients.clone();
            let pool = pool.clone();
            let task_manager = task_manager.clone();
            let task_id = task_id_clone.clone();
            let title_clone = title.clone();
            let script_path_clone = script_path.clone();
            
            async move {
                if m_type == "movie" {
                    let _ = media_core::scraper::scrape_movie(MovieId(id), &title_clone, year, &clients, &pool, script_path_clone.as_deref()).await;
                } else {
                    let _ = media_core::scraper::scrape_tv_show(TvShowId(id), &title_clone, &clients, &pool, script_path_clone.as_deref()).await;
                }
                
                task_manager.broadcast(media_core::models::TaskUpdate {
                    task_id,
                    status: "running".to_string(),
                    progress: (i + 1) as i32,
                    total,
                    message: format!("Processed: {}", title_clone),
                    started_at: Some(start_ms),
                    finished_at: None,
                    debug_info: Some(format!("Batch Scraper: {} ({})", title_clone, m_type)),
                    files_new: None,
                    files_healed: None,
                    files_missing: None,
                });
            }
        }).await;

        task_manager.broadcast(media_core::models::TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: total,
            total,
            message: "Batch scrape completed".to_string(),
            started_at: Some(start_ms),
            finished_at: Some(media_core::models::now_ms()),
            debug_info: None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    Json("Batch scrape started".to_string())
}

async fn cleanup_duplicates(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Vec<PathBuf>> {
    let libraries = db::queries::get_all_libraries(&state.pool).await.unwrap_or_default();
    if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(id)) {
        let cleanup = CleanupService::new(PathBuf::from(lib.path));
        Json(cleanup.remove_duplicate_artwork().unwrap_or_default())
    } else {
        Json(vec![])
    }
}

async fn cleanup_empty_folders(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Vec<PathBuf>> {
    let libraries = db::queries::get_all_libraries(&state.pool).await.unwrap_or_default();
    if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(id)) {
        let cleanup = CleanupService::new(PathBuf::from(lib.path));
        Json(cleanup.remove_empty_folders().unwrap_or_default())
    } else {
        Json(vec![])
    }
}

async fn cleanup_batch(State(state): State<Arc<AppState>>, Json(payload): Json<BatchRequest>) -> Json<String> {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let start_ms = now_ms();
        let total = payload.ids.len() as i32;
        let mut processed = 0;

        if payload.media_type == "movie" {
            let renamer = media_core::renamer::Renamer::new(None, None);
            for id in &payload.ids {
                processed += 1;
                
                if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, MovieId(*id)).await {
                    let libraries = db::queries::get_all_libraries(&pool).await.unwrap_or_default();
                    if let Some(lib) = libraries.into_iter().find(|l| l.id == movie.library_id) {
                        let lib_root = PathBuf::from(&lib.path);
                        
                        // Get file details
                        let file_info: Option<media_core::models::MovieFile> = sqlx::query_as("SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1")
                            .bind(movie.id)
                            .fetch_optional(&pool)
                            .await
                            .unwrap_or_default();
                        
                        if let Some(mut file) = file_info {
                            let old_path = PathBuf::from(&file.file_path);
                            
                            // Optimization: If resolution is missing, try to get it now
                            if file.resolution.is_none() {
                                let path_for_info = old_path.clone();
                                // Try ffprobe first
                                let res = tokio::task::spawn_blocking(move || {
                                    media_core::scanner::mediainfo::get_media_info(&path_for_info).ok()
                                }).await.unwrap_or_default().map(|i| media_core::models::Resolution::from_dimensions(i.width, i.height));
                                
                                if let Some(r) = res {
                                    let _ = sqlx::query("UPDATE movie_files SET resolution = ? WHERE id = ?")
                                        .bind(&r)
                                        .bind(file.id)
                                        .execute(&pool)
                                        .await;
                                    file.resolution = Some(r);
                                }
                            }

                            // Fetch post processing script
                            let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
                            let script_path = settings.get("post_processing_script").map(|s| s.as_str());

                            // 1. Rename File
                            if let Ok(new_path) = renamer.rename_movie(&movie, &old_path, &lib_root, file.resolution, file.codec.as_deref(), script_path) {
                                let new_path_str = new_path.to_string_lossy().to_string();
                                if new_path_str != file.file_path {
                                    let _ = sqlx::query("UPDATE movie_files SET file_path = ? WHERE id = ?")
                                        .bind(&new_path_str)
                                        .bind(file.id)
                                        .execute(&pool)
                                        .await;
                                    file.file_path = new_path_str;
                                }

                                // 2. Deep Metadata Cleanup
                                let parent = new_path.parent().unwrap_or(&lib_root);
                                let cleanup = CleanupService::new(parent.to_path_buf());
                                
                                // Standard stem for cleanup 
                                let standard_stem = new_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                let _ = cleanup.cleanup_metadata_for_movie(parent, standard_stem);
                                let _ = cleanup.remove_duplicate_artwork();
                            }
                        }
                    }

                    task_manager.broadcast(media_core::models::TaskUpdate {
                        task_id: task_id.clone(),
                        status: "running".to_string(),
                        progress: processed,
                        total,
                        message: format!("Cleaned: {}", movie.title),
                        started_at: Some(start_ms),
                        finished_at: None,
                        debug_info:
 Some(format!("Renaming & Cleaning folder for: {}", movie.title)),
                        files_new: None,
                        files_healed: None,
                        files_missing: None,
                    });
                }
            }
        } else {
            // TV shows - cleanup library path using episodes
            for id in &payload.ids {
                processed += 1;
                let files: Vec<(String,)> = sqlx::query_as(
                    "SELECT e.file_path FROM episodes e JOIN seasons s ON e.season_id = s.id WHERE s.show_id = ?"
                )
                .bind(TvShowId(*id))
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

                // Get unique parent folders
                let mut parents = Vec::new();
                for (path_str,) in files {
                    let p = PathBuf::from(&path_str);
                    if let Some(parent) = p.parent() {
                        // typically season folder, we also want show folder
                        if !parents.contains(&parent.to_path_buf()) {
                            parents.push(parent.to_path_buf());
                        }
                        if let Some(grandparent) = parent.parent() {
                            if !parents.contains(&grandparent.to_path_buf()) {
                                parents.push(grandparent.to_path_buf());
                            }
                        }
                    }
                }

                for parent in parents {
                    let cleanup = CleanupService::new(parent);
                    let _ = cleanup.remove_duplicate_artwork();
                }

                task_manager.broadcast(media_core::models::TaskUpdate {
                    task_id: task_id.clone(),
                    status: "running".to_string(),
                    progress: processed,
                    total,
                    message: format!("Cleaned TV Show ID: {}", id),
                    started_at: Some(start_ms),
                    finished_at: None,
                    debug_info:
 Some(format!("Removing duplicate artwork for TV Show ID: {}", id)),
                    files_new: None,
                    files_healed: None,
                    files_missing: None,
                });
            }
        }

        task_manager.broadcast(media_core::models::TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: total,
            total,
            message: "Batch cleanup completed".to_string(),
            started_at: Some(start_ms),
            finished_at: Some(media_core::models::now_ms()),
            debug_info: None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    Json("Batch cleanup started".to_string())
}

#[derive(serde::Deserialize)]
struct ArtworkQuery { path: String }

async fn get_local_artwork(State(state): State<Arc<AppState>>, Query(query): Query<ArtworkQuery>, req: axum::extract::Request) -> impl IntoResponse {
    let mut path = PathBuf::from(&query.path);
    if !path.exists() {
        // Try resolving relative to all libraries
        if let Ok(libraries) = db::queries::get_all_libraries(&state.pool).await {
            for lib in libraries {
                let abs_path = media_core::paths::make_absolute(&query.path, std::path::Path::new(&lib.path));
                if abs_path.exists() {
                    path = abs_path;
                    break;
                }
            }
        }
    }

    if !path.exists() {
        return (StatusCode::NOT_FOUND, "File not found").into_response();
    }

    use tower::ServiceExt;
    use tower_http::services::ServeFile;
    let mime = if query.path.ends_with(".png") { 
        "image/png" 
    } else if query.path.ends_with(".mp4") {
        "video/mp4"
    } else if query.path.ends_with(".webm") {
        "video/webm"
    } else { 
        "image/jpeg" 
    };
    let service = ServeFile::new(path).precompressed_gzip();
    
    match service.oneshot(req).await {
        Ok(res) => {
            let mut res = res.into_response();
            res.headers_mut().insert(header::CONTENT_TYPE, mime.parse().unwrap());
            res
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn rename_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.pool.clone();
    
    match db::queries::get_movie_by_id(&pool, media_core::models::MovieId(id)).await {
        Ok(Some(movie)) => {
            let movie_id = movie.id;
            let libraries = db::queries::get_all_libraries(&pool).await.unwrap_or_default();
            
            if let Some(lib) = libraries.into_iter().find(|l| l.id == movie.library_id) {
                // Get the file path
                let file_info: Option<media_core::models::MovieFile> = sqlx::query_as("SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1")
                    .bind(movie_id)
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or_default();
                
                if let Some(file) = file_info {
                    let pool_clone = pool.clone();
                    let lib_path = lib.path.clone();
                    let old_path_str = file.file_path.clone();
                    
                    tokio::task::spawn_blocking(move || {
                        let renamer = media_core::renamer::Renamer::new(None, None);
                        let old_path = std::path::PathBuf::from(&old_path_str);
                        let lib_root = std::path::PathBuf::from(&lib_path);
                        
                        // Fetch script path before starting the blocking operation
                        let rt = tokio::runtime::Handle::current();
                        let script_path = rt.block_on(async {
                            let settings = db::queries::get_settings(&pool_clone).await.unwrap_or_default();
                            settings.get("post_processing_script").cloned()
                        });

                        match renamer.rename_movie(&movie, &old_path, &lib_root, file.resolution, file.codec.as_deref(), script_path.as_deref()) {
                            Ok(new_path) => {
                                let new_path_str = new_path.to_string_lossy().to_string();
                                // Update DB in a blocking-safe way
                                tokio::runtime::Handle::current().spawn(async move {
                                    let _ = sqlx::query("UPDATE movie_files SET file_path = ? WHERE id = ?")
                                        .bind(new_path_str)
                                        .bind(file.id)
                                        .execute(&pool_clone)
                                        .await;
                                });
                                Ok::<String, String>("Movie renamed successfully".to_string())
                            }
                            Err(e) => Err(e.to_string())
                        }
                    });
                    
                    return (StatusCode::ACCEPTED, "Rename started").into_response();
                }
            }
            (StatusCode::NOT_FOUND, "Library or file not found").into_response()
        }
        _ => (StatusCode::NOT_FOUND, "Movie not found").into_response()
    }
}

async fn play_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let pool = state.pool.clone();
    tokio::spawn(async move {
        if let Ok(Some(path)) = db::queries::get_movie_full_path(&pool, MovieId(id)).await {
            let _ = opener::open(path);
        }
    });
    Json("Playback started".to_string())
}

async fn play_episode(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let pool = state.pool.clone();
    tokio::spawn(async move {
        if let Ok(Some(path)) = db::queries::get_episode_full_path(&pool, EpisodeId(id)).await {
            let _ = opener::open(path);
        }
    });
    Json("Playback started".to_string())
}

async fn get_genres(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::queries::get_unique_genres(&state.pool).await {
        Ok(genres) => Json(genres).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

async fn get_languages(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::queries::get_unique_languages(&state.pool).await {
        Ok(langs) => Json(langs).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

async fn refresh_metadata(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let start_ms = now_ms();
        let clients = media_core::scraper::ScraperClients::from_settings(&pool).await;
        
        let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
        let script_path = settings.get("post_processing_script").map(|s| s.as_str());

        // Try as movie
        if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, media_core::models::MovieId(id)).await {
            task_manager.broadcast(media_core::models::TaskUpdate {
                task_id: task_id.clone(),
                status: "running".to_string(),
                progress: 0,
                total: 1,
                message: format!("Refreshing movie: {}", movie.title),
                started_at: Some(start_ms),
                finished_at: None,
                debug_info: Some(format!("Refetching TMDB data for: {}", movie.title)),
                files_new: None,
                files_healed: None,
                files_missing: None,
            });

            let _ = media_core::scraper::scrape_movie(movie.id, &movie.title, movie.year, &clients, &pool, script_path).await;
        } else {
            // Try as TV show
            let shows = db::queries::get_all_tv_shows(&pool, None, None, None).await.unwrap_or_default();
            if let Some(show) = shows.into_iter().find(|s| s.id == media_core::models::TvShowId(id)) {
                task_manager.broadcast(media_core::models::TaskUpdate {
                    task_id: task_id.clone(),
                    status: "running".to_string(),
                    progress: 0,
                    total: 1,
                    message: format!("Refreshing TV Show: {}", show.title),
                    started_at: Some(start_ms),
                    finished_at: None,
                    debug_info: Some(format!("Refetching TMDB data for TV Show: {}", show.title)),
                    files_new: None,
                    files_healed: None,
                    files_missing: None,
                });
                let _ = media_core::scraper::scrape_tv_show(show.id, &show.title, &clients, &pool, script_path).await;
            }
        }

        task_manager.broadcast(media_core::models::TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: 1,
            total: 1,
            message: "Metadata refresh complete".to_string(),
            started_at: Some(start_ms),
            finished_at: Some(now_ms()),
            debug_info: None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    StatusCode::ACCEPTED.into_response()
}

async fn download_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>, req: axum::extract::Request) -> impl IntoResponse {
    match db::queries::get_movie_full_path(&state.pool, MovieId(id)).await {
        Ok(Some(path)) => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            use tower::ServiceExt;
            use tower_http::services::ServeFile;
            let service = ServeFile::new(path);
            let mut res = service.oneshot(req).await.unwrap().into_response();
            res.headers_mut().insert(
                axum::http::header::CONTENT_DISPOSITION, 
                format!("attachment; filename=\"{}\"", filename).parse().unwrap()
            );
            res
        },
        _ => (StatusCode::NOT_FOUND, "Movie not found").into_response()
    }
}

async fn download_episode(State(state): State<Arc<AppState>>, Path(id): Path<i64>, req: axum::extract::Request) -> impl IntoResponse {
    match db::queries::get_episode_full_path(&state.pool, EpisodeId(id)).await {
        Ok(Some(path)) => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            use tower::ServiceExt;
            use tower_http::services::ServeFile;
            let service = ServeFile::new(path);
            let mut res = service.oneshot(req).await.unwrap().into_response();
            res.headers_mut().insert(
                axum::http::header::CONTENT_DISPOSITION, 
                format!("attachment; filename=\"{}\"", filename).parse().unwrap()
            );
            res
        },
        _ => (StatusCode::NOT_FOUND, "Episode not found").into_response()
    }
}

async fn search_subtitles(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let api_key = std::env::var("OPENSUBTITLES_API_KEY").unwrap_or_default();

    if api_key.is_empty() {
        return (StatusCode::BAD_REQUEST, "OpenSubtitles API Key missing").into_response();
    }

    tokio::spawn(async move {
        let start_ms = now_ms();
        if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, media_core::models::MovieId(id)).await {
            let file_info: Option<(String,)> = sqlx::query_as("SELECT file_path FROM movie_files WHERE movie_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&pool)
                .await
                .unwrap_or_default();

            if let Some((path_str,)) = file_info {
                let dest_path = std::path::PathBuf::from(path_str);
                let client = media_core::subtitles::SubtitleClient::new(api_key);
                let mut results = None;

                // 1. Try Hash Search
                if let Ok(hash) = media_core::subtitles::compute_opensubtitles_hash(&dest_path) {
                    task_manager.broadcast(media_core::models::TaskUpdate {
                        task_id: task_id.clone(),
                        status: "running".to_string(),
                        progress: 0,
                        total: 1,
                        message: format!("Searching subtitles for: {}", movie.title),
                        started_at: Some(start_ms),
                        finished_at: None,
                        debug_info:
 Some(format!("Querying OpenSubtitles by Hash: {}", hash)),
                        files_new: None,
                        files_healed: None,
                        files_missing: None,
                    });

                    if let Ok(hash_results) = client.search_by_hash(&hash, "en").await {
                        if !hash_results.is_empty() {
                            results = Some(hash_results);
                        }
                    }
                }

                // 2. Try IMDB Fallback
                if results.is_none() {
                    if let Some(imdb_id) = movie.imdb_id {
                        task_manager.broadcast(media_core::models::TaskUpdate {
                            task_id: task_id.clone(),
                            status: "running".to_string(),
                            progress: 0,
                            total: 1,
                            message: format!("Searching subtitles for: {}", movie.title),
                            started_at: Some(start_ms),
                            finished_at: None,
                            debug_info:
 Some(format!("Querying OpenSubtitles for IMDB: {}", imdb_id)),
                            files_new: None,
                            files_healed: None,
                            files_missing: None,
                        });

                        if let Ok(imdb_results) = client.search(&imdb_id, "en").await {
                            if !imdb_results.is_empty() {
                                results = Some(imdb_results);
                            }
                        }
                    }
                }

                match results {
                    Some(res) => {
                        if let Some(best) = res.first() {
                            if let Some(file_id) = best.attributes.files.first().map(|f| f.file_id) {
                                match client.download(file_id, &dest_path, "en").await {
                                    Ok(saved_path) => {
                                        task_manager.broadcast(media_core::models::TaskUpdate {
                                            task_id: task_id.clone(),
                                            status: "completed".to_string(),
                                            progress: 1,
                                            total: 1,
                                            message: format!("Subtitle saved: {}", saved_path),
                                            started_at: Some(start_ms),
                                            finished_at: Some(media_core::models::now_ms()),
                                            debug_info: None,
                                            files_new: None,
                                            files_healed: None,
                                            files_missing: None,
                                            });

                                        return;
                                    }
                                    Err(e) => tracing::error!("Download failed: {}", e),
                                }
                            }
                        }
                        task_manager.broadcast(media_core::models::TaskUpdate {
                            task_id: task_id.clone(),
                            status: "completed".to_string(),
                            progress: 1,
                            total: 1,
                            message: "No matching subtitles found".to_string(),
                            started_at: Some(start_ms),
                            finished_at: Some(media_core::models::now_ms()),
                            debug_info: None,
                            files_new: None,
                            files_healed: None,
                            files_missing: None,
                        });
                    }
                    None => {
                        task_manager.broadcast(media_core::models::TaskUpdate {
                            task_id: task_id.clone(),
                            status: "error".to_string(),
                            progress: 0,
                            total: 1,
                            message: "Subtitle search failed or no matches found".to_string(),
                            started_at: Some(start_ms),
                            finished_at: None,
                            debug_info:
 None,
                            files_new: None,
                            files_healed: None,
                            files_missing: None,
                        });
                    }
                }
            } else {
                task_manager.broadcast(media_core::models::TaskUpdate {
                    task_id: task_id.clone(),
                    status: "error".to_string(),
                    progress: 0,
                    total: 1,
                    message: "Movie file not found".to_string(),
                    started_at: Some(start_ms),
                    finished_at: None,
                    debug_info:
 None,
                    files_new: None,
                    files_healed: None,
                    files_missing: None,
                });
            }
        }
    });

    (StatusCode::ACCEPTED, "Subtitle search started").into_response()
}

async fn export_csv(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    let tv_shows = db::queries::get_all_tv_shows(&state.pool, None, None, None).await.unwrap_or_default();
    let csv = Exporter::to_csv(&movies, &tv_shows);
    (
        [
            (header::CONTENT_TYPE, "text/csv"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"library.csv\""),
        ],
        csv,
    )
}

async fn export_html(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    let tv_shows = db::queries::get_all_tv_shows(&state.pool, None, None, None).await.unwrap_or_default();
    let html = Exporter::to_html(&movies, &tv_shows);
    (
        [(header::CONTENT_TYPE, "text/html")],
        html,
    )
}

async fn export_xlsx(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    let tv_shows = db::queries::get_all_tv_shows(&state.pool, None, None, None).await.unwrap_or_default();
    match Exporter::to_xlsx(&movies, &tv_shows) {
        Ok(bytes) => {
            (
                [
                    (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                    (header::CONTENT_DISPOSITION, "attachment; filename=\"library.xlsx\""),
                ],
                bytes,
            ).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn export_json(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    let tv_shows = db::queries::get_all_tv_shows(&state.pool, None, None, None).await.unwrap_or_default();
    match Exporter::to_json(&movies, &tv_shows) {
        Ok(json) => {
            (
                [
                    (header::CONTENT_TYPE, "application/json"),
                    (header::CONTENT_DISPOSITION, "attachment; filename=\"library.json\""),
                ],
                json,
            ).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn sync_trakt(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let _permit = state.task_manager.acquire_heavy_permit().await;
    
    // Check if Trakt OAuth is configured
    let settings_map = db::queries::get_settings(&state.pool).await.unwrap_or_default();
    let access_token = match settings_map.get("trakt_access_token") {
        Some(t) if !t.is_empty() => t.clone(),
        _ => return (StatusCode::BAD_REQUEST, "Trakt is not authenticated. Please configure Trakt OAuth first.").into_response(),
    };

    let scraper_clients = media_core::scraper::ScraperClients::from_settings(&state.pool).await;

    // Get all movies
    let movies = db::queries::get_all_movies(&state.pool, None, None, None).await.unwrap_or_default();
    
    let mut trakt_movies = Vec::new();
    for m in movies {
        if let Some(tmdb) = m.tmdb_id {
            trakt_movies.push(serde_json::json!({
                "ids": {
                    "tmdb": tmdb,
                    "imdb": m.imdb_id
                }
            }));
        }
    }

    if !trakt_movies.is_empty() {
        match scraper_clients.trakt.add_to_collection(&access_token, trakt_movies).await {
            Ok(res) => (StatusCode::OK, Json(res)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    } else {
        (StatusCode::OK, Json(serde_json::json!({"added": 0}))).into_response()
    }
}

async fn task_stream(State(state): State<Arc<AppState>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.task_manager.subscribe();
    let stream = async_stream::stream! {
        while let Ok(update) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&update) {
                yield Ok(Event::default().data(json));
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn get_tasks(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tasks = state.task_manager.get_history();
    Json(tasks).into_response()
}

#[derive(serde::Deserialize)]
struct UpdateMovieRequest {
    title: String,
    year: Option<i32>,
    plot: Option<String>,
    rating: Option<f32>,
    genres: Option<Vec<String>>,
}

async fn update_movie_metadata(
    State(state): State<Arc<AppState>>, 
    Path(id): Path<i64>, 
    Json(payload): Json<UpdateMovieRequest>
) -> impl IntoResponse {
    let genres_json = payload.genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    match db::queries::update_movie(
        &state.pool, 
        media_core::models::MovieId(id), 
        &payload.title, 
        payload.year, 
        payload.plot.as_deref(), 
        payload.rating, 
        genres_json.as_deref()
    ).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct UpdateTvShowRequest {
    title: String,
    plot: Option<String>,
    rating: Option<f32>,
    genres: Option<Vec<String>>,
    tagline: Option<String>,
    runtime: Option<i32>,
    language: Option<String>,
    trailer_url: Option<String>,
}

async fn update_tv_show_metadata(
    State(state): State<Arc<AppState>>, 
    Path(id): Path<i64>, 
    Json(payload): Json<UpdateTvShowRequest>
) -> impl IntoResponse {
    let genres_json = payload.genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    match db::queries::update_tv_show(
        &state.pool, 
        media_core::models::TvShowId(id), 
        &payload.title, 
        payload.plot.as_deref(), 
        payload.rating, 
        genres_json.as_deref(),
        payload.tagline.as_deref(),
        payload.runtime,
        payload.language.as_deref(),
        payload.trailer_url.as_deref()
    ).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::queries::get_settings(&state.pool).await {
        Ok(settings) => Json(settings).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn set_settings(
    State(state): State<Arc<AppState>>, 
    Json(payload): Json<std::collections::HashMap<String, String>>
) -> impl IntoResponse {
    for (key, value) in payload {
        if let Err(e) = db::queries::set_setting(&state.pool, &key, &value).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }
    StatusCode::OK.into_response()
}

async fn process_movie_advanced(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let _permit = task_manager.acquire_heavy_permit().await;
        let start_ms = now_ms();
        if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, MovieId(id)).await {
            let libraries = db::queries::get_all_libraries(&pool).await.unwrap_or_default();
            if let (Some(lib), Ok(Some(path))) = (libraries.into_iter().find(|l| l.id == movie.library_id), db::queries::get_movie_full_path(&pool, movie.id).await) {
                let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM movie_files WHERE movie_id = ? LIMIT 1").bind(id).fetch_optional(&pool).await.unwrap_or_default();
                if let Some((file_id,)) = row {
                    task_manager.broadcast(media_core::models::TaskUpdate {
                        task_id: task_id.clone(),
                        status: "running".to_string(),
                        progress: 0,
                        total: 1,
                        message: format!("Analyzing movie: {}...", movie.title),
                        started_at: Some(start_ms),
                        finished_at: None,
                        debug_info:
 Some("Running FFmpeg cropdetect and thumbnail extraction...".to_string()),
                        files_new: None,
                        files_healed: None,
                        files_missing: None,
                    });

                    if path.exists() {
                        let folder = path.parent().unwrap();
                        let thumb_dest = folder.join(format!("{}.thumb.jpg", path.file_stem().unwrap().to_str().unwrap()));
                        let lib_root = std::path::Path::new(&lib.path);
                        
                        // 1. Detect Ratio
                        let ratio = media_core::scanner::ffmpeg::FfmpegEngine::detect_aspect_ratio(&path).ok();
                        
                        // 2. Extract Thumb
                        let thumb = media_core::scanner::ffmpeg::FfmpegEngine::extract_thumbnail(&path, &thumb_dest, "00:05:00").ok();

                        // 3. Generate Preview (Stash Style)
                        let preview_dest = folder.join(format!("{}.preview.mp4", path.file_stem().unwrap().to_str().unwrap()));
                        let preview = media_core::scanner::ffmpeg::FfmpegEngine::generate_preview(&path, &preview_dest).ok();

                        // Relativize paths for DB
                        let rel_thumb = thumb.as_ref().and_then(|p| {
                            media_core::paths::make_relative(p, lib_root).ok()
                        });
                        let rel_preview = preview.as_ref().and_then(|p| {
                            media_core::paths::make_relative(p, lib_root).ok()
                        });

                        let _ = sqlx::query("UPDATE movie_files SET aspect_ratio = ?, thumbnail_path = ?, preview_path = ? WHERE id = ?")
                            .bind(ratio)
                            .bind(rel_thumb)
                            .bind(rel_preview)
                            .bind(file_id)
                            .execute(&pool)
                            .await;
                    }
                }
            }
        }
        
        task_manager.broadcast(media_core::models::TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: 1,
            total: 1,
            message: "Advanced analysis complete".to_string(),
            started_at: Some(start_ms),
            finished_at: None,
            debug_info:
 None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    StatusCode::ACCEPTED
}

async fn process_tv_show_advanced(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let start_ms = now_ms();
        let seasons = db::queries::get_seasons_by_show_id(&pool, TvShowId(id)).await.unwrap_or_default();
        let mut all_episodes = Vec::new();
        for s in seasons {
            let eps = db::queries::get_episodes_by_season_id(&pool, s.id).await.unwrap_or_default();
            all_episodes.extend(eps);
        }

        let total = all_episodes.len() as i32;
        
        // Find the library for this show to get root path
        let show: Option<(i64,)> = sqlx::query_as("SELECT library_id FROM tv_shows WHERE id = ?").bind(id).fetch_optional(&pool).await.unwrap_or_default();
        let mut lib_root_opt = None;
        if let Some((lib_id,)) = show {
            if let Ok(libraries) = db::queries::get_all_libraries(&pool).await {
                if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(lib_id)) {
                    lib_root_opt = Some(PathBuf::from(lib.path));
                }
            }
        }

        if let Some(lib_root) = lib_root_opt {
            for (i, ep) in all_episodes.into_iter().enumerate() {
                let _permit = task_manager.acquire_heavy_permit().await;
                
                task_manager.broadcast(media_core::models::TaskUpdate {
                    task_id: task_id.clone(),
                    status: "running".to_string(),
                    progress: i as i32,
                    total,
                    message: format!("Analyzing Ep {}/{}", i+1, total),
                    started_at: Some(start_ms),
                    finished_at: None,
                    debug_info:
 Some(format!("FFmpeg deep analysis for: {}", ep.original_name)),
                    files_new: None,
                    files_healed: None,
                    files_missing: None,
                });

                if let Ok(Some(path)) = db::queries::get_episode_full_path(&pool, ep.id).await {
                    if path.exists() {
                        let folder = path.parent().unwrap();
                        let thumb_dest = folder.join(format!("{}.thumb.jpg", path.file_stem().unwrap().to_str().unwrap()));
                        
                        let ratio = media_core::scanner::ffmpeg::FfmpegEngine::detect_aspect_ratio(&path).ok();
                        let thumb = media_core::scanner::ffmpeg::FfmpegEngine::extract_thumbnail(&path, &thumb_dest, "00:05:00").ok();
                        
                        let preview_dest = folder.join(format!("{}.preview.mp4", path.file_stem().unwrap().to_str().unwrap()));
                        let preview = media_core::scanner::ffmpeg::FfmpegEngine::generate_preview(&path, &preview_dest).ok();

                        let rel_thumb = thumb.as_ref().and_then(|p| {
                            media_core::paths::make_relative(p, &lib_root).ok()
                        });
                        let rel_preview = preview.as_ref().and_then(|p| {
                            media_core::paths::make_relative(p, &lib_root).ok()
                        });

                        let _ = sqlx::query("UPDATE episodes SET aspect_ratio = ?, thumbnail_path = ?, preview_path = ? WHERE id = ?")
                            .bind(ratio)
                            .bind(rel_thumb)
                            .bind(rel_preview)
                            .bind(ep.id)
                            .execute(&pool)
                            .await;
                    }
                }
            }
        }
        
        task_manager.broadcast(media_core::models::TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: total,
            total,
            message: "TV Show deep analysis complete".to_string(),
            started_at: Some(start_ms),
            finished_at: None,
            debug_info:
 None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    StatusCode::ACCEPTED
}

async fn process_library_advanced(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let pool = state.pool.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let start_ms = now_ms();
        let libraries = db::queries::get_all_libraries(&pool).await.unwrap_or_default();
        let lib = match libraries.into_iter().find(|l| l.id == LibraryId(id)) {
            Some(l) => l,
            None => return,
        };
        let lib_root = PathBuf::from(&lib.path);
        
        // 1. Process Movies
        if let Ok(movies) = db::queries::get_all_movies(&pool, Some(lib.id), None, None).await {
            let total = movies.len() as i32;
            for (i, movie) in movies.into_iter().enumerate() {
                let _permit = task_manager.acquire_heavy_permit().await;
                let file_info: Option<media_core::models::MovieFile> = sqlx::query_as("SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1")
                    .bind(movie.id)
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or_default();

                if let Some(file) = file_info {
                    task_manager.broadcast(media_core::models::TaskUpdate {
                        task_id: task_id.clone(),
                        status: "running".to_string(),
                        progress: i as i32,
                        total,
                        message: format!("Movies: {}/{}", i+1, total),
                        started_at: Some(start_ms),
                        finished_at: None,
                        debug_info:
 Some(format!("Analyzing: {}", movie.title)),
                        files_new: None,
                        files_healed: None,
                        files_missing: None,
                    });

                    let input_path = media_core::paths::make_absolute(&file.file_path, &lib_root);
                    if input_path.exists() {
                        let folder = input_path.parent().unwrap();
                        let thumb_dest = folder.join(format!("{}.thumb.jpg", input_path.file_stem().unwrap().to_str().unwrap()));
                        
                        let ratio = media_core::scanner::ffmpeg::FfmpegEngine::detect_aspect_ratio(&input_path).ok();
                        let thumb = media_core::scanner::ffmpeg::FfmpegEngine::extract_thumbnail(&input_path, &thumb_dest, "00:05:00").ok();
                        
                        let preview_dest = folder.join(format!("{}.preview.mp4", input_path.file_stem().unwrap().to_str().unwrap()));
                        let preview = media_core::scanner::ffmpeg::FfmpegEngine::generate_preview(&input_path, &preview_dest).ok();

                        let rel_thumb = thumb.as_ref().and_then(|p| media_core::paths::make_relative(p, &lib_root).ok());
                        let rel_preview = preview.as_ref().and_then(|p| media_core::paths::make_relative(p, &lib_root).ok());

                        let _ = sqlx::query("UPDATE movie_files SET aspect_ratio = ?, thumbnail_path = ?, preview_path = ? WHERE id = ?")
                            .bind(ratio)
                            .bind(rel_thumb)
                            .bind(rel_preview)
                            .bind(file.id)
                            .execute(&pool)
                            .await;
                    }
                }
            }
        }

        // 2. Process TV Shows
        if let Ok(shows) = db::queries::get_all_tv_shows(&pool, Some(lib.id), None, None).await {
            let total_shows = shows.len();
            for (si, show) in shows.into_iter().enumerate() {
                let seasons = db::queries::get_seasons_by_show_id(&pool, show.id).await.unwrap_or_default();
                for s in seasons {
                    let eps = db::queries::get_episodes_by_season_id(&pool, s.id).await.unwrap_or_default();
                    let ep_total = eps.len();
                    for (ei, ep) in eps.into_iter().enumerate() {
                        let _permit = task_manager.acquire_heavy_permit().await;
                        
                        task_manager.broadcast(media_core::models::TaskUpdate {
                            task_id: task_id.clone(),
                            status: "running".to_string(),
                            progress: si as i32,
                            total: total_shows as i32,
                            message: format!("Show {}/{}, Ep {}/{}", si+1, total_shows, ei+1, ep_total),
                            started_at: Some(start_ms),
                            finished_at: None,
                            debug_info:
 Some(format!("Analyzing: {} - {}", show.title, ep.original_name)),
                            files_new: None,
                            files_healed: None,
                            files_missing: None,
                        });

                        let input_path = media_core::paths::make_absolute(&ep.file_path, &lib_root);
                        if input_path.exists() {
                            let folder = input_path.parent().unwrap();
                            let thumb_dest = folder.join(format!("{}.thumb.jpg", input_path.file_stem().unwrap().to_str().unwrap()));
                            
                            let ratio = media_core::scanner::ffmpeg::FfmpegEngine::detect_aspect_ratio(&input_path).ok();
                            let thumb = media_core::scanner::ffmpeg::FfmpegEngine::extract_thumbnail(&input_path, &thumb_dest, "00:05:00").ok();
                            
                            let preview_dest = folder.join(format!("{}.preview.mp4", input_path.file_stem().unwrap().to_str().unwrap()));
                            let preview = media_core::scanner::ffmpeg::FfmpegEngine::generate_preview(&input_path, &preview_dest).ok();

                            let rel_thumb = thumb.as_ref().and_then(|p| media_core::paths::make_relative(p, &lib_root).ok());
                            let rel_preview = preview.as_ref().and_then(|p| media_core::paths::make_relative(p, &lib_root).ok());

                            let _ = sqlx::query("UPDATE episodes SET aspect_ratio = ?, thumbnail_path = ?, preview_path = ? WHERE id = ?")
                                .bind(ratio)
                                .bind(rel_thumb)
                                .bind(rel_preview)
                                .bind(ep.id)
                                .execute(&pool)
                                .await;
                        }
                    }
                }
            }
        }
        
        task_manager.broadcast(media_core::models::TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: 1, 
            total: 1,
            message: "Library analysis complete".to_string(),
            started_at: Some(start_ms),
            finished_at: Some(media_core::models::now_ms()),
            debug_info: None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    StatusCode::ACCEPTED
}

async fn create_backup(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let backup_dir = std::path::Path::new("backups");

    // First export all NFOs
    let _ = media_core::maintenance::MaintenanceEngine::export_all_nfos(&state.pool).await;

    match media_core::maintenance::MaintenanceEngine::create_backup(&state.pool, backup_dir).await {
        Ok(path) => (StatusCode::OK, format!("Backup created: {:?}", path)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
async fn check_updates() -> impl IntoResponse {
    match media_core::maintenance::MaintenanceEngine::check_for_updates().await {
        Ok(version) => Json(serde_json::json!({ "latest_version": version, "current_version": "0.1.0" })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/* 
async fn start_streaming(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let output_dir = PathBuf::from("transcodes").join(id.to_string());
    
    // Clean up previous transcodes for this ID
    if output_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&output_dir).await;
    }
    let _ = tokio::fs::create_dir_all(&output_dir).await;

    // Try as movie
    if let Ok(Some(path)) = db::queries::get_movie_full_path(&state.pool, MovieId(id)).await {
        match media_core::scanner::ffmpeg::FfmpegEngine::create_hls_stream(&path, &output_dir) {
            Ok(playlist_path) => {
                // Wait for playlist to be created and non-empty
                let mut attempts = 0;
                while attempts < 30 { // Wait up to 15 seconds
                    if playlist_path.exists() {
                        if let Ok(meta) = std::fs::metadata(&playlist_path) {
                            if meta.len() > 0 {
                                // Small extra delay to ensure the first segments are being written
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                return (StatusCode::OK, Json(format!("/stream/{}/hls/playlist.m3u8", id))).into_response();
                            }
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    attempts += 1;
                }
                return (StatusCode::INTERNAL_SERVER_ERROR, "Streaming failed to start: playlist timeout").into_response();
            },
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }
    
    // Try as episode
    if let Ok(Some(path)) = db::queries::get_episode_full_path(&state.pool, EpisodeId(id)).await {
        match media_core::scanner::ffmpeg::FfmpegEngine::create_hls_stream(&path, &output_dir) {
            Ok(playlist_path) => {
                // Wait for playlist to be created and non-empty
                let mut attempts = 0;
                while attempts < 30 { // Wait up to 15 seconds
                    if playlist_path.exists() {
                        if let Ok(meta) = std::fs::metadata(&playlist_path) {
                            if meta.len() > 0 {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                return (StatusCode::OK, Json(format!("/stream/{}/hls/playlist.m3u8", id))).into_response();
                            }
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    attempts += 1;
                }
                return (StatusCode::INTERNAL_SERVER_ERROR, "Streaming failed to start: playlist timeout").into_response();
            },
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }
    
    (StatusCode::NOT_FOUND, "Media not found").into_response()
}

async fn serve_stream(Path((id, file)): Path<(i64, String)>) -> impl IntoResponse {
    let path = PathBuf::from("transcodes").join(id.to_string()).join(&file);
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "File not found").into_response();
    }

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = if file.ends_with(".m3u8") { "application/vnd.apple.mpegurl" } else { "video/mp2t" };
            (
                [(header::CONTENT_TYPE, mime)],
                bytes,
            ).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
*/

#[derive(serde::Deserialize)]
struct PlaybackHeartbeat {
    media_id: i64,
    media_type: String,
    position_ms: i32,
    duration_ms: i32,
    is_finished: bool,
}

async fn update_playback_progress(State(state): State<Arc<AppState>>, Json(payload): Json<PlaybackHeartbeat>) -> impl IntoResponse {
    // MVP 2: Update stream manager heartbeat to keep FFmpeg alive
    let stream_id = if payload.media_type == "movie" {
        format!("movie_{}", payload.media_id)
    } else {
        format!("episode_{}", payload.media_id)
    };
    state.stream_manager.update_heartbeat(&stream_id).await;

    match sqlx::query(
        r#"
        INSERT INTO playback_state (media_id, media_type, position_ms, duration_ms, is_finished, updated_at)
        VALUES (?, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(media_id, media_type) DO UPDATE SET
            position_ms = excluded.position_ms,
            duration_ms = excluded.duration_ms,
            is_finished = excluded.is_finished,
            updated_at = excluded.updated_at
        "#
    )
    .bind(payload.media_id)
    .bind(&payload.media_type)
    .bind(payload.position_ms)
    .bind(payload.duration_ms)
    .bind(payload.is_finished)
    .execute(&state.pool)
    .await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_playback_status(State(state): State<Arc<AppState>>, Path((m_type, id)): Path<(String, i64)>) -> impl IntoResponse {
    let res: Option<(i32, i32, bool)> = sqlx::query_as("SELECT position_ms, duration_ms, is_finished FROM playback_state WHERE media_id = ? AND media_type = ?")
        .bind(id)
        .bind(m_type)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or_default();

    match res {
        Some((pos, dur, finished)) => Json(serde_json::json!({ "position_ms": pos, "duration_ms": dur, "is_finished": finished })).into_response(),
        None => Json(serde_json::json!({ "position_ms": 0, "duration_ms": 0, "is_finished": false })).into_response(),
    }
}
async fn start_movie_stream(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    tracing::info!("Stream requested for movie ID: {}", id);
    
    let file_info: Option<(String, Option<String>)> = sqlx::query_as("SELECT file_path, codec FROM movie_files WHERE movie_id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or_default();

    if let Some((path_str, codec)) = file_info {
        let path = if let Ok(Some(full_path)) = db::queries::get_movie_full_path(&state.pool, MovieId(id)).await {
            full_path
        } else {
            PathBuf::from(&path_str)
        };
        
        // Direct Play Check
        let is_mp4 = path.extension().and_then(|e| e.to_str()).unwrap_or("").eq_ignore_ascii_case("mp4");
        let codec_str = codec.as_deref().unwrap_or("").to_lowercase();
        let is_compatible_codec = codec_str == "h264" || codec_str == "hvc1" || codec_str == "hevc" || codec_str == "avc1";

        if is_mp4 && is_compatible_codec {
            tracing::info!("Direct play enabled for movie ID: {}", id);
            return (StatusCode::OK, Json(format!("/api/stream/direct/movie/{}", id))).into_response();
        }

        tracing::debug!("Found path for HLS streaming: {:?}", path);
        let stream_id = format!("movie_{}", id);

        match state.stream_manager.start_hls(&stream_id, &path).await {
            Ok(_) => {
                tracing::info!("HLS Stream started successfully for {}", stream_id);
                (StatusCode::OK, Json(format!("/api/stream/hls/{}/playlist.m3u8", stream_id))).into_response()
            },
            Err(e) => {
                tracing::error!("HLS Stream failed to start: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            },
        }
    } else {
        tracing::error!("Movie ID {} not found or has no associated file", id);
        (StatusCode::NOT_FOUND, "Movie not found").into_response()
    }
}

async fn start_episode_stream(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    tracing::info!("Stream requested for episode ID: {}", id);

    let file_info: Option<(String, Option<String>)> = sqlx::query_as("SELECT file_path, codec FROM episodes WHERE id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or_default();

    if let Some((path_str, codec)) = file_info {
        let path = if let Ok(Some(full_path)) = db::queries::get_episode_full_path(&state.pool, EpisodeId(id)).await {
            full_path
        } else {
            PathBuf::from(&path_str)
        };
        
        // Direct Play Check
        let is_mp4 = path.extension().and_then(|e| e.to_str()).unwrap_or("").eq_ignore_ascii_case("mp4");
        let codec_str = codec.as_deref().unwrap_or("").to_lowercase();
        let is_compatible_codec = codec_str == "h264" || codec_str == "hvc1" || codec_str == "hevc" || codec_str == "avc1";

        if is_mp4 && is_compatible_codec {
            tracing::info!("Direct play enabled for episode ID: {}", id);
            return (StatusCode::OK, Json(format!("/api/stream/direct/episode/{}", id))).into_response();
        }

        let stream_id = format!("episode_{}", id);

        match state.stream_manager.start_hls(&stream_id, &path).await {
            Ok(_) => {
                (StatusCode::OK, Json(format!("/api/stream/hls/{}/playlist.m3u8", stream_id))).into_response()
            },
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "Episode not found").into_response()
    }
}

async fn serve_direct_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>, req: axum::extract::Request) -> impl IntoResponse {
    if let Ok(Some(path)) = db::queries::get_movie_full_path(&state.pool, MovieId(id)).await {
        use tower::ServiceExt;
        tower_http::services::ServeFile::new(path).oneshot(req).await.unwrap().into_response()
    } else {
        (StatusCode::NOT_FOUND, "Movie not found").into_response()
    }
}

async fn serve_direct_episode(State(state): State<Arc<AppState>>, Path(id): Path<i64>, req: axum::extract::Request) -> impl IntoResponse {
    if let Ok(Some(path)) = db::queries::get_episode_full_path(&state.pool, EpisodeId(id)).await {
        use tower::ServiceExt;
        tower_http::services::ServeFile::new(path).oneshot(req).await.unwrap().into_response()
    } else {
        (StatusCode::NOT_FOUND, "Episode not found").into_response()
    }
}

async fn serve_stream_file(
    State(state): State<Arc<AppState>>,
    Path((id, file)): Path<(String, String)>
) -> impl IntoResponse {
    tracing::info!("Stream file requested: {}/{}", id, file);

    if file == "playlist.m3u8" {
        let (m_type, m_id) = if id.starts_with("movie_") {
            ("movie", id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0))
        } else if id.starts_with("episode_") {
            ("episode", id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0))
        } else {
            ("", 0)
        };

        if m_id > 0 {
            let mut duration: Option<i32> = if m_type == "movie" {
                sqlx::query_scalar("SELECT duration_secs FROM movie_files WHERE movie_id = ?")
                    .bind(m_id)
                    .fetch_optional(&state.pool)
                    .await
                    .unwrap_or(None)
            } else {
                sqlx::query_scalar("SELECT duration_secs FROM episodes WHERE id = ?")
                    .bind(m_id)
                    .fetch_optional(&state.pool)
                    .await
                    .unwrap_or(None)
            };

            // Fallback to ffprobe if duration missing or zero in DB
            if duration.unwrap_or(0) <= 0 {
                tracing::info!("Duration for {} is {} in DB, attempting ffprobe fallback...", id, duration.unwrap_or(0));
                let path = if m_type == "movie" {
                    db::queries::get_movie_full_path(&state.pool, MovieId(m_id)).await.ok().flatten()
                } else {
                    db::queries::get_episode_full_path(&state.pool, EpisodeId(m_id)).await.ok().flatten()
                };

                if let Some(p) = path {
                    tracing::info!("FFprobe fallback: checking file {:?}", p);
                    match media_core::scanner::mediainfo::get_media_info(&p) {
                        Ok(info) => {
                            let found_dur = info.duration_secs as i32;
                            tracing::info!("FFprobe found duration: {}s for {:?}", found_dur, p);
                            if found_dur > 0 {
                                duration = Some(found_dur);
                                // Update DB so we don't have to ffprobe every time
                                let pool = state.pool.clone();
                                let m_type_clone = m_type.to_string();
                                tokio::spawn(async move {
                                    if m_type_clone == "movie" {
                                        let _ = sqlx::query("UPDATE movie_files SET duration_secs = ? WHERE movie_id = ?")
                                            .bind(found_dur)
                                            .bind(m_id)
                                            .execute(&pool)
                                            .await;
                                    } else {
                                        let _ = sqlx::query("UPDATE episodes SET duration_secs = ? WHERE id = ?")
                                            .bind(found_dur)
                                            .bind(m_id)
                                            .execute(&pool)
                                            .await;
                                    }
                                });
                            }
                        },
                        Err(e) => {
                            tracing::error!("FFprobe fallback failed for {:?}: {}", p, e);
                        }
                    }
                } else {
                    tracing::error!("FFprobe fallback failed: could not resolve path for {} ID {}", m_type, m_id);
                }
            }

            if let Some(dur) = duration {
                tracing::info!("Generating in-memory manifest for {} ({}s)", id, dur);
                let manifest = generate_hls_manifest(&id, dur as f64);
                return (
                    [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
                    manifest,
                ).into_response();
            } else {
                tracing::warn!("Could not determine duration for {}, manifest generation failed", id);
            }
        }
    }

    // Use configured transcode directory
    let transcode_dir = media_core::config::get_hls_transcode_dir();
    let base_dir = PathBuf::from(&transcode_dir).join(&id);
    let file_path = base_dir.join(&file);

    if file.ends_with(".ts") {
        // Extract segment index
        let segment_index = file
            .strip_prefix("seg_")
            .and_then(|s| s.strip_suffix(".ts"))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        let m_path = if id.starts_with("movie_") {
            let m_id = id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0);
            db::queries::get_movie_full_path(&state.pool, MovieId(m_id)).await.ok().flatten()
        } else if id.starts_with("episode_") {
            let e_id = id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0);
            db::queries::get_episode_full_path(&state.pool, EpisodeId(e_id)).await.ok().flatten()
        } else {
            None
        };

        if let Some(path) = m_path {
            let _ = state.stream_manager.request_segment(&id, &path, segment_index).await;
        }

        // Wait for segment via tokio watch channel
        let _ = state.stream_manager.wait_for_segment(&id, segment_index).await;
    }

    if !file_path.exists() {
        return (StatusCode::NOT_FOUND, "Stream file not found").into_response();
    }

    match tokio::fs::read(&file_path).await {
        Ok(bytes) => {
            let mime = if file.ends_with(".m3u8") {
                "application/vnd.apple.mpegurl"
            } else if file.ends_with(".ts") {
                "video/mp2t"
            } else {
                "application/octet-stream"
            };

            (
                [(header::CONTENT_TYPE, mime)],
                bytes,
            ).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
