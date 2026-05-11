// core/src/lib.rs
pub mod config {
    use std::sync::RwLock;
    use once_cell::sync::Lazy;

    static FFMPEG_PATH: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new("ffmpeg".to_string()));
    static FFPROBE_PATH: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new("ffprobe".to_string()));

    pub fn set_ffmpeg_path(path: String) {
        if let Ok(mut p) = FFMPEG_PATH.write() { *p = path; }
    }
    pub fn get_ffmpeg_path() -> String {
        FFMPEG_PATH.read().map(|p| p.clone()).unwrap_or_else(|_| "ffmpeg".to_string())
    }

    pub fn set_ffprobe_path(path: String) {
        if let Ok(mut p) = FFPROBE_PATH.write() { *p = path; }
    }
    pub fn get_ffprobe_path() -> String {
        FFPROBE_PATH.read().map(|p| p.clone()).unwrap_or_else(|_| "ffprobe".to_string())
    }
}

pub mod db;
pub mod errors;
pub mod models;
pub mod parser;
pub mod scanner;
pub mod paths;
pub mod task_manager;
pub mod scraper;
pub mod nfo;
pub mod renamer;
pub mod cleanup;
pub mod subtitles;
pub mod exporter;
pub mod hooks;
pub mod maintenance;
pub mod notifications;

pub fn init() {
    println!("Core initialized");
}
