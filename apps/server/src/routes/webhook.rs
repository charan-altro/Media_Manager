use axum::{
    routing::post,
    extract::{State, Path},
    Json,
    response::IntoResponse,
    http::StatusCode,
    Router,
};
use std::sync::Arc;
use crate::state::AppState;
use media_core::db::LibraryReader;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/webhooks/:source", post(handle_webhook))
}

async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    Path(source): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let task_id = uuid::Uuid::new_v4().to_string();

    tracing::info!("Received webhook from {}: {:?}", source, payload);

    let scanner_service = state.scanner_service.clone();
    let repos = state.repos.clone();

    tokio::spawn(async move {
        // Trigger a global scan or specific library scan based on webhook logic
        let libraries = repos.library.find_all().await.unwrap_or_default();
        let target_lib = match source.as_str() {
            "radarr" => libraries.into_iter().find(|l| l.media_type == media_core::models::MediaType::Movie),
            "sonarr" => libraries.into_iter().find(|l| l.media_type == media_core::models::MediaType::Tv),
            _ => libraries.into_iter().next(),
        };

        if let Some(lib) = target_lib {
            scanner_service.scan_library(&lib, task_id).await.ok();
        }
    });

    StatusCode::ACCEPTED
}
