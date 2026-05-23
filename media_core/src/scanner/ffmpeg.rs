// core/src/scanner/ffmpeg.rs
use std::process::Command;
use std::path::{Path, PathBuf};
use crate::errors::{Result, CoreError};
use tracing::{info, error};

pub struct FfmpegEngine;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamStrategy {
    DirectCopy,
    SmartRemux { video_copy: bool, audio_copy: bool },
    FullTranscode,
}

impl FfmpegEngine {
    /// Determines the best streaming strategy based on media details and browser support.
    pub fn get_stream_strategy(details: &crate::scanner::mediainfo::MediaDetails) -> StreamStrategy {
        let video_supported = ["h264", "vp9", "av1"].contains(&details.video_codec.as_str());
        let audio_supported = ["aac", "mp3"].contains(&details.audio_codec.as_str());
        let rotation_ok = details.rotation == 0;

        match (video_supported, audio_supported, rotation_ok) {
            (true, true, true) => StreamStrategy::DirectCopy,
            (true, false, true) => StreamStrategy::SmartRemux { video_copy: true, audio_copy: false },
            (false, true, _) | (true, true, false) => StreamStrategy::SmartRemux { video_copy: false, audio_copy: true },
            (true, false, false) | (false, false, _) => StreamStrategy::FullTranscode,
        }
    }

    pub fn probe_hw_codecs() -> Vec<String> {
        Self::probe_hw_codecs_with_path("ffmpeg")
    }

    pub fn probe_hw_codecs_with_path(ffmpeg_path: &str) -> Vec<String> {
        let mut supported = Vec::new();
        let codecs_to_test = ["h264_v4l2m2m", "h264_nvenc", "h264_qsv", "h264_videotoolbox", "h264_vaapi"];
        
        for codec in codecs_to_test {
            let output = Command::new(ffmpeg_path)
                .args(&[
                    "-v", "error",
                    "-f", "lavfi",
                    "-i", "color=c=black:s=128x128:r=1",
                    "-c:v", codec,
                    "-t", "0.5",
                    "-f", "null",
                    "-"
                ])
                .output();
                
            if let Ok(out) = output {
                if out.status.success() {
                    supported.push(codec.to_string());
                }
            }
        }
        supported
    }

    pub fn probe_hw_decoders() -> Vec<String> {
        Self::probe_hw_decoders_with_path("ffmpeg")
    }

    pub fn probe_hw_decoders_with_path(ffmpeg_path: &str) -> Vec<String> {
        let mut supported = Vec::new();
        let decoders_to_test = ["h264_v4l2m2m", "hevc_v4l2m2m", "h264_cuvid", "hevc_cuvid", "h264_qsv", "hevc_qsv"];
        
        for decoder in decoders_to_test {
            let output = Command::new(ffmpeg_path)
                .args(&[
                    "-v", "error",
                    "-decoders"
                ])
                .output();
                
            if let Ok(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains(decoder) {
                    supported.push(decoder.to_string());
                }
            }
        }
        supported
    }

    pub fn get_hw_decoder(source_codec: &str, supported_decoders: &[String]) -> Option<String> {
        match source_codec {
            "h264" => {
                if supported_decoders.contains(&"h264_v4l2m2m".to_string()) {
                    Some("h264_v4l2m2m".to_string())
                } else if supported_decoders.contains(&"h264_cuvid".to_string()) {
                    Some("h264_cuvid".to_string())
                } else {
                    None
                }
            },
            "hevc" => {
                if supported_decoders.contains(&"hevc_v4l2m2m".to_string()) {
                    Some("hevc_v4l2m2m".to_string())
                } else if supported_decoders.contains(&"hevc_cuvid".to_string()) {
                    Some("hevc_cuvid".to_string())
                } else {
                    None
                }
            },
            _ => None
        }
    }

    pub fn check_ffmpeg() -> Result<()> {
        Self::check_ffmpeg_with_path("ffmpeg")
    }

