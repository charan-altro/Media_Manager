// core/src/scraper/mod.rs
pub mod tmdb;
pub mod omdb;
pub mod fanart;
pub mod trakt;
pub mod tvdb;
pub mod anidb;
pub mod imdb;
pub mod moviemeter;
pub mod thesportsdb;
pub mod ofdb;
pub mod kyradb;
pub mod kodi;
pub mod mpdb;
pub mod tvmaze;
pub mod imdbapi;

use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use strsim::jaro_winkler;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScraperSettings {
    pub primary_movie_scraper: String, // "tmdb", "imdb", "universal", "kodi"
    pub primary_tv_scraper: String,    // "tmdb", "tvdb", "anidb"
    pub movie_title_source: String,
    pub movie_plot_source: String,
    pub movie_rating_source: String,
    pub movie_artwork_source: String,
    pub fallback_language: String,
    pub include_adult: bool,
}

impl Default for ScraperSettings {
    fn default() -> Self {
        Self {
            primary_movie_scraper: "tmdb".to_string(),
            primary_tv_scraper: "tmdb".to_string(),
            movie_title_source: "tmdb".to_string(),
            movie_plot_source: "tmdb".to_string(),
            movie_rating_source: "omdb".to_string(),
            movie_artwork_source: "fanart".to_string(),
            fallback_language: "en".to_string(),
            include_adult: false,
        }
    }
}

pub trait MediaScraper: Send + Sync {
    fn search_movie<'a>(&'a self, title: &'a str, year: Option<i32>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<crate::scraper::tmdb::TmdbSearchResult>>> + Send + 'a>>;
    fn get_movie_details<'a>(&'a self, id: i32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::scraper::tmdb::TmdbMovieDetails>> + Send + 'a>>;
    fn search_tv_show<'a>(&'a self, title: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<crate::scraper::tmdb::TmdbTvSearchResult>>> + Send + 'a>>;
    fn get_tv_details<'a>(&'a self, id: i32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::scraper::tmdb::TmdbTvDetails>> + Send + 'a>>;
}

fn clean_for_matching(s: &str) -> String {
    let s = s.to_lowercase();
    let re = regex::Regex::new(r"(?i)\b(the|a|an)\b|[^\w\s]").unwrap();
    re.replace_all(&s, "").trim().replace("  ", " ")
}

pub struct ScraperClients {
    pub tmdb: Box<dyn MediaScraper>,
    pub omdb: omdb::OmdbClient,
    pub fanart: fanart::FanartClient,
    pub trakt: trakt::TraktClient,
    pub tvdb: tvdb::TvdbClient,
    pub anidb: anidb::AnidbClient,
    pub imdb: imdb::ImdbClient,
    pub moviemeter: moviemeter::MovieMeterClient,
    pub sportsdb: thesportsdb::TheSportsDbClient,
    pub ofdb: ofdb::OfdbClient,
    pub kyra: kyradb::KyraDbClient,
    pub mpdb: mpdb::MpdbClient,
    pub tvmaze: tvmaze::TvmazeClient,
    pub imdbapi: imdbapi::ImdbApiClient,
    pub rate_limiter: tokio::sync::Semaphore,
}

impl ScraperClients {
    pub async fn from_settings(pool: &sqlx::SqlitePool) -> Self {
        let settings = crate::db::queries::get_settings(pool).await.unwrap_or_default();
        Self::new(
            std::env::var("TMDB_API_KEY").unwrap_or_else(|_| settings.get("tmdb_api_key").cloned().unwrap_or_default()),
            std::env::var("OMDB_API_KEY").unwrap_or_else(|_| settings.get("omdb_api_key").cloned().unwrap_or_default()),
            std::env::var("FANART_API_KEY").unwrap_or_else(|_| settings.get("fanart_api_key").cloned().unwrap_or_default()),
            std::env::var("TRAKT_API_KEY").unwrap_or_else(|_| settings.get("trakt_api_key").cloned().unwrap_or_default()),
            std::env::var("TVDB_API_KEY").unwrap_or_else(|_| settings.get("tvdb_api_key").cloned().unwrap_or_default()),
            std::env::var("MOVIEMETER_API_KEY").unwrap_or_else(|_| settings.get("moviemeter_api_key").cloned().unwrap_or_default()),
            std::env::var("SPORTSDB_API_KEY").unwrap_or_else(|_| settings.get("sportsdb_api_key").cloned().unwrap_or_default()),
            std::env::var("KYRADB_API_KEY").unwrap_or_else(|_| settings.get("kyradb_api_key").cloned().unwrap_or_default()),
        )
    }

