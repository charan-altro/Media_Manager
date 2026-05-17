use media_core::{db, scanner, models};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use media_core::db::{LibraryReader, LibraryWriter, MovieReader, MediaRepository, Repositories};

#[tokio::test]
async fn test_stash_parity_integration() -> anyhow::Result<()> {
    // 1. Setup fresh DB
    let db_url = "sqlite::memory:"; 
    let pool = db::init_pool(db_url).await?;
    let repos = Arc::new(Repositories::new(pool.clone()));

    // 2. Setup test media
    let test_dir = PathBuf::from("test_media_stash_parity");
    if test_dir.exists() { fs::remove_dir_all(&test_dir)?; }
    fs::create_dir_all(&test_dir)?;

    let file_name = "integration_test.mp4";
    let file_path = test_dir.join(file_name);
    {
        let mut f = File::create(&file_path)?;
        // Create a 200KB file to ensure it's large enough for OSHash if needed
        let buf = vec![1u8; 200 * 1024]; 
        f.write_all(&buf)?;
    }

    // 3. Add library and scan
    let lib_id = repos.library.insert("Test Lib", test_dir.to_str().unwrap(), models::MediaType::Movie).await?;
    let libraries = repos.library.find_all().await?;
    let lib = libraries.into_iter().find(|l| l.id == lib_id).unwrap();

    let task_manager = Arc::new(media_core::task_manager::TaskManager::new());
    scanner::worker::scan_library(repos.clone(), &lib, "test_task".into(), &task_manager).await?;

    // 4. Verify initial record and fingerprint
    let movie_file = repos.movie.find_file_by_path(file_name).await?.expect("File should be in DB");
    let fingerprint = movie_file.fingerprint.expect("Fingerprint should be set");
    println!("Initial Fingerprint: {}", fingerprint);

    // 5. Manually populate MediaStream and GeneratedAsset (simulating advanced analysis/assets generation)
    // We use the fingerprint as the link, which is what 'Stash Parity' means.
    let stream = models::MediaStream {
        id: 0,
        file_hash: fingerprint.clone(),
        stream_index: 0,
        stream_type: "video".to_string(),
        codec: Some("h264".to_string()),
        language: None,
        title: None,
        channels: None,
        is_default: true,
    };
    repos.media.upsert_stream(&stream).await?;
    repos.media.upsert_generated_asset(&fingerprint, "preview", "assets/preview.mp4").await?;

    // 6. Move file (Simulate file rename/move within library)
    let moved_file_name = "moved_test.mp4";
    let moved_file_path = test_dir.join(moved_file_name);
    fs::rename(&file_path, &moved_file_path)?;

    // 7. Scan again
    // The scanner should see the old path is gone, see the new path, calculate same fingerprint, and 'heal' the record.
    scanner::worker::scan_library(repos.clone(), &lib, "test_task_2".into(), &task_manager).await?;

    // 8. Verify Identity Resolution (Healing)
    let movie_file_after = repos.movie.find_file_by_fingerprint(&fingerprint).await?.expect("Should find file by fingerprint");
    assert_eq!(movie_file_after.file_path, moved_file_name, "Path should be updated to new location");
    assert_eq!(movie_file_after.fingerprint.unwrap(), fingerprint, "Fingerprint must be the same");

    // 9. Verify Assets are still linked via fingerprint
    // This is the core of the 'Stash Parity' feature: assets survive file moves.
    let stream_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_streams WHERE file_hash = ?")
        .bind(&fingerprint)
        .fetch_one(&pool).await?;
    assert_eq!(stream_count, 1, "MediaStream should still be linked to fingerprint");

    let asset_path: String = sqlx::query_scalar("SELECT path FROM generated_assets WHERE file_hash = ? AND asset_type = ?")
        .bind(&fingerprint)
        .bind("preview")
        .fetch_one(&pool).await?;
    assert_eq!(asset_path, "assets/preview.mp4", "GeneratedAsset should still be linked to fingerprint");

    // Cleanup
    if test_dir.exists() { fs::remove_dir_all(&test_dir)?; }
    
    Ok(())
}
