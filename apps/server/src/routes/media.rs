use axum::{
    routing::{get, put},
    Router,
    extract::{State, Path, Query},
    Json,
    response::IntoResponse,
    http::StatusCode,
};
use std::sync::Arc;
use media_core::models::{MovieId, TvShowId, LibraryId, SeasonId};
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
