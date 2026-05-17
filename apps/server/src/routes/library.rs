use axum::{
    routing::{get, post},
    Router,
    extract::{State, Path},
    Json,
    response::IntoResponse,
    http::StatusCode,
};
use std::sync::Arc;
use media_core::models::LibraryId;
use crate::state::AppState;
use media_core::db::{LibraryReader, LibraryWriter};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/libraries", get(get_libraries).post(create_library))
        .route("/libraries/:id", axum::routing::delete(delete_library))
        .route("/libraries/:id/scan", post(scan_library))
}

async fn get_libraries(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.repos.library.find_all().await {
        Ok(libs) => Json(libs).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch libraries: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub path: String,
    pub media_type: media_core::models::MediaType,
}

async fn create_library(State(state): State<Arc<AppState>>, Json(payload): Json<CreateLibraryRequest>) -> impl IntoResponse {
    tracing::info!("Creating library: {} at {}", payload.name, payload.path);
    match state.repos.library.insert(&payload.name, &payload.path, payload.media_type).await {
        Ok(id) => {
            tracing::info!("Library created with ID: {}", id);
            
            // Trigger automatic scan
            let service = state.scanner_service.clone();
            let task_id = uuid::Uuid::new_v4().to_string();
            let library = media_core::models::Library {
                id,
                name: payload.name,
                path: payload.path,
                media_type: payload.media_type,
                created_at: "".to_string(), // Not used by service
            };
            
            tokio::spawn(async move {
                let _ = service.scan_library(&library, task_id).await;
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
    match state.repos.library.delete(LibraryId(id)).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to delete library {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn scan_library(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let service = state.scanner_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    
    let libraries = state.repos.library.find_all().await.unwrap_or_default();
    if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(id)) {
        tokio::spawn(async move {
            let _ = service.scan_library(&lib, task_id).await;
        });
        Json("Scan started".to_string())
    } else {
        Json("Library not found".to_string())
    }
}
