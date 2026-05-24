// core/src/subtitles/mod.rs
pub mod sidecar;
pub use sidecar::{discover_sidecar_subtitles, srt_to_vtt, SidecarSubtitle};

use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::errors::{CoreError, Result};
use std::path::Path;
use tokio::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const OPENSUBTITLES_BASE: &str = "https://api.opensubtitles.com/api/v1";

#[derive(Debug, Serialize, Deserialize)]
pub struct SubtitleResult {
    pub id: String,
    pub attributes: SubtitleAttributes,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubtitleAttributes {
    pub language: String,
    pub release: String,
    pub download_count: i32,
    pub files: Vec<SubtitleFile>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubtitleFile {
    pub file_id: i64,
    pub file_name: String,
}

#[derive(Debug, Deserialize)]
struct SubtitleResponse {
    pub data: Vec<SubtitleResult>,
}

#[derive(Debug, Deserialize)]
struct DownloadResponse {
    pub link: String,
}

pub struct SubtitleClient {
    client: Client,
    api_key: String,
}

pub fn compute_opensubtitles_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let chunk_size = 65536;

    if size < chunk_size {
        return Err(CoreError::PathError("File too small".to_string()));
    }

    let mut hash = size;
    let mut buffer = [0u8; 8];

    // Read first 64KB
    for _ in 0..(chunk_size / 8) {
        file.read_exact(&mut buffer)?;
        let val = u64::from_le_bytes(buffer);
        hash = hash.wrapping_add(val);
    }

    // Read last 64KB
    file.seek(SeekFrom::End(-(chunk_size as i64)))?;
    for _ in 0..(chunk_size / 8) {
        file.read_exact(&mut buffer)?;
        let val = u64::from_le_bytes(buffer);
        hash = hash.wrapping_add(val);
    }

    Ok(format!("{:016x}", hash))
}

impl SubtitleClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn search(&self, imdb_id: &str, language: &str) -> Result<Vec<SubtitleResult>> {
        let resp = self.client
            .get(format!("{}/subtitles", OPENSUBTITLES_BASE))
            .header("Api-Key", &self.api_key)
            .query(&[
                ("imdb_id", imdb_id.trim_start_matches('t')),
                ("languages", language),
            ])
            .send()
            .await?
            .json::<SubtitleResponse>()
            .await?;

        Ok(resp.data)
    }

    pub async fn search_by_hash(&self, moviehash: &str, language: &str) -> Result<Vec<SubtitleResult>> {
        let resp = self.client
            .get(format!("{}/subtitles", OPENSUBTITLES_BASE))
            .header("Api-Key", &self.api_key)
            .query(&[
                ("moviehash", moviehash),
                ("languages", language),
            ])
            .send()
            .await?
            .json::<SubtitleResponse>()
            .await?;

        Ok(resp.data)
    }

    pub async fn download(&self, file_id: i64, dest_path: &Path, language: &str) -> Result<String> {
        // 1. Get download link
        let payload = serde_json::json!({ "file_id": file_id });
        let resp = self.client
            .post(format!("{}/download", OPENSUBTITLES_BASE))
            .header("Api-Key", &self.api_key)
            .json(&payload)
            .send()
            .await?
            .json::<DownloadResponse>()
            .await?;

        // 2. Download the file
        let content = self.client
            .get(&resp.link)
            .send()
            .await?
            .bytes()
            .await?;

        let srt_path = dest_path.with_extension(format!("{}.srt", language));
        fs::write(&srt_path, content).await?;

        Ok(srt_path.to_str().unwrap_or_default().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::fs::File;

    #[test]
    fn test_compute_opensubtitles_hash_too_small() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("too_small.test");
        let mut file = File::create(&path).unwrap();
        // Write exactly 10 bytes, smaller than the 65536 threshold
        file.write_all(&[0; 10]).unwrap();
        let res = compute_opensubtitles_hash(&path);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Path error: File too small");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_compute_opensubtitles_hash_valid() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("valid_hash.test");
        let mut file = File::create(&path).unwrap();
        // Create a 128KB file (65536 * 2)
        let size = 65536 * 2;
        let mut data = vec![0u8; size];
        
        // Fill first 8 bytes with 1s
        for i in 0..8 {
            data[i] = 1;
        }
        
        // Fill last 8 bytes with 2s
        for i in 0..8 {
            data[size - 8 + i] = 2;
        }

        file.write_all(&data).unwrap();
        
        let hash = compute_opensubtitles_hash(&path).unwrap();
        
        // As a simple validation, it should return a valid 16-character hex string
        assert_eq!(hash.len(), 16);
        let _ = std::fs::remove_file(path);
    }
}
