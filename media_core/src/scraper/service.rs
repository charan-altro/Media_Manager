// media_core/src/scraper/service.rs
#![allow(async_fn_in_trait)]
use std::sync::Arc;
use crate::errors::{CoreError, Result};
use crate::models::{MovieId, TvShowId, LibraryId, MediaStatus, CastMember, TaskUpdate, now_ms};
use crate::db::{Repositories, MovieReader, MovieWriter, TvReader, TvWriter, SettingsRepository};
use crate::task_manager::ProgressSink;
use crate::scraper::{ScraperClients, ScraperSettings};
use strsim::jaro_winkler;
use futures::StreamExt;
use regex::Regex;
use serde_json;
use tracing::Instrument;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ScraperService: Send + Sync {
    async fn scrape_movie(
        &self,
        movie_id: MovieId,
        task_id: String,
    ) -> Result<()>;

    async fn scrape_tv_show(
        &self,
        show_id: TvShowId,
        task_id: String,
    ) -> Result<()>;

    async fn bulk_scrape_library(
        &self,
        library_id: LibraryId,
        task_id: String,
    ) -> Result<()>;
}

pub struct DefaultScraperService {
    pub repos: Arc<Repositories>,
    pub progress: Arc<dyn ProgressSink>,
    pub clients: Arc<ScraperClients>,
}

impl DefaultScraperService {
    pub fn new(repos: Arc<Repositories>, progress: Arc<dyn ProgressSink>, clients: Arc<ScraperClients>) -> Self {
        Self { repos, progress, clients }
    }
}

