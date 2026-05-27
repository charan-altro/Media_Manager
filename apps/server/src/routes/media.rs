use axum::{
    routing::{get, put, delete, post},
    Router,
    extract::{State, Path, Query},
    Json,
    response::IntoResponse,
    http::StatusCode,
};
use std::sync::Arc;
use media_core::models::{MovieId, TvShowId, LibraryId, SeasonId, MovieFileId, EpisodeId};
use crate::state::AppState;
use media_core::db::{MovieReader, MovieWriter, TvReader, TvWriter, MediaRepository};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/movies", get(get_movies))
        .route("/tvshows", get(get_tv_shows))
        .route("/genres", get(get_genres))
        .route("/languages", get(get_languages))
        .route("/tvshows/:id/seasons", get(get_seasons))
        .route("/seasons/:id/episodes", get(get_episodes))
        .route("/movies/:id", put(update_movie_metadata))
        .route("/tvshows/:id", put(update_tv_show_metadata))
        .route("/movies/:id/files", get(get_movie_files))
        .route("/movies/:id", delete(delete_movie))
        .route("/movies/files/:file_id", delete(delete_movie_file))
        .route("/movies/files/:file_id/play", post(play_movie_file))
        .route("/tvshows/:id", delete(delete_tv_show))
        .route("/episodes/:id", delete(delete_episode))
}

#[derive(serde::Deserialize)]
pub struct MovieQuery { 
    pub library_id: Option<i64>,
    pub genre: Option<String>,
    pub language: Option<String>,
}

