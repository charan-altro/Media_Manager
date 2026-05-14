// core/src/db/queries.rs
use sqlx::sqlite::SqlitePool;
use crate::db::Result;
use crate::models::{Library, MediaType, Movie, LibraryId, MovieId, TvShowId, SeasonId, EpisodeId, MovieFileId};

pub async fn delete_library(pool: &SqlitePool, id: LibraryId) -> Result<()> {
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

pub async fn insert_library(pool: &SqlitePool, name: &str, path: &str, media_type: MediaType) -> Result<LibraryId> {
    let mt_str = match media_type {
        MediaType::Movie => "movie",
        MediaType::Tv => "tv",
    };
    
    let normalized_path = crate::paths::normalize_slashes(path);
    
    sqlx::query(
        r#"
        INSERT INTO libraries (name, path, media_type)
        VALUES (?, ?, ?)
        ON CONFLICT(path) DO UPDATE SET name=excluded.name
        "#
    )
    .bind(name)
    .bind(&normalized_path)
    .bind(mt_str)
    .execute(pool)
    .await?;

    let row: (LibraryId,) = sqlx::query_as("SELECT id FROM libraries WHERE path = ?")
        .bind(&normalized_path)
        .fetch_one(pool)
        .await?;
    
    Ok(row.0)
}

pub async fn upsert_movie<'c, E>(executor: E, library_id: LibraryId, title: &str, year: Option<i32>) -> Result<MovieId> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let row: (MovieId,) = sqlx::query_as(
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

pub async fn upsert_tv_show<'c, E>(executor: E, library_id: LibraryId, title: &str) -> Result<TvShowId> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let row: (TvShowId,) = sqlx::query_as(
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

pub async fn upsert_season<'c, E>(executor: E, show_id: TvShowId, season_number: i32) -> Result<SeasonId> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let row: (SeasonId,) = sqlx::query_as(
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
    season_id: SeasonId, 
    episode_number: i32, 
    file_path: &str, 
    original_name: &str, 
    size_bytes: i64,
    mtime: Option<i64>,
    resolution: Option<crate::models::Resolution>,
    codec: Option<&str>,
    duration_secs: Option<i32>,
    hash: Option<&str>,
    fingerprint: Option<&str>
) -> Result<EpisodeId> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let normalized_path = crate::paths::normalize_slashes(file_path);
    let row: (EpisodeId,) = sqlx::query_as(
        r#"
        INSERT INTO episodes (season_id, episode_number, file_path, original_name, size_bytes, mtime, resolution, codec, duration_secs, hash, fingerprint, is_missing, last_scanned)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'))
        ON CONFLICT(file_path) DO UPDATE SET 
            size_bytes = excluded.size_bytes,
            mtime = excluded.mtime,
            resolution = excluded.resolution,
            codec = excluded.codec,
            duration_secs = excluded.duration_secs,
            hash = excluded.hash,
            fingerprint = excluded.fingerprint,
            is_missing = 0,
            last_scanned = datetime('now'),
            updated_at = datetime('now')
        RETURNING id
        "#
    )
    .bind(season_id)
    .bind(episode_number)
    .bind(&normalized_path)
    .bind(original_name)
    .bind(size_bytes)
    .bind(mtime)
    .bind(resolution)
    .bind(codec)
    .bind(duration_secs)
    .bind(hash)
    .bind(fingerprint)
    .fetch_one(executor)
    .await?;

    Ok(row.0)
}

pub async fn upsert_movie_file<'c, E>(
    executor: E, 
    movie_id: MovieId, 
    file_path: &str, 
    original_name: &str, 
    size_bytes: i64,
    mtime: Option<i64>,
    resolution: Option<crate::models::Resolution>,
    codec: Option<&str>,
    duration_secs: Option<i32>,
    hash: Option<&str>,
    fingerprint: Option<&str>
) -> Result<MovieFileId> 
where E: sqlx::Executor<'c, Database = sqlx::Sqlite> {
    let normalized_path = crate::paths::normalize_slashes(file_path);
    let row: (MovieFileId,) = sqlx::query_as(
        r#"
        INSERT INTO movie_files (movie_id, file_path, original_name, size_bytes, mtime, resolution, codec, duration_secs, hash, fingerprint, is_missing, last_scanned)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'))
        ON CONFLICT(file_path) DO UPDATE SET 
            size_bytes=excluded.size_bytes,
            mtime=excluded.mtime,
            resolution=excluded.resolution,
            codec=excluded.codec,
            duration_secs=excluded.duration_secs,
            hash=excluded.hash,
            fingerprint=excluded.fingerprint,
            is_missing=0,
            last_scanned=datetime('now'),
            updated_at=datetime('now')
        RETURNING id
        "#
    )
    .bind(movie_id)
    .bind(&normalized_path)
    .bind(original_name)
    .bind(size_bytes)
    .bind(mtime)
    .bind(resolution)
    .bind(codec)
    .bind(duration_secs)
    .bind(hash)
    .bind(fingerprint)
    .fetch_one(executor)
    .await?;

    Ok(row.0)
}

