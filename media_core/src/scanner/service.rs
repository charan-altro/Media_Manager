// media_core/src/scanner/service.rs
#![allow(async_fn_in_trait)]
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::Arc;
use walkdir::WalkDir;
use anyhow::Result;
use sqlx::Row;
use crate::models::{Library, MediaType, TaskUpdate, MediaStatus, Resolution, CastMember, MediaStream};
use crate::db::{Repositories, MovieReader, MovieWriter, TvReader, TvWriter, MediaRepository};
use crate::task_manager::ProgressSink;
use crate::{parser, nfo, paths};
use crate::scanner::mediainfo;
use crate::scanner::hash;

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "wmv", "m4v", "ts", "m2ts", "mts",
    "mpg", "mpeg", "vob", "divx", "xvid", "webm", "flv", "ogv", "iso",
];

struct ParsedFile {
    path: PathBuf,
    parsed: parser::ParsedMedia,
    size: i64,
    mtime: i64,
    is_skipped: bool,
    metadata: nfo::reader::NfoMetadata,
    fingerprint: Option<String>,
    media_info: Option<mediainfo::MediaDetails>,
}

enum ProcessAction {
    Added,
    Healed,
    Updated,
    Skipped,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ScannerService: Send + Sync {
    async fn scan_library(
        &self,
        library: &Library,
        task_id: String,
    ) -> Result<()>;

    async fn scan_single_file(
        &self,
        library: &Library,
        path: PathBuf,
        task_id: String,
    ) -> Result<()>;
}

pub struct DefaultScannerService {
    pub repos: Arc<Repositories>,
    pub progress: Arc<dyn ProgressSink>,
}

impl DefaultScannerService {
    pub fn new(repos: Arc<Repositories>, progress: Arc<dyn ProgressSink>) -> Self {
        Self { repos, progress }
    }