    pub fn check_ffmpeg_with_path(ffmpeg_path: &str) -> Result<()> {
        let output = Command::new(ffmpeg_path)
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
        Self::extract_thumbnail_with_path("ffmpeg", input_path, dest_path, time_offset)
    }

    pub fn extract_thumbnail_with_path(ffmpeg_path: &str, input_path: &Path, dest_path: &Path, time_offset: &str) -> Result<PathBuf> {
        Self::check_ffmpeg_with_path(ffmpeg_path)?;
        info!("Extracting thumbnail from {:?} at {}", input_path, time_offset);
        
        let output = Command::new(ffmpeg_path)
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

    pub fn generate_preview(input_path: &Path, output_path: &Path) -> Result<PathBuf> {
        Self::generate_preview_with_path("ffmpeg", input_path, output_path)
    }

    pub fn generate_preview_with_path(ffmpeg_path: &str, input_path: &Path, output_path: &Path) -> Result<PathBuf> {
        Self::check_ffmpeg_with_path(ffmpeg_path)?;
        info!("Generating 10s preview for {:?} at {:?}", input_path, output_path);
        
        let output = Command::new(ffmpeg_path)
            .args(&[
                "-ss", "00:05:00",
                "-i", input_path.to_str().ok_or_else(|| CoreError::PathError("Invalid input path".to_string()))?,
                "-t", "10",
                "-vf", "scale=w=480:h=-2",
                "-c:v", "libx264",
                "-preset", "ultrafast",
                "-crf", "28",
                "-an",
                "-y",
                output_path.to_str().ok_or_else(|| CoreError::PathError("Invalid dest path".to_string()))?,
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            error!("FFmpeg preview generation failed: {}", err);
            return Err(CoreError::FfmpegError(format!("FFmpeg failed: {}", err)));
        }

        Ok(output_path.to_path_buf())
    }

    pub fn generate_advanced_assets(input_path: &Path, hash: &str, generated_root: &Path, duration_secs: f64) -> Result<()> {
        Self::generate_advanced_assets_with_path("ffmpeg", "ffprobe", input_path, hash, generated_root, duration_secs)
    }

    pub fn generate_advanced_assets_with_path(ffmpeg_path: &str, _ffprobe_path: &str, input_path: &Path, hash: &str, generated_root: &Path, duration_secs: f64) -> Result<()> {
        let dest_dir = generated_root.join(hash);
        if !dest_dir.exists() {
            std::fs::create_dir_all(&dest_dir)?;
        }

        let thumb_path = dest_dir.join("thumb.jpg");
        Self::extract_thumbnail_with_path(ffmpeg_path, input_path, &thumb_path, "00:01:00")?;

        let sprite_path = dest_dir.join("sprite.webp");
        Self::generate_sprite_sheet_with_path(ffmpeg_path, input_path, &sprite_path, duration_secs)?;

        let preview_path = dest_dir.join("preview.mp4");
        Self::generate_preview_with_path(ffmpeg_path, input_path, &preview_path)?;

        Ok(())
    }

    fn format_vtt_time(seconds: f64) -> String {
        let hrs = (seconds / 3600.0).floor() as i32;
        let mins = ((seconds % 3600.0) / 60.0).floor() as i32;
        let secs = (seconds % 60.0).floor() as i32;
        let ms = ((seconds % 1.0) * 1000.0).floor() as i32;
        format!("{:02}:{:02}:{:02}.{:03}", hrs, mins, secs, ms)
    }

    pub fn generate_sprite_sheet(input_path: &Path, output_path: &Path, duration_secs: f64) -> Result<PathBuf> {
        Self::generate_sprite_sheet_with_path("ffmpeg", input_path, output_path, duration_secs)
    }

    pub fn generate_sprite_sheet_with_path(ffmpeg_path: &str, input_path: &Path, output_path: &Path, duration_secs: f64) -> Result<PathBuf> {
        Self::check_ffmpeg_with_path(ffmpeg_path)?;
        let interval = duration_secs / 100.0;
        
        info!("Generating 10x10 sprite sheet for {:?} at interval {}s", input_path, interval);

        let output = Command::new(ffmpeg_path)
            .args(&[
                "-i", input_path.to_str().unwrap(),
                "-vf", &format!("fps=1/{},scale=160:-1,tile=10x10", interval),
                "-y",
                output_path.to_str().unwrap(),
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(CoreError::FfmpegError(format!("FFmpeg sprite failed: {}", err)));
        }

        let vtt_path = output_path.with_extension("vtt");
        let sprite_filename = output_path.file_name().unwrap_or_default().to_string_lossy();
        let mut vtt = String::from("WEBVTT\n\n");
        
        for i in 0..100 {
            let start_time = Self::format_vtt_time(i as f64 * interval);
            let end_time = Self::format_vtt_time((i + 1) as f64 * interval);
            
            let x = (i % 10) * 160;
            let y = (i / 10) * 90;
            
            vtt.push_str(&format!("{} --> {}\n", start_time, end_time));
            vtt.push_str(&format!("{}#xywh={},{},160,90\n\n", sprite_filename, x, y));
        }
        
        let _ = std::fs::write(&vtt_path, vtt);

        Ok(output_path.to_path_buf())
    }

    pub fn detect_aspect_ratio(input_path: &Path) -> Result<String> {
        Self::detect_aspect_ratio_with_path("ffmpeg", input_path)
    }

    pub fn detect_aspect_ratio_with_path(ffmpeg_path: &str, input_path: &Path) -> Result<String> {
        Self::check_ffmpeg_with_path(ffmpeg_path)?;
        info!("Detecting aspect ratio for {:?}", input_path);
        
        let output = Command::new(ffmpeg_path)
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
        
        let mut crops = std::collections::HashMap::new();
        for line in stderr.lines() {
            if let Some(pos) = line.find("crop=") {
                let crop_val = line[pos..].split_whitespace().next().unwrap_or_default();
                *crops.entry(crop_val.to_string()).or_insert(0) += 1;
            }
        }

        let best_crop = crops.into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(crop, _)| crop)
            .ok_or_else(|| CoreError::PathError("No crop detected".to_string()))?;

        let parts: Vec<&str> = best_crop.trim_start_matches("crop=").split(':').collect();
        if parts.len() >= 2 {
            let w: f32 = parts[0].parse()?;
            let h: f32 = parts[1].parse()?;
            
            let ratio = w / h;
            if (ratio - 1.77).abs() < 0.1 { return Ok("16:9".to_string()); }
            if (ratio - 2.39).abs() < 0.1 { return Ok("2.39:1".to_string()); }
            if (ratio - 1.33).abs() < 0.1 { return Ok("4:3".to_string()); }
            
            return Ok(format!("{:.2}:1", ratio));
        }

        Err(CoreError::FfmpegError("Failed to parse crop results".to_string()))
    }

    pub fn create_hls_stream(input_path: &Path, output_dir: &Path) -> Result<PathBuf> {
        Self::create_hls_stream_with_path("ffmpeg", input_path, output_dir)
    }

    pub fn create_hls_stream_with_path(ffmpeg_path: &str, input_path: &Path, output_dir: &Path) -> Result<PathBuf> {
        Self::check_ffmpeg_with_path(ffmpeg_path)?;
        info!("Starting HLS transcode for {:?} into {:?}", input_path, output_dir);
        
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        let playlist_path = output_dir.join("playlist.m3u8");
        let segment_pattern = output_dir.join("seg_%03d.ts");

        let supported_codecs = Self::probe_hw_codecs_with_path(ffmpeg_path);
        let encoder = if let Some(codec) = supported_codecs.first() {
            codec.as_str()
        } else {
            "libx264"
        };

        let _ = Command::new(ffmpeg_path)
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
