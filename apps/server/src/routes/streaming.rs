use axum::{
    routing::{get, post},
    Router,
    extract::{State, Path, Query},
    Json,
    response::IntoResponse,
    http::{header, StatusCode},
};
use std::sync::Arc;
use std::path::PathBuf;
use crate::state::AppState;
use crate::utils;
use media_core::models::{MovieId, EpisodeId};
use media_core::task_manager::ProgressSink;
use tower::ServiceExt;
use tower_http::services::ServeFile;
use media_core::db::{MovieReader, MovieWriter, TvReader, TvWriter};
use tokio_util::io::ReaderStream;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/movies/:id/play", post(play_movie))
        .route("/episodes/:id/play", post(play_episode))
        .route("/stream/movie/:id", post(start_movie_stream))
        .route("/stream/episode/:id", post(start_episode_stream))
        .route("/stream/direct/movie/:id", get(serve_direct_movie))
        .route("/stream/direct/episode/:id", get(serve_direct_episode))
        .route("/stream/hls/:id/:file", get(serve_stream_file))
        .route("/stream/dash/:id/manifest.mpd", get(serve_dash_manifest))
        .route("/stream/dash/:id/:file", get(serve_stream_file))
        .route("/playback/heartbeat", post(update_playback_progress))
        .route("/playback/status/:type/:id", get(get_playback_status))
        .route("/movies/:id/download", get(download_movie))
        .route("/episodes/:id/download", get(download_episode))
        .route("/movies/:id/subtitles/search", get(search_subtitles))
}

async fn play_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let repos = state.repos.clone();
    tokio::spawn(async move {
        if let Ok(Some(path)) = repos.movie.get_full_path(MovieId(id)).await {
            let _ = opener::open(path);
        }
    });
    Json("Playback started".to_string())
}

async fn play_episode(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let repos = state.repos.clone();
    tokio::spawn(async move {
        if let Ok(Some(path)) = repos.tv.get_episode_full_path(EpisodeId(id)).await {
            let _ = opener::open(path);
        }
    });
    Json("Playback started".to_string())
}

async fn download_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>, req: axum::extract::Request) -> impl IntoResponse {
    match state.repos.movie.get_full_path(MovieId(id)).await {
        Ok(Some(path)) => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let service = ServeFile::new(path);
            let mut res = service.oneshot(req).await.unwrap().into_response();
            res.headers_mut().insert(
                header::CONTENT_DISPOSITION, 
                format!("attachment; filename=\"{}\"", filename).parse().unwrap()
            );
            res
        },
        _ => (StatusCode::NOT_FOUND, "Movie not found").into_response()
    }
}

async fn download_episode(State(state): State<Arc<AppState>>, Path(id): Path<i64>, req: axum::extract::Request) -> impl IntoResponse {
    match state.repos.tv.get_episode_full_path(EpisodeId(id)).await {
        Ok(Some(path)) => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let service = ServeFile::new(path);
            let mut res = service.oneshot(req).await.unwrap().into_response();
            res.headers_mut().insert(
                header::CONTENT_DISPOSITION, 
                format!("attachment; filename=\"{}\"", filename).parse().unwrap()
            );
            res
        },
        _ => (StatusCode::NOT_FOUND, "Episode not found").into_response()
    }
}

async fn search_subtitles(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let api_key = std::env::var("OPENSUBTITLES_API_KEY").unwrap_or_default();

    if api_key.is_empty() {
        return (StatusCode::BAD_REQUEST, "OpenSubtitles API Key missing").into_response();
    }

    tokio::spawn(async move {
        let start_ms = utils::now_ms();
        if let Ok(Some(movie)) = repos.movie.find_by_id(media_core::models::MovieId(id)).await {
            let file_info = repos.movie.find_file_by_movie_id(movie.id).await.unwrap_or_default();

            if let Some(file) = file_info {
                let dest_path = std::path::PathBuf::from(file.file_path);
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
                        debug_info: Some(format!("Querying OpenSubtitles by Hash: {}", hash)),
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
                            debug_info: Some(format!("Querying OpenSubtitles for IMDB: {}", imdb_id)),
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
                            debug_info: None,
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
                    debug_info: None,
                    files_new: None,
                    files_healed: None,
                    files_missing: None,
                });
            }
        }
    });

    (StatusCode::ACCEPTED, "Subtitle search started").into_response()
}

