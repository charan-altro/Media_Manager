use media_core::{db, scanner, models, paths};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use sqlx::sqlite::SqlitePool;

#[tokio::test]
async fn test_identity_healing() -> anyhow::Result<()> {
    // 1. Setup fresh DB
    let db_url = "sqlite::memory:"; // Use in-memory for testing
    let pool = db::init_pool(db_url).await?;

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
    let lib_id = db::queries::insert_library(&pool, "Test Lib", test_dir.to_str().unwrap(), models::MediaType::Movie).await?;
    let libraries = db::queries::get_all_libraries(&pool).await?;
    let lib = libraries.into_iter().find(|l| l.id == lib_id).unwrap();

    let task_manager = std::sync::Arc::new(media_core::task_manager::TaskManager::new());
    scanner::worker::scan_library(&pool, &lib, "test_task".into(), &task_manager).await?;

    // 4. Verify initial fingerprint
    let movies = db::queries::get_all_movies(&pool, Some(lib_id), None, None).await?;
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
    let new_file_path = archive_dir.join(file_name);
    fs::rename(&file_path, &new_file_path)?;

    // 6. Scan again
    scanner::worker::scan_library(&pool, &lib, "test_task_2".into(), &task_manager).await?;

    // 7. Verify Healing (Fingerprint remains same, path updates)
    let movies_after = db::queries::get_all_movies(&pool, Some(lib_id), None, None).await?;
    assert_eq!(movies_after.len(), 1, "Should still have 1 movie after move");

    let movie_file_after = sqlx::query_as::<_, models::MovieFile>("SELECT * FROM movie_files WHERE movie_id = ?")
        .bind(movies_after[0].id)
        .fetch_one(&pool).await?;

    println!("After Move Fingerprint: {}", movie_file_after.fingerprint.as_ref().unwrap());
    println!("After Move Path: {}", movie_file_after.file_path);

    assert_eq!(movie_file_after.fingerprint.unwrap(), original_fingerprint, "Fingerprint must match");
    assert_ne!(movie_file_after.file_path, original_path, "Path must have updated");
    assert!(movie_file_after.file_path.contains("Archive"), "Path should contain Archive");

    // Cleanup
    fs::remove_dir_all(&test_dir)?;
    
    Ok(())
}
