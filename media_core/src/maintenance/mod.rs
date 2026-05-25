// core/src/maintenance/mod.rs
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Read, Write};
use crate::errors::{CoreError, Result};
use zip::write::FileOptions;
use chrono::Local;
use tracing::info;
use crate::nfo::writer::NfoWriter;
use crate::db::{Repositories, MovieReader, TvReader};
use sqlx::SqlitePool;

pub struct MaintenanceEngine;

impl MaintenanceEngine {
    pub async fn export_all_nfos(repos: &Repositories) -> Result<()> {
        info!("Starting bulk NFO export...");
        
        // Export Movies
        if let Ok(movies) = repos.movie.find_all(None, None, None).await {
            for movie in movies {
                if let Ok(Some(file)) = repos.movie.find_file_by_movie_id(movie.id).await {
                    let path = PathBuf::from(&file.file_path);
                    let nfo_path = path.with_extension("nfo");
                    let _ = NfoWriter::write_movie_nfo(&movie, &nfo_path).await;
                }
            }
        }

        // Export TV Shows
        if let Ok(shows) = repos.tv.find_all_shows(None, None, None).await {
            for show in shows {
                let mut show_path = None;
                if let Ok(seasons) = repos.tv.find_seasons_by_show_id(show.id).await {
                    for s in seasons {
                        if let Ok(eps) = repos.tv.find_episodes_by_season_id(s.id).await {
                            for ep in eps {
                                let ep_path = PathBuf::from(&ep.file_path);
                                let ep_nfo = ep_path.with_extension("nfo");
                                let _ = NfoWriter::write_episode_nfo(&ep, s.season_number, &ep_nfo).await;

                                if show_path.is_none() {
                                    if let Some(parent) = ep_path.parent() {
                                        if let Some(grandparent) = parent.parent() {
                                            show_path = Some(grandparent.to_path_buf());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(p) = show_path {
                    let show_nfo = p.join("tvshow.nfo");
                    let _ = NfoWriter::write_tvshow_nfo(&show, &show_nfo).await;
                }
            }
        }

        info!("Bulk NFO export completed.");
        Ok(())
    }

    pub async fn create_backup(pool: &SqlitePool, backup_dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(backup_dir)?;

        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = backup_dir.join(format!("mediavault_backup_{}.zip", timestamp));
        let temp_db_path = backup_dir.join(format!("temp_backup_{}.db", timestamp));
        
        info!("Creating safe WAL backup at {:?}", backup_path);

        // 1. Vacuum into a temporary file
        let query = format!("VACUUM INTO '{}'", temp_db_path.to_string_lossy().replace("\\", "/"));
        sqlx::query(&query).execute(pool).await?;

        // 2. Zip the temporary file
        let file = File::create(&backup_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        zip.start_file("mediavault.db", options)?;
        let mut db_file = File::open(&temp_db_path)?;
        let mut buffer = Vec::new();
        db_file.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;

        zip.finish()?;
        
        // 3. Delete the temporary file
        let _ = fs::remove_file(temp_db_path);
        
        info!("Backup created successfully.");
        Ok(backup_path)
    }

    pub fn restore_backup(backup_zip: &Path, db_path: &Path) -> Result<()> {
        if !backup_zip.exists() {
            return Err(CoreError::PathError(format!("Backup zip not found at {:?}", backup_zip)));
        }

        info!("Restoring backup from {:?}", backup_zip);

        let file = File::open(backup_zip)?;
        let mut archive = zip::ZipArchive::new(file)?;

        // Find the database file in the zip
        let mut db_in_zip = archive.by_name("mediavault.db")?;
        
        let mut buffer = Vec::new();
        db_in_zip.read_to_end(&mut buffer)?;
        
        fs::write(db_path, buffer)?;
        
        info!("Restore completed successfully. Application restart recommended.");
        Ok(())
    }

    pub async fn check_for_updates() -> Result<String> {
        info!("Checking for updates via GitHub Releases API...");
        
        let client = crate::scraper::build_http_client();

        match client
            .get("https://api.github.com/repos/selfhost-media-orchestrator/media-manager/releases/latest")
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json: serde_json::Value = resp.json().await
                        .unwrap_or(serde_json::json!({}));
                    let version = json["tag_name"]
                        .as_str()
                        .unwrap_or("unknown")
                        .trim_start_matches('v')
                        .to_string();
                    info!("Latest version from GitHub: {}", version);
                    Ok(version)
                } else {
                    info!("GitHub API returned {}, using current version", resp.status());
                    Ok("0.2.0".to_string())
                }
            }
            Err(e) => {
                tracing::warn!("Update check failed (network error?): {}", e);
                // Return current version so UI doesn't show a false upgrade prompt
                Ok("0.2.0".to_string())
            }
        }
    }
}
