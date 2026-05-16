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
use crate::utils::now_ms;
use media_core::db;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/webhooks/:source", post(handle_webhook))
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
