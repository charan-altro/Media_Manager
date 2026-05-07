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
    metadata: nfo::reader::NfoMetadata,
}

pub async fn scan_library(
    pool: &SqlitePool,
    library: &Library,
    task_id: String,
    tx: &crate::task_manager::TaskManager,
) -> Result<()> {
    let start_time = Some(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0));

    tracing::info!("Starting scan for library '{}' at path '{}'", library.name, library.path);

    let skip_dirs: HashSet<&str> = HashSet::from([
        ".git", "node_modules", ".actors", "@eaDir", "#recycle", 
        "System Volume Information", "$RECYCLE.BIN", "Config.Msi", "$Recycle.Bin"
    ]);

    // Check path exists
    if !std::path::Path::new(&library.path).exists() {
        let msg = format!("Library path does not exist: {}", library.path);
        tracing::error!("{}", msg);
        tx.broadcast(TaskUpdate {
            task_id: task_id.clone(),
            status: "error".to_string(),
            progress: 0,
            total: 0,
            message: msg,
            started_at: None,
            debug_info: None,
        });
        return Ok(());
    }

    let files: Vec<PathBuf> = WalkDir::new(&library.path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !skip_dirs.contains(e.file_name().to_str().unwrap_or("")))
        .filter_map(|e| {
            match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    let path = err.path().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "unknown".to_string());
                    tracing::warn!("WalkDir error at [{}]: {}", path, err);
                    None
                }
            }
        })
        .filter(|e| e.file_type().is_file() && is_video_file(e.path()))
        .map(|e| e.into_path())
        .collect();

    let total = files.len() as i32;
    tracing::info!("Found {} video files in '{}'", total, library.path);

    tx.broadcast(TaskUpdate {
        task_id: task_id.clone(),
        status: "running".to_string(),
        progress: 0,
        total,
        message: format!("Found {} files. Analyzing metadata...", total),
        started_at: start_time,
        debug_info: None,
    });

    if total == 0 {
        tx.broadcast(TaskUpdate {
            task_id: task_id.clone(),
            status: "completed".to_string(),
            progress: 0,
            total: 0,
            message: "Scan complete: no video files found. Check library path.".to_string(),
            started_at: start_time,
            debug_info: None,
        });
        return Ok(());
    }

    // Parse all files in parallel (CPU-bound work)
    let parsed: Vec<ParsedFile> = {
        use rayon::prelude::*;
        files
            .par_iter()
            .map(|path| {
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let metadata = nfo::reader::detect_metadata(path);
                tracing::debug!(
                    "Parsed '{}' -> title='{}' nfo={} poster={}",
                    filename,
                    parser::parse_filename(filename).title,
                    metadata.nfo.is_some(),
                    metadata.poster_path.is_some()
                );
                ParsedFile {
                    path: path.clone(),
                    parsed: parser::parse_filename(filename),
                    size: path.metadata().map(|m| m.len() as i64).unwrap_or(0),
                    metadata,
                }
            })
            .collect()
    };

    // Write to DB sequentially (async IO)
    for (i, item) in parsed.iter().enumerate() {
        let result = process_file(pool, library, &item).await;
        if let Err(e) = result {
            tracing::error!("Failed to process file {:?}: {}", item.path, e);
        }

        let progress = (i + 1) as i32;
        if progress % 10 == 0 || progress == total {
            tx.broadcast(TaskUpdate {
                task_id: task_id.clone(),
                status: "running".to_string(),
                progress,
                total,
                message: format!("Scanned {}/{}: {}", progress, total,
                    item.path.file_name().and_then(|s| s.to_str()).unwrap_or("")),
                started_at: start_time,
                debug_info: Some(format!("Analyzing: {:?}", item.path)),
            });
        }
    }

    tx.broadcast(TaskUpdate {
        task_id: task_id.clone(),
        status: "completed".to_string(),
        progress: total,
        total,
        message: format!("Library scan complete. {} files processed.", total),
        started_at: start_time,
        debug_info: None,
    });

    tracing::info!("Scan complete for library '{}'", library.name);
    Ok(())
}

async fn process_file(pool: &SqlitePool, library: &Library, item: &ParsedFile) -> Result<()> {
    let file_path_str = item.path.to_str().unwrap_or("").to_string();
    let filename = item.path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    if library.media_type == MediaType::Movie {
        // ... (existing movie logic)
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

        // Extract technical info (FFprobe)
        let file_path_for_info = std::path::PathBuf::from(&file_path_str);
        let media_info = tokio::task::spawn_blocking(move || {
            crate::scanner::mediainfo::get_media_info(&file_path_for_info).ok()
        }).await?;

        let res = media_info.as_ref().map(|i| crate::models::Resolution::from_dimensions(i.width, i.height));
        let codec = media_info.as_ref().map(|i| i.video_codec.as_str());

        // Upsert the movie record (no duplicates on rescan)
        let movie_id = db::queries::upsert_movie(pool, library.id, &title, year).await?;

        // Always track the file path
        db::queries::upsert_movie_file(pool, movie_id, &file_path_str, filename, item.size, res, codec).await?;

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
        let has_artwork = item.metadata.poster_path.is_some() || item.metadata.backdrop_path.is_some();

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
            .bind(&item.metadata.poster_path)
            .bind(&item.metadata.backdrop_path)
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

        // Extract technical info (FFprobe)
        let file_path_for_info = std::path::PathBuf::from(&file_path_str);
        let media_info = tokio::task::spawn_blocking(move || {
            crate::scanner::mediainfo::get_media_info(&file_path_for_info).ok()
        }).await?;

        let res = media_info.as_ref().map(|i| crate::models::Resolution::from_dimensions(i.width, i.height));
        let codec = media_info.as_ref().map(|i| i.video_codec.as_str());

        db::queries::upsert_episode(pool, season_id, ep_num, &file_path_str, filename, item.size, res, codec).await?;

        // Update show metadata from tvshow.nfo if available
        if let Some(ref nfo) = item.metadata.tv_nfo {
            let tmdb_id = nfo.tmdb_id.as_ref().and_then(|s| s.trim().parse::<i32>().ok());
            
            // Download cast images for TV shows too
            let path = std::path::Path::new(&file_path_str);
            if let Some(folder) = path.parent() {
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
                                            member_image = Some(dest.to_string_lossy().to_string());
                                        }
                                    }
                                }
                             } else {
                                member_image = Some(dest.to_string_lossy().to_string());
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

    Ok(())
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
    let metadata = nfo::reader::detect_metadata(&path);
    
    let item = ParsedFile {
        path: path.clone(),
        parsed: parser::parse_filename(filename),
        size: path.metadata().map(|m| m.len() as i64).unwrap_or(0),
        metadata,
    };

    tx.broadcast(TaskUpdate {
        task_id: task_id.clone(),
        status: "running".to_string(),
        progress: 0,
        total: 1,
        message: format!("Processing new file: {}", filename),
        started_at: None,
        debug_info: None,
    });

    if let Err(e) = process_file(pool, library, &item).await {
        tracing::error!("Failed to process single file {:?}: {}", item.path, e);
    }

    tx.broadcast(TaskUpdate {
        task_id,
        status: "completed".to_string(),
        progress: 1,
        total: 1,
        message: "File processing complete".to_string(),
        started_at: None,
        debug_info: None,
    });

    Ok(())
}
