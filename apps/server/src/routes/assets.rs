use axum::{
    extract::{State, Path, Query},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::path::PathBuf;
use media_core::models::{MovieId, TvShowId, LibraryId};
use media_core::task_manager::ProgressSink;
use crate::state::AppState;
use crate::utils;
use uuid::Uuid;
use media_core::db::{LibraryReader, MovieReader, MovieWriter, TvReader, TvWriter, MediaRepository};

#[derive(serde::Deserialize)]
pub struct ArtworkQuery { 
    pub path: String 
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/artwork/local", get(get_local_artwork))
        .route("/assets/:hash/:type", get(get_asset_handler))
        .route("/movies/:id/process-advanced", post(process_movie_advanced))
        .route("/tvshows/:id/process-advanced", post(process_tv_show_advanced))
        .route("/libraries/:id/process-advanced", post(process_library_advanced))
}

pub async fn get_local_artwork(State(state): State<Arc<AppState>>, Query(query): Query<ArtworkQuery>) -> impl IntoResponse {
    let normalized_query_path = media_core::paths::normalize_slashes(&query.path);
    let mut path = PathBuf::from(&normalized_query_path);
    tracing::debug!("Artwork request for path: {:?}", path);

    if !path.exists() {
        // Try resolving relative to all libraries
        if let Ok(libraries) = state.repos.library.find_all().await {
            for lib in libraries {
                let abs_path = media_core::paths::make_absolute(&normalized_query_path, std::path::Path::new(&lib.path));
                if abs_path.exists() {
                    path = abs_path;
                    break;
                }
            }
        }
    }

    if !path.exists() {
        tracing::warn!("Artwork not found: {:?}", query.path);
        return (StatusCode::NOT_FOUND, "File not found").into_response();
    }

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = if normalized_query_path.ends_with(".png") { 
                "image/png" 
            } else if normalized_query_path.ends_with(".mp4") {
                "video/mp4"
            } else if normalized_query_path.ends_with(".webm") {
                "video/webm"
            } else if normalized_query_path.ends_with(".webp") {
                "image/webp"
            } else if normalized_query_path.ends_with(".svg") {
                "image/svg+xml"
            } else { 
                "image/jpeg" 
            };
            
            (
                [(header::CONTENT_TYPE, mime)],
                bytes,
            ).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to read artwork file {:?}: {}", path, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

pub async fn get_asset_handler(Path((hash, a_type)): Path<(String, String)>) -> impl IntoResponse {
    let ext = match a_type.as_str() {
        "sprite" => "webp",
        "preview" => "mp4",
        "vtt" => "vtt",
        "thumb" => "jpg",
        _ => "jpg",
    };
    
    let path = std::path::PathBuf::from("data/generated").join(&hash).join(format!("{}.{}", a_type, ext));
    
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "Asset not found").into_response();
    }

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = match ext {
                "webp" => "image/webp",
                "mp4" => "video/mp4",
                "vtt" => "text/vtt",
                _ => "image/jpeg",
            };
            ([(header::CONTENT_TYPE, mime)], bytes).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn process_movie_advanced(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let _permit = task_manager.acquire_heavy_permit().await;
        let start_ms = utils::now_ms();
        if let Ok(Some(movie)) = repos.movie.find_by_id(MovieId(id)).await {
            let file_info = repos.movie.find_file_by_movie_id(movie.id).await.unwrap_or_default();

            if let Some(file) = file_info {
                if let Ok(Some(path)) = repos.movie.get_full_path(movie.id).await {
                    task_manager.broadcast(media_core::models::TaskUpdate {
                        task_id: task_id.clone(),
                        status: "running".to_string(),
                        progress: 0,
                        total: 1,
                        message: format!("Analyzing movie: {}...", movie.title),
                        started_at: Some(start_ms),
                        finished_at: None,
                        debug_info: Some("Generating centralized sprites, previews, and thumbnails...".to_string()),
                        files_new: None,
                        files_healed: None,
                        files_missing: None,
                    });

                    if path.exists() {
                        // Ensure fingerprint exists
                        let fingerprint = match file.fingerprint {
                            Some(f) if !f.is_empty() => f,
                            _ => {
                                let f = media_core::scanner::hash::calculate_oshash(&path).unwrap_or_default();
                                let _ = repos.movie.update_file_fingerprint(file.id, &f).await;
                                f
                            }
                        };

                        if !fingerprint.is_empty() {
                            let generated_root = std::path::Path::new("data/generated");
                            let duration = file.duration_secs.unwrap_or(0) as f64;
                            
                            // Generate Assets (Centralized)
                            if let Ok(_) = media_core::scanner::ffmpeg::FfmpegEngine::generate_advanced_assets(&path, &fingerprint, generated_root, duration) {
                                // Record in generated_assets table
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
        
        task_manager.broadcast(media_core::models::TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: 1,
            total: 1,
            message: "Advanced analysis complete".to_string(),
            started_at: Some(start_ms),
            finished_at: Some(utils::now_ms()),
            debug_info: None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    StatusCode::ACCEPTED
}

pub async fn process_tv_show_advanced(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let start_ms = utils::now_ms();
        let seasons = repos.tv.find_seasons_by_show_id(TvShowId(id)).await.unwrap_or_default();
        let mut all_episodes = Vec::new();
        for s in seasons {
            let eps = repos.tv.find_episodes_by_season_id(s.id).await.unwrap_or_default();
            all_episodes.extend(eps);
        }

        let total = all_episodes.len() as i32;
        
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
                debug_info: Some(format!("Generating assets for: {}", ep.original_name)),
                files_new: None,
                files_healed: None,
                files_missing: None,
            });

            if let Ok(Some(path)) = repos.tv.get_episode_full_path(ep.id).await {
                if path.exists() {
                    // Ensure fingerprint exists
                    let fingerprint = match ep.fingerprint {
                        Some(f) if !f.is_empty() => f,
                        _ => {
                            let f = media_core::scanner::hash::calculate_oshash(&path).unwrap_or_default();
                            let _ = repos.tv.update_episode_fingerprint(ep.id, &f).await;
                            f
                        }
                    };

                    if !fingerprint.is_empty() {
                        let generated_root = std::path::Path::new("data/generated");
                        let duration = ep.duration_secs.unwrap_or(0) as f64;
                        
                        if let Ok(_) = media_core::scanner::ffmpeg::FfmpegEngine::generate_advanced_assets(&path, &fingerprint, generated_root, duration) {
                            let _ = repos.media.upsert_generated_asset(&fingerprint, "thumb", &format!("{}/thumb.jpg", fingerprint)).await;
                            let _ = repos.media.upsert_generated_asset(&fingerprint, "sprite", &format!("{}/sprite.webp", fingerprint)).await;
                            let _ = repos.media.upsert_generated_asset(&fingerprint, "vtt", &format!("{}/sprite.vtt", fingerprint)).await;
                            let _ = repos.media.upsert_generated_asset(&fingerprint, "preview", &format!("{}/preview.mp4", fingerprint)).await;
                        }
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
            finished_at: Some(utils::now_ms()),
            debug_info: None,
            files_new: None,
            files_healed: None,
            files_missing: None,
        });
    });

    StatusCode::ACCEPTED
}

pub async fn process_library_advanced(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let start_ms = utils::now_ms();
        let libraries = repos.library.find_all().await.unwrap_or_default();
        let lib = match libraries.into_iter().find(|l| l.id == LibraryId(id)) {
            Some(l) => l,
            None => return,
        };
        let lib_root = PathBuf::from(&lib.path);
        
        // 1. Process Movies
        if let Ok(movies) = repos.movie.find_all(Some(lib.id), None, None).await {
            let total = movies.len() as i32;
            for (i, movie) in movies.into_iter().enumerate() {
                let _permit = task_manager.acquire_heavy_permit().await;
                let file_info = repos.movie.find_file_by_movie_id(movie.id).await.unwrap_or_default();

                if let Some(file) = file_info {
                    task_manager.broadcast(media_core::models::TaskUpdate {
                        task_id: task_id.clone(),
                        status: "running".to_string(),
                        progress: i as i32,
                        total,
                        message: format!("Movies: {}/{}", i+1, total),
                        started_at: Some(start_ms),
                        finished_at: None,
                        debug_info: Some(format!("Analyzing: {}", movie.title)),
                        files_new: None,
                        files_healed: None,
                        files_missing: None,
                    });

                    let input_path = media_core::paths::make_absolute(&file.file_path, &lib_root);
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

        // 2. Process TV Shows
        if let Ok(shows) = repos.tv.find_all_shows(Some(lib.id), None, None).await {
            let total_shows = shows.len();
            for (si, show) in shows.into_iter().enumerate() {
                let seasons = repos.tv.find_seasons_by_show_id(show.id).await.unwrap_or_default();
                for s in seasons {
                    let eps = repos.tv.find_episodes_by_season_id(s.id).await.unwrap_or_default();
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
                            debug_info: Some(format!("Analyzing: {} - {}", show.title, ep.original_name)),
                            files_new: None,
                            files_healed: None,
                            files_missing: None,
                        });

                        let input_path = media_core::paths::make_absolute(&ep.file_path, &lib_root);
                        if input_path.exists() {
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
