use axum::{
    routing::post,
    Router,
    extract::{State, Path},
    Json,
    response::IntoResponse,
    http::StatusCode,
};
use std::sync::Arc;
use std::path::PathBuf;
use crate::state::AppState;
use crate::utils;
use crate::routes::scraper::BatchRequest;
use media_core::models::{MovieId, TvShowId, LibraryId};
use media_core::cleanup::CleanupService;
use media_core::task_manager::ProgressSink;
use media_core::db::{LibraryReader, MovieReader, MovieWriter, TvReader, SettingsRepository};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/libraries/:id/cleanup/duplicates", post(cleanup_duplicates))
        .route("/libraries/:id/cleanup/empty-folders", post(cleanup_empty_folders))
        .route("/cleanup/batch", post(cleanup_batch))
        .route("/movies/:id/rename", post(rename_movie))
}

async fn cleanup_duplicates(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Vec<PathBuf>> {
    let libraries = state.repos.library.find_all().await.unwrap_or_default();
    if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(id)) {
        let cleanup = CleanupService::new(PathBuf::from(lib.path));
        Json(cleanup.remove_duplicate_artwork().unwrap_or_default())
    } else {
        Json(vec![])
    }
}

async fn cleanup_empty_folders(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Vec<PathBuf>> {
    let libraries = state.repos.library.find_all().await.unwrap_or_default();
    if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(id)) {
        let cleanup = CleanupService::new(PathBuf::from(lib.path));
        Json(cleanup.remove_empty_folders().unwrap_or_default())
    } else {
        Json(vec![])
    }
}

async fn cleanup_batch(State(state): State<Arc<AppState>>, Json(payload): Json<BatchRequest>) -> Json<String> {
    let repos = state.repos.clone();
    let task_manager = state.task_manager.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let start_ms = utils::now_ms();
        let total = payload.ids.len() as i32;
        let mut processed = 0;

        if payload.media_type == "movie" {
            let renamer = media_core::renamer::Renamer::new(None, None);
            for id in &payload.ids {
                processed += 1;
                
                if let Ok(Some(movie)) = repos.movie.find_by_id(MovieId(*id)).await {
                    let libraries = repos.library.find_all().await.unwrap_or_default();
                    if let Some(lib) = libraries.into_iter().find(|l| l.id == movie.library_id) {
                        let lib_root = PathBuf::from(&lib.path);
                        
                        // Get file details
                        let files = repos.movie.find_files_by_movie_id(movie.id).await.unwrap_or_default();
                        
                        for mut file in files {
                            let old_path = PathBuf::from(&file.file_path);
                            
                            // Optimization: If resolution is missing, try to get it now
                            if file.resolution.is_none() {
                                let path_for_info = old_path.clone();
                                // Try ffprobe first
                                let res = tokio::task::spawn_blocking(move || {
                                    media_core::scanner::mediainfo::get_media_info(&path_for_info).ok()
                                }).await.unwrap_or_default().map(|i| media_core::models::Resolution::from_dimensions(i.width, i.height));
                                
                                if let Some(r) = res {
                                    let _ = repos.movie.update_file_resolution(file.id, r).await;
                                    file.resolution = Some(r);
                                }
                            }

                            // Fetch post processing script
                            let settings = repos.settings.get_all().await.unwrap_or_default();
                            let script_path = settings.get("post_processing_script").map(|s| s.as_str());

                            // 1. Rename File
                            if let Ok(new_path) = renamer.rename_movie(&movie, &old_path, &lib_root, file.resolution, file.codec.as_deref(), script_path) {
                                let new_path_str = new_path.to_string_lossy().to_string();
                                if new_path_str != file.file_path {
                                    let _ = repos.movie.update_file_path(file.id, &new_path_str).await;
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
                        debug_info: Some(format!("Renaming & Cleaning folder for: {}", movie.title)),
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
                let seasons = repos.tv.find_seasons_by_show_id(TvShowId(*id)).await.unwrap_or_default();
                let mut files = Vec::new();
                for s in seasons {
                    let eps = repos.tv.find_episodes_by_season_id(s.id).await.unwrap_or_default();
                    files.extend(eps.into_iter().map(|e| e.file_path));
                }

                // Get unique parent folders
                let mut parents = Vec::new();
                for path_str in files {
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
                    debug_info: Some(format!("Removing duplicate artwork for TV Show ID: {}", id)),
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

async fn rename_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let repos = state.repos.clone();
    
    match repos.movie.find_by_id(media_core::models::MovieId(id)).await {
        Ok(Some(movie)) => {
            let movie_id = movie.id;
            let libraries = repos.library.find_all().await.unwrap_or_default();
            
            if let Some(lib) = libraries.into_iter().find(|l| l.id == movie.library_id) {
                // Get all movie files
                let files = repos.movie.find_files_by_movie_id(movie_id).await.unwrap_or_default();
                
                if !files.is_empty() {
                    let repos_clone = repos.clone();
                    let lib_path = lib.path.clone();
                    
                    tokio::task::spawn_blocking(move || {
                        let renamer = media_core::renamer::Renamer::new(None, None);
                        let lib_root = std::path::PathBuf::from(&lib_path);
                        
                        // Fetch script path before starting the blocking operation
                        let rt = tokio::runtime::Handle::current();
                        let script_path = rt.block_on(async {
                            let settings = repos_clone.settings.get_all().await.unwrap_or_default();
                            settings.get("post_processing_script").cloned()
                        });

                        for file in files {
                            let old_path = std::path::PathBuf::from(&file.file_path);
                            if let Ok(new_path) = renamer.rename_movie(&movie, &old_path, &lib_root, file.resolution, file.codec.as_deref(), script_path.as_deref()) {
                                let new_path_str = new_path.to_string_lossy().to_string();
                                if new_path_str != file.file_path {
                                    // Update DB in a blocking-safe way
                                    let repos_inner = repos_clone.clone();
                                    tokio::runtime::Handle::current().spawn(async move {
                                        let _ = repos_inner.movie.update_file_path(file.id, &new_path_str).await;
                                    });
                                }
                            }
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