    fn is_video_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| VIDEO_EXTENSIONS.iter().any(|&v| v.eq_ignore_ascii_case(ext)))
            .unwrap_or(false)
    }

    async fn process_file(&self, library: &Library, item: &ParsedFile, is_known: bool) -> Result<ProcessAction> {
        let library_root = Path::new(&library.path);
        let relative_path = paths::make_relative(&item.path, library_root)?;
        let filename = item.path.file_name().and_then(|s| s.to_str()).unwrap_or("");

        // Fast-Skip Shortcut
        if item.is_skipped {
            if library.media_type == MediaType::Movie {
                if let Ok(Some(existing)) = self.repos.movie.find_file_by_path(&relative_path).await {
                    self.repos.movie.update_file_last_scanned(existing.id).await?;
                    return Ok(ProcessAction::Skipped);
                }
            } else {
                if let Ok(Some(existing)) = self.repos.tv.find_episode_by_path(&relative_path).await {
                    self.repos.tv.update_episode_last_scanned(existing.id).await?;
                    return Ok(ProcessAction::Skipped);
                }
            }
        }

        // Smart tracking: check if file moved (find by fingerprint)
        if let Some(ref fingerprint) = item.fingerprint {
            if library.media_type == MediaType::Movie {
                if let Ok(Some(existing)) = self.repos.movie.find_file_by_fingerprint(fingerprint).await {
                    if existing.file_path != relative_path {
                        let old_path = library_root.join(&existing.file_path);
                        if !old_path.exists() {
                            tracing::info!("File moved detected (healing): {} -> {}", existing.file_path, relative_path);
                            self.repos.movie.update_file_path(existing.id, &relative_path).await?;
                            return Ok(ProcessAction::Healed);
                        } else {
                            tracing::debug!("Duplicate file ignored (same fingerprint): {}", relative_path);
                            return Ok(ProcessAction::Skipped);
                        }
                    }
                }
            } else {
                if let Ok(Some(existing)) = self.repos.tv.find_episode_by_fingerprint(fingerprint).await {
                    if existing.file_path != relative_path {
                        let old_path = library_root.join(&existing.file_path);
                        if !old_path.exists() {
                            tracing::info!("File moved detected (healing): {} -> {}", existing.file_path, relative_path);
                            self.repos.tv.update_episode_path(existing.id, &relative_path).await?;
                            return Ok(ProcessAction::Healed);
                        } else {
                            tracing::debug!("Duplicate episode ignored: {}", relative_path);
                            return Ok(ProcessAction::Skipped);
                        }
                    }
                }
            }
        }

        if library.media_type == MediaType::Movie {
            let mut title = item.parsed.title.clone();
            let mut year = item.parsed.year;

            if let Some(ref nfo) = item.metadata.nfo {
                if let Some(nfo_title) = nfo.title.first().filter(|t| !t.is_empty()) {
                    title = nfo_title.clone();
                }
                if let Some(nfo_year) = nfo.year.first().and_then(|y| y.trim().parse::<i32>().ok()) {
                    year = Some(nfo_year);
                }
            }

            tracing::info!("Processing movie: '{}' ({:?})", title, year);

            let res = item.media_info.as_ref().map(|i| Resolution::from_dimensions(i.width, i.height));
            let codec = item.media_info.as_ref().map(|i| i.video_codec.as_str());
            let duration = item.media_info.as_ref().map(|i| i.duration_secs);

            let movie_id = self.repos.movie.upsert(library.id, &title, year).await?;
            self.repos.movie.upsert_file(movie_id, &relative_path, filename, item.size, Some(item.mtime), res, codec, duration, None, item.fingerprint.as_deref()).await?;

            // Metadata from NFO
            let mut tmdb_id: Option<i32> = None;
            let mut imdb_id: Option<String> = None;
            let mut plot: Option<String> = None;
            let mut tagline: Option<String> = None;
            let mut runtime: Option<i32> = None;
            let mut rating: Option<f32> = None;
            let mut genres: Option<String> = None;
            let mut language: Option<String> = None;
            let mut cast_list: Option<String> = None;

            if let Some(ref nfo) = item.metadata.nfo {
                tmdb_id = nfo.tmdb_id.as_ref().and_then(|s| s.trim().parse::<i32>().ok());
                imdb_id = nfo.imdb_id.clone().filter(|s| !s.is_empty());
                plot = nfo.plot.first().cloned().filter(|s| !s.is_empty());
                tagline = nfo.tagline.first().cloned().filter(|s| !s.is_empty());
                runtime = nfo.runtime.first().cloned();
                rating = nfo.rating.first().cloned();
                if !nfo.genre.is_empty() {
                    genres = serde_json::to_string(&nfo.genre).ok();
                }
                if !nfo.language.is_empty() {
                    language = Some(nfo.language.first().cloned().unwrap_or_default());
                }
                if !nfo.actor.is_empty() {
                    let actors: Vec<CastMember> = nfo.actor.iter().map(|a| CastMember {
                        name: a.name.clone(),
                        role: a.role.clone(),
                        image: a.thumb.clone(),
                    }).collect();
                    cast_list = serde_json::to_string(&actors).ok();
                }
            }

            let has_nfo_data = tmdb_id.is_some() || imdb_id.is_some() || plot.is_some();
            let rel_poster = item.metadata.poster_path.as_ref().and_then(|p| {
                paths::make_relative(Path::new(p), library_root).ok()
            }).or(item.metadata.poster_path.clone());
            
            let rel_backdrop = item.metadata.backdrop_path.as_ref().and_then(|p| {
                paths::make_relative(Path::new(p), library_root).ok()
            }).or(item.metadata.backdrop_path.clone());

            let has_artwork = rel_poster.is_some() || rel_backdrop.is_some();

            if has_nfo_data || has_artwork {
                let new_status = if has_nfo_data { MediaStatus::Matched } else { MediaStatus::Unmatched };
                self.repos.movie.update_metadata(
                    movie_id,
                    tmdb_id,
                    imdb_id,
                    new_status,
                    plot,
                    rating,
                    tagline,
                    runtime,
                    genres,
                    language,
                    cast_list,
                    rel_poster,
                    rel_backdrop,
                ).await?;
            }
        } else {
            // TV Show Logic
            let mut title = item.parsed.title.clone();
            let mut genres: Option<String> = None;
            let mut language: Option<String> = None;
            let mut cast_list: Option<String> = None;

            if let Some(ref nfo) = item.metadata.tv_nfo {
                if let Some(nfo_title) = nfo.title.first().filter(|t| !t.is_empty()) {
                    title = nfo_title.clone();
                }
                if !nfo.genre.is_empty() {
                    genres = serde_json::to_string(&nfo.genre).ok();
                }
                if !nfo.language.is_empty() {
                    language = Some(nfo.language.first().cloned().unwrap_or_default());
                }
                if !nfo.actor.is_empty() {
                    let actors: Vec<CastMember> = nfo.actor.iter().map(|a| CastMember {
                        name: a.name.clone(),
                        role: a.role.clone(),
                        image: a.thumb.clone(),
                    }).collect();
                    cast_list = serde_json::to_string(&actors).ok();
                }
            }

            let mut extracted_show_title = item.parsed.title.clone();
            let mut extracted_season = item.parsed.season.unwrap_or(1);
            let extracted_episode = item.parsed.episode.unwrap_or(1);

            if let Some(parent) = item.path.parent() {
                let parent_name = parent.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let lc_parent = parent_name.to_lowercase();
                
                if lc_parent.starts_with("season") || lc_parent.starts_with("series") {
                    if let Some(num) = lc_parent.split_whitespace().last().and_then(|s| s.parse::<i32>().ok()) {
                        extracted_season = num;
                    }
                    if let Some(grandparent) = parent.parent() {
                        let gp_name = grandparent.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        if !gp_name.is_empty() {
                            extracted_show_title = gp_name.to_string();
                        }
                    }
                } else if item.parsed.season.is_none() {
                    extracted_show_title = parent_name.to_string();
                }
            }
            
            if title == item.parsed.title && extracted_show_title != item.parsed.title {
                title = extracted_show_title;
            }

            let show_id = self.repos.tv.upsert_show(library.id, &title).await?;
            let season_id = self.repos.tv.upsert_season(show_id, extracted_season).await?;
            
            let res = item.media_info.as_ref().map(|i| Resolution::from_dimensions(i.width, i.height));
            let codec = item.media_info.as_ref().map(|i| i.video_codec.as_str());
            let duration = item.media_info.as_ref().map(|i| i.duration_secs);

            self.repos.tv.upsert_episode(season_id, extracted_episode, &relative_path, filename, item.size, Some(item.mtime), res, codec, duration, None, item.fingerprint.as_deref()).await?;

            if let Some(ref nfo) = item.metadata.tv_nfo {
                let tmdb_id = nfo.tmdb_id.as_ref().and_then(|s| s.trim().parse::<i32>().ok());
                self.repos.tv.update_show_metadata(
                    show_id,
                    tmdb_id,
                    nfo.plot.first().cloned(),
                    nfo.rating.first().cloned(),
                    genres,
                    language,
                    cast_list,
                    None, None, None,
                    MediaStatus::Matched,
                ).await?;
            }
        }

        // Bridge streams
        if let (Some(ref info), Some(ref fingerprint)) = (&item.media_info, &item.fingerprint) {
            for stream in &info.streams {
                let media_stream = MediaStream {
                    id: 0,
                    file_hash: fingerprint.clone(),
                    stream_index: stream.index,
                    stream_type: stream.stream_type.clone(),
                    codec: Some(stream.codec.clone()),
                    language: stream.language.clone(),
                    title: stream.title.clone(),
                    channels: stream.channels,
                    is_default: false,
                };
                self.repos.media.upsert_stream(&media_stream).await?;
            }
        }

        Ok(if is_known { ProcessAction::Updated } else { ProcessAction::Added })
    }
}

