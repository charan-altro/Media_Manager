// core/src/subtitles/sidecar.rs
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarSubtitle {
    pub file_path: String,
    pub language: String,
    pub name: String,
}

pub fn discover_sidecar_subtitles(media_path: &Path) -> std::io::Result<Vec<SidecarSubtitle>> {
    let parent = match media_path.parent() {
        Some(p) => p,
        None => return Ok(vec![]),
    };
    
    let file_stem = match media_path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return Ok(vec![]),
    };
    
    let mut results = vec![];
    
    let entries = std::fs::read_dir(parent)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.to_lowercase() == "srt" {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        if filename.starts_with(file_stem) {
                            let mut lang = "en".to_string();
                            let stem_len = file_stem.len();
                            let suffix = &filename[stem_len..];
                            
                            if suffix.starts_with('.') {
                                let parts: Vec<&str> = suffix.split('.').collect();
                                if parts.len() >= 3 {
                                    lang = parts[1].to_lowercase();
                                }
                            }
                            
                            let name = match lang.as_str() {
                                "en" | "eng" => "English".to_string(),
                                "zh" | "zho" | "chi" => "Chinese".to_string(),
                                "es" | "spa" => "Spanish".to_string(),
                                "fr" | "fra" | "fre" => "French".to_string(),
                                "de" | "deu" | "ger" => "German".to_string(),
                                "it" | "ita" => "Italian".to_string(),
                                "ja" | "jpn" => "Japanese".to_string(),
                                "ko" | "kor" => "Korean".to_string(),
                                _ => lang.to_uppercase(),
                            };

                            results.push(SidecarSubtitle {
                                file_path: path.to_string_lossy().to_string(),
                                language: lang,
                                name,
                            });
                        }
                    }
                }
            }
        }
    }
    
    Ok(results)
}

pub fn srt_to_vtt(srt: &str) -> String {
    let mut vtt = String::with_capacity(srt.len() + 32);
    vtt.push_str("WEBVTT\n\n");
    
    let mut lines = srt.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            vtt.push('\n');
            continue;
        }
        
        if trimmed.parse::<u32>().is_ok() {
            if let Some(next_line) = lines.peek() {
                if next_line.contains("-->") {
                    continue;
                }
            }
        }
        
        if trimmed.contains("-->") {
            let replaced = trimmed.replace(',', ".");
            vtt.push_str(&replaced);
            vtt.push('\n');
        } else {
            vtt.push_str(line);
            vtt.push('\n');
        }
    }
    vtt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_to_vtt() {
        let srt = "1\n00:01:20,300 --> 00:01:23,400\nHello World\n\n2\n00:01:24,100 --> 00:01:25,500\nThis is a test";
        let vtt = srt_to_vtt(srt);
        assert!(vtt.contains("WEBVTT"));
        assert!(vtt.contains("00:01:20.300 --> 00:01:23.400"));
        assert!(vtt.contains("Hello World"));
        assert!(!vtt.contains("1\n00:01:20"));
    }
}
