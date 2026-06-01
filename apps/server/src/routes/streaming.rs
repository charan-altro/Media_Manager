use axum::{
    routing::{get, post, delete},
    Router,
    extract::{State, Path, Query},
    Json,
    response::{IntoResponse, Redirect},
    http::{header, StatusCode},
};

fn is_safari(headers: &axum::http::HeaderMap) -> bool {
    if let Some(user_agent) = headers.get(header::USER_AGENT).and_then(|h| h.to_str().ok()) {
        let ua = user_agent.to_lowercase();
        ua.contains("safari") && !ua.contains("chrome") && !ua.contains("chromium") && !ua.contains("crios")
    } else {
        false
    }
}
use std::sync::Arc;
use std::path::PathBuf;
use crate::state::AppState;
use crate::utils;
use media_core::models::{MovieId, EpisodeId, MovieFileId};
use media_core::task_manager::ProgressSink;
use tower::ServiceExt;
use tower_http::services::ServeFile;
use media_core::db::{MovieReader, MovieWriter, TvReader, TvWriter, MediaRepository};
use tokio_util::io::ReaderStream;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/movies/:id/play", post(play_movie))
        .route("/episodes/:id/play", post(play_episode))
        .route("/stream/movie/:id", post(start_movie_stream))
        .route("/stream/movie_file/:id", post(start_movie_file_stream))
        .route("/stream/episode/:id", post(start_episode_stream))
        .route("/stream/direct/:id/playlist.m3u8", get(serve_direct_hls_manifest))
        .route("/stream/direct/:id/stream.:ext", get(serve_direct_stream_generic))
        .route("/stream/jit/movie/:id", get(serve_jit_movie))
        .route("/stream/jit/episode/:id", get(serve_jit_episode))
        .route("/stream/direct/movie/:id", get(serve_direct_movie))
        .route("/stream/direct/movie_file/:id", get(serve_direct_movie_file))
        .route("/stream/direct/episode/:id", get(serve_direct_episode))
        .route("/stream/hls/:id/:file", get(serve_stream_file))
        .route("/stream/dash/:id/manifest.mpd", get(serve_dash_manifest))
        .route("/stream/dash/:id/:file", get(serve_stream_file))
        .route("/playback/heartbeat", post(update_playback_progress))
        .route("/playback/status/:type/:id", get(get_playback_status))
        .route("/movies/:id/download", get(download_movie))
        .route("/movies/files/:id/download", get(download_movie_file))
        .route("/episodes/:id/download", get(download_episode))
        .route("/movies/:id/subtitles/search", get(search_subtitles))
        .route("/media/:media_type/:media_id/markers", get(get_markers).post(create_marker))
        .route("/media/markers/:marker_id", delete(delete_marker))
        .route("/media/:media_type/:media_id/subtitles", get(get_sidecar_subtitles))
        .route("/media/:media_type/:media_id/subtitles/:lang", get(serve_sidecar_subtitle_vtt))
}

async fn play_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let repos = state.repos.clone();
    tokio::spawn(async move {
        if let Ok(Some(path)) = repos.movie.get_full_path(MovieId(id)).await {
            // Standardize path separators for the OS
            let path_str = path.to_string_lossy();
            #[cfg(target_os = "windows")]
            let clean_path = std::path::PathBuf::from(path_str.replace('/', "\\"));
            #[cfg(not(target_os = "windows"))]
            let clean_path = std::path::PathBuf::from(path_str.replace('\\', "/"));

            tracing::info!("Attempting to open movie locally: {:?}", clean_path);
            match opener::open(&clean_path) {
                Ok(_) => tracing::info!("Successfully opened local player for {:?}", clean_path),
                Err(e) => tracing::error!("Failed to open local player for {:?}: {}", clean_path, e),
            }
        } else {
            tracing::warn!("Failed to retrieve full path for Movie ID: {}", id);
        }
    });
    Json("Playback started".to_string())
}