#[async_trait::async_trait]
impl ScannerService for DefaultScannerService {
    #[tracing::instrument(skip(self), err)]
    async fn scan_library(
        &self,
        library: &Library,
        task_id: String,
    ) -> Result<()> {
        let start_time = Some(crate::models::now_ms());
        tracing::info!("Starting scan for library '{}' at path '{}'", library.name, library.path);

        let mut files_new = 0;
        let mut files_healed = 0;
        let mut files_missing = 0;

        self.progress.broadcast(TaskUpdate {
            task_id: task_id.clone(),
            status: "running".to_string(),
            progress: 0,
            total: 0,
            message: format!("Initializing scan for '{}'...", library.name),
            started_at: start_time,
            files_new: Some(0),
            files_healed: Some(0),
            files_missing: Some(0),
            ..Default::default()
        });

        let skip_dirs: HashSet<&str> = HashSet::from([
            ".git", "node_modules", ".actors", "@eaDir", "#recycle", 
            "System Volume Information", "$RECYCLE.BIN", "Config.Msi", "$Recycle.Bin"
        ]);

        if !Path::new(&library.path).exists() {
            let msg = format!("Library path does not exist: {}", library.path);
            tracing::error!("{}", msg);
            self.progress.broadcast(TaskUpdate {
                task_id,
                status: "error".to_string(),
                message: msg,
                started_at: start_time,
                ..Default::default()
            });
            return Ok(());
        }

        let mut files = Vec::new();
        let mut total_visited = 0;
        let mut last_feedback = std::time::Instant::now();

        for entry in WalkDir::new(&library.path)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| !skip_dirs.contains(e.file_name().to_str().unwrap_or("")))
        {
            total_visited += 1;
            if last_feedback.elapsed().as_secs() >= 5 || total_visited % 5000 == 0 {
                self.progress.broadcast(TaskUpdate {
                    task_id: task_id.clone(),
                    status: "running".to_string(),
                    message: format!("Walking directory... visited {} items ({} videos found)", total_visited, files.len()),
                    started_at: start_time,
                    ..Default::default()
                });
                last_feedback = std::time::Instant::now();
            }

            match entry {
                Ok(e) => {
                    if e.file_type().is_file() && self.is_video_file(e.path()) {
                        files.push(e.into_path());
                    }
                }
                Err(err) => {
                    tracing::warn!("WalkDir error: {}", err);
                }
            }
        }

