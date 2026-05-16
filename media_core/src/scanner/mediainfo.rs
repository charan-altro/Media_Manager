// core/src/scanner/mediainfo.rs
use std::process::Command;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::errors::{Result, CoreError};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MediaStreamInfo {
    pub index: i32,
    pub stream_type: String,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub channels: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaDetails {
    pub width: i32,
    pub height: i32,
    pub video_codec: String,
    pub audio_codec: String,
    pub audio_channels: i32,
    pub size_bytes: i64,
    pub duration_secs: i32,
    pub rotation: i32,
    pub bit_depth: i32,
    pub streams: Vec<MediaStreamInfo>,
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
            rotation: 0,
            bit_depth: 0,
            streams: Vec::new(),
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
    parse_media_info_from_json(&json)
}

pub(crate) fn parse_media_info_from_json(json: &serde_json::Value) -> Result<MediaDetails> {
    let streams_array = json["streams"].as_array().ok_or_else(|| CoreError::PathError("No streams found".to_string()))?;
    
    let mut video_stream = None;
    let mut audio_stream = None;
    let mut streams = Vec::new();

    for stream in streams_array {
        let stream_type = stream["codec_type"].as_str().unwrap_or("unknown").to_string();
        let codec = stream["codec_name"].as_str().unwrap_or("unknown").to_string();
        let index = stream["index"].as_i64().unwrap_or(0) as i32;
        
        let language = stream["tags"]["language"].as_str().map(|s| s.to_string());
        let title = stream["tags"]["title"].as_str().map(|s| s.to_string());
        
        let channels = stream["channels"].as_i64().map(|c| c as i32);

        streams.push(MediaStreamInfo {
            index,
            stream_type: stream_type.clone(),
            codec: codec.clone(),
            language,
            title,
            channels,
        });

        match stream_type.as_str() {
            "video" if video_stream.is_none() => video_stream = Some(stream),
            "audio" if audio_stream.is_none() => audio_stream = Some(stream),
            _ => {}
        }
    }

    let video = video_stream.ok_or_else(|| CoreError::PathError("No video stream".to_string()))?;
    let width = video["width"].as_i64().unwrap_or(0) as i32;
    let height = video["height"].as_i64().unwrap_or(0) as i32;
    let video_codec = video["codec_name"].as_str().unwrap_or("unknown").to_string();
    
    // Parse rotation
    let mut rotation = 0;
    if let Some(rotate_val) = video["tags"]["rotate"].as_str() {
        if let Ok(r) = rotate_val.parse::<i32>() {
            rotation = r;
        }
    } else if let Some(rotate_val) = video["tags"]["rotate"].as_i64() {
        rotation = rotate_val as i32;
    }
    
    // Some newer ffprobe versions might expose rotation in side_data_list
    if rotation == 0 {
        if let Some(side_data) = video["side_data_list"].as_array() {
            for data in side_data {
                if data["side_data_type"] == "Display Matrix" {
                    if let Some(rot) = data["rotation"].as_i64() {
                        rotation = rot as i32;
                    } else if let Some(rot_str) = data["rotation"].as_str() {
                        if let Ok(r) = rot_str.parse::<i32>() {
                            rotation = r;
                        }
                    }
                }
            }
        }
    }

    // Parse bit depth
    let mut bit_depth = 0;
    if let Some(bits) = video["bits_per_raw_sample"].as_str() {
        if let Ok(b) = bits.parse::<i32>() {
            bit_depth = b;
        }
    } else if let Some(bits) = video["bits_per_raw_sample"].as_i64() {
        bit_depth = bits as i32;
    }

    if bit_depth == 0 {
        // Fallback for some codecs that use bits_per_sample
        if let Some(bits) = video["bits_per_sample"].as_i64() {
            bit_depth = bits as i32;
        } else if let Some(bits) = video["bits_per_sample"].as_str() {
            if let Ok(b) = bits.parse::<i32>() {
                bit_depth = b;
            }
        }
    }

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
        rotation,
        bit_depth,
        streams,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_media_info_from_json() {
        let mock_json = json!({
            "streams": [
                {
                    "index": 0,
                    "codec_name": "hevc",
                    "codec_type": "video",
                    "width": 1920,
                    "height": 1080,
                    "bits_per_raw_sample": "10",
                    "tags": {
                        "rotate": "90"
                    }
                },
                {
                    "index": 1,
                    "codec_name": "aac",
                    "codec_type": "audio",
                    "channels": 6,
                    "tags": {
                        "language": "eng",
                        "title": "Surround 5.1"
                    }
                },
                {
                    "index": 2,
                    "codec_name": "subrip",
                    "codec_type": "subtitle",
                    "tags": {
                        "language": "eng",
                        "title": "English (SDH)"
                    }
                }
            ],
            "format": {
                "size": "1500000000",
                "duration": "7200.5"
            }
        });

        let details = parse_media_info_from_json(&mock_json).expect("Failed to parse");

        assert_eq!(details.width, 1920);
        assert_eq!(details.height, 1080);
        assert_eq!(details.video_codec, "hevc");
        assert_eq!(details.audio_codec, "aac");
        assert_eq!(details.audio_channels, 6);
        assert_eq!(details.size_bytes, 1500000000);
        assert_eq!(details.duration_secs, 7200);
        assert_eq!(details.rotation, 90);
        assert_eq!(details.bit_depth, 10);
        
        assert_eq!(details.streams.len(), 3);
        
        let video_stream = &details.streams[0];
        assert_eq!(video_stream.index, 0);
        assert_eq!(video_stream.stream_type, "video");
        assert_eq!(video_stream.codec, "hevc");
        assert_eq!(video_stream.channels, None);
        
        let audio_stream = &details.streams[1];
        assert_eq!(audio_stream.index, 1);
        assert_eq!(audio_stream.stream_type, "audio");
        assert_eq!(audio_stream.codec, "aac");
        assert_eq!(audio_stream.language.as_deref(), Some("eng"));
        assert_eq!(audio_stream.title.as_deref(), Some("Surround 5.1"));
        assert_eq!(audio_stream.channels, Some(6));

        let subtitle_stream = &details.streams[2];
        assert_eq!(subtitle_stream.index, 2);
        assert_eq!(subtitle_stream.stream_type, "subtitle");
        assert_eq!(subtitle_stream.codec, "subrip");
        assert_eq!(subtitle_stream.language.as_deref(), Some("eng"));
    }
}