async fn play_episode(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let repos = state.repos.clone();
    tokio::spawn(async move {
        if let Ok(Some(path)) = repos.tv.get_episode_full_path(EpisodeId(id)).await {
            // Standardize path separators for the OS
            let path_str = path.to_string_lossy();
            #[cfg(target_os = "windows")]
            let clean_path = std::path::PathBuf::from(path_str.replace('/', "\\"));
            #[cfg(not(target_os = "windows"))]
            let clean_path = std::path::PathBuf::from(path_str.replace('\\', "/"));

            tracing::info!("Attempting to open episode locally: {:?}", clean_path);
            match opener::open(&clean_path) {
                Ok(_) => tracing::info!("Successfully opened local player for {:?}", clean_path),
                Err(e) => tracing::error!("Failed to open local player for {:?}: {}", clean_path, e),
            }
        } else {
            tracing::warn!("Failed to retrieve full path for Episode ID: {}", id);
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

async fn download_movie_file(State(state): State<Arc<AppState>>, Path(id): Path<i64>, req: axum::extract::Request) -> impl IntoResponse {
    let fid = media_core::models::MovieFileId(id);
    match state.repos.movie.get_file_full_path(fid).await {
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
        _ => (StatusCode::NOT_FOUND, "Movie file not found").into_response()
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
    pub duration_ms: Option<i32>,
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

    let duration_ms = payload.duration_ms.unwrap_or(0);

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
    .bind(duration_ms)
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
    
    let protocol = query.protocol.as_deref().unwrap_or("direct");
    let stream_id = format!("movie_{}", id);

    let file_info: Option<(String, Option<String>)> = sqlx::query_as("SELECT file_path, codec FROM movie_files WHERE movie_id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or_default();

    if let Some((path_str, _codec)) = file_info {
        let path = if let Ok(Some(full_path)) = state.repos.movie.get_full_path(MovieId(id)).await {
            full_path
        } else {
            PathBuf::from(&path_str)
        };

        // Tier 2: Direct Streaming (Static File Range Serving or Piped Remux)
        if protocol == "direct" || protocol == "jit" {
            let mut playable = false;
            let mut video_ok = false;
            if let Ok(details) = media_core::scanner::mediainfo::get_media_info(&path) {
                playable = media_core::scanner::streaming::is_direct_playable(&path, &details);
                let v_codec = details.video_codec.to_lowercase();
                video_ok = ["h264", "vp9", "av1", "hevc"].contains(&v_codec.as_str());
            }

            if playable {
                tracing::info!("Tier 2: File is browser-compatible, enabling native static direct play for movie ID: {}", id);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
                return (StatusCode::OK, Json(format!("/api/stream/direct/movie/{}?ext={}", id, ext))).into_response();
            } else if video_ok {
                tracing::info!("Tier 2: Video is compatible but audio is incompatible, enabling progressive piped remux with audio transcode for movie ID: {}", id);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_lowercase();
                let stream_ext = if ext == "mkv" || ext == "webm" { "mkv" } else { "mp4" };
                return (StatusCode::OK, Json(format!("/api/stream/direct/{}/stream.{}", stream_id, stream_ext))).into_response();
            } else {
                tracing::info!("Tier 2: File is browser-incompatible, enabling piped HLS remux/transcode for movie ID: {}", id);
                return (StatusCode::OK, Json(format!("/api/stream/direct/{}/playlist.m3u8", stream_id))).into_response();
            }
        }

        if protocol == "hls" || protocol == "dash" {
            tracing::info!("Tier 3: {} streaming enabled for movie ID: {} (requested)", protocol.to_uppercase(), id);
            let result = if protocol == "dash" {
                state.stream_manager.start_dash(&stream_id, &path).await
            } else {
                state.stream_manager.start_hls(&stream_id, &path).await
            };

            match result {
                Ok(_) => {
                    let url = if protocol == "dash" {
                        format!("/api/stream/dash/{}/manifest.mpd", stream_id)
                    } else {
                        format!("/api/stream/hls/{}/playlist.m3u8", stream_id)
                    };
                    return (StatusCode::OK, Json(url)).into_response();
                },
                Err(e) => {
                    tracing::error!("Stream failed to start: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                },
            }
        }
    }

    (StatusCode::NOT_FOUND, "Movie not found").into_response()
}

async fn start_movie_file_stream(
    State(state): State<Arc<AppState>>, 
    Path(file_id): Path<i64>,
    Query(query): Query<StreamQuery>
) -> impl IntoResponse {
    tracing::info!("Stream requested for movie file ID: {}", file_id);
    
    let protocol = query.protocol.as_deref().unwrap_or("direct");
    let stream_id = format!("movie_file_{}", file_id);
    let fid = MovieFileId(file_id);

    let file_info = state.repos.movie.find_file_by_id(fid).await.unwrap_or_default();

    if let Some(file) = file_info {
        let path = if let Ok(Some(full_path)) = state.repos.movie.get_file_full_path(fid).await {
            full_path
        } else {
            PathBuf::from(&file.file_path)
        };

        // Tier 2: Direct Streaming (Static File Range Serving or Piped Remux)
        if protocol == "direct" || protocol == "jit" {
            let mut playable = false;
            let mut video_ok = false;
            if let Ok(details) = media_core::scanner::mediainfo::get_media_info(&path) {
                playable = media_core::scanner::streaming::is_direct_playable(&path, &details);
                let v_codec = details.video_codec.to_lowercase();
                video_ok = ["h264", "vp9", "av1", "hevc"].contains(&v_codec.as_str());
            }

            if playable {
                tracing::info!("Tier 2: File is browser-compatible, enabling native static direct play for movie file ID: {}", file_id);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
                return (StatusCode::OK, Json(format!("/api/stream/direct/movie_file/{}?ext={}", file_id, ext))).into_response();
            } else if video_ok {
                tracing::info!("Tier 2: Video is compatible but audio is incompatible, enabling progressive piped remux with audio transcode for movie file ID: {}", file_id);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_lowercase();
                let stream_ext = if ext == "mkv" || ext == "webm" { "mkv" } else { "mp4" };
                return (StatusCode::OK, Json(format!("/api/stream/direct/{}/stream.{}", stream_id, stream_ext))).into_response();
            } else {
                tracing::info!("Tier 2: File is browser-incompatible, enabling piped HLS remux/transcode for movie file ID: {}", file_id);
                return (StatusCode::OK, Json(format!("/api/stream/direct/{}/playlist.m3u8", stream_id))).into_response();
            }
        }

        if protocol == "hls" || protocol == "dash" {
            tracing::info!("Tier 3: {} streaming enabled for movie file ID: {} (requested)", protocol.to_uppercase(), file_id);
            let result = if protocol == "dash" {
                state.stream_manager.start_dash(&stream_id, &path).await
            } else {
                state.stream_manager.start_hls(&stream_id, &path).await
            };

            match result {
                Ok(_) => {
                    let url = if protocol == "dash" {
                        format!("/api/stream/dash/{}/manifest.mpd", stream_id)
                    } else {
                        format!("/api/stream/hls/{}/playlist.m3u8", stream_id)
                    };
                    return (StatusCode::OK, Json(url)).into_response();
                }
                Err(e) => {
                    tracing::error!("Stream failed to start: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                }
            }
        } else {
            (StatusCode::BAD_REQUEST, "Invalid protocol").into_response()
        }
    } else {
        (StatusCode::NOT_FOUND, "Movie file not found").into_response()
    }
}

async fn start_episode_stream(
    State(state): State<Arc<AppState>>, 
    Path(id): Path<i64>,
    Query(query): Query<StreamQuery>
) -> impl IntoResponse {
    tracing::info!("Stream requested for episode ID: {}", id);

    let protocol = query.protocol.as_deref().unwrap_or("direct");
    let stream_id = format!("episode_{}", id);

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

        // Tier 2: Direct Streaming (Static File Range Serving or Piped Remux)
        if protocol == "direct" || protocol == "jit" {
            let mut playable = false;
            let mut video_ok = false;
            if let Ok(details) = media_core::scanner::mediainfo::get_media_info(&path) {
                playable = media_core::scanner::streaming::is_direct_playable(&path, &details);
                let v_codec = details.video_codec.to_lowercase();
                video_ok = ["h264", "vp9", "av1", "hevc"].contains(&v_codec.as_str());
            }

            if playable {
                tracing::info!("Tier 2: File is browser-compatible, enabling native static direct play for episode ID: {}", id);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
                return (StatusCode::OK, Json(format!("/api/stream/direct/episode/{}?ext={}", id, ext))).into_response();
            } else if video_ok {
                tracing::info!("Tier 2: Video is compatible but audio is incompatible, enabling progressive piped remux with audio transcode for episode ID: {}", id);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_lowercase();
                let stream_ext = if ext == "mkv" || ext == "webm" { "mkv" } else { "mp4" };
                return (StatusCode::OK, Json(format!("/api/stream/direct/{}/stream.{}", stream_id, stream_ext))).into_response();
            } else {
                tracing::info!("Tier 2: File is browser-incompatible, enabling piped HLS remux/transcode for episode ID: {}", id);
                return (StatusCode::OK, Json(format!("/api/stream/direct/{}/playlist.m3u8", stream_id))).into_response();
            }
        }

        if protocol == "hls" || protocol == "dash" {
            tracing::info!("Tier 3: {} streaming requested for episode ID: {}", protocol.to_uppercase(), id);
            let result = if protocol == "dash" {
                state.stream_manager.start_dash(&stream_id, &path).await
            } else {
                state.stream_manager.start_hls(&stream_id, &path).await
            };

            match result {
                Ok(_) => {
                    let url = if protocol == "dash" {
                        format!("/api/stream/dash/{}/manifest.mpd", stream_id)
                    } else {
                        format!("/api/stream/hls/{}/playlist.m3u8", stream_id)
                    };
                    return (StatusCode::OK, Json(url)).into_response();
                },
                Err(e) => {
                    tracing::error!("Stream failed to start: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
                },
            }
        }
    }

    (StatusCode::NOT_FOUND, "Episode not found").into_response()
}

async fn serve_direct_hls_manifest(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<JitStreamQuery>,
) -> impl IntoResponse {
    let start_time = query.start.unwrap_or(0.0);
    let (m_type, m_id) = if id.starts_with("movie_file_") {
        ("movie_file", id.strip_prefix("movie_file_").unwrap().parse::<i64>().unwrap_or(0))
    } else if id.starts_with("movie_") {
        ("movie", id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0))
    } else if id.starts_with("episode_") {
        ("episode", id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0))
    } else {
        ("", 0)
    };

    if m_id > 0 {
        let duration: Option<i32> = if m_type == "movie_file" {
            sqlx::query_scalar("SELECT duration_secs FROM movie_files WHERE id = ?")
                .bind(m_id)
                .fetch_optional(&state.pool)
                .await
                .unwrap_or(None)
        } else if m_type == "movie" {
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

        if let Some(dur) = duration {
            let path_str: Option<String> = if m_type == "movie_file" {
                sqlx::query_scalar("SELECT file_path FROM movie_files WHERE id = ?")
                    .bind(m_id)
                    .fetch_optional(&state.pool)
                    .await
                    .unwrap_or(None)
            } else if m_type == "movie" {
                sqlx::query_scalar("SELECT file_path FROM movie_files WHERE movie_id = ?")
                    .bind(m_id)
                    .fetch_optional(&state.pool)
                    .await
                    .unwrap_or(None)
            } else {
                sqlx::query_scalar("SELECT file_path FROM episodes WHERE id = ?")
                    .bind(m_id)
                    .fetch_optional(&state.pool)
                    .await
                    .unwrap_or(None)
            };

            if let Some(_path) = path_str {
                // We use fragmented MP4 (.mp4) for the internal HLS segment to support
                // modern codecs (like HEVC, AV1, and Opus) natively via Hls.js fMP4.
                let stream_url = format!("/api/stream/direct/{}/stream.mp4?start={}", id, start_time);
                let manifest = media_core::scanner::streaming::generate_direct_hls_manifest(dur as f64, &stream_url);
                return (
                    [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
                    manifest,
                ).into_response();
            }
        }
    }

    (StatusCode::NOT_FOUND, "Media info not found for Custom HLS manifest").into_response()
}

#[derive(serde::Deserialize)]
pub struct JitStreamQuery {
    pub start: Option<f64>,
}

async fn serve_direct_stream_generic(
    State(state): State<Arc<AppState>>,
    Path((id, ext)): Path<(String, String)>,
    Query(query): Query<JitStreamQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let start_time = query.start.unwrap_or(0.0);
    
    if is_safari(&headers) && (ext == "mp4" || ext == "webm" || ext == "mkv") {
        tracing::info!("Safari detected on direct stream {}, redirecting to HLS playlist", id);
        return Redirect::temporary(&format!("/api/stream/direct/{}/playlist.m3u8", id)).into_response();
    }
    
    let path = if id.starts_with("movie_file_") {
        let file_id = id.strip_prefix("movie_file_").unwrap().parse::<i64>().unwrap_or(0);
        state.repos.movie.get_file_full_path(MovieFileId(file_id)).await.ok().flatten()
    } else if id.starts_with("movie_") {
        let m_id = id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0);
        state.repos.movie.get_full_path(MovieId(m_id)).await.ok().flatten()
    } else if id.starts_with("episode_") {
        let e_id = id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0);
        state.repos.tv.get_episode_full_path(EpisodeId(e_id)).await.ok().flatten()
    } else {
        None
    };

    if let Some(path) = path {
        tracing::info!("Direct {} stream requested for {} at {}s", ext, id, start_time);
        
        let mime = match ext.as_str() {
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "ts" => "video/mp2t",
            _ => "video/x-matroska",
        };

        match state.stream_manager.start_direct_stream(&path, start_time, &ext).await {
            Ok(stream) => {
                (
                    [
                        (header::CONTENT_TYPE, mime),
                        (header::CACHE_CONTROL, "no-store"),
                        (header::ACCEPT_RANGES, "none"),
                        (header::CONNECTION, "keep-alive"),
                    ],
                    axum::body::Body::from_stream(stream),
                ).into_response()
            },
            Err(e) => {
                tracing::error!("Direct stream failed: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("FFmpeg error: {}", e)).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, "Media not found").into_response()
    }
}

async fn serve_jit_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<JitStreamQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let start_time = query.start.unwrap_or(0.0);
    
    if is_safari(&headers) {
        tracing::info!("Safari detected on JIT movie stream {}, redirecting to HLS playlist", id);
        return Redirect::temporary(&format!("/api/stream/direct/movie_{}/playlist.m3u8", id)).into_response();
    }
    if let Ok(Some(path)) = state.repos.movie.get_full_path(MovieId(id)).await {
        tracing::info!("JIT stream requested for movie {} at {}s", id, start_time);
        match state.stream_manager.start_direct_stream(&path, start_time, "mp4").await {
            Ok(stream) => {
                (
                    [
                        (header::CONTENT_TYPE, "video/mp4"),
                        (header::CACHE_CONTROL, "no-store"),
                        (header::CONNECTION, "close"),
                    ],
                    axum::body::Body::from_stream(stream),
                ).into_response()
            },
            Err(e) => {
                tracing::error!("JIT stream failed: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("FFmpeg error: {}", e)).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, "Movie not found").into_response()
    }
}

async fn serve_jit_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<JitStreamQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let start_time = query.start.unwrap_or(0.0);
    
    if is_safari(&headers) {
        tracing::info!("Safari detected on JIT episode stream {}, redirecting to HLS playlist", id);
        return Redirect::temporary(&format!("/api/stream/direct/episode_{}/playlist.m3u8", id)).into_response();
    }
    if let Ok(Some(path)) = state.repos.tv.get_episode_full_path(EpisodeId(id)).await {
        tracing::info!("JIT stream requested for episode {} at {}s", id, start_time);
        match state.stream_manager.start_direct_stream(&path, start_time, "mp4").await {
            Ok(stream) => {
                (
                    [
                        (header::CONTENT_TYPE, "video/mp4"),
                        (header::CACHE_CONTROL, "no-store"),
                        (header::CONNECTION, "close"),
                    ],
                    axum::body::Body::from_stream(stream),
                ).into_response()
            },
            Err(e) => {
                tracing::error!("JIT stream failed: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("FFmpeg error: {}", e)).into_response()
            }
        }
    } else {
        (StatusCode::NOT_FOUND, "Episode not found").into_response()
    }
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

async fn serve_direct_movie_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    let fid = MovieFileId(id);
    if let Ok(Some(path)) = state.repos.movie.get_file_full_path(fid).await {
        let service = ServeFile::new(path);
        service.oneshot(req).await.unwrap().into_response()
    } else {
        (StatusCode::NOT_FOUND, "Movie file not found").into_response()
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
    let (m_type, m_id) = if id.starts_with("movie_file_") {
        ("movie_file", id.strip_prefix("movie_file_").unwrap().parse::<i64>().unwrap_or(0))
    } else if id.starts_with("movie_") {
        ("movie", id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0))
    } else if id.starts_with("episode_") {
        ("episode", id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0))
    } else {
        ("", 0)
    };

    if m_id > 0 {
        let mut info: Option<(i32, i32, i32)> = if m_type == "movie_file" {
            sqlx::query_as("SELECT duration_secs, width, height FROM movie_files WHERE id = ?")
                .bind(m_id)
                .fetch_optional(&state.pool)
                .await
                .unwrap_or(None)
        } else if m_type == "movie" {
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

        // Fallback to ffprobe if metadata is missing or incomplete in DB
        if info.is_none() || info.as_ref().map(|(d, w, h)| *d <= 0 || *w <= 0 || *h <= 0).unwrap_or(true) {
            tracing::info!("Metadata for {} is incomplete in DB, attempting ffprobe fallback...", id);
            let path = if m_type == "movie_file" {
                state.repos.movie.get_file_full_path(MovieFileId(m_id)).await.ok().flatten()
            } else if m_type == "movie" {
                state.repos.movie.get_full_path(MovieId(m_id)).await.ok().flatten()
            } else {
                state.repos.tv.get_episode_full_path(EpisodeId(m_id)).await.ok().flatten()
            };

            if let Some(p) = path {
                match media_core::scanner::mediainfo::get_media_info(&p) {
                    Ok(details) => {
                        let dur = details.duration_secs as i32;
                        let width = details.width;
                        let height = details.height;
                        if dur > 0 && width > 0 && height > 0 {
                            info = Some((dur, width, height));
                            // Update DB so we don't have to ffprobe every time
                            let repos = state.repos.clone();
                            let m_type_clone = m_type.to_string();
                            tokio::spawn(async move {
                                if m_type_clone == "movie" {
                                    let file_info = repos.movie.find_file_by_movie_id(MovieId(m_id)).await.unwrap_or_default();
                                    if let Some(file) = file_info {
                                        let _ = repos.movie.update_file_metadata(file.id, dur, width, height).await;
                                    }
                                } else {
                                    let _ = repos.tv.update_episode_metadata(EpisodeId(m_id), dur, width, height).await;
                                }
                            });
                        }
                    },
                    Err(e) => tracing::error!("FFprobe fallback failed for DASH manifest: {}", e),
                }
            }
        }

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
        let (m_type, m_id) = if id.starts_with("movie_file_") {
            ("movie_file", id.strip_prefix("movie_file_").unwrap().parse::<i64>().unwrap_or(0))
        } else if id.starts_with("movie_") {
            ("movie", id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0))
        } else if id.starts_with("episode_") {
            ("episode", id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0))
        } else {
            ("", 0)
        };

        if m_id > 0 {
            let mut duration: Option<i32> = if m_type == "movie_file" {
                sqlx::query_scalar("SELECT duration_secs FROM movie_files WHERE id = ?")
                    .bind(m_id)
                    .fetch_optional(&state.pool)
                    .await
                    .unwrap_or(None)
            } else if m_type == "movie" {
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
                let path = if m_type == "movie_file" {
                    state.repos.movie.get_file_full_path(MovieFileId(m_id)).await.ok().flatten()
                } else if m_type == "movie" {
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
                                    if m_type_clone == "movie_file" {
                                        let _ = repos.movie.update_file_duration(MovieFileId(m_id), found_dur).await;
                                    } else if m_type_clone == "movie" {
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
    let transcode_dir = state.playback_service.transcode_dir();
    let base_dir = PathBuf::from(transcode_dir).join(&id);
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

        let m_path = if id.starts_with("movie_file_") {
            let file_id = id.strip_prefix("movie_file_").unwrap().parse::<i64>().unwrap_or(0);
            state.repos.movie.get_file_full_path(MovieFileId(file_id)).await.ok().flatten()
        } else if id.starts_with("movie_") {
            let m_id = id.strip_prefix("movie_").unwrap().parse::<i64>().unwrap_or(0);
            state.repos.movie.get_full_path(MovieId(m_id)).await.ok().flatten()
        } else if id.starts_with("episode_") {
            let e_id = id.strip_prefix("episode_").unwrap().parse::<i64>().unwrap_or(0);
            state.repos.tv.get_episode_full_path(EpisodeId(e_id)).await.ok().flatten()
        } else {
            None
        };

        if let Some(path) = m_path {
            if let Err(e) = state.stream_manager.request_segment(&id, &path, segment_index, &file).await {
                tracing::error!("Failed to request segment {} for {}: {}", segment_index, id, e);
            }
        }

        // Wait for segment via tokio watch channel
        match state.stream_manager.wait_for_segment(&id, segment_index, &file).await {
            Ok(true) => {
                // Segment ready
            },
            Ok(false) => {
                tracing::warn!("Timeout waiting for segment {} of {}", segment_index, id);
            },
            Err(e) => {
                tracing::error!("Error waiting for segment {} of {}: {}", segment_index, id, e);
            }
        }
    }

    if !file_path.exists() {
        tracing::warn!("Stream file not found on disk: {:?}", file_path);
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

// --- Scene Markers Handler ---

#[derive(serde::Deserialize)]
struct CreateSceneMarkerRequest {
    seconds: f64,
    title: String,
}

async fn get_markers(
    State(state): State<Arc<AppState>>,
    Path((media_type, media_id)): Path<(String, i64)>,
) -> impl IntoResponse {
    match state.repos.media.get_scene_markers(media_id, &media_type).await {
        Ok(markers) => (StatusCode::OK, Json(markers)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get scene markers: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn create_marker(
    State(state): State<Arc<AppState>>,
    Path((media_type, media_id)): Path<(String, i64)>,
    Json(payload): Json<CreateSceneMarkerRequest>,
) -> impl IntoResponse {
    match state.repos.media.create_scene_marker(media_id, &media_type, payload.seconds, &payload.title).await {
        Ok(marker) => (StatusCode::CREATED, Json(marker)).into_response(),
        Err(e) => {
            tracing::error!("Failed to create scene marker: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn delete_marker(
    State(state): State<Arc<AppState>>,
    Path(marker_id): Path<i64>,
) -> impl IntoResponse {
    match state.repos.media.delete_scene_marker(marker_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to delete scene marker: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// --- Sidecar Subtitles Handlers ---

async fn get_sidecar_subtitles(
    State(state): State<Arc<AppState>>,
    Path((media_type, media_id)): Path<(String, i64)>,
) -> impl IntoResponse {
    let path = if media_type == "movie" {
        state.repos.movie.get_full_path(MovieId(media_id)).await.ok().flatten()
    } else if media_type == "movie_file" {
        state.repos.movie.get_file_full_path(MovieFileId(media_id)).await.ok().flatten()
    } else {
        state.repos.tv.get_episode_full_path(EpisodeId(media_id)).await.ok().flatten()
    };

    let path = match path {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Media not found").into_response(),
    };

    match media_core::subtitles::discover_sidecar_subtitles(&path) {
        Ok(subs) => (StatusCode::OK, Json(subs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn serve_sidecar_subtitle_vtt(
    State(state): State<Arc<AppState>>,
    Path((media_type, media_id, lang)): Path<(String, i64, String)>,
) -> impl IntoResponse {
    let path = if media_type == "movie" {
        state.repos.movie.get_full_path(MovieId(media_id)).await.ok().flatten()
    } else if media_type == "movie_file" {
        state.repos.movie.get_file_full_path(MovieFileId(media_id)).await.ok().flatten()
    } else {
        state.repos.tv.get_episode_full_path(EpisodeId(media_id)).await.ok().flatten()
    };

    let path = match path {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Media not found").into_response(),
    };

    let subs = match media_core::subtitles::discover_sidecar_subtitles(&path) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let matching_sub = subs.into_iter().find(|s| s.language == lang);
    let sub_file_path = match matching_sub {
        Some(s) => std::path::PathBuf::from(s.file_path),
        None => return (StatusCode::NOT_FOUND, "Subtitle language not found").into_response(),
    };

    match tokio::fs::read_to_string(&sub_file_path).await {
        Ok(srt_content) => {
            let vtt_content = media_core::subtitles::srt_to_vtt(&srt_content);
            (
                [(header::CONTENT_TYPE, "text/vtt")],
                vtt_content,
            ).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to read sidecar subtitle file: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