    pub fn new(
        tmdb_key: String, 
        omdb_key: String,
        fanart_key: String,
        trakt_key: String,
        tvdb_key: String,
        moviemeter_key: String,
        sportsdb_key: String,
        kyra_key: String,
    ) -> Self {
        Self {
            tmdb: Box::new(tmdb::TmdbClient::new(tmdb_key)),
            omdb: omdb::OmdbClient::new(omdb_key),
            fanart: fanart::FanartClient::new(fanart_key),
            trakt: trakt::TraktClient::new(trakt_key),
            tvdb: tvdb::TvdbClient::new(tvdb_key),
            anidb: anidb::AnidbClient::new("mediaorchestrator".to_string(), "1".to_string()),
            imdb: imdb::ImdbClient::new(),
            moviemeter: moviemeter::MovieMeterClient::new(moviemeter_key),
            sportsdb: thesportsdb::TheSportsDbClient::new(sportsdb_key),
            ofdb: ofdb::OfdbClient::new(),
            kyra: kyradb::KyraDbClient::new(kyra_key),
            mpdb: mpdb::MpdbClient::new(String::new(), String::new()),
            tvmaze: tvmaze::TvmazeClient::new(),
            imdbapi: imdbapi::ImdbApiClient::new(String::new()),
            rate_limiter: tokio::sync::Semaphore::new(10),
        }
    }
}

