// core/src/lib.rs
pub mod config;
pub use config::{AppConfig, CoreContext};

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
pub mod services;

pub fn init() {
    println!("Core initialized");
}