#[async_trait::async_trait]
impl ScraperService for DefaultScraperService {
    #[tracing::instrument(skip(self), err)]
    async fn scrape_movie(
        &self,
        movie_id: MovieId,
        _task_id: String,
    ) -> Result<()> {
        let movie = self.repos.movie.find_by_id(movie_id).await?
            .ok_or_else(|| CoreError::Internal(format!("Movie with ID {} not found", movie_id.0)))?;

        let title = &movie.title;
        let year = movie.year;

        // Acquire rate limit permit
        let _permit = self.clients.rate_limiter.acquire().await
            .map_err(|e| CoreError::Internal(format!("Failed to acquire rate limiter permit: {}", e)))?;

        // Fetch settings for scraper selection
        let settings_map = self.repos.settings.get_all().await.unwrap_or_default();
        let settings: ScraperSettings = settings_map.get("scraper_settings")
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let script_path = settings_map.get("post_processing_script").cloned();

        // 1. Search for the movie
        let results = self.clients.tmdb.search_movie(title, year)
            .instrument(tracing::info_span!("tmdb_search_movie", title = title, year = year))
            .await
            .map_err(|e| CoreError::Internal(format!("TMDB movie search failed: {}", e)))?;

        let best_match = find_best_movie_match(title, year, &results);

        let tmdb_id = match best_match {
            Some(m) => Some(m.id),
            None => {
                tracing::warn!("No match found for movie '{}'", title);
                None
            }
        };

        if tmdb_id.is_none() {
            return Ok(());
        }

        let tmdb_id = tmdb_id.unwrap();
        let tmdb_id_i32 = tmdb_id.parse::<i32>().unwrap_or(0);

        // 2. Fetch full details
        let details = self.clients.tmdb.get_movie_details(&tmdb_id)
            .instrument(tracing::info_span!("tmdb_get_movie_details", tmdb_id = tmdb_id))
            .await
            .map_err(|e| CoreError::Internal(format!("Failed to fetch TMDB movie details: {}", e)))?;
        
        // Artwork: Start with TMDB, then try Fanart.tv for higher quality
        let mut poster_url = details.poster_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/original{}", p));
        let mut backdrop_url = details.backdrop_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/original{}", p));

        // Fanart.tv upgrade
        if settings.movie_artwork_source == "fanart" {
            if let Ok(fanart_data) = self.clients.fanart.get_movie_images(tmdb_id_i32)
                .instrument(tracing::info_span!("fanart_get_movie_images", tmdb_id = tmdb_id))
                .await 
            {
                if let Some(p) = fanart_data.movieposter.and_then(|v| v.first().map(|i| i.url.clone())) {
                    poster_url = Some(p);
                }
                if let Some(b) = fanart_data.moviebackground.and_then(|v| v.first().map(|i| i.url.clone())) {
                    backdrop_url = Some(b);
                }
            }
        }

        // Determine final fields based on sources
        let final_title = details.title.clone();
        let final_plot = details.overview.clone();
        let mut final_rating = Some(details.vote_average as f32);
        let final_imdb_id = details.imdb_id.clone();
        
        // OMDB fallback for ratings if configured
        if settings.movie_rating_source == "omdb" && final_imdb_id.is_some() {
            let imdb_id = final_imdb_id.as_ref().unwrap();
            if let Ok(omdb_data) = self.clients.omdb.get_ratings(imdb_id)
                .instrument(tracing::info_span!("omdb_get_ratings", imdb_id = imdb_id))
                .await 
            {
                if let Some(r) = omdb_data.imdb_rating.and_then(|r| r.parse::<f32>().ok()) {
                    final_rating = Some(r);
                }
            }
        }

        let language = details.original_language.clone();

        let genres_json = serde_json::to_string(&details.genres).ok();

        // 3. Localize Artwork if movie folder is known
        let movie_file_path = self.repos.movie.get_full_path(movie_id).await.unwrap_or_default();
        
        let mut final_cast = Vec::new();

        if let Some(path) = movie_file_path {
            if let Some(folder) = path.parent() {
                if let Some(url) = poster_url.as_deref() {
                    let dest = folder.join("poster.jpg");
                    let _ = download_to_file(url, &dest).await;
                    poster_url = Some(dest.to_string_lossy().to_string());
                }
                if let Some(url) = backdrop_url.as_deref() {
                    let dest = folder.join("fanart.jpg");
                    let _ = download_to_file(url, &dest).await;
                    backdrop_url = Some(dest.to_string_lossy().to_string());
                }

                // Cast Images
                let actors_dir = folder.join(".actors");
                let _ = std::fs::create_dir_all(&actors_dir);
                for member in details.cast.iter().take(15) {
                    let mut member_image = None;
                    if let Some(ref p_path) = member.profile_path {
                        let clean_name = member.name.replace(|c: char| !c.is_alphanumeric(), "_");
                        let dest = actors_dir.join(format!("{}.jpg", clean_name));
                        let url = format!("https://image.tmdb.org/t/p/w185{}", p_path);
                        if download_to_file(&url, &dest).await.is_ok() {
                            member_image = Some(dest.to_string_lossy().to_string());
                        }
                    }
                    final_cast.push(CastMember {
                        name: member.name.clone(),
                        role: Some(member.character.clone()),
                        image: member_image,
                    });
                }
            }
        }

        let cast_json = serde_json::to_string(&final_cast).ok();

        {
            self.repos.movie.update_metadata(
                movie_id,
                Some(tmdb_id_i32),
                final_imdb_id.clone(),
                MediaStatus::Matched,
                final_plot,
                final_rating,
                details.tagline.clone(),
                details.runtime,
                genres_json,
                language,
                cast_json,
                poster_url,
                backdrop_url,
            ).await.map_err(|e| CoreError::Internal(format!("Failed to update movie metadata in database: {}", e)))?;
        }

        // Fire post-processing hook
        if let Some(path) = script_path {
            if !path.is_empty() {
                let mut ctx = std::collections::HashMap::new();
                ctx.insert("title".to_string(), final_title.clone());
                ctx.insert("tmdb_id".to_string(), tmdb_id.to_string());
                ctx.insert("media_type".to_string(), "movie".to_string());
                if let Some(ref imdb) = final_imdb_id {
                    ctx.insert("imdb_id".to_string(), imdb.clone());
                }
                crate::hooks::run_post_processing(&path, "scrape_complete", ctx).await;
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn scrape_tv_show(
        &self,
        show_id: TvShowId,
        _task_id: String,
    ) -> Result<()> {
        let existing = self.repos.tv.find_show_by_id(show_id).await?
            .ok_or_else(|| CoreError::Internal(format!("TV show with ID {} not found", show_id.0)))?;

        let title = &existing.title;

        // Acquire rate limit permit
        let _permit = self.clients.rate_limiter.acquire().await
            .map_err(|e| CoreError::Internal(format!("Failed to acquire rate limiter permit: {}", e)))?;

        // Fetch settings for scraper selection
        let settings_map = self.repos.settings.get_all().await.unwrap_or_default();
        let settings: ScraperSettings = settings_map.get("scraper_settings")
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let script_path = settings_map.get("post_processing_script").cloned();

        let tmdb_id = if let Some(id) = existing.tmdb_id {
            Some(id.to_string())
        } else {
            // 1. Try TMDB first (default)
            let results = self.clients.tmdb.search_tv_show(title)
                .instrument(tracing::info_span!("tmdb_search_tv", title = title))
                .await
                .unwrap_or_default();

            let best_match = find_best_tv_match(title, &results);
            
            if let Some(result) = best_match {
                Some(result.id.clone())
            } else {
                // 2. Fallback: Try TVDB if TMDB returned no results
                tracing::info!("TMDB returned no results for '{}', trying TVDB fallback...", title);
                let tvdb_match = {
                    match self.clients.tvdb.search_series(title)
                        .instrument(tracing::info_span!("tvdb_search_series", title = title))
                        .await 
                    {
                        Ok(tvdb_results) => {
                            if let Some(first) = tvdb_results.first() {
                                tracing::info!("TVDB found match: {} (ID: {})", first.name, first.id);
                                // Try to find this show on TMDB using the TVDB name for better metadata
                                let retry = self.clients.tmdb.search_tv_show(&first.name).await.unwrap_or_default();
                                find_best_tv_match(&first.name, &retry).map(|r| r.id.clone())
                            } else {
                                None
                            }
                        }
                        Err(e) => {
                            tracing::warn!("TVDB fallback failed for '{}': {}", title, e);
                            None
                        }
                    }
                };

                // 3. Fallback: Try TVMaze search
                if tvdb_match.is_none() {
                    tracing::info!("Trying TVMaze fallback for '{}'...", title);
                    match self.clients.tvmaze.search_show(title)
                        .instrument(tracing::info_span!("tvmaze_search_show", title = title))
                        .await 
                    {
                        Ok(tvmaze_results) => {
                            if let Some(best) = tvmaze_results.first() {
                                // Try to find this show on TMDB using the TVMaze name
                                let retry = self.clients.tmdb.search_tv_show(&best.show.name).await.unwrap_or_default();
                                find_best_tv_match(&best.show.name, &retry).map(|r| r.id.clone())
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    }
                } else {
                    tvdb_match
                }
            }
        };

        // 4. Final fallback: Try Trakt search
        let tmdb_id = if tmdb_id.is_none() && self.clients.trakt.is_configured() {
            tracing::info!("Trying Trakt fallback for '{}'...", title);
            match self.clients.trakt.search_show(title)
                .instrument(tracing::info_span!("trakt_search_show", title = title))
                .await 
            {
                Ok(trakt_results) => {
                    trakt_results.iter()
                        .filter_map(|r| r.show.as_ref())
                        .find_map(|s| s.ids.tmdb.map(|id| id.to_string()))
                }
                Err(_) => None,
            }
        } else {
            tmdb_id
        };

        let tmdb_id = match tmdb_id {
            Some(id) => id,
            None => {
                tracing::warn!("No match found for TV show '{}' across TMDB, TVDB, TVMaze, and Trakt", title);
                return Ok(());
            }
        };

        let tmdb_id_i32 = tmdb_id.parse::<i32>().unwrap_or(0);

        let details = self.clients.tmdb.get_tv_details(&tmdb_id)
            .instrument(tracing::info_span!("tmdb_get_tv_details", tmdb_id = tmdb_id))
            .await
            .map_err(|e| CoreError::Internal(format!("Failed to fetch TMDB TV details: {}", e)))?;
        
        // Artwork: Start with TMDB, then try Fanart.tv for higher quality
        let mut poster_url = details.poster_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/original{}", p));
        let mut backdrop_url = details.backdrop_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/original{}", p));

        // Fanart.tv artwork upgrade for TV shows
        if settings.movie_artwork_source == "fanart" {
            if let Ok(fanart_data) = self.clients.fanart.get_movie_images(tmdb_id_i32)
                .instrument(tracing::info_span!("fanart_get_tv_images", tmdb_id = tmdb_id_i32))
                .await 
            {
                if let Some(p) = fanart_data.movieposter.and_then(|v| v.first().map(|i| i.url.clone())) {
                    poster_url = Some(p);
                }
                if let Some(b) = fanart_data.moviebackground.and_then(|v| v.first().map(|i| i.url.clone())) {
                    backdrop_url = Some(b);
                }
            }
        }

        // Find the show's folder from a representative episode to download artwork locally
        let seasons = self.repos.tv.find_seasons_by_show_id(show_id).await.unwrap_or_default();
        let show_folder = if let Some(s) = seasons.first() {
            let eps = self.repos.tv.find_episodes_by_season_id(s.id).await.unwrap_or_default();
            eps.first().map(|e| e.file_path.clone())
        } else {
            None
        };

        let mut final_cast = Vec::new();

        if let Some(ep_file_path) = show_folder {
            let ep_path = std::path::Path::new(&ep_file_path);
            // Walk up: episode → season folder → show folder
            if let Some(season_folder) = ep_path.parent() {
                let show_root = season_folder.parent().unwrap_or(season_folder);

                if let Some(url) = poster_url.as_deref() {
                    let dest = show_root.join("poster.jpg");
                    let _ = download_to_file(url, &dest).await;
                    poster_url = Some(dest.to_string_lossy().to_string());
                }
                if let Some(url) = backdrop_url.as_deref() {
                    let dest = show_root.join("fanart.jpg");
                    let _ = download_to_file(url, &dest).await;
                    backdrop_url = Some(dest.to_string_lossy().to_string());
                }

                let actors_dir = show_root.join(".actors");
                let _ = std::fs::create_dir_all(&actors_dir);
                for member in details.cast.iter().take(10) {
                    let mut member_image = None;
                    if let Some(ref p_path) = member.profile_path {
                        let clean_name = member.name.replace(|c: char| !c.is_alphanumeric(), "_");
                        let dest = actors_dir.join(format!("{}.jpg", clean_name));
                        let url = format!("https://image.tmdb.org/t/p/w185{}", p_path);
                        if download_to_file(&url, &dest).await.is_ok() {
                            member_image = Some(dest.to_string_lossy().to_string());
                        }
                    }
                    final_cast.push(CastMember {
                        name: member.name.clone(),
                        role: Some(member.character.clone()),
                        image: member_image,
                    });
                }
            }
        }

        let cast_json = serde_json::to_string(&final_cast).ok();
        let trailer_url = details.videos.iter()
            .find(|v| v.site == "YouTube" && v.video_type == "Trailer")
            .map(|v| format!("https://www.youtube.com/watch?v={}", v.key));
        let genres_json = serde_json::to_string(&details.genres).ok();
        let language = details.original_language.clone();

        {
            self.repos.tv.update_show_metadata(
                show_id,
                Some(tmdb_id_i32),
                details.overview.clone(),
                Some(details.vote_average as f32),
                genres_json,
                language,
                cast_json,
                poster_url,
                backdrop_url,
                trailer_url,
                MediaStatus::Matched,
            ).await.map_err(|e| CoreError::Internal(format!("Failed to update TV show metadata in database: {}", e)))?;
        }

        // Fire post-processing hook
        if let Some(path) = script_path {
            if !path.is_empty() {
                let mut ctx = std::collections::HashMap::new();
                ctx.insert("title".to_string(), details.name.clone());
                ctx.insert("tmdb_id".to_string(), tmdb_id.to_string());
                ctx.insert("media_type".to_string(), "tv".to_string());
                crate::hooks::run_post_processing(&path, "scrape_complete", ctx).await;
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn bulk_scrape_library(
        &self,
        library_id: LibraryId,
        task_id: String,
    ) -> Result<()> {
        let start_ms = now_ms();

        // Fetch settings
        let settings_map = self.repos.settings.get_all().await.unwrap_or_default();
        let _script_path = settings_map.get("post_processing_script").cloned();

        let mut all_tasks = Vec::new();

        if let Ok(movies) = self.repos.movie.find_all(Some(library_id), None, None).await {
            let unmatched: Vec<_> = movies.into_iter().filter(|m| m.status == MediaStatus::Unmatched).collect();
            for m in unmatched {
                all_tasks.push((m.id.0, m.title, m.year, "movie"));
            }
        }

        if let Ok(shows) = self.repos.tv.find_all_shows(Some(library_id), None, None).await {
            let unmatched: Vec<_> = shows.into_iter().filter(|s| s.status == MediaStatus::Unmatched).collect();
            for s in unmatched {
                all_tasks.push((s.id.0, s.title, None, "tv"));
            }
        }

        let total = all_tasks.len() as i32;
        if total == 0 {
            self.progress.broadcast(TaskUpdate {
                task_id: task_id.clone(),
                status: "completed".to_string(),
                progress: 0,
                total: 0,
                message: "No unmatched media found in library".to_string(),
                started_at: Some(start_ms),
                finished_at: Some(now_ms()),
                ..Default::default()
            });
            return Ok(());
        }

        let stream = futures::stream::iter(all_tasks.into_iter().enumerate());
        
        stream.for_each_concurrent(10, |(i, (id, title, _year, m_type))| {
            let task_id = task_id.clone();
            let title_clone = title.clone();
            
            async move {
                let res = if m_type == "movie" {
                    self.scrape_movie(MovieId(id), task_id.clone()).await
                } else {
                    self.scrape_tv_show(TvShowId(id), task_id.clone()).await
                };

                if let Err(e) = res {
                    tracing::error!("Bulk scrape failed for {} ({}): {}", title_clone, m_type, e);
                }
                
                self.progress.broadcast(TaskUpdate {
                    task_id,
                    status: "running".to_string(),
                    progress: (i + 1) as i32,
                    total,
                    message: format!("Processed: {}", title_clone),
                    started_at: Some(start_ms),
                    finished_at: None,
                    debug_info: Some(format!("Scraping {}/{} ({}): {}", i+1, total, m_type, title_clone)),
                    ..Default::default()
                });
            }
        }).await;

        self.progress.broadcast(TaskUpdate {
            task_id,
            status: "completed".to_string(),
            progress: total,
            total,
            message: "Enrichment completed".to_string(),
            started_at: Some(start_ms),
            finished_at: Some(now_ms()),
            ..Default::default()
        });

        Ok(())
    }
}

// --- Helper Functions ---

fn clean_for_matching(s: &str) -> String {
    let s = s.to_lowercase();
    let re = Regex::new(r"(?i)\b(the|a|an)\b|[^\w\s]").unwrap();
    re.replace_all(&s, "").trim().replace("  ", " ")
}

fn find_best_movie_match(
    search_title: &str,
    search_year: Option<i32>,
    results: &[crate::scraper::provider::ScrapedMovieSearchResult],
) -> Option<crate::scraper::provider::ScrapedMovieSearchResult> {
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
    results: &[crate::scraper::provider::ScrapedTvSearchResult],
) -> Option<crate::scraper::provider::ScrapedTvSearchResult> {
    let clean_search = clean_for_matching(search_title);
    results.iter().max_by_key(|res| {
        let clean_res = clean_for_matching(&res.name);
        let title_sim = jaro_winkler(&clean_search, &clean_res);
        (title_sim * 1000.0) as i32
    }).cloned()
}

async fn download_to_file(url: &str, dest: &std::path::Path) -> Result<()> {
    if dest.exists() { return Ok(()); }
    let resp = reqwest::get(url)
        .instrument(tracing::info_span!("download_file", url = url))
        .await?;
    let bytes = resp.bytes().await?;
    std::fs::write(dest, bytes)?;
    Ok(())
}
