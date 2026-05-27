// media_core/src/scanner/service.rs
#![allow(async_fn_in_trait)]
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::Arc;
use walkdir::WalkDir;
use crate::errors::Result;
use sqlx::Row;
use crate::models::{Library, MediaType, TaskUpdate, MediaStatus, Resolution, CastMember, MediaStream};
use crate::db::{Repositories, MovieReader, MovieWriter, TvReader, TvWriter, MediaRepository};
use crate::task_manager::ProgressSink;
use crate::{parser, nfo, paths, CoreContext};
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
    pub ctx: CoreContext,
    pub repos: Arc<Repositories>,
    pub progress: Arc<dyn ProgressSink>,
}

impl DefaultScannerService {
    pub fn new(ctx: CoreContext, progress: Arc<dyn ProgressSink>) -> Self {
        let repos = ctx.repos.clone();
        Self { ctx, repos, progress }
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
            let audio_codec = item.media_info.as_ref().map(|i| i.audio_codec.as_str());
            let duration = item.media_info.as_ref().map(|i| i.duration_secs);

            let movie_id = self.repos.movie.upsert(library.id, &title, year).await?;
            self.repos.movie.upsert_file(movie_id, &relative_path, filename, item.size, Some(item.mtime), res, codec, audio_codec, duration, None, item.fingerprint.as_deref()).await?;

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
            let mut genres: Option<String> = None;
            let mut language: Option<String> = None;
            let mut cast_list: Option<String> = None;

            if let Some(ref nfo) = item.metadata.tv_nfo {
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

            let extracted_show_title;
            let mut extracted_season = item.parsed.season.unwrap_or(1);
            let extracted_episode = item.parsed.episode.unwrap_or(1);

            let library_root = Path::new(&library.path);
            let relative_path_buf = paths::make_relative(&item.path, library_root).unwrap_or_default();
            
            let mut parts: Vec<&str> = Vec::new();
            let mut current = Path::new(&relative_path_buf).parent();
            while let Some(p) = current {
                if p.as_os_str().is_empty() || p == Path::new("") {
                    break;
                }
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    parts.insert(0, name);
                }
                current = p.parent();
            }

            if parts.is_empty() {
                extracted_show_title = item.parsed.title.clone();
            } else if parts.len() == 1 {
                let folder_name = parts[0];
                static RE_SEASON_FOLDER: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
                    regex::Regex::new(r"(?i)\b(?:season|series|s)\s*(\d+)\b").unwrap()
                });
                if let Some(caps) = RE_SEASON_FOLDER.captures(folder_name) {
                    if let Some(s_num) = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                        extracted_season = s_num;
                    }
                    let clean_show_name = RE_SEASON_FOLDER.replace(folder_name, "");
                    extracted_show_title = clean_show_name.to_string();
                } else {
                    extracted_show_title = folder_name.to_string();
                }
            } else {
                let show_folder = parts[0];
                extracted_show_title = show_folder.to_string();
                
                let mut found_season = None;
                static RE_SEASON_FOLDER_NESTED: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
                    regex::Regex::new(r"(?i)\b(?:season|series|s)\s*(\d+)\b").unwrap()
                });
                for part in parts.iter().rev() {
                    if let Some(caps) = RE_SEASON_FOLDER_NESTED.captures(part) {
                        if let Some(s_num) = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                            found_season = Some(s_num);
                            break;
                        }
                    } else if part.to_lowercase().contains("special") {
                        found_season = Some(0);
                        break;
                    }
                }
                if let Some(s) = found_season {
                    extracted_season = s;
                }
            }

            let mut db_title = clean_show_title_for_db(&extracted_show_title);
            if db_title.is_empty() {
                db_title = extracted_show_title;
            }

            // NFO title override takes precedence
            if let Some(ref nfo) = item.metadata.tv_nfo {
                if let Some(nfo_title) = nfo.title.first().filter(|t| !t.is_empty()) {
                    db_title = nfo_title.clone();
                }
            }

            let show_id = self.repos.tv.upsert_show(library.id, &db_title).await?;
            let season_id = self.repos.tv.upsert_season(show_id, extracted_season).await?;
            
            let res = item.media_info.as_ref().map(|i| Resolution::from_dimensions(i.width, i.height));
            let codec = item.media_info.as_ref().map(|i| i.video_codec.as_str());
            let audio_codec = item.media_info.as_ref().map(|i| i.audio_codec.as_str());
            let duration = item.media_info.as_ref().map(|i| i.duration_secs);

            let (ep_title, ep_plot) = if let Some(ref ep_nfo) = item.metadata.episode {
                (
                    ep_nfo.title.first().cloned(),
                    ep_nfo.plot.first().cloned()
                )
            } else {
                (None, None)
            };

            let episode_id = self.repos.tv.upsert_episode(
                season_id,
                extracted_episode,
                &relative_path,
                filename,
                item.size,
                Some(item.mtime),
                res,
                codec,
                audio_codec,
                duration,
                None,
                item.fingerprint.as_deref(),
                ep_title.as_deref(),
                ep_plot.as_deref(),
            ).await?;

            if let Some(ref ep_nfo) = item.metadata.episode {
                let rating = ep_nfo.rating.first().cloned();
                let thumb = ep_nfo.thumb.first().cloned();
                if rating.is_some() || thumb.is_some() {
                    let _ = self.repos.tv.update_episode_scraped_metadata(
                        episode_id,
                        None,
                        None,
                        rating,
                        thumb,
                    ).await;
                }
            }

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

        let ffprobe_path = self.ctx.config.ffprobe_path.clone();
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
                    media_info: mediainfo::get_media_info_with_path(path, &ffprobe_path).ok(),
                }
            }).collect()
        };

        let mut progress_count = 0;
        for chunk in parsed_items.chunks(100) {
            let mut txn = self.repos.pool.begin().await?;
            let tx_ptr = crate::db::base::TxPointer(&mut *txn as *mut sqlx::SqliteConnection);

            let mut chunk_new = 0;
            let mut chunk_healed = 0;

            let chunk_res: crate::errors::Result<()> = crate::db::base::ACTIVE_TX.scope(Some(tx_ptr), async {
                for item in chunk {
                    let relative_path = paths::make_relative(&item.path, library_root).unwrap_or_default();
                    let is_known = existing_files.contains_key(&relative_path);

                    match self.process_file(library, item, is_known).await {
                        Ok(ProcessAction::Added) => chunk_new += 1,
                        Ok(ProcessAction::Healed) => chunk_healed += 1,
                        _ => {}
                    }
                }
                Ok(())
            }).await;

            chunk_res?;
            txn.commit().await?;

            files_new += chunk_new;
            files_healed += chunk_healed;

            progress_count += chunk.len() as i32;
            self.progress.broadcast(TaskUpdate {
                task_id: task_id.clone(),
                status: "running".to_string(),
                progress: progress_count,
                total,
                message: format!("Scanned {}/{}: {}", progress_count, total,
                    chunk.last().and_then(|item| item.path.file_name()).and_then(|s| s.to_str()).unwrap_or("")),
                started_at: start_time,
                files_new: Some(files_new),
                files_healed: Some(files_healed),
                files_missing: Some(files_missing),
                ..Default::default()
            });
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
            media_info: mediainfo::get_media_info_with_path(&path, &self.ctx.config.ffprobe_path).ok(),
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

fn clean_show_title_for_db(raw: &str) -> String {
    let replaced = raw.replace('.', " ").replace('_', " ");

    static RE_SEASON: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)\s*\b(?:season|series|s)\s*\d+\b.*").unwrap()
    });
    static RE_QUALITY: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)\s+\b(2160p|1080p|720p|480p|576p|x264|x265|h264|h265|10bit|hdtv|web-dl|webdl|bluray)\b.*").unwrap()
    });
    static RE_SXXEXX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)\s*\b[Ss]\d{1,2}[Ee]\d{1,2}\b.*").unwrap()
    });
    static RE_TORRENT_SITE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)^(?:www\s+)?(?:torrenting|eztv|yts|rarbg|1337x|kickass|tgx|limetorrents|zooqle)(?:\s+com)?\s*[-–—:]\s*").unwrap()
    });
    static RE_BRACKETS: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)\s*[\[\({][^\]\)}]*?(?:eztv|torrenting|yts|rarbg|1337x|x264|x265|1080p|720p|2160p|h264|h265|bluray|web-dl|hdtv|memento|kontrast|minx)[^\]\)}]*?[\]\)}]").unwrap()
    });

    let cleaned = RE_SEASON.replace_all(&replaced, "");
    let cleaned = RE_QUALITY.replace_all(&cleaned, "");
    let cleaned = RE_SXXEXX.replace_all(&cleaned, "");
    
    let mut final_title = RE_TORRENT_SITE.replace(&cleaned, "").to_string();
    final_title = RE_BRACKETS.replace_all(&final_title, "").to_string();

    static RE_SPACES: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\s+").unwrap()
    });
    RE_SPACES.replace_all(&final_title, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_show_title_for_db() {
        assert_eq!(clean_show_title_for_db("Better Call Saul (2015)"), "Better Call Saul (2015)");
        assert_eq!(clean_show_title_for_db("Better Call Saul Season 2 (1080p x265 10bit Joy)"), "Better Call Saul");
        assert_eq!(clean_show_title_for_db("Better Call Saul Season 3 Complete 720p HDTV x264 [i_c]"), "Better Call Saul");
        assert_eq!(clean_show_title_for_db("Breaking Bad (2008)"), "Breaking Bad (2008)");
        assert_eq!(clean_show_title_for_db("Friends (1994)"), "Friends (1994)");
        assert_eq!(clean_show_title_for_db("Better.Call.Saul..1080p.BluRay.x265-KONTRAST"), "Better Call Saul");
        assert_eq!(clean_show_title_for_db("www.Torrenting.com - Game of Thrones S08E03 The Long Night"), "Game of Thrones");
    }
}


