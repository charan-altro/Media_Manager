// media_core/src/scanner/worker.rs
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use walkdir::WalkDir;
use sqlx::sqlite::SqlitePool;
use anyhow::Result;
use crate::models::{Library, MediaType, TaskUpdate};
use crate::parser;
use crate::db;
use crate::nfo;

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
    media_info: Option<crate::scanner::mediainfo::MediaDetails>,
}

enum ProcessAction {
    Added,
    Healed,
    Updated,
    Skipped,
}

pub async fn scan_library(
    pool: &SqlitePool,
    library: &Library,
    task_id: String,
    tx: &crate::task_manager::TaskManager,
) -> Result<()> {
    // 1. Concurrency Lock: Prevent multiple scans of the same library
    if !tx.try_lock_library_scan(library.id).await {
        tx.broadcast(TaskUpdate {
            task_id: task_id.to_string(),
            status: "completed".to_string(),
            progress: 0,
            total: 0,
            message: format!("Scan for '{}' already in progress. Skipping duplicate request.", library.name),
            started_at: None,
            finished_at: None,
            debug_info: None,
            files_new: Some(0),
            files_healed: Some(0),
            files_missing: Some(0),
        });
        return Ok(());
    }

    // Use a wrapper to ensure we always unlock
    let result = scan_library_internal(pool, library, &task_id, tx).await;

    tx.unlock_library_scan(library.id).await;
    result
}

