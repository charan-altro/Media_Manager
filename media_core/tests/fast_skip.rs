use media_core::{db, scanner, models, AppConfig, CoreContext};
use media_core::scanner::service::ScannerService;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use media_core::db::{LibraryReader, LibraryWriter, Repositories};

#[tokio::test]
async fn test_fast_skip_logic() -> anyhow::Result<()> {
    // 1. Setup fresh DB
    let db_url = "sqlite::memory:";
    let pool = db::init_pool(db_url).await?;
    let repos = Arc::new(Repositories::new(pool.clone()));

    // 2. Setup test media
    let test_dir = PathBuf::from("test_media_fast_skip");
    if test_dir.exists() { fs::remove_dir_all(&test_dir)?; }
    fs::create_dir_all(&test_dir)?;

    let file_name = "test_movie.mp4";
    let file_path = test_dir.join(file_name);
    {
        let mut f = File::create(&file_path)?;
        let buf = vec![1u8; 100 * 1024]; // 100KB
        f.write_all(&buf)?;
    }

    // 3. Add library and initial scan
    let lib_id = repos.library.insert("Test Lib", test_dir.to_str().unwrap(), models::MediaType::Movie).await?;
    let libraries = repos.library.find_all().await?;
    let lib = libraries.into_iter().find(|l| l.id == lib_id).unwrap();

    let task_manager = Arc::new(media_core::task_manager::TaskManager::new());
    let config = AppConfig {
        ffmpeg_path: "ffmpeg".to_string(),
        ffprobe_path: "ffprobe".to_string(),
        hls_transcode_dir: "tmp".to_string(),
    };
    let ctx = CoreContext::new(config, repos.clone(), task_manager.clone());

    let scanner_service = scanner::service::DefaultScannerService::new(ctx, task_manager.clone());
    scanner_service.scan_library(&lib, "initial_scan".into()).await?;

    // 4. Verify mtime is set
    let movie_file: models::MovieFile = sqlx::query_as("SELECT * FROM movie_files LIMIT 1")
        .fetch_one(&pool).await?;
    
    assert!(movie_file.mtime.is_some());
    assert!(movie_file.mtime.unwrap() > 0);
    let original_mtime = movie_file.mtime.unwrap();

    // 5. MANUALLY sabotage the DB to prove skip
    // We change the resolution to something else.
    // If the scanner skips this file, it will keep our fake value.
    sqlx::query("UPDATE movie_files SET resolution = '2160p' WHERE id = ?")
        .bind(movie_file.id)
        .execute(&pool).await?;

    // 6. Rescan (Should Skip)
    scanner_service.scan_library(&lib, "rescan_skip".into()).await?;

    let movie_file_after_skip: models::MovieFile = sqlx::query_as("SELECT * FROM movie_files LIMIT 1")
        .fetch_one(&pool).await?;
    
    // It should STILL be '2160p' because we skipped the probe and used our update shortcut
    assert_eq!(movie_file_after_skip.resolution, Some(media_core::models::Resolution::R2160p));
    assert_eq!(movie_file_after_skip.mtime, Some(original_mtime));

    // 7. Touch the file (Change mtime)
    // Small delay to ensure mtime definitely changes if the filesystem has low resolution
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    {
        let mut f = File::options().append(true).open(&file_path)?;
        f.write_all(&[2u8])?; // Append one byte to change size and mtime
    }
    
    let new_mtime = file_path.metadata()?.modified()?
        .duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
    assert_ne!(new_mtime, original_mtime);

    // 8. Rescan (Should NOT Skip)
    scanner_service.scan_library(&lib, "rescan_full".into()).await?;

    let movie_file_after_full: models::MovieFile = sqlx::query_as("SELECT * FROM movie_files LIMIT 1")
        .fetch_one(&pool).await?;
    
    // It should NO LONGER be '2160p' because it was re-processed
    assert_ne!(movie_file_after_full.resolution, Some(media_core::models::Resolution::R2160p));
    assert_eq!(movie_file_after_full.mtime, Some(new_mtime));

    // Cleanup
    fs::remove_dir_all(&test_dir)?;
    
    Ok(())
}
