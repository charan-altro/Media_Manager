use media_core::db::{init_pool, SqliteMediaRepository, MediaRepository};
use media_core::models::MediaStream;
use std::sync::Arc;

#[tokio::test]
async fn test_stash_parity_db_operations() {
    let pool = init_pool("sqlite::memory:").await.unwrap();
    let repo = SqliteMediaRepository::new(Arc::new(pool.clone()));

    // Test upsert_media_stream
    let stream = MediaStream {
        id: 0,
        file_hash: "test_hash".to_string(),
        stream_index: 0,
        stream_type: "video".to_string(),
        codec: Some("h264".to_string()),
        language: Some("en".to_string()),
        title: Some("Main Title".to_string()),
        channels: None,
        is_default: true,
    };

    repo.upsert_stream(&stream).await.unwrap();

    // Verify insertion
    let row: (String, String) = sqlx::query_as("SELECT codec, language FROM media_streams WHERE file_hash = ? AND stream_index = ?")
        .bind("test_hash")
        .bind(0)
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(row.0, "h264");
    assert_eq!(row.1, "en");

    // Test update on conflict
    let updated_stream = MediaStream {
        id: 0,
        file_hash: "test_hash".to_string(),
        stream_index: 0,
        stream_type: "video".to_string(),
        codec: Some("hevc".to_string()),
        language: Some("jp".to_string()),
        title: Some("New Title".to_string()),
        channels: None,
        is_default: false,
    };

    repo.upsert_stream(&updated_stream).await.unwrap();

    let row: (String, String, bool) = sqlx::query_as("SELECT codec, language, is_default FROM media_streams WHERE file_hash = ? AND stream_index = ?")
        .bind("test_hash")
        .bind(0)
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(row.0, "hevc");
    assert_eq!(row.1, "jp");
    assert_eq!(row.2, false);

    // Test upsert_generated_asset
    repo.upsert_generated_asset("test_hash", "preview", "/path/to/preview.mp4").await.unwrap();

    let path: String = sqlx::query_scalar("SELECT path FROM generated_assets WHERE file_hash = ? AND asset_type = ?")
        .bind("test_hash")
        .bind("preview")
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(path, "/path/to/preview.mp4");

    // Test update on conflict
    repo.upsert_generated_asset("test_hash", "preview", "/new/path.mp4").await.unwrap();

    let path: String = sqlx::query_scalar("SELECT path FROM generated_assets WHERE file_hash = ? AND asset_type = ?")
        .bind("test_hash")
        .bind("preview")
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(path, "/new/path.mp4");
}
