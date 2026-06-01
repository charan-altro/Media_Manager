// media_core/src/scanner/service.rs
#![allow(async_fn_in_trait)]
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::Arc;
use walkdir::WalkDir;
use crate::errors::Result;
use sqlx::Row;
use crate::models::{Library, MediaType, TaskUpdate, MediaStatus, Resolution, CastMember, MediaStream, EpisodeId};
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

pub struct ScanGuard {
    task_manager: Arc<crate::task_manager::TaskManager>,
    library_id: crate::models::LibraryId,
}

impl Drop for ScanGuard {
    fn drop(&mut self) {
        let task_manager = self.task_manager.clone();
        let library_id = self.library_id;
        tokio::spawn(async move {
            task_manager.unlock_library_scan(library_id).await;
        });
    }
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

#[derive(Clone)]
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
        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            let filename_lower = filename.to_lowercase();
            if filename_lower.contains("preview") || filename_lower.contains("sample") {
                return false;
            }
        }
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

/// Normalise a show title to a stable grouping key used by the organiser.
/// This is case-insensitive and strips years / quality / noise tokens so that
/// `Band Of Brothers`, `Band of Brothers`, and `Band.Of.Brothers.S01.720p.BRRip`
/// all collapse to the same key: `band of brothers`.
fn normalize_organiser_key(title: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // Strip bracket site prefix: "[TorrentCouch net] ..."
    static RE_BRACKET: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)^\s*\[[^\]]*\]\s*").unwrap()
    });
    // Strip quality / noise tokens and everything after them
    static RE_QUALITY: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?i)\s+\b(2160p|1080p|720p|480p|576p|x264|x265|h264|h265|10bit|hdtv|web[- ]?dl|bluray|brrip|webrip|hevc|complete|mkvcage|eztv|yts|rarbg|1337x|tgx|kontrast|minx|memento)\b.*"
        ).unwrap()
    });
    // Strip season / episode markers and everything after them
    static RE_SEASON: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\s*\b[Ss]\d{1,2}(?:[Ee]\d{1,2})?\b.*").unwrap()
    });
    // Strip trailing standalone year: "Game of Thrones 2012" → "Game of Thrones"
    static RE_YEAR: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\s+\b(?:19|20)\d{2}\b\s*$").unwrap()
    });
    static RE_SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

    let s = RE_BRACKET.replace(title.trim(), "");
    let s = s.replace('.', " ").replace('_', " ");
    let s = RE_SEASON.replace(&s, "");
    let s = RE_QUALITY.replace(&s, "");
    let s = RE_YEAR.replace(s.trim_end(), "");
    RE_SPACES.replace_all(s.trim(), " ").trim().to_lowercase()
}

