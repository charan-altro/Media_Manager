// core/src/db/queries.rs
use sqlx::sqlite::SqlitePool;
use anyhow::Result;
use crate::models::{Library, MediaType, Movie};

pub async fn delete_library(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_all_libraries(pool: &SqlitePool) -> Result<Vec<Library>> {
    let libraries = sqlx::query_as::<_, Library>(
        r#"
        SELECT id, name, path, media_type, created_at
        FROM libraries
        "#
    )
    .fetch_all(pool)
    .await?;
    
    Ok(libraries)
}

pub async fn insert_library(pool: &SqlitePool, name: &str, path: &str, media_type: MediaType) -> Result<i64> {
    let mt_str = match media_type {
        MediaType::Movie => "movie",
        MediaType::Tv => "tv",
    };
    
    sqlx::query(
        r#"
        INSERT INTO libraries (name, path, media_type)
        VALUES (?, ?, ?)
        ON CONFLICT(path) DO UPDATE SET name=excluded.name
        "#
    )
    .bind(name)
    .bind(path)
    .bind(mt_str)
    .execute(pool)
    .await?;

    let row: (i64,) = sqlx::query_as("SELECT id FROM libraries WHERE path = ?")
        .bind(path)
        .fetch_one(pool)
        .await?;
    
    Ok(row.0)
}

pub async fn upsert_movie<'c, E>(executor: E, library_id: i64, title: &str, year: Option<i32>) -> Result<i64> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO movies (library_id, title, year)
        VALUES (?, ?, ?)
        ON CONFLICT(library_id, title, IFNULL(year, 0)) DO UPDATE SET updated_at = datetime('now')
        RETURNING id
        "#
    )
    .bind(library_id)
    .bind(title)
    .bind(year)
    .fetch_one(executor)
    .await?;

    Ok(row.0)
}

pub async fn upsert_tv_show<'c, E>(executor: E, library_id: i64, title: &str) -> Result<i64> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO tv_shows (library_id, title) 
        VALUES (?, ?) 
        ON CONFLICT(library_id, title) DO UPDATE SET updated_at = datetime('now')
        RETURNING id
        "#
    )
    .bind(library_id)
    .bind(title)
    .fetch_one(executor)
    .await?;

    Ok(row.0)
}

pub async fn upsert_season<'c, E>(executor: E, show_id: i64, season_number: i32) -> Result<i64> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO seasons (show_id, season_number) 
        VALUES (?, ?) 
        ON CONFLICT(show_id, season_number) DO UPDATE SET updated_at = datetime('now')
        RETURNING id
        "#
    )
    .bind(show_id)
    .bind(season_number)
    .fetch_one(executor)
    .await?;

    Ok(row.0)
}

pub async fn upsert_episode<'c, E>(
    executor: E, 
    season_id: i64, 
    episode_number: i32, 
    file_path: &str, 
    original_name: &str, 
    size_bytes: i64,
    resolution: Option<crate::models::Resolution>,
    codec: Option<&str>
) -> Result<i64> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO episodes (season_id, episode_number, file_path, original_name, size_bytes, resolution, codec)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(file_path) DO UPDATE SET 
            size_bytes = excluded.size_bytes,
            resolution = excluded.resolution,
            codec = excluded.codec,
            updated_at = datetime('now')
        RETURNING id
        "#
    )
    .bind(season_id)
    .bind(episode_number)
    .bind(file_path)
    .bind(original_name)
    .bind(size_bytes)
    .bind(resolution)
    .bind(codec)
    .fetch_one(executor)
    .await?;

    Ok(row.0)
}

/// Insert or ignore a movie_file record (keyed on file_path).
pub async fn upsert_movie_file<'c, E>(
    executor: E, 
    movie_id: i64, 
    file_path: &str, 
    original_name: &str, 
    size_bytes: i64,
    resolution: Option<crate::models::Resolution>,
    codec: Option<&str>
) -> Result<i64> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO movie_files (movie_id, file_path, original_name, size_bytes, resolution, codec)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(file_path) DO UPDATE SET 
            size_bytes=excluded.size_bytes,
            resolution=excluded.resolution,
            codec=excluded.codec,
            updated_at=datetime('now')
        RETURNING id
        "#
    )
    .bind(movie_id)
    .bind(file_path)
    .bind(original_name)
    .bind(size_bytes)
    .bind(resolution)
    .bind(codec)
    .fetch_one(executor)
    .await?;

    Ok(row.0)
}

pub async fn get_all_movies(
    pool: &SqlitePool, 
    library_id: Option<i64>,
    genre: Option<String>,
    language: Option<String>
) -> Result<Vec<Movie>> {
    let mut query = String::from("SELECT * FROM movies WHERE 1=1");
    if library_id.is_some() {
        query.push_str(" AND library_id = ?");
    }
    if genre.is_some() {
        query.push_str(" AND genres LIKE ?");
    }
    if language.is_some() {
        query.push_str(" AND language = ?");
    }
    query.push_str(" ORDER BY title ASC");

    let mut q = sqlx::query_as::<_, Movie>(&query);
    if let Some(id) = library_id {
        q = q.bind(id);
    }
    if let Some(ref g) = genre {
        q = q.bind(format!("%\"{}\"%", g));
    }
    if let Some(ref l) = language {
        q = q.bind(l);
    }

    let movies = q.fetch_all(pool).await?;
    Ok(movies)
}

