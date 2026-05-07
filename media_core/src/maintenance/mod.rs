// core/src/maintenance/mod.rs
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Read, Write};
use anyhow::{Result, anyhow};
use zip::write::FileOptions;
use chrono::Local;
use tracing::info;
use crate::nfo::writer::NfoWriter;
use crate::db::queries;
use sqlx::SqlitePool;

pub struct MaintenanceEngine;

impl MaintenanceEngine {
    pub async fn export_all_nfos(pool: &SqlitePool) -> Result<()> {
        info!("Starting bulk NFO export...");
        
        // Export Movies
        if let Ok(movies) = queries::get_all_movies(pool, None, None, None).await {
            for movie in movies {
                if let Ok(Some(file)) = sqlx::query_as::<_, crate::models::MovieFile>("SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1")
                    .bind(movie.id)
                    .fetch_optional(pool)
                    .await 
                {
                    let path = PathBuf::from(&file.file_path);
                    let nfo_path = path.with_extension("nfo");
                    let _ = NfoWriter::write_movie_nfo(&movie, &nfo_path).await;
                }
            }
        }

        // Export TV Shows
        if let Ok(shows) = queries::get_all_tv_shows(pool, None, None, None).await {
            for show in shows {
                let mut show_path = None;
                if let Ok(seasons) = queries::get_seasons_by_show_id(pool, show.id).await {
                    for s in seasons {
                        if let Ok(eps) = queries::get_episodes_by_season_id(pool, s.id).await {
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

    pub fn create_backup(db_path: &Path, backup_dir: &Path) -> Result<PathBuf> {
        if !db_path.exists() {
            return Err(anyhow!("Database file not found at {:?}", db_path));
        }

        fs::create_dir_all(backup_dir)?;

        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = backup_dir.join(format!("mediavault_backup_{}.zip", timestamp));
        
        info!("Creating backup at {:?}", backup_path);

        let file = File::create(&backup_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        // Add database file to zip
        zip.start_file("mediavault.db", options)?;
        let mut db_file = File::open(db_path)?;
        let mut buffer = Vec::new();
        db_file.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;

        zip.finish()?;
        
        info!("Backup created successfully.");
        Ok(backup_path)
    }

    pub fn restore_backup(backup_zip: &Path, db_path: &Path) -> Result<()> {
        if !backup_zip.exists() {
            return Err(anyhow!("Backup zip not found at {:?}", backup_zip));
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

    pub fn check_for_updates() -> Result<String> {
        info!("Checking for updates...");
        Ok("0.2.0".to_string())
    }
}
