use axum::{
    routing::{get, post},
    Router,
    extract::State,
    Json,
    response::{Sse, IntoResponse},
    http::{header, StatusCode},
};
use axum::response::sse::{Event, KeepAlive};
use std::sync::Arc;
use futures::stream::Stream;
use std::convert::Infallible;
use media_core::exporter::Exporter;
use crate::state::AppState;
use media_core::db::{MovieReader, TvReader, SettingsRepository};
use sqlx::Row;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/tasks", get(get_tasks))
        .route("/tasks/stream", get(task_stream))
        .route("/export/csv", get(export_csv))
        .route("/export/html", get(export_html))
        .route("/export/xlsx", get(export_xlsx))
        .route("/export/json", get(export_json))
        .route("/maintenance/backup", post(create_backup))
        .route("/system/update-check", get(check_updates))
        .route("/system/disk-space", get(get_disk_space))
        .route("/sync/trakt", post(sync_trakt))
        .route("/settings", get(get_settings).post(set_settings))
}

async fn export_csv(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    let tv_shows = state.repos.tv.find_all_shows(None, None, None).await.unwrap_or_default();
    let csv = Exporter::to_csv(&movies, &tv_shows);
    (
        [
            (header::CONTENT_TYPE, "text/csv"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"library.csv\""),
        ],
        csv,
    )
}

async fn export_html(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    let tv_shows = state.repos.tv.find_all_shows(None, None, None).await.unwrap_or_default();
    let html = Exporter::to_html(&movies, &tv_shows);
    (
        [(header::CONTENT_TYPE, "text/html")],
        html,
    )
}

async fn export_xlsx(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    let tv_shows = state.repos.tv.find_all_shows(None, None, None).await.unwrap_or_default();
    match Exporter::to_xlsx(&movies, &tv_shows) {
        Ok(bytes) => {
            (
                [
                    (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                    (header::CONTENT_DISPOSITION, "attachment; filename=\"library.xlsx\""),
                ],
                bytes,
            ).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn export_json(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    let tv_shows = state.repos.tv.find_all_shows(None, None, None).await.unwrap_or_default();
    match Exporter::to_json(&movies, &tv_shows) {
        Ok(json) => {
            (
                [
                    (header::CONTENT_TYPE, "application/json"),
                    (header::CONTENT_DISPOSITION, "attachment; filename=\"library.json\""),
                ],
                json,
            ).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn sync_trakt(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let _permit = state.task_manager.acquire_heavy_permit().await;
    
    // Check if Trakt OAuth is configured
    let settings_map = state.repos.settings.get_all().await.unwrap_or_default();
    let access_token = match settings_map.get("trakt_access_token") {
        Some(t) if !t.is_empty() => t.clone(),
        _ => return (StatusCode::BAD_REQUEST, "Trakt is not authenticated. Please configure Trakt OAuth first.").into_response(),
    };

    let scraper_clients = media_core::scraper::ScraperClients::from_settings(&state.repos).await;

    // Get all movies
    let movies = state.repos.movie.find_all(None, None, None).await.unwrap_or_default();
    
    let mut trakt_movies = Vec::new();
    for m in movies {
        if let Some(tmdb) = m.tmdb_id {
            trakt_movies.push(serde_json::json!({
                "ids": {
                    "tmdb": tmdb,
                    "imdb": m.imdb_id
                }
            }));
        }
    }

    if !trakt_movies.is_empty() {
        match scraper_clients.trakt.add_to_collection(&access_token, trakt_movies).await {
            Ok(res) => (StatusCode::OK, Json(res)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    } else {
        (StatusCode::OK, Json(serde_json::json!({"added": 0}))).into_response()
    }
}

async fn task_stream(State(state): State<Arc<AppState>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.task_manager.subscribe();
    let stream = async_stream::stream! {
        while let Ok(update) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&update) {
                yield Ok(Event::default().data(json));
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn get_tasks(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tasks = state.task_manager.get_history();
    Json(tasks).into_response()
}

async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.repos.settings.get_all().await {
        Ok(settings) => Json(settings).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn set_settings(
    State(state): State<Arc<AppState>>, 
    Json(payload): Json<std::collections::HashMap<String, String>>
) -> impl IntoResponse {
    for (key, value) in payload {
        if let Err(e) = state.repos.settings.set(&key, &value).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }
    StatusCode::OK.into_response()
}

async fn create_backup(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let backup_dir = std::path::Path::new("backups");

    // First export all NFOs
    let _ = media_core::maintenance::MaintenanceEngine::export_all_nfos(&state.repos).await;

    match media_core::maintenance::MaintenanceEngine::create_backup(&state.pool, backup_dir).await {
        Ok(path) => (StatusCode::OK, format!("Backup created: {:?}", path)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn check_updates() -> impl IntoResponse {
    match media_core::maintenance::MaintenanceEngine::check_for_updates().await {
        Ok(version) => Json(serde_json::json!({ "latest_version": version, "current_version": "0.1.0" })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_disk_space(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let movies_res = sqlx::query(
        r#"
        SELECT m.id, m.title, m.year, SUM(mf.size_bytes) as total_size
        FROM movies m
        JOIN movie_files mf ON m.id = mf.movie_id
        GROUP BY m.id
        ORDER BY total_size DESC
        "#
    )
    .fetch_all(&state.pool)
    .await;

    let tv_shows_res = sqlx::query(
        r#"
        SELECT t.id, t.title, SUM(e.size_bytes) as total_size
        FROM tv_shows t
        JOIN seasons s ON t.id = s.show_id
        JOIN episodes e ON s.id = e.season_id
        GROUP BY t.id
        ORDER BY total_size DESC
        "#
    )
    .fetch_all(&state.pool)
    .await;

    let movies = match movies_res {
        Ok(rows) => rows.into_iter().map(|row| {
            serde_json::json!({
                "id": row.get::<i64, _>("id"),
                "title": row.get::<String, _>("title"),
                "year": row.get::<Option<i32>, _>("year"),
                "size_bytes": row.get::<i64, _>("total_size"),
            })
        }).collect::<Vec<_>>(),
        Err(_) => vec![],
    };

    let tv_shows = match tv_shows_res {
        Ok(rows) => rows.into_iter().map(|row| {
            serde_json::json!({
                "id": row.get::<i64, _>("id"),
                "title": row.get::<String, _>("title"),
                "size_bytes": row.get::<i64, _>("total_size"),
            })
        }).collect::<Vec<_>>(),
        Err(_) => vec![],
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "movies": movies,
            "tv_shows": tv_shows,
        }))
    )
}
