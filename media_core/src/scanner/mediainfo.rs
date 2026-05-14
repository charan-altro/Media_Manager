// core/src/scanner/mediainfo.rs
use std::process::Command;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::errors::{Result, CoreError};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaDetails {
    pub width: i32,
    pub height: i32,
    pub video_codec: String,
    pub audio_codec: String,
    pub audio_channels: i32,
    pub size_bytes: i64,
    pub duration_secs: i32,
}

impl Default for MediaDetails {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            video_codec: "unknown".to_string(),
            audio_codec: "unknown".to_string(),
            audio_channels: 0,
            size_bytes: 0,
            duration_secs: 0,
        }
    }
}

fn check_ffprobe() -> Result<()> {
    let ffprobe_path = crate::config::get_ffprobe_path();
    let output = Command::new(&ffprobe_path)
        .arg("-version")
        .output();

    match output {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(CoreError::MediaInfoError(format!("ffprobe not found at {}: Please ensure FFmpeg (which includes ffprobe) is installed and in your system PATH or bundled as a sidecar.", ffprobe_path)))
        }
        Err(e) => Err(CoreError::MediaInfoError(format!("Failed to check ffprobe at {}: {}", ffprobe_path, e))),
    }
}

pub fn get_media_info(path: &Path) -> Result<MediaDetails> {
    check_ffprobe()?;
    let output = Command::new(crate::config::get_ffprobe_path())
        .args(&[
            "-v", "quiet",
            "-print_format", "json",
            "-show_streams",
            "-show_format",
            path.to_str().ok_or_else(|| CoreError::PathError("Invalid path".to_string()))?,
        ])
        .output()?;

    if !output.status.success() {
        return Err(CoreError::MediaInfoError("ffprobe failed".to_string()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    
    let streams = json["streams"].as_array().ok_or_else(|| CoreError::PathError("No streams found".to_string()))?;
    
    let mut video_stream = None;
    let mut audio_stream = None;

    for stream in streams {
        match stream["codec_type"].as_str() {
            Some("video") if video_stream.is_none() => video_stream = Some(stream),
            Some("audio") if audio_stream.is_none() => audio_stream = Some(stream),
            _ => {}
        }
    }

    let video = video_stream.ok_or_else(|| CoreError::PathError("No video stream".to_string()))?;
    let width = video["width"].as_i64().unwrap_or(0) as i32;
    let height = video["height"].as_i64().unwrap_or(0) as i32;
    let video_codec = video["codec_name"].as_str().unwrap_or("unknown").to_string();
    
    let audio_codec = audio_stream.map(|s| s["codec_name"].as_str().unwrap_or("unknown")).unwrap_or("none").to_string();
    let audio_channels = audio_stream.map(|s| s["channels"].as_i64().unwrap_or(0) as i32).unwrap_or(0);
    
    let size_bytes = json["format"]["size"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
    let duration_secs = json["format"]["duration"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0) as i32;

    Ok(MediaDetails {
        width,
        height,
        video_codec,
        audio_codec,
        audio_channels,
        size_bytes,
        duration_secs,
    })
}
