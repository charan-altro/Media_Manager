use axum::{
    routing::post,
    Router,
    extract::{State, Path},
    Json,
    response::IntoResponse,
    http::StatusCode,
};
use std::sync::Arc;
use crate::state::AppState;
use media_core::models::{MovieId, TvShowId, LibraryId};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/libraries/:id/scrape", post(bulk_scrape))
        .route("/movies/:id/scrape", post(scrape_single_movie))
        .route("/tvshows/:id/scrape", post(scrape_single_tv_show))
        .route("/scrape/batch", post(scrape_batch))
        .route("/movies/:id/refresh", post(refresh_metadata))
}

async fn bulk_scrape(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let service = state.scraper_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let _ = service.bulk_scrape_library(LibraryId(id), task_id).await;
    });

    Json("Scrape started".to_string())
}

async fn scrape_single_movie(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let service = state.scraper_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let _ = service.scrape_movie(MovieId(id), task_id).await;
    });

    Json("Scrape started".to_string())
}

async fn scrape_single_tv_show(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<String> {
    let service = state.scraper_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        let _ = service.scrape_tv_show(TvShowId(id), task_id).await;
    });

    Json("TV show scrape started".to_string())
}

#[derive(serde::Deserialize)]
pub struct BatchRequest {
    pub ids: Vec<i64>,
    pub media_type: String,
}

async fn scrape_batch(State(state): State<Arc<AppState>>, Json(payload): Json<BatchRequest>) -> Json<String> {
    let service = state.scraper_service.clone();
    
    tokio::spawn(async move {
        for id in payload.ids {
            let task_id = uuid::Uuid::new_v4().to_string();
            if payload.media_type == "movie" {
                let _ = service.scrape_movie(MovieId(id), task_id).await;
            } else {
                let _ = service.scrape_tv_show(TvShowId(id), task_id).await;
            }
        }
    });

    Json("Batch scrape started".to_string())
}

async fn refresh_metadata(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    let service = state.scraper_service.clone();
    let task_id = uuid::Uuid::new_v4().to_string();

    tokio::spawn(async move {
        // Try as movie
        let res = service.scrape_movie(MovieId(id), task_id.clone()).await;
        if res.is_err() {
            // Try as TV show
            let _ = service.scrape_tv_show(TvShowId(id), task_id).await;
        }
    });

    StatusCode::ACCEPTED.into_response()
}