async fn get_movies(State(state): State<Arc<AppState>>, Query(query): Query<MovieQuery>) -> impl IntoResponse {
    match state.repos.movie.find_all(query.library_id.map(LibraryId), query.genre, query.language).await {
        Ok(movies) => Json(movies).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch movies: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_tv_shows(State(state): State<Arc<AppState>>, Query(query): Query<MovieQuery>) -> impl IntoResponse {
    match state.repos.tv.find_all_shows(query.library_id.map(LibraryId), query.genre, query.language).await {
        Ok(shows) => Json(shows).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch TV shows: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_seasons(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match state.repos.tv.find_seasons_by_show_id(TvShowId(id)).await {
        Ok(seasons) => Json(seasons).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch seasons: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_episodes(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match state.repos.tv.find_episodes_by_season_id(SeasonId(id)).await {
        Ok(episodes) => Json(episodes).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch episodes: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn get_genres(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.repos.media.get_unique_genres().await {
        Ok(genres) => Json(genres).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

async fn get_languages(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.repos.media.get_unique_languages().await {
        Ok(langs) => Json(langs).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

#[derive(serde::Deserialize)]
pub struct UpdateMovieRequest {
    pub title: String,
    pub year: Option<i32>,
    pub plot: Option<String>,
    pub rating: Option<f32>,
    pub genres: Option<Vec<String>>,
}

async fn update_movie_metadata(
    State(state): State<Arc<AppState>>, 
    Path(id): Path<i64>, 
    Json(payload): Json<UpdateMovieRequest>
) -> impl IntoResponse {
    let genres_json = payload.genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    match state.repos.movie.update(
        MovieId(id), 
        &payload.title, 
        payload.year, 
        payload.plot.as_deref(), 
        payload.rating, 
        genres_json.as_deref()
    ).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct UpdateTvShowRequest {
    pub title: String,
    pub plot: Option<String>,
    pub rating: Option<f32>,
    pub genres: Option<Vec<String>>,
    pub tagline: Option<String>,
    pub runtime: Option<i32>,
    pub language: Option<String>,
    pub trailer_url: Option<String>,
}

async fn update_tv_show_metadata(
    State(state): State<Arc<AppState>>, 
    Path(id): Path<i64>, 
    Json(payload): Json<UpdateTvShowRequest>
) -> impl IntoResponse {
    let genres_json = payload.genres.map(|g| serde_json::to_string(&g).unwrap_or_default());
    match state.repos.tv.update_show(
        TvShowId(id), 
        &payload.title, 
        payload.plot.as_deref(), 
        payload.rating, 
        genres_json.as_deref(),
        payload.tagline.as_deref(),
        payload.runtime,
        payload.language.as_deref(),
        payload.trailer_url.as_deref()
    ).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// --- Multi-version media management and deletion handlers ---

async fn get_movie_files(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.repos.movie.find_files_by_movie_id(MovieId(id)).await {
        Ok(files) => Json(files).into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch movie files: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn delete_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let movie_id = MovieId(id);
    match state.repos.movie.find_files_by_movie_id(movie_id).await {
        Ok(files) => {
            for file in files {
                if let Ok(Some(path)) = state.repos.movie.get_file_full_path(file.id).await {
                    let _ = delete_media_file_and_sidecars(&path);
                    clean_empty_parent_dirs(&path);
                }
                let _ = state.repos.movie.delete_file(file.id).await;
            }
            match state.repos.movie.delete(movie_id).await {
                Ok(_) => StatusCode::OK.into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_movie_file(
    State(state): State<Arc<AppState>>,
    Path(file_id): Path<i64>,
) -> impl IntoResponse {
    let fid = MovieFileId(file_id);
    match state.repos.movie.find_file_by_id(fid).await {
        Ok(Some(file)) => {
            let movie_id = file.movie_id;
            if let Ok(Some(path)) = state.repos.movie.get_file_full_path(fid).await {
                let _ = delete_media_file_and_sidecars(&path);
                clean_empty_parent_dirs(&path);
            }
            match state.repos.movie.delete_file(fid).await {
                Ok(_) => {
                    if let Ok(remaining) = state.repos.movie.find_files_by_movie_id(movie_id).await {
                        if remaining.is_empty() {
                            let _ = state.repos.movie.delete(movie_id).await;
                        }
                    }
                    StatusCode::OK.into_response()
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Movie file not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn play_movie_file(
    State(state): State<Arc<AppState>>,
    Path(file_id): Path<i64>,
) -> impl IntoResponse {
    let fid = MovieFileId(file_id);
    match state.repos.movie.get_file_full_path(fid).await {
        Ok(Some(path)) => {
            match opener::open(path) {
                Ok(_) => StatusCode::OK.into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Movie file not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_tv_show(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let show_id = TvShowId(id);
    match state.repos.tv.find_seasons_by_show_id(show_id).await {
        Ok(seasons) => {
            for season in seasons {
                if let Ok(episodes) = state.repos.tv.find_episodes_by_season_id(season.id).await {
                    for episode in episodes {
                        if let Ok(Some(path)) = state.repos.tv.get_episode_full_path(episode.id).await {
                            let _ = delete_media_file_and_sidecars(&path);
                            let _ = state.repos.tv.delete_episode(episode.id).await;
                            clean_empty_parent_dirs(&path);
                        }
                    }
                }
            }
            match state.repos.tv.delete_show(show_id).await {
                Ok(_) => StatusCode::OK.into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let ep_id = EpisodeId(id);
    match state.repos.tv.find_episode_by_id(ep_id).await {
        Ok(Some(_episode)) => {
            if let Ok(Some(path)) = state.repos.tv.get_episode_full_path(ep_id).await {
                let _ = delete_media_file_and_sidecars(&path);
                clean_empty_parent_dirs(&path);
            }
            match state.repos.tv.delete_episode(ep_id).await {
                Ok(_) => StatusCode::OK.into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Episode not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// --- Helpers for physical media deletion and directory cleanup ---

fn delete_media_file_and_sidecars(file_path: &std::path::Path) -> std::io::Result<()> {
    if !file_path.exists() {
        return Ok(());
    }
    if let (Some(parent), Some(stem)) = (file_path.parent(), file_path.file_stem().and_then(|s| s.to_str())) {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if name.starts_with(stem) {
                            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                                let ext_lower = ext.to_lowercase();
                                if ext_lower == "srt" || ext_lower == "nfo" || ext_lower == "vtt" {
                                    let rest = &name[stem.len()..];
                                    if rest.is_empty() || rest.starts_with('.') {
                                        let _ = std::fs::remove_file(&path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    std::fs::remove_file(file_path)?;
    Ok(())
}

fn clean_empty_parent_dirs(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        if let Ok(mut entries) = std::fs::read_dir(parent) {
            if entries.next().is_none() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}