/// Choose the better (canonical) of two show title candidates.
/// Prefers: no dots/brackets, no quality tokens, then shorter.
fn pick_canonical_title(a: &str, b: &str) -> String {
    fn score(s: &str) -> usize {
        // Penalise release-noise indicators
        let noise = if s.contains('.') || s.contains('[') || s.to_ascii_lowercase().contains("p.blu") { 1000 } else { 0 };
        noise + s.len()
    }
    if score(a) <= score(b) { a.to_string() } else { b.to_string() }
}


    async fn organise_tv_library(&self, library: &Library, parsed_items: &mut [ParsedFile]) -> Result<()> {
        let library_root = Path::new(&library.path);
        
        // 1. Fetch all episodes for this library from the database to map existing files
        let db_episodes: Vec<(String, i32, i32, Option<String>, i64, String)> = sqlx::query(
            r#"
            SELECT t.title, s.season_number, e.episode_number, e.title, e.id, e.file_path
            FROM episodes e
            JOIN seasons s ON e.season_id = s.id
            JOIN tv_shows t ON s.show_id = t.id
            WHERE t.library_id = ?
            "#
        )
        .bind(library.id)
        .fetch_all(&self.repos.pool)
        .await?
        .into_iter()
        .map(|r| (
            r.get::<String, _>(0),
            r.get::<i32, _>(1),
            r.get::<i32, _>(2),
            r.get::<Option<String>, _>(3),
            r.get::<i64, _>(4),
            r.get::<String, _>(5),
        ))
        .collect();
        
        let db_episodes_map: std::collections::HashMap<String, (String, i32, i32, Option<String>, i64)> = db_episodes
            .into_iter()
            .map(|(show_title, season, ep_num, ep_title, ep_id, file_path)| {
                (file_path, (show_title, season, ep_num, ep_title, ep_id))
            })
            .collect();

        // 2. Identify TV episodes from parsed_items and group them by normalised key.
        //    The key is (lowercase-normalised-show-title, season, episode) so that
        //    "Band of Brothers", "Band Of Brothers", "Band.Of.Brothers.S01.720p..."
        //    all land in the same bucket.
        //    We track the canonical (cleanest) title per group separately.
        let mut groups: std::collections::HashMap<(String, i32, i32), Vec<usize>> =
            std::collections::HashMap::new();
        // Maps (norm_key, season, episode) → best human-readable title for destination path
        let mut canonical_titles: std::collections::HashMap<(String, i32, i32), String> =
            std::collections::HashMap::new();
        
        for (idx, item) in parsed_items.iter().enumerate() {
            let relative_path = paths::make_relative(&item.path, library_root).unwrap_or_default();
            
            let (show_title, season, episode) = if let Some(db_info) = db_episodes_map.get(&relative_path) {
                (db_info.0.clone(), db_info.1, db_info.2)
            } else {
                // Use directory-aware parsing for grouping so release-folder files are bucketed
                // under the correct canonical show title.
                let dir_parsed = parser::parse_file_path(&item.path, library_root, true);
                let mut show_title = clean_show_title_for_db(&dir_parsed.title);
                if show_title.is_empty() {
                    show_title = dir_parsed.title.clone();
                }
                if show_title.is_empty() {
                    // Last resort: fall back to the pre-computed parsed title
                    show_title = clean_show_title_for_db(&item.parsed.title);
                    if show_title.is_empty() {
                        show_title = item.parsed.title.clone();
                    }
                }
                if let Some(ref nfo) = item.metadata.tv_nfo {
                    if let Some(nfo_title) = nfo.title.first().filter(|t| !t.is_empty()) {
                        show_title = nfo_title.clone();
                    }
                }
                let season = dir_parsed.season.or(item.parsed.season).unwrap_or(1);
                let episode = dir_parsed.episode.or(item.parsed.episode).unwrap_or(1);
                (show_title, season, episode)
            };

            // Skip files with empty show title (e.g. orphan Season folders directly at library root)
            if show_title.is_empty() {
                tracing::warn!("Could not determine show title for {:?}, skipping organise", item.path);
                continue;
            }
            
            let norm_key = (Self::normalize_organiser_key(&show_title), season, episode);

            // Track the cleanest (canonical) title seen for this group
            canonical_titles
                .entry(norm_key.clone())
                .and_modify(|existing| {
                    *existing = Self::pick_canonical_title(existing, &show_title);
                })
                .or_insert_with(|| show_title);

            groups.entry(norm_key).or_default().push(idx);
        }

        // 3. Process each group to rename and move the files
        for ((norm_key, season, episode), indices) in groups {
            // Use the canonical (cleanest) title from the tracker, fall back to norm_key
            let show_title = canonical_titles
                .get(&(norm_key.clone(), season, episode))
                .cloned()
                .unwrap_or_else(|| norm_key.clone());
            let sanitized_show_title = sanitize_name(&show_title);
            let dest_dir = library_root.join(&sanitized_show_title).join(format!("Season {:02}", season));
            
            // Get suffixes for files in this group
            let group_files: Vec<&ParsedFile> = indices.iter().map(|&idx| &parsed_items[idx]).collect();
            let suffixes = get_episode_suffixes(&group_files);
            
            for (i, &idx) in indices.iter().enumerate() {
                let item = &parsed_items[idx];
                let relative_path = paths::make_relative(&item.path, library_root).unwrap_or_default();
                
                // Get episode title
                let ep_title = if let Some(db_info) = db_episodes_map.get(&relative_path) {
                    db_info.3.clone()
                } else {
                    item.metadata.episode.as_ref()
                        .and_then(|e| e.title.first().cloned())
                        .or_else(|| extract_episode_title(item.path.file_name().and_then(|s| s.to_str()).unwrap_or("")))
                };
                
                let ext = item.path.extension().and_then(|s| s.to_str()).unwrap_or("mkv");
                let suffix = &suffixes[i];
                
                let new_filename = if let Some(title) = ep_title {
                    let sanitized_title = sanitize_name(&title);
                    format!("{} - S{:02}E{:02} - {}{}.{}", sanitized_show_title, season, episode, sanitized_title, suffix, ext)
                } else {
                    format!("{} - S{:02}E{:02}{}.{}", sanitized_show_title, season, episode, suffix, ext)
                };
                
                let dest_path = dest_dir.join(&new_filename);
                
                // If it is already in the correct place, do nothing
                if item.path == dest_path {
                    continue;
                }
                
                // Ensure unique destination path to avoid collisions
                let unique_dest_path = get_unique_dest_path(&dest_path);
                
                // Move files
                let old_dir = item.path.parent().unwrap().to_path_buf();
                let old_stem = item.path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                let new_stem = unique_dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                
                tracing::info!("Moving TV episode: {:?} -> {:?}", item.path, unique_dest_path);
                
                if let Err(err) = std::fs::create_dir_all(&dest_dir) {
                    tracing::error!("Failed to create directory {:?}: {}", dest_dir, err);
                    continue;
                }
                
                if let Err(err) = move_item(&parsed_items[idx].path, &unique_dest_path) {
                    tracing::error!("Failed to move file {:?}: {}", parsed_items[idx].path, err);
                    continue;
                }
                
                // Move companion files
                if let Ok(entries) = std::fs::read_dir(&old_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if !p.is_file() || p == unique_dest_path || p == parsed_items[idx].path {
                            continue;
                        }
                        if let Some(file_name) = p.file_name().and_then(|s| s.to_str()) {
                            if file_name.starts_with(&old_stem) {
                                let new_name = file_name.replace(&old_stem, &new_stem);
                                let _ = move_item(&p, &dest_dir.join(new_name));
                            }
                        }
                    }
                }
                
                // Move show level files
                let new_show_dir = library_root.join(&sanitized_show_title);
                move_show_level_files(&old_dir, &new_show_dir, season);
                if let Some(grandparent) = old_dir.parent() {
                    move_show_level_files(grandparent, &new_show_dir, season);
                }
                
                // Update database record path if it already exists
                let new_relative_path = paths::make_relative(&unique_dest_path, library_root).unwrap_or_default();
                if let Some(db_info) = db_episodes_map.get(&relative_path) {
                    let episode_id = db_info.4;
                    if let Err(err) = self.repos.tv.update_episode_path(EpisodeId(episode_id), &new_relative_path).await {
                        tracing::error!("Failed to update database episode path for ID {}: {}", episode_id, err);
                    }
                }
                
                // Update ParsedFile in memory so scanner uses the new path
                parsed_items[idx].path = unique_dest_path;
                parsed_items[idx].is_skipped = false; // Scan to refresh it at the new path
            }
        }
        
        // 4. Clean up empty parent directories
        remove_empty_directories(library_root, library_root);
        
        Ok(())
    }

    async fn organise_movie_library(&self, library: &Library, parsed_items: &mut [ParsedFile]) -> Result<()> {
        let library_root = Path::new(&library.path);
        
        // 1. Fetch all movies for this library from the database to map existing files
        let db_movies: Vec<(String, Option<i32>, i64, String)> = sqlx::query(
            r#"
            SELECT m.title, m.year, mf.id, mf.file_path
            FROM movie_files mf
            JOIN movies m ON mf.movie_id = m.id
            WHERE m.library_id = ?
            "#
        )
        .bind(library.id)
        .fetch_all(&self.repos.pool)
        .await?
        .into_iter()
        .map(|r| (
            r.get::<String, _>(0),
            r.get::<Option<i32>, _>(1),
            r.get::<i64, _>(2),
            r.get::<String, _>(3),
        ))
        .collect();
        
        let db_movies_map: std::collections::HashMap<String, (String, Option<i32>, i64)> = db_movies
            .into_iter()
            .map(|(title, year, file_id, file_path)| {
                (file_path, (title, year, file_id))
            })
            .collect();

        // 2. Identify movies from parsed_items and group them by (title, year)
        let mut groups: std::collections::HashMap<(String, Option<i32>), Vec<usize>> = std::collections::HashMap::new();
        
        for (idx, item) in parsed_items.iter().enumerate() {
            let relative_path = paths::make_relative(&item.path, library_root).unwrap_or_default();
            
            let (title, year) = if let Some(db_info) = db_movies_map.get(&relative_path) {
                (db_info.0.clone(), db_info.1)
            } else {
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
                (title, year)
            };
            
            groups.entry((title, year)).or_default().push(idx);
        }

        // 3. Process each group to rename and move the files
        for ((title, year), indices) in groups {
            let sanitized_title = sanitize_name(&title);
            let folder_name = if let Some(y) = year {
                format!("{} ({})", sanitized_title, y)
            } else {
                sanitized_title.clone()
            };
            let dest_dir = library_root.join(&folder_name);
            
            // Get suffixes for files in this group
            let group_files: Vec<&ParsedFile> = indices.iter().map(|&idx| &parsed_items[idx]).collect();
            let suffixes = get_movie_suffixes(&group_files);
            
            for (i, &idx) in indices.iter().enumerate() {
                let item = &parsed_items[idx];
                let relative_path = paths::make_relative(&item.path, library_root).unwrap_or_default();
                
                let ext = item.path.extension().and_then(|s| s.to_str()).unwrap_or("mkv");
                let suffix = &suffixes[i];
                
                let new_filename = if let Some(y) = year {
                    format!("{} ({}){}.{}", sanitized_title, y, suffix, ext)
                } else {
                    format!("{}{}.{}", sanitized_title, suffix, ext)
                };
                
                let dest_path = dest_dir.join(&new_filename);
                
                // If it is already in the correct place, do nothing
                if item.path == dest_path {
                    continue;
                }
                
                // Ensure unique destination path to avoid collisions
                let unique_dest_path = get_unique_dest_path(&dest_path);
                
                // Move files
                let old_dir = item.path.parent().unwrap().to_path_buf();
                let old_stem = item.path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                let new_stem = unique_dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                
                tracing::info!("Moving movie: {:?} -> {:?}", item.path, unique_dest_path);
                
                if let Err(err) = std::fs::create_dir_all(&dest_dir) {
                    tracing::error!("Failed to create directory {:?}: {}", dest_dir, err);
                    continue;
                }
                
                if let Err(err) = move_item(&parsed_items[idx].path, &unique_dest_path) {
                    tracing::error!("Failed to move file {:?}: {}", parsed_items[idx].path, err);
                    continue;
                }
                
                // Move companion files
                if let Ok(entries) = std::fs::read_dir(&old_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if !p.is_file() || p == unique_dest_path || p == parsed_items[idx].path {
                            continue;
                        }
                        if let Some(file_name) = p.file_name().and_then(|s| s.to_str()) {
                            if file_name.starts_with(&old_stem) {
                                let new_name = file_name.replace(&old_stem, &new_stem);
                                let _ = move_item(&p, &dest_dir.join(new_name));
                            }
                        }
                    }
                }
                
                // Move movie level files
                let movie_assets = ["poster.jpg", "fanart.jpg", "banner.jpg", "clearlogo.png", "movie.nfo"];
                for asset in &movie_assets {
                    let src = old_dir.join(asset);
                    if src.exists() {
                        let dest = dest_dir.join(asset);
                        if !dest.exists() {
                            let _ = move_item(&src, &dest);
                        }
                    }
                }
                
                // Update database record path if it already exists
                let new_relative_path = paths::make_relative(&unique_dest_path, library_root).unwrap_or_default();
                if let Some(db_info) = db_movies_map.get(&relative_path) {
                    let file_id = db_info.2;
                    use crate::models::MovieFileId;
                    if let Err(err) = self.repos.movie.update_file_path(MovieFileId(file_id), &new_relative_path).await {
                        tracing::error!("Failed to update database movie path for ID {}: {}", file_id, err);
                    }
                }
                
                // Update ParsedFile in memory so scanner uses the new path
                parsed_items[idx].path = unique_dest_path;
                parsed_items[idx].is_skipped = false; // Scan to refresh it at the new path
            }
        }
        
        // 4. Clean up empty parent directories
        remove_empty_directories(library_root, library_root);
        
        Ok(())
    }

    async fn run_background_analysis_and_organization(&self, library: Library) -> Result<()> {
        let library_root = Path::new(&library.path);
        let ffprobe_path = self.ctx.config.ffprobe_path.clone();

        tracing::info!("Starting background analysis & organization for library '{}'", library.name);

        if library.media_type == MediaType::Movie {
            let db_files = sqlx::query(
                r#"
                SELECT mf.id, mf.file_path, mf.size_bytes
                FROM movie_files mf
                JOIN movies m ON mf.movie_id = m.id
                WHERE m.library_id = ?
                "#
            )
            .bind(library.id)
            .fetch_all(&self.repos.pool)
            .await?;

            for row in &db_files {
                let file_id: i64 = row.get(0);
                let rel_path: String = row.get(1);
                let _size: i64 = row.get(2);
                let abs_path = library_root.join(&rel_path);

                if abs_path.exists() {
                    let file_record = self.repos.movie.find_file_by_id(crate::models::MovieFileId(file_id)).await?;
                    if let Some(fr) = file_record {
                        if fr.fingerprint.is_none() || fr.resolution.is_none() {
                            tracing::info!("Analyzing movie file in background: {:?}", abs_path);
                            let fingerprint = hash::calculate_oshash(&abs_path).ok();
                            let media_info = mediainfo::get_media_info_with_path(&abs_path, &ffprobe_path).ok();

                            if let Some(ref fp) = fingerprint {
                                // Smart tracking in background:
                                // Check if this fingerprint already exists in the database.
                                if let Ok(Some(existing_file)) = self.repos.movie.find_file_by_fingerprint(fp).await {
                                    if existing_file.id != crate::models::MovieFileId(file_id) {
                                        let old_path = library_root.join(&existing_file.file_path);
                                        if !old_path.exists() {
                                            tracing::info!("File moved detected in background (healing): {} -> {}", existing_file.file_path, rel_path);
                                            // IMPORTANT: Delete the temp record FIRST (to free up the path for UNIQUE constraint),
                                            // then update the old record's path to point to the new location.
                                            //
                                            // Step 1: Check if temp movie needs deletion BEFORE we delete the file
                                            let temp_movie_id = fr.movie_id;
                                            // Step 2: Delete the temp file record (frees the path constraint)
                                            self.repos.movie.delete_file(crate::models::MovieFileId(file_id)).await?;
                                            // Step 3: Now update the original record's path safely
                                            self.repos.movie.update_file_path(existing_file.id, &rel_path).await?;
                                            // Step 4: Delete the temp movie if it has no remaining files
                                            let other_files: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM movie_files WHERE movie_id = ?")
                                                .bind(temp_movie_id)
                                                .fetch_one(&self.repos.pool)
                                                .await
                                                .map(|row| row.0)
                                                .unwrap_or(0);
                                            if other_files == 0 {
                                                self.repos.movie.delete(temp_movie_id).await?;
                                            }
                                            // Since we healed, continue to next file.
                                            continue;
                                        }
                                    }
                                }

                                let _ = self.repos.movie.update_file_fingerprint(crate::models::MovieFileId(file_id), fp).await;
                            }

                            if let Some(ref info) = media_info {
                                let res = Resolution::from_dimensions(info.width, info.height);
                                let _ = self.repos.movie.update_file_metadata(crate::models::MovieFileId(file_id), info.duration_secs, info.width, info.height).await;
                                let _ = self.repos.movie.update_file_resolution(crate::models::MovieFileId(file_id), res).await;

                                if let Some(ref fp) = fingerprint {
                                    for stream in &info.streams {
                                        let media_stream = MediaStream {
                                            id: 0,
                                            file_hash: fp.clone(),
                                            stream_index: stream.index,
                                            stream_type: stream.stream_type.clone(),
                                            codec: Some(stream.codec.clone()),
                                            language: stream.language.clone(),
                                            title: stream.title.clone(),
                                            channels: stream.channels,
                                            is_default: false,
                                        };
                                        let _ = self.repos.media.upsert_stream(&media_stream).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let db_movies = sqlx::query(
                r#"
                SELECT m.title, m.year, mf.id, mf.file_path, mf.size_bytes, mf.mtime, mf.resolution, mf.fingerprint, mf.duration_secs
                FROM movie_files mf
                JOIN movies m ON mf.movie_id = m.id
                WHERE m.library_id = ?
                "#
            )
            .bind(library.id)
            .fetch_all(&self.repos.pool)
            .await?;

            let mut parsed_items = Vec::new();
            for r in db_movies {
                let _m_title: String = r.get(0);
                let _m_year: Option<i32> = r.get(1);
                let _mf_id: i64 = r.get(2);
                let file_path_str: String = r.get(3);
                let size_bytes: i64 = r.get(4);
                let mtime: i64 = r.get(5);
                let resolution_str: Option<String> = r.get(6);
                let fingerprint: Option<String> = r.get(7);
                let duration_secs: Option<i32> = r.get(8);

                let parsed_res = resolution_str.and_then(|s| s.parse::<Resolution>().ok());
                let parsed = parser::parse_file_path(Path::new(&file_path_str), library_root, false);

                let media_info = duration_secs.map(|d| {
                    let width = match parsed_res {
                        Some(Resolution::R2160p) => 3840,
                        Some(Resolution::R1080p) => 1920,
                        Some(Resolution::R720p) => 1280,
                        _ => 720,
                    };
                    mediainfo::MediaDetails {
                        width,
                        height: 720,
                        duration_secs: d,
                        video_codec: String::new(),
                        audio_codec: String::new(),
                        rotation: 0,
                        streams: Vec::new(),
                        ..Default::default()
                    }
                });

                parsed_items.push(ParsedFile {
                    path: library_root.join(file_path_str),
                    parsed,
                    size: size_bytes,
                    mtime,
                    is_skipped: false,
                    metadata: nfo::reader::NfoMetadata::default(),
                    fingerprint,
                    media_info,
                });
            }

            self.organise_movie_library(&library, &mut parsed_items).await?;

        } else {
            let db_episodes = sqlx::query(
                r#"
                SELECT e.id, e.file_path, e.size_bytes
                FROM episodes e
                JOIN seasons s ON e.season_id = s.id
                JOIN tv_shows t ON s.show_id = t.id
                WHERE t.library_id = ?
                "#
            )
            .bind(library.id)
            .fetch_all(&self.repos.pool)
            .await?;

            for row in db_episodes {
                let episode_id: i64 = row.get(0);
                let rel_path: String = row.get(1);
                let _size: i64 = row.get(2);
                let abs_path = library_root.join(&rel_path);

                if abs_path.exists() {
                    let ep_record = self.repos.tv.find_episode_by_id(crate::models::EpisodeId(episode_id)).await?;
                    if let Some(er) = ep_record {
                        if er.fingerprint.is_none() || er.resolution.is_none() {
                            tracing::info!("Analyzing TV episode in background: {:?}", abs_path);
                            let fingerprint = hash::calculate_oshash(&abs_path).ok();
                            let media_info = mediainfo::get_media_info_with_path(&abs_path, &ffprobe_path).ok();

                            if let Some(ref fp) = fingerprint {
                                // Smart tracking in background:
                                // Check if this fingerprint already exists in the database.
                                if let Ok(Some(existing_ep)) = self.repos.tv.find_episode_by_fingerprint(fp).await {
                                    if existing_ep.id != crate::models::EpisodeId(episode_id) {
                                        let old_path = library_root.join(&existing_ep.file_path);
                                        if !old_path.exists() {
                                            tracing::info!("TV episode moved detected in background (healing): {} -> {}", existing_ep.file_path, rel_path);
                                            // IMPORTANT: Delete the temp episode FIRST to free the path for the UNIQUE constraint.
                                            // Step 1: Save season_id before deleting the temp episode
                                            let season_id = er.season_id;
                                            // Step 2: Delete the temporary episode record (frees the path constraint)
                                            self.repos.tv.delete_episode(crate::models::EpisodeId(episode_id)).await?;
                                            // Step 3: Now update the original record's path safely
                                            self.repos.tv.update_episode_path(existing_ep.id, &rel_path).await?;
                                            // Step 4: Find if the season / show associated with episode_id needs to be deleted.
                                            let other_eps: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM episodes WHERE season_id = ?")
                                                .bind(season_id)
                                                .fetch_one(&self.repos.pool)
                                                .await
                                                .map(|row| row.0)
                                                .unwrap_or(0);
                                            if other_eps == 0 {
                                                // Fetch show_id before deleting season
                                                let show_id_val: Option<(i64,)> = sqlx::query_as("SELECT show_id FROM seasons WHERE id = ?")
                                                    .bind(season_id)
                                                    .fetch_optional(&self.repos.pool)
                                                    .await
                                                    .unwrap_or(None);
                                                // Delete empty season
                                                let _ = sqlx::query("DELETE FROM seasons WHERE id = ?").bind(season_id).execute(&self.repos.pool).await;
                                                
                                                if let Some(sid) = show_id_val.map(|row| row.0) {
                                                    let other_seasons: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM seasons WHERE show_id = ?")
                                                        .bind(sid)
                                                        .fetch_one(&self.repos.pool)
                                                        .await
                                                        .map(|row| row.0)
                                                        .unwrap_or(0);
                                                    if other_seasons == 0 {
                                                        let _ = self.repos.tv.delete_show(crate::models::TvShowId(sid)).await;
                                                    }
                                                }
                                            }
                                            // Since we healed, we should continue to next file.
                                            continue;
                                        }
                                    }
                                }

                                let _ = self.repos.tv.update_episode_fingerprint(crate::models::EpisodeId(episode_id), fp).await;
                            }

                            if let Some(ref info) = media_info {
                                let _ = self.repos.tv.update_episode_metadata(crate::models::EpisodeId(episode_id), info.duration_secs, info.width, info.height).await;

                                if let Some(ref fp) = fingerprint {
                                    for stream in &info.streams {
                                        let media_stream = MediaStream {
                                            id: 0,
                                            file_hash: fp.clone(),
                                            stream_index: stream.index,
                                            stream_type: stream.stream_type.clone(),
                                            codec: Some(stream.codec.clone()),
                                            language: stream.language.clone(),
                                            title: stream.title.clone(),
                                            channels: stream.channels,
                                            is_default: false,
                                        };
                                        let _ = self.repos.media.upsert_stream(&media_stream).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let db_eps = sqlx::query(
                r#"
                SELECT t.title, s.season_number, e.episode_number, e.title, e.id, e.file_path, e.size_bytes, e.mtime, e.resolution, e.fingerprint, e.duration_secs
                FROM episodes e
                JOIN seasons s ON e.season_id = s.id
                JOIN tv_shows t ON s.show_id = t.id
                WHERE t.library_id = ?
                "#
            )
            .bind(library.id)
            .fetch_all(&self.repos.pool)
            .await?;

            let mut parsed_items = Vec::new();
            for r in db_eps {
                let _show_title: String = r.get(0);
                let _season_num: i32 = r.get(1);
                let _ep_num: i32 = r.get(2);
                let _ep_title: Option<String> = r.get(3);
                let _ep_id: i64 = r.get(4);
                let file_path_str: String = r.get(5);
                let size_bytes: i64 = r.get(6);
                let mtime: i64 = r.get(7);
                let resolution_str: Option<String> = r.get(8);
                let fingerprint: Option<String> = r.get(9);
                let duration_secs: Option<i32> = r.get(10);

                let parsed_res = resolution_str.and_then(|s| s.parse::<Resolution>().ok());
                let parsed = parser::parse_file_path(Path::new(&file_path_str), library_root, true);

                let media_info = duration_secs.map(|d| {
                    let width = match parsed_res {
                        Some(Resolution::R2160p) => 3840,
                        Some(Resolution::R1080p) => 1920,
                        Some(Resolution::R720p) => 1280,
                        _ => 720,
                    };
                    mediainfo::MediaDetails {
                        width,
                        height: 720,
                        duration_secs: d,
                        video_codec: String::new(),
                        audio_codec: String::new(),
                        rotation: 0,
                        streams: Vec::new(),
                        ..Default::default()
                    }
                });

                parsed_items.push(ParsedFile {
                    path: library_root.join(file_path_str),
                    parsed,
                    size: size_bytes,
                    mtime,
                    is_skipped: false,
                    metadata: nfo::reader::NfoMetadata::default(),
                    fingerprint,
                    media_info,
                });
            }

            self.organise_tv_library(&library, &mut parsed_items).await?;
        }

        tracing::info!("Background analysis & organization complete for library '{}'", library.name);
        Ok(())
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
        if !self.ctx.task_manager.try_lock_library_scan(library.id).await {
            tracing::warn!("Library scan already in progress for library ID {:?}", library.id);
            return Ok(());
        }

        let guard = ScanGuard {
            task_manager: self.ctx.task_manager.clone(),
            library_id: library.id,
        };

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

        let is_tv = library.media_type == MediaType::Tv;
        let parsed_items: Vec<ParsedFile> = {
            use rayon::prelude::*;
            files.par_iter().map(|path| {
                let metadata = path.metadata().ok();
                let mtime = metadata.as_ref().and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let size = metadata.map(|m| m.len() as i64).unwrap_or(0);
                let relative_path = paths::make_relative(path, library_root).unwrap_or_default();

                // Use directory-aware parsing for both TV and movie libraries so titles/seasons/years
                // can be resolved from the parent directory hierarchy.
                let parsed = parser::parse_file_path(path, library_root, is_tv);

                if let Some((db_size, db_mtime)) = existing_files.get(&relative_path) {
                    if *db_size == size && *db_mtime == mtime {
                        return ParsedFile {
                            path: path.clone(),
                            parsed,
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
                    parsed,
                    size, mtime,
                    is_skipped: false,
                    metadata: nfo::reader::detect_metadata(path),
                    fingerprint: None,
                    media_info: None,
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

        // Spawn background task taking ownership of the lock guard
        let self_clone = self.clone();
        let library_clone = library.clone();
        tokio::spawn(async move {
            let _guard = guard; // move lock guard here
            if let Err(e) = self_clone.run_background_analysis_and_organization(library_clone).await {
                tracing::error!("Failed background analysis & organization: {}", e);
            }
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

    static RE_BRACKET_PREFIX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)^\s*\[[^\]]*\]\s*").unwrap()
    });
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

    let replaced_prefix = RE_BRACKET_PREFIX.replace(&replaced, "");
    let cleaned = RE_SEASON.replace_all(&replaced_prefix, "");
    let cleaned = RE_QUALITY.replace_all(&cleaned, "");
    let cleaned = RE_SXXEXX.replace_all(&cleaned, "");
    
    let mut final_title = RE_TORRENT_SITE.replace(&cleaned, "").to_string();
    final_title = RE_BRACKETS.replace_all(&final_title, "").to_string();

    static RE_SPACES: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\s+").unwrap()
    });
    RE_SPACES.replace_all(&final_title, " ").trim().to_string()
}

fn sanitize_name(name: &str) -> String {
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r#"[\\/*?:"<>|]"#).unwrap()
    });
    RE.replace_all(name, "").trim().to_string()
}

fn extract_episode_title(filename: &str) -> Option<String> {
    let stem = Path::new(filename).file_stem()?.to_str()?;
    
    static RE_SXXEXX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)[Ss]\d{2}[Ee]\d{2}").unwrap()
    });
    
    let caps_loc = RE_SXXEXX.find(stem)?;
    let after_sxxexx = &stem[caps_loc.end()..];
    
    static RE_QUALITY: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)\b(2160p|1080p|720p|480p|576p|x264|x265|h264|h265|10bit|hdtv|web-dl|webdl|bluray|brrip|mkvcap|mkvcage|hevc|psa|eztv|torrenting|yts|rarbg|1337x|tgx)\b.*").unwrap()
    });
    
    let cleaned = RE_QUALITY.replace_all(after_sxxexx, "").to_string();
    let cleaned = cleaned.replace('.', " ").replace('_', " ");
    
    static RE_JUNK: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"[\[\({}\)\]\-–—\s]+").unwrap()
    });
    
    let cleaned = RE_JUNK.replace_all(&cleaned, " ").to_string();
    let final_title = cleaned.trim().to_string();
    if final_title.is_empty() {
        None
    } else {
        Some(final_title)
    }
}

fn get_episode_suffixes(group_files: &[&ParsedFile]) -> Vec<String> {
    if group_files.len() <= 1 {
        return vec!["".to_string()];
    }
    
    let mut resolutions = std::collections::HashSet::new();
    let mut all_res_distinct = true;
    for f in group_files {
        let res = f.parsed.resolution.or(f.media_info.as_ref().map(|i| Resolution::from_dimensions(i.width, i.height)));
        if let Some(r) = res {
            if !resolutions.insert(r.as_str().to_string()) {
                all_res_distinct = false;
            }
        } else {
            all_res_distinct = false;
        }
    }
    
    let mut suffixes = Vec::new();
    for f in group_files {
        let res = f.parsed.resolution.or(f.media_info.as_ref().map(|i| Resolution::from_dimensions(i.width, i.height)));
        let res_str = res.map(|r| r.as_str().to_string()).unwrap_or_else(|| "unknown".to_string());
        if all_res_distinct {
            suffixes.push(format!(" [{}]", res_str));
        } else {
            let size_mb = f.size / (1024 * 1024);
            suffixes.push(format!(" [{}-{}MB]", res_str, size_mb));
        }
    }
    suffixes
}

fn get_movie_suffixes(group_files: &[&ParsedFile]) -> Vec<String> {
    if group_files.len() <= 1 {
        return vec!["".to_string()];
    }
    
    let mut resolutions = std::collections::HashSet::new();
    let mut all_res_distinct = true;
    for f in group_files {
        let res = f.parsed.resolution.or(f.media_info.as_ref().map(|i| Resolution::from_dimensions(i.width, i.height)));
        if let Some(r) = res {
            if !resolutions.insert(r.as_str().to_string()) {
                all_res_distinct = false;
            }
        } else {
            all_res_distinct = false;
        }
    }
    
    let mut suffixes = Vec::new();
    for f in group_files {
        let res = f.parsed.resolution.or(f.media_info.as_ref().map(|i| Resolution::from_dimensions(i.width, i.height)));
        let res_str = res.map(|r| r.as_str().to_string()).unwrap_or_else(|| "unknown".to_string());
        if all_res_distinct {
            suffixes.push(format!(" [{}]", res_str));
        } else {
            let size_mb = f.size / (1024 * 1024);
            suffixes.push(format!(" [{}-{}MB]", res_str, size_mb));
        }
    }
    suffixes
}

fn get_unique_dest_path(dest_path: &Path) -> PathBuf {
    if !dest_path.exists() {
        return dest_path.to_path_buf();
    }
    
    let parent = dest_path.parent().unwrap();
    let stem = dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = dest_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    
    let mut counter = 1;
    loop {
        let new_name = if ext.is_empty() {
            format!("{}_{}", stem, counter)
        } else {
            format!("{}_{}.{}", stem, counter, ext)
        };
        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}

fn move_item(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::Other || e.to_string().contains("cross-device") => {
            let options = fs_extra::file::CopyOptions::new();
            fs_extra::file::move_file(from, to, &options)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed cross-device move: {}", err)))?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn move_show_level_files(old_show_dir: &Path, new_show_dir: &Path, season: i32) {
    if old_show_dir == new_show_dir {
        return;
    }
    let show_assets = ["tvshow.nfo", "poster.jpg", "fanart.jpg", "banner.jpg", "clearlogo.png"];
    for asset in &show_assets {
        let src = old_show_dir.join(asset);
        if src.exists() {
            let dest = new_show_dir.join(asset);
            if !dest.exists() {
                let _ = move_item(&src, &dest);
            }
        }
    }
    let season_poster = format!("season{:02}-poster.jpg", season);
    let src = old_show_dir.join(&season_poster);
    if src.exists() {
        let dest = new_show_dir.join(&season_poster);
        if !dest.exists() {
            let _ = move_item(&src, &dest);
        }
    }
}

fn remove_empty_directories(dir: &Path, library_root: &Path) {
    if !dir.is_dir() {
        return;
    }
    // Recurse into sub-dirs first (depth-first, so children are cleaned before parents)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                remove_empty_directories(&path, library_root);
            }
        }
    }

    // Never remove the library root itself
    if dir == library_root {
        return;
    }

    // Check remaining contents after recursion
    let entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(r) => r.flatten().collect(),
        Err(_) => return,
    };

    if entries.is_empty() {
        // Truly empty directory — remove it
        if let Err(e) = std::fs::remove_dir(dir) {
            tracing::warn!("Could not remove empty dir {:?}: {}", dir, e);
        } else {
            tracing::info!("Removed empty directory: {:?}", dir);
        }
        return;
    }

    // If directory only contains non-video metadata files (artwork / .nfo), it is a
    // leftover release-group folder whose video files have been moved. Remove it too.
    const METADATA_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "gif", "tbn", "svg",
        "nfo", "xml", "json", "txt", "pdf",
        "srt", "sub", "idx", "ass", "ssa", "vtt",
        "sfv", "md5", "sha1",
        "url", "torrent", "parts",
        "ds_store", "db", "desktop", "ini",
    ];
    let has_video = entries.iter().any(|e| {
        let p = e.path();
        if !p.is_file() { return true; } // sub-directory still present → keep parent
        let ext = p.extension()
            .and_then(|x| x.to_str())
            .map(|x| x.to_lowercase())
            .unwrap_or_default();
        let is_meta = if ext.is_empty() {
            let name = p.file_name()
                .and_then(|x| x.to_str())
                .map(|x| x.to_lowercase())
                .unwrap_or_default();
            name.starts_with('.') || name == "readme" || name == "install" || name == "license"
        } else {
            METADATA_EXTS.contains(&ext.as_str())
        };
        !is_meta
    });

    if !has_video {
        // Remove leftover metadata files then the directory itself
        for entry in &entries {
            let _ = std::fs::remove_file(entry.path());
        }
        if let Err(e) = std::fs::remove_dir(dir) {
            tracing::warn!("Could not remove leftover dir {:?}: {}", dir, e);
        } else {
            tracing::info!("Removed leftover metadata-only directory: {:?}", dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_empty_directories() {
        use std::fs::File;
        let temp_dir = tempfile::tempdir().unwrap();
        let library_root = temp_dir.path();

        // 1. Create a release folder that only has metadata, torrent and OS leftovers (should be deleted)
        let dup_dir = library_root.join("Show.S01.720p.BRRip.MkvCage");
        std::fs::create_dir_all(&dup_dir).unwrap();
        File::create(dup_dir.join("poster.jpg")).unwrap();
        File::create(dup_dir.join("show.nfo")).unwrap();
        File::create(dup_dir.join("some_link.url")).unwrap();
        File::create(dup_dir.join("download.torrent")).unwrap();
        File::create(dup_dir.join(".DS_Store")).unwrap();
        File::create(dup_dir.join("Thumbs.db")).unwrap();

        // 2. Create a clean folder that has video files (should be kept)
        let keep_dir = library_root.join("Show");
        let keep_season = keep_dir.join("Season 01");
        std::fs::create_dir_all(&keep_season).unwrap();
        File::create(keep_season.join("Show - S01E01.mkv")).unwrap();
        File::create(keep_season.join("Show - S01E01.nfo")).unwrap();

        // Run cleanup
        remove_empty_directories(library_root, library_root);

        // Assertions
        assert!(!dup_dir.exists(), "Leftover release-group directory should be deleted");
        assert!(keep_dir.exists(), "Clean show directory with video files should not be deleted");
        assert!(keep_season.exists(), "Clean season directory with video files should not be deleted");
    }

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

    #[test]
    fn test_extract_episode_title() {
        assert_eq!(extract_episode_title("Show.S01E01.Episode.Title.1080p.mkv"), Some("Episode Title".to_string()));
        assert_eq!(extract_episode_title("Show S01E01 Episode - Title [1080p].mkv"), Some("Episode Title".to_string()));
        assert_eq!(extract_episode_title("Show S01E01.mkv"), None);
    }
}


