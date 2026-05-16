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
use media_core::db;
use media_core::models::{MovieId, TvShowId, LibraryId};
use media_core::cleanup::CleanupService;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/libraries/:id/cleanup/duplicates", post(cleanup_duplicates))
        .route("/libraries/:id/cleanup/empty-folders", post(cleanup_empty_folders))
        .route("/cleanup/batch", post(cleanup_batch))
        .route("/movies/:id/rename", post(rename_movie))
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
        let start_ms = utils::now_ms();
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
