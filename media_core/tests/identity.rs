use media_core::{db, scanner, models, AppConfig, CoreContext};
use media_core::scanner::service::ScannerService;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use media_core::db::{LibraryReader, LibraryWriter, MovieReader, Repositories};

#[tokio::test]
async fn test_identity_healing() -> anyhow::Result<()> {
    // 1. Setup fresh DB
    let db_url = "sqlite::memory:"; // Use in-memory for testing
    let pool = db::init_pool(db_url).await?;
    let repos = Arc::new(Repositories::new(pool.clone()));

    // 2. Setup test media
    let test_dir = PathBuf::from("test_media_mvp1_1");
    if test_dir.exists() { fs::remove_dir_all(&test_dir)?; }
    fs::create_dir_all(&test_dir)?;

    let file_name = "test_café.mp4"; // Test NFC
    let file_path = test_dir.join(file_name);
    {
        let mut f = File::create(&file_path)?;
        let buf = vec![1u8; 200 * 1024]; // 200KB of '1's
        f.write_all(&buf)?;
    }

    // 3. Add library and scan
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
    scanner_service.scan_library(&lib, "test_task".into()).await?;

    // Wait for background analysis to finish
    let mut attempts = 0;
    while task_manager.is_library_scanning(lib_id).await {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        attempts += 1;
        if attempts > 200 {
            panic!("Library scan 1 did not finish in time");
        }
    }

    // 4. Verify initial fingerprint
    let movies = repos.movie.find_all(Some(lib_id), None, None).await?;
    assert_eq!(movies.len(), 1);
    
    let movie_file = sqlx::query_as::<_, models::MovieFile>("SELECT * FROM movie_files WHERE movie_id = ?")
        .bind(movies[0].id)
        .fetch_one(&pool).await?;
    
    let original_fingerprint = movie_file.fingerprint.expect("Fingerprint should be set");
    let original_path = movie_file.file_path;
    println!("Initial Fingerprint: {}", original_fingerprint);
    println!("Initial Path: {}", original_path);

    // 5. Move file
    let archive_dir = test_dir.join("Archive");
    fs::create_dir_all(&archive_dir)?;
    let organized_file_path = test_dir.join("test café").join("test café.mp4");
    let new_file_path = archive_dir.join("different_movie.mp4");
    fs::rename(&organized_file_path, &new_file_path)?;

    // 6. Scan again
    scanner_service.scan_library(&lib, "test_task_2".into()).await?;

    // Wait for background analysis to finish
    let mut attempts = 0;
    while task_manager.is_library_scanning(lib_id).await {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        attempts += 1;
        if attempts > 200 {
            panic!("Library scan 2 did not finish in time");
        }
    }

    // 7. Verify Healing (Fingerprint remains same, path updates)
    let movies_after = repos.movie.find_all(Some(lib_id), None, None).await?;
    for m in &movies_after {
        println!("DEBUG Movie in DB: id={:?}, title={}, year={:?}", m.id, m.title, m.year);
        let files = sqlx::query_as::<_, models::MovieFile>("SELECT * FROM movie_files WHERE movie_id = ?")
            .bind(m.id)
            .fetch_all(&pool).await?;
        for f in files {
            println!("  DEBUG File: id={:?}, path={}, fingerprint={:?}, is_missing={}", f.id, f.file_path, f.fingerprint, f.is_missing);
        }
    }
    assert_eq!(movies_after.len(), 1, "Should still have 1 movie after move");

    let movie_file_after = sqlx::query_as::<_, models::MovieFile>("SELECT * FROM movie_files WHERE movie_id = ?")
        .bind(movies_after[0].id)
        .fetch_one(&pool).await?;

    println!("After Move Fingerprint: {}", movie_file_after.fingerprint.as_ref().unwrap_or(&"None".to_string()));
    println!("After Move Path: {}", movie_file_after.file_path);

    // The fingerprint must be preserved — this verifies it's the same physical file
    assert_eq!(movie_file_after.fingerprint.unwrap(), original_fingerprint, "Fingerprint must match");

    // The file must exist on disk at the path recorded in the DB.
    // The organiser may have moved the file back to its canonical location
    // (e.g. "test café/test café.mp4") based on the title metadata.
    let abs_path_after = test_dir.join(&movie_file_after.file_path);
    assert!(abs_path_after.exists(), "File must exist at the DB path: {}", movie_file_after.file_path);

    // Cleanup
    fs::remove_dir_all(&test_dir)?;
    
    Ok(())
}
