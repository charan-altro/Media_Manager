// core/src/scanner/ffmpeg.rs
use std::process::Command;
use std::path::{Path, PathBuf};
use crate::errors::{Result, CoreError};
use tracing::{info, error};

pub struct FfmpegEngine;

impl FfmpegEngine {
    fn check_ffmpeg() -> Result<()> {
        let ffmpeg_path = crate::config::get_ffmpeg_path();
        let output = Command::new(&ffmpeg_path)
            .arg("-version")
            .output();

        match output {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                error!("FFmpeg was not found at {}. Please install FFmpeg to enable streaming and analysis.", ffmpeg_path);
                Err(CoreError::FfmpegError(format!("FFmpeg not found at {}: Please ensure 'ffmpeg' is installed and added to your system PATH or bundled as a sidecar.", ffmpeg_path)))
            }
            Err(e) => Err(CoreError::FfmpegError(format!("Failed to check FFmpeg at {}: {}", ffmpeg_path, e))),
        }
    }

    pub fn extract_thumbnail(input_path: &Path, dest_path: &Path, time_offset: &str) -> Result<PathBuf> {
        Self::check_ffmpeg()?;
        info!("Extracting thumbnail from {:?} at {}", input_path, time_offset);
        
        let output = Command::new(crate::config::get_ffmpeg_path())
            .args(&[
                "-ss", time_offset,
                "-i", input_path.to_str().ok_or_else(|| CoreError::PathError("Invalid input path".to_string()))?,
                "-vframes", "1",
                "-q:v", "2",
                "-y",
                dest_path.to_str().ok_or_else(|| CoreError::PathError("Invalid dest path".to_string()))?,
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            error!("FFmpeg thumbnail extraction failed: {}", err);
            return Err(CoreError::FfmpegError(format!("FFmpeg failed: {}", err)));
        }

        Ok(dest_path.to_path_buf())
    }

    pub fn detect_aspect_ratio(input_path: &Path) -> Result<String> {
        Self::check_ffmpeg()?;
        info!("Detecting aspect ratio for {:?}", input_path);
        
        // We'll analyze 10 frames around the 5-minute mark to avoid credits/intros
        let output = Command::new(crate::config::get_ffmpeg_path())
            .args(&[
                "-ss", "00:05:00",
                "-i", input_path.to_str().ok_or_else(|| CoreError::PathError("Invalid path".to_string()))?,
                "-vf", "cropdetect=24:16:0",
                "-vframes", "20",
                "-f", "null",
                "-",
            ])
            .output()?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Parse cropdetect output: crop=1920:800:0:140
        let mut crops = std::collections::HashMap::new();
        for line in stderr.lines() {
            if let Some(pos) = line.find("crop=") {
                let crop_val = line[pos..].split_whitespace().next().unwrap_or_default();
                *crops.entry(crop_val.to_string()).or_insert(0) += 1;
            }
        }

        // Find the most frequent crop
        let best_crop = crops.into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(crop, _)| crop)
            .ok_or_else(|| CoreError::PathError("No crop detected".to_string()))?;

        // Extract width and height from crop=W:H:X:Y
        let parts: Vec<&str> = best_crop.trim_start_matches("crop=").split(':').collect();
        if parts.len() >= 2 {
            let w: f32 = parts[0].parse()?;
            let h: f32 = parts[1].parse()?;
            
            // Calculate ratio and return a pretty string (e.g. 2.40:1 or 16:9)
            let ratio = w / h;
            if (ratio - 1.77).abs() < 0.1 { return Ok("16:9".to_string()); }
            if (ratio - 2.39).abs() < 0.1 { return Ok("2.39:1".to_string()); }
            if (ratio - 1.33).abs() < 0.1 { return Ok("4:3".to_string()); }
            
            return Ok(format!("{:.2}:1", ratio));
        }

        Err(CoreError::FfmpegError("Failed to parse crop results".to_string()))
    }

    pub fn create_hls_stream(input_path: &Path, output_dir: &Path) -> Result<PathBuf> {
        Self::check_ffmpeg()?;
        info!("Starting HLS transcode for {:?} into {:?}", input_path, output_dir);
        
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        let playlist_path = output_dir.join("playlist.m3u8");
        let segment_pattern = output_dir.join("seg_%03d.ts");

        // Simple Hardware Acceleration check
        // In a full implementation, we'd probe with `ffmpeg -encoders`
        let encoder = if cfg!(target_os = "macos") {
            "h264_videotoolbox"
        } else {
            "libx264"
        };

        let _ = Command::new(crate::config::get_ffmpeg_path())
            .args(&[
                "-i", input_path.to_str().ok_or_else(|| CoreError::PathError("Invalid path".to_string()))?,
                "-c:v", encoder,
                "-preset", "ultrafast",
                "-crf", "22",
                "-c:a", "aac",
                "-b:a", "128k",
                "-ac", "2",
                "-f", "hls",
                "-hls_time", "4",
                "-hls_list_size", "0", 
                "-hls_flags", "independent_segments",
                "-hls_segment_filename", segment_pattern.to_str().unwrap(),
                playlist_path.to_str().unwrap(),
            ])
            .spawn()?;
        
        Ok(playlist_path)
    }
}