        let total = files.len() as i32;
        tracing::info!("Found {} video files in '{}'", total, library.path);

        if total == 0 {
            self.progress.broadcast(TaskUpdate {
                task_id,
                status: "completed".to_string(),
                message: "Scan complete: no video files found.".to_string(),
                started_at: start_time,
                finished_at: Some(crate::models::now_ms()),
                ..Default::default()
            });
            return Ok(());
        }

        let library_root = Path::new(&library.path);
        let existing_files: std::collections::HashMap<String, (i64, i64)> = if library.media_type == MediaType::Movie {
            sqlx::query("SELECT mf.file_path, mf.size_bytes, mf.mtime FROM movie_files mf JOIN movies m ON mf.movie_id = m.id WHERE m.library_id = ?")
                .bind(library.id)
                .fetch_all(&self.repos.pool)
                .await?
                .into_iter()
                .map(|r| (r.get::<String, _>(0), (r.get::<i64, _>(1), r.get::<i64, _>(2))))
                .collect()
        } else {
            sqlx::query("SELECT e.file_path, e.size_bytes, e.mtime FROM episodes e JOIN seasons s ON e.season_id = s.id JOIN tv_shows t ON s.show_id = t.id WHERE t.library_id = ?")
                .bind(library.id)
                .fetch_all(&self.repos.pool)
                .await?
                .into_iter()
                .map(|r| (r.get::<String, _>(0), (r.get::<i64, _>(1), r.get::<i64, _>(2))))
                .collect()
        };