async fn scan_library_internal(
    pool: &SqlitePool,
    library: &Library,
    task_id: &str,
    tx: &crate::task_manager::TaskManager,
) -> Result<()> {
    let start_time = Some(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0));

    tracing::info!("Starting scan for library '{}' at path '{}'", library.name, library.path);

    let mut files_new = 0;
    let mut files_healed = 0;
    let mut files_missing = 0;

    tx.broadcast(TaskUpdate {
        task_id: task_id.to_string(),
        status: "running".to_string(),
        progress: 0,
        total: 0,
        message: format!("Initializing scan for '{}'...", library.name),
        started_at: start_time,
        finished_at: None,
        debug_info: None,
        files_new: Some(files_new),
        files_healed: Some(files_healed),
        files_missing: Some(files_missing),
    });
    let skip_dirs: HashSet<&str> = HashSet::from([
        ".git", "node_modules", ".actors", "@eaDir", "#recycle", 
        "System Volume Information", "$RECYCLE.BIN", "Config.Msi", "$Recycle.Bin"
    ]);

    // Check path exists
    if !std::path::Path::new(&library.path).exists() {
        let msg = format!("Library path does not exist: {}", library.path);
        tracing::error!("{}", msg);
        tx.broadcast(TaskUpdate {
            task_id: task_id.to_string(),
            status: "error".to_string(),
            progress: 0,
            total: 0,
            message: msg,
            started_at: start_time,
            finished_at: None,
            debug_info: None,
            files_new: Some(0),
            files_healed: Some(0),
            files_missing: Some(0),
        });
        return Ok(());
    }

    // 2. Optimized WalkDir with progress feedback
    let mut files = Vec::new();
    let mut total_visited = 0;
    let mut last_feedback = std::time::Instant::now();

    for entry in WalkDir::new(&library.path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !skip_dirs.contains(e.file_name().to_str().unwrap_or("")))
    {
        total_visited += 1;

        // Provide feedback every 5 seconds or every 5000 items
        if last_feedback.elapsed().as_secs() >= 5 || total_visited % 5000 == 0 {
            tx.broadcast(TaskUpdate {
                task_id: task_id.to_string(),
                status: "running".to_string(),
                progress: 0,
                total: 0,
                message: format!("Walking directory... visited {} items ({} videos found)", total_visited, files.len()),
                started_at: start_time,
                finished_at: None,
                debug_info: None,
                files_new: Some(files_new),
                files_healed: Some(files_healed),
                files_missing: Some(files_missing),
            });
            last_feedback = std::time::Instant::now();
        }

        match entry {
            Ok(e) => {
                if e.file_type().is_file() && is_video_file(e.path()) {
                    files.push(e.into_path());
                }
            }
            Err(err) => {
                let path = err.path().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "unknown".to_string());
                tracing::warn!("WalkDir error at [{}]: {}", path, err);
            }
        }
    }

    let total = files.len() as i32;
    tracing::info!("Found {} video files in '{}'", total, library.path);

    tx.broadcast(TaskUpdate {
        task_id: task_id.to_string(),
        status: "running".to_string(),
        progress: 0,
        total,
        message: format!("Found {} files. Analyzing metadata...", total),
        started_at: start_time,
        finished_at: None,
        debug_info: None,
        files_new: Some(files_new),
        files_healed: Some(files_healed),
        files_missing: Some(files_missing),
    });

    if total == 0 {
        tx.broadcast(TaskUpdate {
            task_id: task_id.to_string(),
            status: "completed".to_string(),
            progress: 0,
            total: 0,
            message: "Scan complete: no video files found in the specified path.".to_string(),
            started_at: start_time,
            finished_at: Some(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64),
            debug_info: None,
            files_new: Some(0),
            files_healed: Some(0),
            files_missing: Some(0),
            });

        return Ok(());
    }

    // Fetch existing files from DB to support fast-skip
    use sqlx::Row;
    let library_root = Path::new(&library.path);
    let existing_files: std::collections::HashMap<String, (i64, i64)> = if library.media_type == MediaType::Movie {
        sqlx::query("SELECT mf.file_path, mf.size_bytes, mf.mtime FROM movie_files mf JOIN movies m ON mf.movie_id = m.id WHERE m.library_id = ?")
            .bind(library.id)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| (r.get::<String, _>(0), (r.get::<i64, _>(1), r.get::<i64, _>(2))))
            .collect()
    } else {
        sqlx::query("SELECT e.file_path, e.size_bytes, e.mtime FROM episodes e JOIN seasons s ON e.season_id = s.id JOIN tv_shows t ON s.show_id = t.id WHERE t.library_id = ?")
            .bind(library.id)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| (r.get::<String, _>(0), (r.get::<i64, _>(1), r.get::<i64, _>(2))))
            .collect()
    };

    // Parse all files in parallel (CPU-bound work)
    let parsed: Vec<ParsedFile> = {
        use rayon::prelude::*;
        files
            .par_iter()
            .map(|path| {
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let metadata = path.metadata().ok();
                let mtime = metadata.as_ref().and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let size = metadata.map(|m| m.len() as i64).unwrap_or(0);

                let relative_path = crate::paths::make_relative(path, library_root).unwrap_or_default();

                // Fast-Skip Check
                if let Some((db_size, db_mtime)) = existing_files.get(&relative_path) {
                    if *db_size == size && *db_mtime == mtime {
                        tracing::debug!("Skipping unchanged file: {}", relative_path);
                        return ParsedFile {
                            path: path.clone(),
                            parsed: parser::parse_filename(filename),
                            size,
                            mtime,
                            is_skipped: true,
                            metadata: nfo::reader::NfoMetadata::default(),
                            fingerprint: None,
                            media_info: None,
                        };
                    }
                }

                let nfo_metadata = nfo::reader::detect_metadata(path);
                tracing::debug!(
                    "Parsed '{}' -> title='{}' nfo={} poster={}",
                    filename,
                    parser::parse_filename(filename).title,
                    nfo_metadata.nfo.is_some(),
                    nfo_metadata.poster_path.is_some()
                );
                ParsedFile {
                    path: path.clone(),
                    parsed: parser::parse_filename(filename),
                    size,
                    mtime,
                    is_skipped: false,
                    metadata: nfo_metadata,
                    fingerprint: crate::scanner::hash::calculate_oshash(path).ok(),
                    media_info: crate::scanner::mediainfo::get_media_info(path).ok(),
                }
            })
            .collect()
    };

    // Write to DB sequentially (async IO)
    for (i, item) in parsed.iter().enumerate() {
        // Pass existing_files to determine if it's new
        let library_root = Path::new(&library.path);
        let relative_path = crate::paths::make_relative(&item.path, library_root).unwrap_or_default();
        let is_known = existing_files.contains_key(&relative_path);

        let result = process_file(pool, library, &item, is_known).await;
        match result {
            Ok(action) => match action {
                ProcessAction::Added => files_new += 1,
                ProcessAction::Healed => files_healed += 1,
                _ => {}
            },
            Err(e) => {
                tracing::error!("Failed to process file {:?}: {}", item.path, e);
            }
        }

        let progress = (i + 1) as i32;
        if progress % 10 == 0 || progress == total {
            tx.broadcast(TaskUpdate {
                task_id: task_id.to_string(),
                status: "running".to_string(),
                progress,
                total,
                message: format!("Scanned {}/{}: {}", progress, total,
                    item.path.file_name().and_then(|s| s.to_str()).unwrap_or("")),
                started_at: start_time,
                finished_at: None,
                debug_info: Some(format!("Analyzing: {:?}", item.path)),
                files_new: Some(files_new),
                files_healed: Some(files_healed),
                files_missing: Some(files_missing),
            });
        }
    }

    // 3. Missing File Pass
    tracing::info!("Starting missing file pass for library '{}'", library.name);
    tx.broadcast(TaskUpdate {
        task_id: task_id.to_string(),
        status: "running".to_string(),
        progress: total,
        total,
        message: "Identifying missing files...".to_string(),
        started_at: start_time,
        finished_at: None,
        debug_info: None,
        files_new: Some(files_new),
        files_healed: Some(files_healed),
        files_missing: Some(files_missing),
    });

    if library.media_type == MediaType::Movie {
        // Mark files as missing if they weren't scanned this time
        let rows = sqlx::query(
            r#"
            UPDATE movie_files
            SET is_missing = 1
            WHERE movie_id IN (SELECT id FROM movies WHERE library_id = ?)
            AND last_scanned < datetime('now', '-1 minute')
            RETURNING id
            "#
        )
        .bind(library.id)
        .fetch_all(pool)
        .await?;
        files_missing = rows.len() as i32;
    } else {
        let rows = sqlx::query(
            r#"
            UPDATE episodes
            SET is_missing = 1
            WHERE season_id IN (SELECT s.id FROM seasons s JOIN tv_shows t ON s.show_id = t.id WHERE t.library_id = ?)
            AND last_scanned < datetime('now', '-1 minute')
            RETURNING id
            "#
        )
        .bind(library.id)
        .fetch_all(pool)
        .await?;
        files_missing = rows.len() as i32;
    }

    tx.broadcast(TaskUpdate {
        task_id: task_id.to_string(),
        status: "completed".to_string(),
        progress: total,
        total,
        message: format!("Library scan complete. {} new, {} healed, {} missing.", files_new, files_healed, files_missing),
        started_at: start_time,
        finished_at: Some(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64),
        debug_info: None,
        files_new: Some(files_new),
        files_healed: Some(files_healed),
        files_missing: Some(files_missing),
    });

    tracing::info!("Scan complete for library '{}': {} new, {} healed, {} missing", 
        library.name, files_new, files_healed, files_missing);
    Ok(())
}