#[derive(serde::Deserialize)]
pub struct PlaybackHeartbeat {
    pub media_id: i64,
    pub media_type: String,
    pub position_ms: i32,
    pub duration_ms: i32,
    pub is_finished: bool,
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

#[derive(serde::Deserialize)]
pub struct StreamQuery {
    pub protocol: Option<String>,
}

async fn start_movie_stream(
    State(state): State<Arc<AppState>>, 
    Path(id): Path<i64>,
    Query(query): Query<StreamQuery>
) -> impl IntoResponse {
    tracing::info!("Stream requested for movie ID: {}", id);
    
    let is_dash = query.protocol.as_deref() == Some("dash");
    let is_hls = query.protocol.as_deref() == Some("hls");

    if is_dash || is_hls {
        let file_info: Option<(String, Option<String>)> = sqlx::query_as("SELECT file_path, codec FROM movie_files WHERE movie_id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or_default();

        if let Some((path_str, _)) = file_info {
            let path = if let Ok(Some(full_path)) = state.repos.movie.get_full_path(MovieId(id)).await {
                full_path
            } else {
                PathBuf::from(&path_str)
            };
            
            tracing::info!("Tier 3: {} streaming enabled for movie ID: {} (requested)", if is_dash { "DASH" } else { "HLS" }, id);
            let stream_id = format!("movie_{}", id);
            let result = if is_dash {
                state.stream_manager.start_dash(&stream_id, &path).await
            } else {
                state.stream_manager.start_hls(&stream_id, &path).await
            };

            match result {
                Ok(_) => {
                    let url = if is_dash {
                        format!("/api/stream/dash/{}/manifest.mpd", stream_id)
                    } else {
                        format!("/api/stream/hls/{}/playlist.m3u8", stream_id)
                    };
                    return (StatusCode::OK, Json(url)).into_response();
                },
                Err(e) => {
                    tracing::error!("Stream failed to start: {}", e);
                },
            }
        }
    }

    // Default: Direct Play via ServeFile
    tracing::info!("Defaulting to Direct Play for movie ID: {}", id);
    (StatusCode::OK, Json(format!("/api/stream/direct/movie/{}", id))).into_response()
}

async fn start_episode_stream(
    State(state): State<Arc<AppState>>, 
    Path(id): Path<i64>,
    Query(query): Query<StreamQuery>
) -> impl IntoResponse {
    tracing::info!("Stream requested for episode ID: {}", id);

    let is_dash = query.protocol.as_deref() == Some("dash");
    let is_hls = query.protocol.as_deref() == Some("hls");

    if is_dash || is_hls {
        let file_info: Option<(String, Option<String>)> = sqlx::query_as("SELECT file_path, codec FROM episodes WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or_default();

        if let Some((path_str, _)) = file_info {
            let path = if let Ok(Some(full_path)) = state.repos.tv.get_episode_full_path(EpisodeId(id)).await {
                full_path
            } else {
                PathBuf::from(&path_str)
            };
            
            tracing::info!("Tier 3: {} streaming requested for episode ID: {}", if is_dash { "DASH" } else { "HLS" }, id);
            let stream_id = format!("episode_{}", id);
            let result = if is_dash {
                state.stream_manager.start_dash(&stream_id, &path).await
            } else {
                state.stream_manager.start_hls(&stream_id, &path).await
            };

            match result {
                Ok(_) => {
                    let url = if is_dash {
                        format!("/api/stream/dash/{}/manifest.mpd", stream_id)
                    } else {
                        format!("/api/stream/hls/{}/playlist.m3u8", stream_id)
                    };
                    return (StatusCode::OK, Json(url)).into_response();
                },
                Err(e) => {
                    tracing::error!("Stream failed to start: {}", e);
                },
            }
        }
    }

    // Default: Direct Play via ServeFile
    tracing::info!("Defaulting to Direct Play for episode ID: {}", id);
    (StatusCode::OK, Json(format!("/api/stream/direct/episode/{}", id))).into_response()
}

async fn serve_direct_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    if let Ok(Some(path)) = state.repos.movie.get_full_path(MovieId(id)).await {
        let service = ServeFile::new(path);
        service.oneshot(req).await.unwrap().into_response()
    } else {
        (StatusCode::NOT_FOUND, "Movie not found").into_response()
    }
}

async fn serve_direct_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    if let Ok(Some(path)) = state.repos.tv.get_episode_full_path(EpisodeId(id)).await {
        let service = ServeFile::new(path);
        service.oneshot(req).await.unwrap().into_response()
    } else {
        (StatusCode::NOT_FOUND, "Episode not found").into_response()
    }
}

async fn serve_dash_manifest(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>
) -> impl IntoResponse {
    let (m_type, m_id) = if id.starts_with("movie_") {
        ("movie", id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0))
    } else if id.starts_with("episode_") {
        ("episode", id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0))
    } else {
        ("", 0)
    };

    if m_id > 0 {
        let info: Option<(i32, i32, i32)> = if m_type == "movie" {
            sqlx::query_as("SELECT duration_secs, width, height FROM movie_files WHERE movie_id = ?")
                .bind(m_id)
                .fetch_optional(&state.pool)
                .await
                .unwrap_or(None)
        } else {
            sqlx::query_as("SELECT duration_secs, width, height FROM episodes WHERE id = ?")
                .bind(m_id)
                .fetch_optional(&state.pool)
                .await
                .unwrap_or(None)
        };

        if let Some((dur, width, height)) = info {
            let manifest = media_core::scanner::streaming::generate_dash_manifest(dur as f64, width, height);
            return (
                [(header::CONTENT_TYPE, "application/dash+xml")],
                manifest,
            ).into_response();
        }
    }

    (StatusCode::NOT_FOUND, "Media info not found for DASH manifest").into_response()
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
                    state.repos.movie.get_full_path(MovieId(m_id)).await.ok().flatten()
                } else {
                    state.repos.tv.get_episode_full_path(EpisodeId(m_id)).await.ok().flatten()
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
                                let repos = state.repos.clone();
                                let m_type_clone = m_type.to_string();
                                tokio::spawn(async move {
                                    if m_type_clone == "movie" {
                                        let file_info = repos.movie.find_file_by_movie_id(MovieId(m_id)).await.unwrap_or_default();
                                        if let Some(file) = file_info {
                                            let _ = repos.movie.update_file_duration(file.id, found_dur).await;
                                        }
                                    } else {
                                        let _ = repos.tv.update_episode_duration(EpisodeId(m_id), found_dur).await;
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
                let manifest = media_core::scanner::streaming::generate_hls_manifest(dur);
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

    if file.ends_with(".ts") || file.ends_with(".webm") {
        // Extract segment index
        let segment_index = if file.ends_with(".ts") {
            file.strip_prefix("seg_")
                .and_then(|s| s.strip_suffix(".ts"))
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
        } else {
            // DASH segments are chunk-streamX-XXXXX.webm
            file.split('-')
                .nth(2)
                .and_then(|s| s.strip_suffix(".webm"))
                .and_then(|s| s.parse::<usize>().ok())
                .map(|idx| if idx > 0 { idx - 1 } else { 0 }) // DASH often starts at 1
                .unwrap_or(0)
        };

        let m_path = if id.starts_with("movie_") {
            let m_id = id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0);
            state.repos.movie.get_full_path(MovieId(m_id)).await.ok().flatten()
        } else if id.starts_with("episode_") {
            let e_id = id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0);
            state.repos.tv.get_episode_full_path(EpisodeId(e_id)).await.ok().flatten()
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

    let mime = if file.ends_with(".m3u8") {
        "application/vnd.apple.mpegurl"
    } else if file.ends_with(".ts") {
        "video/mp2t"
    } else if file.ends_with(".webm") {
        "video/webm"
    } else if file.ends_with(".mpd") {
        "application/dash+xml"
    } else {
        "application/octet-stream"
    };

    match tokio::fs::File::open(&file_path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            (
                [(header::CONTENT_TYPE, mime)],
                axum::body::Body::from_stream(stream),
            ).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