        let parsed_items: Vec<ParsedFile> = {
            use rayon::prelude::*;
            files.par_iter().map(|path| {
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let metadata = path.metadata().ok();
                let mtime = metadata.as_ref().and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let size = metadata.map(|m| m.len() as i64).unwrap_or(0);
                let relative_path = paths::make_relative(path, library_root).unwrap_or_default();

                if let Some((db_size, db_mtime)) = existing_files.get(&relative_path) {
                    if *db_size == size && *db_mtime == mtime {
                        return ParsedFile {
                            path: path.clone(),
                            parsed: parser::parse_filename(filename),
                            size, mtime,
                            is_skipped: true,
                            metadata: nfo::reader::NfoMetadata::default(),
                            fingerprint: None,
                            media_info: None,
                        };
                    }
                }

                ParsedFile {
                    path: path.clone(),
                    parsed: parser::parse_filename(filename),
                    size, mtime,
                    is_skipped: false,
                    metadata: nfo::reader::detect_metadata(path),
                    fingerprint: hash::calculate_oshash(path).ok(),
                    media_info: mediainfo::get_media_info(path).ok(),
                }
            }).collect()
        };

        for (i, item) in parsed_items.iter().enumerate() {
            let relative_path = paths::make_relative(&item.path, library_root).unwrap_or_default();
            let is_known = existing_files.contains_key(&relative_path);

            match self.process_file(library, item, is_known).await {
                Ok(ProcessAction::Added) => files_new += 1,
                Ok(ProcessAction::Healed) => files_healed += 1,
                _ => {}
            }

            let progress = (i + 1) as i32;
            if progress % 10 == 0 || progress == total {
                self.progress.broadcast(TaskUpdate {
                    task_id: task_id.clone(),
                    status: "running".to_string(),
                    progress,
                    total,
                    message: format!("Scanned {}/{}: {}", progress, total,
                        item.path.file_name().and_then(|s| s.to_str()).unwrap_or("")),
                    started_at: start_time,
                    files_new: Some(files_new),
                    files_healed: Some(files_healed),
                    files_missing: Some(files_missing),
                    ..Default::default()
                });
            }
        }

        if library.media_type == MediaType::Movie {
            files_missing = self.repos.movie.mark_missing_in_library(library.id).await?;
        } else {
            files_missing = self.repos.tv.mark_missing_in_library(library.id).await?;
        }

        self.progress.broadcast(TaskUpdate {
            task_id,
            status: "completed".to_string(),
            progress: total,
            total,
            message: format!("Scan complete. {} new, {} healed, {} missing.", files_new, files_healed, files_missing),
            started_at: start_time,
            finished_at: Some(crate::models::now_ms()),
            files_new: Some(files_new),
            files_healed: Some(files_healed),
            files_missing: Some(files_missing),
            ..Default::default()
        });

        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn scan_single_file(
        &self,
        library: &Library,
        path: PathBuf,
        task_id: String,
    ) -> Result<()> {
        if !path.exists() || !self.is_video_file(&path) {
            return Ok(());
        }

        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let metadata_fs = path.metadata().ok();
        let mtime = metadata_fs.as_ref().and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let size = metadata_fs.map(|m| m.len() as i64).unwrap_or(0);
        
        let item = ParsedFile {
            path: path.clone(),
            parsed: parser::parse_filename(filename),
            size, mtime,
            is_skipped: false,
            metadata: nfo::reader::detect_metadata(&path),
            fingerprint: hash::calculate_oshash(&path).ok(),
            media_info: mediainfo::get_media_info(&path).ok(),
        };

        let library_root = Path::new(&library.path);
        let relative_path = paths::make_relative(&item.path, library_root).unwrap_or_default();
        
        let is_known = if library.media_type == MediaType::Movie {
            self.repos.movie.find_file_by_path(&relative_path).await?.is_some()
        } else {
            self.repos.tv.find_episode_by_path(&relative_path).await?.is_some()
        };

        self.process_file(library, &item, is_known).await?;

        self.progress.broadcast(TaskUpdate {
            task_id,
            status: "completed".to_string(),
            progress: 1,
            total: 1,
            message: "File processing complete".to_string(),
            finished_at: Some(crate::models::now_ms()),
            ..Default::default()
        });

        Ok(())
    }
}
