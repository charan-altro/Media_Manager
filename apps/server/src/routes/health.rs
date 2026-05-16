use axum::{routing::get, Router, response::IntoResponse, http::StatusCode};
use std::sync::Arc;
use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health_check))
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