pub async fn scrape_movie(
    movie_id: i64,
    title: &str,
    year: Option<i32>,
    clients: &ScraperClients,
    pool: &SqlitePool,
    script_path: Option<&str>,
) -> Result<()> {
    // Acquire rate limit permit
    let _permit = clients.rate_limiter.acquire().await?;

    // Fetch settings for Universal Scraper logic
    let settings_map = crate::db::queries::get_settings(pool).await.unwrap_or_default();
    let settings: ScraperSettings = settings_map.get("scraper_settings")
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // Optimization: Check if we already have a tmdb_id to skip searching
    let existing: Option<(Option<i32>, Option<String>)> = sqlx::query_as("SELECT tmdb_id, imdb_id FROM movies WHERE id = ?")
        .bind(movie_id)
        .fetch_optional(pool)
        .await?;

    let (mut tmdb_id, mut imdb_id) = match existing {
        Some((tid, iid)) => (tid, iid),
        None => (None, None),
    };

    if tmdb_id.is_none() {
        // Search logic
        let mut tmdb_results = clients.tmdb.search_movie(title, year).await?;

        if tmdb_results.is_empty() && year.is_some() {
            tmdb_results = clients.tmdb.search_movie(title, None).await?;
        }

        // Try IMDb Search if enabled or as fallback
        if tmdb_results.is_empty() || settings.primary_movie_scraper == "imdb" {
            if let Ok(imdb_results) = clients.imdb.search(title).await {
                if let Some(best_imdb) = imdb_results.first() {
                    imdb_id = Some(best_imdb.id.clone());
                }
            }
        }

        // Fallback: Try Trakt if TMDB returned nothing
        if tmdb_results.is_empty() && clients.trakt.is_configured() {
            tracing::info!("Trying Trakt fallback for movie '{}'", title);
            if let Ok(trakt_results) = clients.trakt.search_movie(title).await {
                if let Some(best) = trakt_results.first() {
                    if let Some(ref movie) = best.movie {
                        if let Some(tid) = movie.ids.tmdb {
                            tmdb_id = Some(tid);
                        }
                        if imdb_id.is_none() {
                            imdb_id = movie.ids.imdb.clone();
                        }
                    }
                }
            }
        }

        // Fallback: Try MovieMeter (Dutch) if configured
        if tmdb_results.is_empty() && settings.primary_movie_scraper == "moviemeter" {
            tracing::info!("Trying MovieMeter for '{}'", title);
            if let Ok(mm_results) = clients.moviemeter.search(title).await {
                if let Some(best) = mm_results.first() {
                    // Use the MovieMeter title to retry TMDB
                    let retry = clients.tmdb.search_movie(&best.title, best.year).await.unwrap_or_default();
                    if let Some(r) = find_best_movie_match(&best.title, best.year, &retry) {
                        tmdb_id = Some(r.id);
                    }
                }
            }
        }

        // Fallback: Try OFDb (German) if configured
        if tmdb_results.is_empty() && settings.primary_movie_scraper == "ofdb" {
            tracing::info!("Trying OFDb for '{}'", title);
            if let Ok(ofdb_results) = clients.ofdb.search(title).await {
                if let Some((ofdb_title, _)) = ofdb_results.first() {
                    let retry = clients.tmdb.search_movie(ofdb_title, year).await.unwrap_or_default();
                    if let Some(r) = find_best_movie_match(ofdb_title, year, &retry) {
                        tmdb_id = Some(r.id);
                    }
                }
            }
        }

        // Fallback: Try MPDb (French) if configured
        if tmdb_results.is_empty() && clients.mpdb.is_configured() {
            tracing::info!("Trying MPDb for '{}'", title);
            if let Ok(mpdb_results) = clients.mpdb.search(title).await {
                if let Some(best) = mpdb_results.first() {
                    let retry = clients.tmdb.search_movie(&best.title, best.year).await.unwrap_or_default();
                    if let Some(r) = find_best_movie_match(&best.title, best.year, &retry) {
                        tmdb_id = Some(r.id);
                    }
                }
            }
        }

        let best_match = find_best_movie_match(title, year, &tmdb_results);
        if let Some(tmdb_result) = best_match {
            tmdb_id = Some(tmdb_result.id);
        }
    }

    if tmdb_id.is_none() && imdb_id.is_none() {
        tracing::warn!("No match found for '{}' ({:?}) across TMDB, IMDb, Trakt, MovieMeter, OFDb, MPDb", title, year);
        return Ok(());
    }

    // Fetch primary details from TMDB
    let details = if let Some(tid) = tmdb_id {
        Some(clients.tmdb.get_movie_details(tid).await?)
    } else {
        None
    };
    
    // UNIVERSAL SCRAPER LOGIC: Combine data from sources
    let final_title = details.as_ref().map(|d| d.title.clone()).unwrap_or_else(|| title.to_string());
    let mut final_plot = details.as_ref().and_then(|d| d.overview.clone());
    let mut final_rating = details.as_ref().map(|d| d.vote_average).unwrap_or(0.0);
    let final_imdb_id = imdb_id.or_else(|| details.as_ref().and_then(|d| d.imdb_id.clone()));
    
    // 1. Plot from IMDb if requested
    if settings.movie_plot_source == "imdb" && final_imdb_id.is_some() {
        if let Ok(imdb_details) = clients.imdb.get_details(final_imdb_id.as_ref().unwrap()).await {
            if let Some(plot) = imdb_details.get("description").and_then(|v| v.as_str()) {
                final_plot = Some(plot.to_string());
            }
        }
    }

    // 2. Rating from OMDb if requested
    if (settings.movie_rating_source == "omdb" || settings.movie_rating_source == "universal") && final_imdb_id.is_some() {
        if let Ok(omdb_data) = clients.omdb.get_ratings(final_imdb_id.as_ref().unwrap()).await {
            if let Some(ref r_str) = omdb_data.imdb_rating {
                if let Ok(r) = r_str.parse::<f32>() {
                    final_rating = r;
                }
            }
        }
    }

    // 3. Artwork from Fanart.tv if requested
    let mut poster_url = details.as_ref().and_then(|d| d.poster_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/original{}", p)));
    let mut backdrop_url = details.as_ref().and_then(|d| d.backdrop_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/original{}", p)));

    if (settings.movie_artwork_source == "fanart" || settings.movie_artwork_source == "universal") && tmdb_id.is_some() {
        if let Ok(fanart_data) = clients.fanart.get_movie_images(tmdb_id.unwrap()).await {
            if let Some(p) = fanart_data.movieposter.and_then(|v| v.first().map(|i| i.url.clone())) {
                poster_url = Some(p);
            }
            if let Some(b) = fanart_data.moviebackground.and_then(|v| v.first().map(|i| i.url.clone())) {
                backdrop_url = Some(b);
            }
        }
    }

    // 4. KyraDB artwork as additional/fallback source
    if settings.movie_artwork_source == "kyradb" && tmdb_id.is_some() {
        if let Ok(kyra_data) = clients.kyra.get_artwork(&tmdb_id.unwrap().to_string(), "movie").await {
            if poster_url.is_none() {
                if let Some(p) = kyra_data.get("poster").and_then(|v| v.as_str()) {
                    poster_url = Some(p.to_string());
                }
            }
            if backdrop_url.is_none() {
                if let Some(b) = kyra_data.get("backdrop").and_then(|v| v.as_str()) {
                    backdrop_url = Some(b.to_string());
                }
            }
        }
    }

    let trailer_url = details.as_ref().and_then(|d| d.videos.results.iter()
        .find(|v| v.site == "YouTube" && v.video_type == "Trailer")
        .map(|v| format!("https://www.youtube.com/watch?v={}", v.key)));

    let genres_json = details.as_ref().map(|d| serde_json::to_string(&d.genres.iter().map(|g| &g.name).collect::<Vec<_>>()).unwrap_or_default());
    let language = details.as_ref().and_then(|d| d.spoken_languages.first().map(|l| l.english_name.as_ref().unwrap_or(&l.name).clone()))
        .or(details.as_ref().and_then(|d| d.original_language.clone()));
    
    let mut final_cast = Vec::new();
    let movie_file: Option<(String,)> = sqlx::query_as("SELECT file_path FROM movie_files WHERE movie_id = ? LIMIT 1")
        .bind(movie_id)
        .fetch_optional(pool)
        .await?;

    if let Some((file_path,)) = movie_file {
        let path = std::path::Path::new(&file_path);
        if let Some(folder) = path.parent() {
            let base_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("movie");
            
            // Download Artwork (Simplified call)
            if let Some(ref url) = poster_url {
                let dest = folder.join(format!("{}-poster.jpg", base_name));
                let _ = download_to_file(url, &dest).await;
                poster_url = Some(dest.to_string_lossy().to_string());
            }
            if let Some(ref url) = backdrop_url {
                let dest = folder.join(format!("{}-fanart.jpg", base_name));
                let _ = download_to_file(url, &dest).await;
                backdrop_url = Some(dest.to_string_lossy().to_string());
            }

            // Cast processing
            if let Some(d) = details.as_ref() {
                let actors_dir = folder.join(".actors");
                let _ = std::fs::create_dir_all(&actors_dir);
                for member in d.credits.cast.iter().take(10) {
                    let mut member_image = None;
                    if let Some(ref p_path) = member.profile_path {
                        let clean_name = member.name.replace(|c: char| !c.is_alphanumeric(), "_");
                        let dest = actors_dir.join(format!("{}.jpg", clean_name));
                        let url = format!("https://image.tmdb.org/t/p/w185{}", p_path);
                        if download_to_file(&url, &dest).await.is_ok() {
                            member_image = Some(dest.to_string_lossy().to_string());
                        }
                    }
                    final_cast.push(crate::models::CastMember {
                        name: member.name.clone(),
                        role: Some(member.character.clone()),
                        image: member_image,
                    });
                }
            }
        }
    }

    let cast_json = serde_json::to_string(&final_cast).ok();

    sqlx::query(
        r#"
        UPDATE movies 
        SET tmdb_id = ?, imdb_id = ?, status = ?, plot = ?, rating = ?, 
            poster_url = ?, backdrop_url = ?, tagline = ?, runtime = ?, genres = ?,
            language = ?, cast_list = ?, trailer_url = ?, updated_at = datetime('now')
        WHERE id = ?
        "#
    )
    .bind(tmdb_id)
    .bind(&final_imdb_id)
    .bind(crate::models::MediaStatus::Matched)
    .bind(&final_plot)
    .bind(final_rating)
    .bind(&poster_url)
    .bind(&backdrop_url)
    .bind(details.as_ref().and_then(|d| d.tagline.clone()))
    .bind(details.as_ref().and_then(|d| d.runtime))
    .bind(genres_json)
    .bind(language)
    .bind(cast_json)
    .bind(trailer_url)
    .bind(movie_id)
    .execute(pool)
    .await?;

    // Fire post-processing hook (C5 fix)
    if let Some(path) = script_path {
        if !path.is_empty() {
            let mut ctx = std::collections::HashMap::new();
            ctx.insert("title".to_string(), final_title.clone());
            ctx.insert("tmdb_id".to_string(), tmdb_id.map(|i| i.to_string()).unwrap_or_default());
            ctx.insert("media_type".to_string(), "movie".to_string());
            if let Some(ref imdb) = final_imdb_id {
                ctx.insert("imdb_id".to_string(), imdb.clone());
            }
            crate::hooks::run_post_processing(path, "scrape_complete", ctx).await;
        }
    }

    Ok(())
}

async fn download_to_file(url: &str, dest: &std::path::Path) -> Result<()> {
    if dest.exists() { return Ok(()); }
    let resp = reqwest::get(url).await?;
    let bytes = resp.bytes().await?;
    std::fs::write(dest, bytes)?;
    Ok(())
}

pub async fn scrape_tv_show(
    show_id: i64,
    title: &str,
    clients: &ScraperClients,
    pool: &SqlitePool,
    script_path: Option<&str>,
) -> Result<()> {
    // Acquire rate limit permit
    let _permit = clients.rate_limiter.acquire().await?;

    // Fetch settings for scraper selection
    let settings_map = crate::db::queries::get_settings(pool).await.unwrap_or_default();
    let settings: ScraperSettings = settings_map.get("scraper_settings")
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let existing: Option<(Option<i32>,)> = sqlx::query_as("SELECT tmdb_id FROM tv_shows WHERE id = ?")
        .bind(show_id)
        .fetch_optional(pool)
        .await?;

    let tmdb_id = if let Some((Some(id),)) = existing {
        Some(id)
    } else {
        // 1. Try TMDB first (default)
        let results = clients.tmdb.search_tv_show(title).await?;
        let best_match = find_best_tv_match(title, &results);
        
        if let Some(result) = best_match {
            Some(result.id)
        } else {
            // 2. Fallback: Try TVDB if TMDB returned no results
            tracing::info!("TMDB returned no results for '{}', trying TVDB fallback...", title);
            match clients.tvdb.search_series(title).await {
                Ok(tvdb_results) => {
                    if let Some(first) = tvdb_results.first() {
                        tracing::info!("TVDB found match: {} (ID: {})", first.name, first.id);
                        // Try to find this show on TMDB using the TVDB name for better metadata
                        let retry = clients.tmdb.search_tv_show(&first.name).await.unwrap_or_default();
                        find_best_tv_match(&first.name, &retry).map(|r| r.id)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    tracing::warn!("TVDB fallback failed for '{}': {}", title, e);
                    None
                }
            }
        }
    };

    // 3. Fallback: Try TVMaze search
    let tmdb_id = if tmdb_id.is_none() {
        tracing::info!("Trying TVMaze fallback for '{}'...", title);
        match clients.tvmaze.search_show(title).await {
            Ok(tvmaze_results) => {
                if let Some(best) = tvmaze_results.first() {
                    // Try to find this show on TMDB using the TVMaze name
                    let retry = clients.tmdb.search_tv_show(&best.show.name).await.unwrap_or_default();
                    find_best_tv_match(&best.show.name, &retry).map(|r| r.id)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    } else {
        tmdb_id
    };

    // 4. Final fallback: Try Trakt search
    let tmdb_id = if tmdb_id.is_none() && clients.trakt.is_configured() {
        tracing::info!("Trying Trakt fallback for '{}'...", title);
        match clients.trakt.search_show(title).await {
            Ok(trakt_results) => {
                trakt_results.iter()
                    .filter_map(|r| r.show.as_ref())
                    .find_map(|s| s.ids.tmdb)
            }
            Err(_) => None,
        }
    } else {
        tmdb_id
    };

    let tmdb_id = match tmdb_id {
        Some(id) => id,
        None => {
            tracing::warn!("No match found for TV show '{}' across TMDB, TVDB, and Trakt", title);
            return Ok(());
        }
    };

    let details = clients.tmdb.get_tv_details(tmdb_id).await?;
    
    // Artwork: Start with TMDB, then try Fanart.tv for higher quality
    let mut poster_url = details.poster_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/original{}", p));
    let mut backdrop_url = details.backdrop_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/original{}", p));

    // Fanart.tv artwork upgrade for TV shows
    if settings.movie_artwork_source == "fanart" {
        if let Ok(fanart_data) = clients.fanart.get_movie_images(tmdb_id).await {
            if let Some(p) = fanart_data.movieposter.and_then(|v| v.first().map(|i| i.url.clone())) {
                poster_url = Some(p);
            }
            if let Some(b) = fanart_data.moviebackground.and_then(|v| v.first().map(|i| i.url.clone())) {
                backdrop_url = Some(b);
            }
        }
    }

    // Find the show's folder from a representative episode to download artwork locally
    let show_folder: Option<(String,)> = sqlx::query_as(
        "SELECT e.file_path FROM episodes e JOIN seasons s ON e.season_id = s.id WHERE s.show_id = ? LIMIT 1"
    )
    .bind(show_id)
    .fetch_optional(pool)
    .await?;

    let mut final_cast = Vec::new();

    if let Some((ep_file_path,)) = show_folder {
        let ep_path = std::path::Path::new(&ep_file_path);
        // Walk up: episode → season folder → show folder
        if let Some(season_folder) = ep_path.parent() {
            let show_root = season_folder.parent().unwrap_or(season_folder);

            if let Some(ref url) = poster_url {
                let dest = show_root.join("poster.jpg");
                let _ = download_to_file(url, &dest).await;
                poster_url = Some(dest.to_string_lossy().to_string());
            }
            if let Some(ref url) = backdrop_url {
                let dest = show_root.join("fanart.jpg");
                let _ = download_to_file(url, &dest).await;
                backdrop_url = Some(dest.to_string_lossy().to_string());
            }

            let actors_dir = show_root.join(".actors");
            let _ = std::fs::create_dir_all(&actors_dir);
            for member in details.credits.cast.iter().take(10) {
                let mut member_image = None;
                if let Some(ref p_path) = member.profile_path {
                    let clean_name = member.name.replace(|c: char| !c.is_alphanumeric(), "_");
                    let dest = actors_dir.join(format!("{}.jpg", clean_name));
                    let url = format!("https://image.tmdb.org/t/p/w185{}", p_path);
                    if download_to_file(&url, &dest).await.is_ok() {
                        member_image = Some(dest.to_string_lossy().to_string());
                    }
                }
                final_cast.push(crate::models::CastMember {
                    name: member.name.clone(),
                    role: Some(member.character.clone()),
                    image: member_image,
                });
            }
        }
    }

    let cast_json = serde_json::to_string(&final_cast).ok();
    let trailer_url = details.videos.results.iter()
        .find(|v| v.site == "YouTube" && v.video_type == "Trailer")
        .map(|v| format!("https://www.youtube.com/watch?v={}", v.key));
    let genres_json = serde_json::to_string(&details.genres.iter().map(|g| &g.name).collect::<Vec<_>>()).ok();
    let language = details.spoken_languages.first()
        .map(|l| l.english_name.as_ref().unwrap_or(&l.name).clone())
        .or(details.original_language.clone());

    sqlx::query(
        r#"
        UPDATE tv_shows
        SET tmdb_id = ?, status = ?, plot = ?, rating = ?,
            poster_url = ?, backdrop_url = ?, genres = ?, language = ?,
            cast_list = ?, trailer_url = ?, updated_at = datetime('now')
        WHERE id = ?
        "#
    )
    .bind(tmdb_id)
    .bind(crate::models::MediaStatus::Matched)
    .bind(&details.overview)
    .bind(details.vote_average)
    .bind(&poster_url)
    .bind(&backdrop_url)
    .bind(genres_json)
    .bind(language)
    .bind(cast_json)
    .bind(trailer_url)
    .bind(show_id)
    .execute(pool)
    .await?;

    // Fire post-processing hook (C5 fix)
    if let Some(path) = script_path {
        if !path.is_empty() {
            let mut ctx = std::collections::HashMap::new();
            ctx.insert("title".to_string(), details.name.clone());
            ctx.insert("tmdb_id".to_string(), tmdb_id.to_string());
            ctx.insert("media_type".to_string(), "tv".to_string());
            crate::hooks::run_post_processing(path, "scrape_complete", ctx).await;
        }
    }

    Ok(())
}

fn find_best_movie_match(
    search_title: &str,
    search_year: Option<i32>,
    results: &[crate::scraper::tmdb::TmdbSearchResult],
) -> Option<crate::scraper::tmdb::TmdbSearchResult> {
    let clean_search = clean_for_matching(search_title);
    results.iter().max_by_key(|res| {
        let clean_res = clean_for_matching(&res.title);
        let title_sim = jaro_winkler(&clean_search, &clean_res);
        
        let res_year = res.release_date.as_ref()
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse::<i32>().ok());

        let year_score = match (search_year, res_year) {
            (Some(sy), Some(ry)) => {
                if sy == ry { 1.0 }
                else if (sy - ry).abs() == 1 { 0.8 }
                else { 0.0 }
            },
            _ => 0.5,
        };

        ( (title_sim * 0.7 + year_score * 0.3) * 1000.0 ) as i32
    }).cloned()
}

fn find_best_tv_match(
    search_title: &str,
    results: &[crate::scraper::tmdb::TmdbTvSearchResult],
) -> Option<crate::scraper::tmdb::TmdbTvSearchResult> {
    let clean_search = clean_for_matching(search_title);
    results.iter().max_by_key(|res| {
        let clean_res = clean_for_matching(&res.name);
        let title_sim = jaro_winkler(&clean_search, &clean_res);
        (title_sim * 1000.0) as i32
    }).cloned()
}