async fn process_file(pool: &SqlitePool, library: &Library, item: &ParsedFile, is_known: bool) -> Result<ProcessAction> {
    let library_root = Path::new(&library.path);
    let relative_path = crate::paths::make_relative(&item.path, library_root)?;
    let filename = item.path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    // Fast-Skip Shortcut: If we already determined this file hasn't changed, 
    // just update its last_scanned timestamp and move on.
    if item.is_skipped {
        if library.media_type == MediaType::Movie {
            if let Ok(Some(existing)) = db::queries::get_movie_file_by_path(pool, &relative_path).await {
                db::queries::update_movie_file_last_scanned(pool, existing.id).await?;
                return Ok(ProcessAction::Skipped);
            }
        } else {
            if let Ok(Some(existing)) = db::queries::get_episode_by_path(pool, &relative_path).await {
                db::queries::update_episode_last_scanned(pool, existing.id).await?;
                return Ok(ProcessAction::Skipped);
            }
        }
    }

    // Smart tracking: check if file moved (find by fingerprint)
    if let Some(ref fingerprint) = item.fingerprint {
        if library.media_type == MediaType::Movie {
            if let Ok(Some(existing)) = db::queries::get_movie_file_by_fingerprint(pool, fingerprint).await {
                if existing.file_path != relative_path {
                    // Critical Fix: Only heal if the OLD file is actually missing.
                    // If it still exists, this is a DUPLICATE, not a MOVE.
                    let old_path = library_root.join(&existing.file_path);
                    if !old_path.exists() {
                        tracing::info!("File moved detected (healing): {} -> {}", existing.file_path, relative_path);
                        db::queries::update_movie_file_path(pool, existing.id, &relative_path).await?;
                        return Ok(ProcessAction::Healed);
                    } else {
                        tracing::debug!("Duplicate file ignored (same fingerprint): {}", relative_path);
                        return Ok(ProcessAction::Skipped);
                    }
                }
            }
        } else {
            if let Ok(Some(existing)) = db::queries::get_episode_by_fingerprint(pool, fingerprint).await {
                if existing.file_path != relative_path {
                    let old_path = library_root.join(&existing.file_path);
                    if !old_path.exists() {
                        tracing::info!("File moved detected (healing): {} -> {}", existing.file_path, relative_path);
                        db::queries::update_episode_path(pool, existing.id, &relative_path).await?;
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

        // Extract technical info (from pre-calculated item)
        let res = item.media_info.as_ref().map(|i| crate::models::Resolution::from_dimensions(i.width, i.height));
        let codec = item.media_info.as_ref().map(|i| i.video_codec.as_str());
        let duration = item.media_info.as_ref().map(|i| i.duration_secs);

        // Upsert the movie record (no duplicates on rescan)
        let movie_id = db::queries::upsert_movie(pool, library.id, &title, year).await?;

        // Always track the file path (store as relative)
        db::queries::upsert_movie_file(pool, movie_id, &relative_path, filename, item.size, Some(item.mtime), res, codec, duration, None, item.fingerprint.as_deref()).await?;

        // If NFO has IDs or we have local artwork, update the movie
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
                let actors: Vec<crate::models::CastMember> = nfo.actor.iter().map(|a| crate::models::CastMember {
                    name: a.name.clone(),
                    role: a.role.clone(),
                    image: a.thumb.clone(),
                }).collect();
                cast_list = serde_json::to_string(&actors).ok();
            }
        }

        let has_nfo_data = tmdb_id.is_some() || imdb_id.is_some() || plot.is_some();
        
        // Relativize local artwork paths
        let rel_poster = item.metadata.poster_path.as_ref().and_then(|p| {
            crate::paths::make_relative(Path::new(p), library_root).ok()
        }).or(item.metadata.poster_path.clone());
        
        let rel_backdrop = item.metadata.backdrop_path.as_ref().and_then(|p| {
            crate::paths::make_relative(Path::new(p), library_root).ok()
        }).or(item.metadata.backdrop_path.clone());

        let has_artwork = rel_poster.is_some() || rel_backdrop.is_some();

        if has_nfo_data || has_artwork {
            let new_status = if has_nfo_data { crate::models::MediaStatus::Matched } else { crate::models::MediaStatus::Unmatched };
            sqlx::query(
                r#"
                UPDATE movies
                SET tmdb_id = COALESCE(?, tmdb_id),
                    imdb_id = COALESCE(?, imdb_id),
                    status   = ?,
                    plot     = COALESCE(?, plot),
                    rating   = COALESCE(?, rating),
                    tagline  = COALESCE(?, tagline),
                    runtime  = COALESCE(?, runtime),
                    genres   = COALESCE(?, genres),
                    language = COALESCE(?, language),
                    cast_list = COALESCE(?, cast_list),
                    poster_url   = COALESCE(?, poster_url),
                    backdrop_url = COALESCE(?, backdrop_url),
                    updated_at   = datetime('now')
                WHERE id = ?
                "#
            )
            .bind(tmdb_id)
            .bind(&imdb_id)
            .bind(new_status)
            .bind(&plot)
            .bind(rating)
            .bind(&tagline)
            .bind(runtime)
            .bind(&genres)
            .bind(&language)
            .bind(&cast_list)
            .bind(&rel_poster)
            .bind(&rel_backdrop)
            .bind(movie_id)
            .execute(pool)
            .await?;
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
                let actors: Vec<crate::models::CastMember> = nfo.actor.iter().map(|a| crate::models::CastMember {
                    name: a.name.clone(),
                    role: a.role.clone(),
                    image: a.thumb.clone(),
                }).collect();
                cast_list = serde_json::to_string(&actors).ok();
            }
        }

        // Detect TV show folder structure
        let mut extracted_show_title = item.parsed.title.clone();
        let mut extracted_season = item.parsed.season.unwrap_or(1);
        let extracted_episode = item.parsed.episode.unwrap_or(1);

        // Walk up directory tree to infer show title and season
        if let Some(parent) = item.path.parent() {
            let parent_name = parent.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let lc_parent = parent_name.to_lowercase();
            
            if lc_parent.starts_with("season") || lc_parent.starts_with("series") {
                // E.g., "Season 1"
                if let Some(num) = lc_parent.split_whitespace().last().and_then(|s| s.parse::<i32>().ok()) {
                    extracted_season = num;
                }
                
                // Then the grand-parent might be the show title
                if let Some(grandparent) = parent.parent() {
                    let gp_name = grandparent.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if !gp_name.is_empty() {
                        extracted_show_title = gp_name.to_string();
                    }
                }
            } else if item.parsed.season.is_none() {
                // Just use parent as show title if we couldn't parse season
                extracted_show_title = parent_name.to_string();
            }
        }
        
        // Use extracted title if NFO title is not present
        if title == item.parsed.title && extracted_show_title != item.parsed.title {
            title = extracted_show_title;
        }

        let show_id = db::queries::upsert_tv_show(pool, library.id, &title).await?;
        
        let season_id = db::queries::upsert_season(pool, show_id, extracted_season).await?;
        
        let ep_num = extracted_episode;

        // Extract technical info (from pre-calculated item)
        let res = item.media_info.as_ref().map(|i| crate::models::Resolution::from_dimensions(i.width, i.height));
        let codec = item.media_info.as_ref().map(|i| i.video_codec.as_str());
        let duration = item.media_info.as_ref().map(|i| i.duration_secs);

        db::queries::upsert_episode(pool, season_id, ep_num, &relative_path, filename, item.size, Some(item.mtime), res, codec, duration, None, item.fingerprint.as_deref()).await?;

        // Update show metadata from tvshow.nfo if available
        if let Some(ref nfo) = item.metadata.tv_nfo {
            let tmdb_id = nfo.tmdb_id.as_ref().and_then(|s| s.trim().parse::<i32>().ok());
            
            // Download cast images for TV shows too
            if let Some(folder) = item.path.parent() {
                let actors_dir = folder.join(".actors");
                let _ = std::fs::create_dir_all(&actors_dir);
                
                let mut final_cast = Vec::new();
                for actor in &nfo.actor {
                    let mut member_image = actor.thumb.clone();
                    if let Some(ref thumb_url) = actor.thumb {
                        if thumb_url.starts_with("http") {
                             let clean_name = actor.name.replace(|c: char| !c.is_alphanumeric(), "_");
                             let dest = actors_dir.join(format!("{}.jpg", clean_name));
                             if !dest.exists() {
                                if let Ok(resp) = reqwest::get(thumb_url).await {
                                    if let Ok(bytes) = resp.bytes().await {
                                        if std::fs::write(&dest, bytes).is_ok() {
                                            // Store cast image as relative to library root
                                            member_image = crate::paths::make_relative(&dest, library_root).ok();
                                        }
                                    }
                                }
                             } else {
                                member_image = crate::paths::make_relative(&dest, library_root).ok();
                             }
                        }
                    }
                    final_cast.push(crate::models::CastMember {
                        name: actor.name.clone(),
                        role: actor.role.clone(),
                        image: member_image,
                    });
                }
                cast_list = serde_json::to_string(&final_cast).ok();
            }

            sqlx::query(
                r#"
                UPDATE tv_shows
                SET tmdb_id = COALESCE(?, tmdb_id),
                    plot = COALESCE(?, plot),
                    rating = COALESCE(?, rating),
                    genres = COALESCE(?, genres),
                    language = COALESCE(?, language),
                    cast_list = COALESCE(?, cast_list),
                    status = ?
                WHERE id = ?
                "#
            )
            .bind(tmdb_id)
            .bind(&nfo.plot.first().cloned())
            .bind(nfo.rating.first().cloned())
            .bind(genres)
            .bind(&language)
            .bind(cast_list)
            .bind(crate::models::MediaStatus::Matched)
            .bind(show_id)
            .execute(pool)
            .await?;
        }
    }

    if is_known {
        Ok(ProcessAction::Updated)
    } else {
        Ok(ProcessAction::Added)
    }
}

fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| VIDEO_EXTENSIONS.iter().any(|&v| v.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

pub async fn scan_single_file(
    pool: &SqlitePool,
    library: &Library,
    path: PathBuf,
    task_id: String,
    tx: &crate::task_manager::TaskManager,
) -> Result<()> {
    tracing::info!("Targeted scan for single file: {:?}", path);

    if !path.exists() || !is_video_file(&path) {
        return Ok(());
    }

    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let metadata_fs = path.metadata().ok();
    let mtime = metadata_fs.as_ref().and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let size = metadata_fs.map(|m| m.len() as i64).unwrap_or(0);
    let nfo_metadata = nfo::reader::detect_metadata(&path);
    
    let item = ParsedFile {
        path: path.clone(),
        parsed: parser::parse_filename(filename),
        size,
        mtime,
        is_skipped: false,
        metadata: nfo_metadata,
        fingerprint: crate::scanner::hash::calculate_oshash(&path).ok(),
        media_info: crate::scanner::mediainfo::get_media_info(&path).ok(),
    };

    tx.broadcast(TaskUpdate {
        task_id: task_id.to_string(),
        status: "running".to_string(),
        progress: 0,
        total: 1,
        message: format!("Processing new file: {}", filename),
        started_at: None,
        finished_at: None,
        debug_info: None,
        files_new: Some(0),
        files_healed: Some(0),
        files_missing: Some(0),
    });

    let library_root = Path::new(&library.path);
    let relative_path = crate::paths::make_relative(&item.path, library_root).unwrap_or_default();
    
    let is_known = if library.media_type == MediaType::Movie {
        db::queries::get_movie_file_by_path(pool, &relative_path).await?.is_some()
    } else {
        db::queries::get_episode_by_path(pool, &relative_path).await?.is_some()
    };

    if let Err(e) = process_file(pool, library, &item, is_known).await {
        tracing::error!("Failed to process single file {:?}: {}", item.path, e);
    }

    tx.broadcast(TaskUpdate {
        task_id: task_id.to_string(),
        status: "completed".to_string(),
        progress: 1,
        total: 1,
        message: "File processing complete".to_string(),
        started_at: None,
        finished_at: Some(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64),
        debug_info: None,
        files_new: Some(if is_known { 0 } else { 1 }),
        files_healed: Some(0),
        files_missing: Some(0),
    });


    Ok(())
}