pub async fn get_all_movies(
    pool: &SqlitePool, 
    library_id: Option<LibraryId>,
    genre: Option<String>,
    language: Option<String>
) -> Result<Vec<Movie>> {
    let mut query = String::from("SELECT m.*, mf.preview_path FROM movies m LEFT JOIN movie_files mf ON m.id = mf.movie_id WHERE 1=1");
    if library_id.is_some() {
        query.push_str(" AND m.library_id = ?");
    }
    
    let genre_active = genre.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    if genre_active {
        query.push_str(" AND m.genres LIKE ?");
    }
    
    let language_active = language.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    if language_active {
        query.push_str(" AND m.language = ?");
    }
    query.push_str(" GROUP BY m.id ORDER BY m.title ASC");

    let mut q = sqlx::query_as::<_, Movie>(&query);
    if let Some(id) = library_id {
        q = q.bind(id);
    }
    if genre_active {
        if let Some(ref g) = genre {
            q = q.bind(format!("%\"{}\"%", g));
        }
    }
    if language_active {
        if let Some(ref l) = language {
            q = q.bind(l);
        }
    }

    let movies = q.fetch_all(pool).await?;
    Ok(movies)
}

pub async fn get_all_tv_shows(
    pool: &SqlitePool, 
    library_id: Option<LibraryId>,
    genre: Option<String>,
    language: Option<String>
) -> Result<Vec<crate::models::TVShow>> {
    let mut query = String::from(r#"
        SELECT t.*, 
        (SELECT e.preview_path FROM episodes e JOIN seasons s ON e.season_id = s.id WHERE s.show_id = t.id AND e.preview_path IS NOT NULL LIMIT 1) as preview_path
        FROM tv_shows t WHERE 1=1
    "#);
    if library_id.is_some() {
        query.push_str(" AND t.library_id = ?");
    }
    
    let genre_active = genre.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    if genre_active {
        query.push_str(" AND t.genres LIKE ?");
    }
    
    let language_active = language.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    if language_active {
        query.push_str(" AND t.language = ?");
    }
    query.push_str(" ORDER BY t.title ASC");

    let mut q = sqlx::query_as::<_, crate::models::TVShow>(&query);
    if let Some(id) = library_id {
        q = q.bind(id);
    }
    if genre_active {
        if let Some(ref g) = genre {
            q = q.bind(format!("%\"{}\"%", g));
        }
    }
    if language_active {
        if let Some(ref l) = language {
            q = q.bind(l);
        }
    }

    let shows = q.fetch_all(pool).await?;
    Ok(shows)
}

pub async fn get_seasons_by_show_id(pool: &SqlitePool, show_id: TvShowId) -> Result<Vec<crate::models::Season>> {
    let seasons = sqlx::query_as::<_, crate::models::Season>(
        "SELECT * FROM seasons WHERE show_id = ? ORDER BY season_number ASC"
    )
    .bind(show_id)
    .fetch_all(pool)
    .await?;
    Ok(seasons)
}

pub async fn get_episodes_by_season_id(pool: &SqlitePool, season_id: SeasonId) -> Result<Vec<crate::models::Episode>> {
    let episodes = sqlx::query_as::<_, crate::models::Episode>(
        "SELECT * FROM episodes WHERE season_id = ? ORDER BY episode_number ASC"
    )
    .bind(season_id)
    .fetch_all(pool)
    .await?;
    Ok(episodes)
}

pub async fn get_library_by_id(pool: &SqlitePool, id: LibraryId) -> Result<Option<Library>> {
    let lib = sqlx::query_as::<_, Library>("SELECT * FROM libraries WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(lib)
}

pub async fn get_movie_file_by_path(pool: &SqlitePool, path: &str) -> Result<Option<crate::models::MovieFile>> {
    let normalized = crate::paths::normalize_slashes(path);
    let file = sqlx::query_as::<_, crate::models::MovieFile>("SELECT * FROM movie_files WHERE file_path = ?")
        .bind(normalized)
        .fetch_optional(pool)
        .await?;
    Ok(file)
}

pub async fn get_episode_by_path(pool: &SqlitePool, path: &str) -> Result<Option<crate::models::Episode>> {
    let normalized = crate::paths::normalize_slashes(path);
    let ep = sqlx::query_as::<_, crate::models::Episode>("SELECT * FROM episodes WHERE file_path = ?")
        .bind(normalized)
        .fetch_optional(pool)
        .await?;
    Ok(ep)
}

pub async fn get_movie_file_by_hash(pool: &SqlitePool, hash: &str) -> Result<Option<crate::models::MovieFile>> {
    let file = sqlx::query_as::<_, crate::models::MovieFile>("SELECT * FROM movie_files WHERE hash = ?")
        .bind(hash)
        .fetch_optional(pool)
        .await?;
    Ok(file)
}

pub async fn get_movie_file_by_fingerprint(pool: &SqlitePool, fingerprint: &str) -> Result<Option<crate::models::MovieFile>> {
    let file = sqlx::query_as::<_, crate::models::MovieFile>("SELECT * FROM movie_files WHERE fingerprint = ?")
        .bind(fingerprint)
        .fetch_optional(pool)
        .await?;
    Ok(file)
}

pub async fn get_episode_by_hash(pool: &SqlitePool, hash: &str) -> Result<Option<crate::models::Episode>> {
    let ep = sqlx::query_as::<_, crate::models::Episode>("SELECT * FROM episodes WHERE hash = ?")
        .bind(hash)
        .fetch_optional(pool)
        .await?;
    Ok(ep)
}

pub async fn get_episode_by_fingerprint(pool: &SqlitePool, fingerprint: &str) -> Result<Option<crate::models::Episode>> {
    let ep = sqlx::query_as::<_, crate::models::Episode>("SELECT * FROM episodes WHERE fingerprint = ?")
        .bind(fingerprint)
        .fetch_optional(pool)
        .await?;
    Ok(ep)
}

pub async fn update_movie_file_path(pool: &SqlitePool, id: MovieFileId, new_path: &str) -> Result<()> {
    let normalized = crate::paths::normalize_slashes(new_path);
    sqlx::query("UPDATE movie_files SET file_path = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(normalized)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_episode_path(pool: &SqlitePool, id: EpisodeId, new_path: &str) -> Result<()> {
    let normalized = crate::paths::normalize_slashes(new_path);
    sqlx::query("UPDATE episodes SET file_path = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(normalized)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_episode_last_scanned(pool: &SqlitePool, id: EpisodeId) -> Result<()> {
    sqlx::query("UPDATE episodes SET last_scanned = datetime('now'), is_missing = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_movie_file_last_scanned(pool: &SqlitePool, id: MovieFileId) -> Result<()> {
    sqlx::query("UPDATE movie_files SET last_scanned = datetime('now'), is_missing = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_movie_by_id(pool: &SqlitePool, id: MovieId) -> Result<Option<Movie>> {
    let movie = sqlx::query_as::<_, Movie>("SELECT * FROM movies WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(movie)
}

pub async fn get_unique_genres(pool: &SqlitePool) -> Result<Vec<String>> {
    // H1 fix: query both movies and tv_shows
    let rows: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT genres FROM movies WHERE genres IS NOT NULL
         UNION ALL
         SELECT genres FROM tv_shows WHERE genres IS NOT NULL"
    )
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
    // H1 fix: query both movies and tv_shows
    let rows: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT language FROM movies WHERE language IS NOT NULL
         UNION ALL
         SELECT language FROM tv_shows WHERE language IS NOT NULL"
    )
    .fetch_all(pool)
    .await?;
    
    let langs: std::collections::HashSet<String> = rows.into_iter().filter_map(|(l,)| l).collect();
    let mut result: Vec<String> = langs.into_iter().collect();
    result.sort();
    Ok(result)
}

pub async fn get_movies_by_ids(pool: &SqlitePool, ids: &[MovieId]) -> Result<Vec<Movie>> {
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

pub async fn get_tv_shows_by_ids(pool: &SqlitePool, ids: &[TvShowId]) -> Result<Vec<crate::models::TVShow>> {
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
    id: MovieId, 
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

pub async fn get_tv_show_by_id(pool: &SqlitePool, id: TvShowId) -> Result<Option<crate::models::TVShow>> {
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
    id: TvShowId, 
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

pub async fn get_movie_full_path(pool: &SqlitePool, movie_id: MovieId) -> Result<Option<std::path::PathBuf>> {
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT l.path, mf.file_path 
        FROM movie_files mf 
        JOIN movies m ON mf.movie_id = m.id 
        JOIN libraries l ON m.library_id = l.id 
        WHERE m.id = ? 
        LIMIT 1
        "#
    )
    .bind(movie_id)
    .fetch_optional(pool)
    .await?;

    if let Some((lib_path, rel_path)) = row {
        Ok(Some(crate::paths::make_absolute(&rel_path, std::path::Path::new(&lib_path))))
    } else {
        Ok(None)
    }
}

pub async fn get_episode_full_path(pool: &SqlitePool, episode_id: EpisodeId) -> Result<Option<std::path::PathBuf>> {
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT l.path, e.file_path 
        FROM episodes e 
        JOIN seasons s ON e.season_id = s.id 
        JOIN tv_shows t ON s.show_id = t.id 
        JOIN libraries l ON t.library_id = l.id 
        WHERE e.id = ?
        "#
    )
    .bind(episode_id)
    .fetch_optional(pool)
    .await?;

    if let Some((lib_path, rel_path)) = row {
        Ok(Some(crate::paths::make_absolute(&rel_path, std::path::Path::new(&lib_path))))
    } else {
        Ok(None)
    }
}

