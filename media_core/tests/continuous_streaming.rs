// media_core/tests/continuous_streaming.rs
use media_core::scanner::streaming::{StreamManager, StreamingService};
use tempfile::tempdir;
use std::path::PathBuf;

#[tokio::test]
async fn test_request_segment_starts_session() {
    let tmp_dir = tempdir().expect("Failed to create temp dir");
    let transcode_dir = tmp_dir.path().to_path_buf();
    
    let stream_manager = StreamManager::new(transcode_dir.clone());
    
    let input_dir = tempdir().expect("Failed to create input temp dir");
    let input_file = input_dir.path().join("test_video.mp4");
    std::fs::write(&input_file, b"dummy video content").expect("Failed to write dummy video");

    let stream_id = "test_stream_req";
    
    // This should start a session. It will likely fail to spawn FFmpeg correctly with a dummy file 
    // but we can check if it attempted to create the session or directory.
    let _ = stream_manager.request_segment(stream_id, &input_file, 5, "seg_005.ts").await;
    
    // Check if output dir exists for the id
    assert!(transcode_dir.join(stream_id).exists());
}
