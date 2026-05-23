// media_core/src/services/playback_service.rs
use std::sync::Arc;
use std::path::{Path, PathBuf};
use crate::CoreContext;
use crate::scanner::streaming::StreamingService;
use crate::models::PlaybackState;
use crate::errors::{Result, CoreError};
use crate::db::MediaRepository;

/// Domain service wrapping all streaming, HLS transcoding, and playback history/tracking.
pub struct PlaybackService {
    ctx: CoreContext,
    streaming: Arc<dyn StreamingService>,
}

impl PlaybackService {
    pub fn new(
        ctx: CoreContext,
        streaming: Arc<dyn StreamingService>,
    ) -> Self {
        Self { ctx, streaming }
    }

    /// Starts HLS streaming for a media item.
    pub async fn start_hls(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        self.streaming.start_hls(id, input_path).await.map_err(|e| CoreError::RuntimeError(e.to_string()))
    }

    /// Starts DASH streaming for a media item.
    pub async fn start_dash(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        self.streaming.start_dash(id, input_path).await.map_err(|e| CoreError::RuntimeError(e.to_string()))
    }

    /// Stops HLS/DASH streaming for a media item.
    pub async fn stop_stream(&self, id: &str) {
        self.streaming.stop_stream(id).await;
    }

    /// Updates playback tracking progress in the database and keeps the transcoding active.
    pub async fn update_playback_progress(
        &self,
        media_id: i64,
        media_type: &str,
        position_ms: i32,
        duration_ms: i32,
        is_finished: bool,
    ) -> Result<()> {
        let stream_id = if media_type == "movie" {
            format!("movie_{}", media_id)
        } else {
            format!("episode_{}", media_id)
        };
        
        self.streaming.update_heartbeat(&stream_id).await;
        
        self.ctx.repos.media.update_playback_status(media_id, media_type, position_ms, duration_ms, is_finished).await?;
        Ok(())
    }

    /// Resolves the current saved playback position for a media item.
    pub async fn get_playback_status(&self, media_id: i64, media_type: &str) -> Result<Option<PlaybackState>> {
        Ok(self.ctx.repos.media.get_playback_status(media_id, media_type).await?)
    }

    /// Periodically cleans up streams that have timed out or have no viewers.
    pub async fn cleanup_stale_streams(&self) {
        self.streaming.cleanup_stale_streams().await;
    }

    /// Exposes the HLS transcode directory path.
    pub fn transcode_dir(&self) -> &str {
        &self.ctx.config.hls_transcode_dir
    }
}
