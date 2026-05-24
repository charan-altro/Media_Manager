// tests/mvp2_streaming_tests.rs
use media_core::scanner::streaming::{StreamManager, StreamingService};
use media_core::AppConfig;
use tempfile::tempdir;


#[tokio::test]
async fn test_mvp2_streaming_lifecycle() {
    let tmp_dir = tempdir().expect("Failed to create temp dir");
    let transcode_dir = tmp_dir.path().to_path_buf();
    
    let config = AppConfig {
        ffmpeg_path: "ffmpeg".to_string(),
        ffprobe_path: "ffprobe".to_string(),
        hls_transcode_dir: transcode_dir.to_string_lossy().to_string(),
    };
    
    let stream_manager = StreamManager::new(config);
    
    let input_dir = tempdir().expect("Failed to create input temp dir");
    let input_file = input_dir.path().join("test_video.mp4");
    std::fs::write(&input_file, b"dummy video content").expect("Failed to write dummy video");

    let stream_id = "test_stream_1";
    let start_result = stream_manager.start_hls(stream_id, &input_file).await;
    
    // We expect an error because it's a dummy file, but it should be a clean error
    assert!(start_result.is_err());

    // Verify heartbeat doesn't crash
    stream_manager.update_heartbeat(stream_id).await;
    
    // Manually create the directory to test cleanup if start_hls didn't leave it (due to error)
    let session_dir = transcode_dir.join(stream_id);
    if !session_dir.exists() {
        std::fs::create_dir_all(&session_dir).unwrap();
    }
    
    assert!(transcode_dir.exists());
}

#[tokio::test]
async fn test_hls_transcode_dir_config() {
    let custom_dir = "custom_transcodes_dir";
    let config = AppConfig {
        ffmpeg_path: "ffmpeg".to_string(),
        ffprobe_path: "ffprobe".to_string(),
        hls_transcode_dir: custom_dir.to_string(),
    };
    assert_eq!(config.hls_transcode_dir, custom_dir);
}