pub async fn get_all_tv_shows(
    pool: &SqlitePool, 
    library_id: Option<i64>,
    genre: Option<String>,
    language: Option<String>
) -> Result<Vec<crate::models::TVShow>> {
    let mut query = String::from("SELECT * FROM tv_shows WHERE 1=1");
    if library_id.is_some() {
        query.push_str(" AND library_id = ?");
    }
    if genre.is_some() {
        query.push_str(" AND genres LIKE ?");
    }
    if language.is_some() {
        query.push_str(" AND language = ?");
    }
    query.push_str(" ORDER BY title ASC");

    let mut q = sqlx::query_as::<_, crate::models::TVShow>(&query);
    if let Some(id) = library_id {
        q = q.bind(id);
    }
    if let Some(ref g) = genre {
        q = q.bind(format!("%\"{}\"%", g));
    }
    if let Some(ref l) = language {
        q = q.bind(l);
    }

    let shows = q.fetch_all(pool).await?;
    Ok(shows)
}

pub async fn get_seasons_by_show_id(pool: &SqlitePool, show_id: i64) -> Result<Vec<crate::models::Season>> {
    let seasons = sqlx::query_as::<_, crate::models::Season>(
        "SELECT * FROM seasons WHERE show_id = ? ORDER BY season_number ASC"
    )
    .bind(show_id)
    .fetch_all(pool)
    .await?;
    Ok(seasons)
}

pub async fn get_episodes_by_season_id(pool: &SqlitePool, season_id: i64) -> Result<Vec<crate::models::Episode>> {
    let episodes = sqlx::query_as::<_, crate::models::Episode>(
        "SELECT * FROM episodes WHERE season_id = ? ORDER BY episode_number ASC"
    )
    .bind(season_id)
    .fetch_all(pool)
    .await?;
    Ok(episodes)
}

pub async fn get_movie_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Movie>> {
    let movie = sqlx::query_as::<_, Movie>("SELECT * FROM movies WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(movie)
}

pub async fn get_unique_genres(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows: Vec<(Option<String>,)> = sqlx::query_as("SELECT DISTINCT genres FROM movies WHERE genres IS NOT NULL")
        .fetch_all(pool)
        .await?;
    
    let mut genres = std::collections::HashSet::new();
    for (row,) in rows {
        if let Some(json_str) = row {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&json_str) {
                for g in list { genres.insert(g); }
            }
        }
    }
    
    let mut result: Vec<String> = genres.into_iter().collect();
    result.sort();
    Ok(result)
}

pub async fn get_unique_languages(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows: Vec<(Option<String>,)> = sqlx::query_as("SELECT DISTINCT language FROM movies WHERE language IS NOT NULL")
        .fetch_all(pool)
        .await?;
    
    let mut langs: Vec<String> = rows.into_iter().filter_map(|(l,)| l).collect();
    langs.sort();
    Ok(langs)
}

pub async fn get_movies_by_ids(pool: &SqlitePool, ids: &[i64]) -> Result<Vec<Movie>> {
    if ids.is_empty() { return Ok(vec![]); }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!("SELECT * FROM movies WHERE id IN ({})", placeholders);
    
    let mut q = sqlx::query_as::<_, Movie>(&query);
    for id in ids {
        q = q.bind(*id);
    }
    let movies = q.fetch_all(pool).await?;
    Ok(movies)
}

pub async fn get_tv_shows_by_ids(pool: &SqlitePool, ids: &[i64]) -> Result<Vec<crate::models::TVShow>> {
    if ids.is_empty() { return Ok(vec![]); }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!("SELECT id, library_id, title, tmdb_id, imdb_id, status, plot, rating, poster_url, backdrop_url, tagline, runtime, genres, language, cast_list, trailer_url, nfo_path, created_at, updated_at FROM tv_shows WHERE id IN ({})", placeholders);
    
    let mut q = sqlx::query_as::<_, crate::models::TVShow>(&query);
    for id in ids {
        q = q.bind(*id);
    }
    let shows = q.fetch_all(pool).await?;
    Ok(shows)
}

pub async fn update_movie(
    pool: &SqlitePool, 
    id: i64, 
    title: &str, 
    year: Option<i32>, 
    plot: Option<&str>,
    rating: Option<f32>,
    genres: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE movies 
        SET title = ?, year = ?, plot = ?, rating = ?, genres = ?, updated_at = datetime('now')
        WHERE id = ?
        "#
    )
    .bind(title)
    .bind(year)
    .bind(plot)
    .bind(rating)
    .bind(genres)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_tv_show_by_id(pool: &SqlitePool, id: i64) -> Result<Option<crate::models::TVShow>> {
    let show = sqlx::query_as::<_, crate::models::TVShow>(
        "SELECT id, library_id, title, tmdb_id, imdb_id, status, plot, rating, poster_url, backdrop_url, tagline, runtime, genres, language, cast_list, trailer_url, nfo_path, created_at, updated_at FROM tv_shows WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(show)
}

pub async fn update_tv_show(
    pool: &SqlitePool, 
    id: i64, 
    title: &str, 
    plot: Option<&str>,
    rating: Option<f32>,
    genres: Option<&str>,
    tagline: Option<&str>,
    runtime: Option<i32>,
    language: Option<&str>,
    trailer_url: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE tv_shows 
        SET title = ?, plot = ?, rating = ?, genres = ?, tagline = ?, runtime = ?, language = ?, trailer_url = ?, updated_at = datetime('now')
        WHERE id = ?
        "#
    )
    .bind(title)
    .bind(plot)
    .bind(rating)
    .bind(genres)
    .bind(tagline)
    .bind(runtime)
    .bind(language)
    .bind(trailer_url)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_settings(pool: &SqlitePool) -> Result<std::collections::HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}
