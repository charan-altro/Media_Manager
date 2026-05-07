use clap::{Parser, Subcommand};
use media_core::db;
use media_core::scanner::worker::scan_library;
use media_core::task_manager::TaskManager;
use std::sync::Arc;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "media_cli")]
#[command(about = "Headless Media Manager CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a library for new files
    Scan {
        #[arg(short, long)]
        library_id: i64,
    },
    /// Scrape metadata for unmatched items
    Scrape {
        #[arg(short, long)]
        library_id: i64,
        #[arg(short, long)]
        media_type: String, // "movie" or "tv"
    },
    /// Cleanup duplicates and empty folders
    Cleanup {
        #[arg(short, long)]
        library_id: i64,
    },
    /// Create a database backup
    Backup {
        #[arg(short, long)]
        dest: Option<String>,
    },
    /// Restore database from backup
    Restore {
        #[arg(short, long)]
        file: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:mediavault.db?mode=rwc".to_string());
    let pool = db::init_pool(&database_url).await?;
    let task_manager = Arc::new(TaskManager::new());

    match cli.command {
        Commands::Scan { library_id } => {
            println!("Starting scan for library ID: {}", library_id);
            let libraries = db::queries::get_all_libraries(&pool).await?;
            if let Some(lib) = libraries.into_iter().find(|l| l.id == library_id) {
                let task_id = uuid::Uuid::new_v4().to_string();
                scan_library(&pool, &lib, task_id, &task_manager).await?;
                println!("Scan completed.");
            } else {
                println!("Error: Library not found.");
            }
        }
        Commands::Scrape { library_id, media_type } => {
            println!("Starting bulk scrape for library ID: {} (Type: {})", library_id, media_type);
            
            let tmdb_key = std::env::var("TMDB_API_KEY").unwrap_or_default();
            let omdb_key = std::env::var("OMDB_API_KEY").unwrap_or_default();
            // In a real scenario, we might fetch these from the settings table we added in Phase B
            let settings = db::queries::get_settings(&pool).await.unwrap_or_default();
            let fanart_key = settings.get("fanart_api_key").cloned().unwrap_or_default();
            let trakt_key = settings.get("trakt_api_key").cloned().unwrap_or_default();
            let tvdb_key = settings.get("tvdb_api_key").cloned().unwrap_or_default();

            let clients = Arc::new(media_core::scraper::ScraperClients::new(
                tmdb_key, omdb_key, fanart_key, trakt_key, tvdb_key
            ));

            let mut all_tasks = Vec::new();
            if media_type == "movie" {
                if let Ok(movies) = db::queries::get_all_movies(&pool, Some(library_id), None, None).await {
                    let unmatched: Vec<_> = movies.into_iter().filter(|m| m.status == media_core::models::MediaStatus::Unmatched).collect();
                    all_tasks.extend(unmatched.into_iter().map(|m| (m.id, m.title, m.year, "movie")));
                }
            } else {
                if let Ok(shows) = db::queries::get_all_tv_shows(&pool, Some(library_id), None, None).await {
                    let unmatched: Vec<_> = shows.into_iter().filter(|s| s.status == media_core::models::MediaStatus::Unmatched).collect();
                    all_tasks.extend(unmatched.into_iter().map(|s| (s.id, s.title, None, "tv")));
                }
            }

            let pool_arc = Arc::new(pool);
            let script_path = settings.get("post_processing_script").map(|s| s.as_str());

            for (id, title, year, m_type) in all_tasks {
                println!("Scraping: {}", title);
                if m_type == "movie" {
                    let _ = media_core::scraper::scrape_movie(id, &title, year, &clients, &pool_arc, script_path).await;
                } else {
                    let _ = media_core::scraper::scrape_tv_show(id, &title, &clients, &pool_arc, script_path).await;
                }
            }
            println!("Bulk scrape completed.");
        }
        Commands::Cleanup { library_id } => {
            println!("Starting cleanup for library ID: {}", library_id);
            let libraries = db::queries::get_all_libraries(&pool).await?;
            if let Some(lib) = libraries.into_iter().find(|l| l.id == library_id) {
                let cleanup = media_core::cleanup::CleanupService::new(PathBuf::from(lib.path));
                let dupes = cleanup.remove_duplicate_artwork()?;
                println!("Removed {} duplicate artwork files.", dupes.len());
                let empty = cleanup.remove_empty_folders()?;
                println!("Removed {} empty folders.", empty.len());
            } else {
                println!("Error: Library not found.");
            }
        }
        Commands::Backup { dest } => {
            let db_path = std::path::Path::new("mediavault.db");
            let backup_dir = dest.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("backups"));
            match media_core::maintenance::MaintenanceEngine::create_backup(db_path, &backup_dir) {
                Ok(path) => println!("Backup created successfully: {:?}", path),
                Err(e) => println!("Error creating backup: {}", e),
            }
        }
        Commands::Restore { file } => {
            let db_path = std::path::Path::new("mediavault.db");
            let backup_zip = std::path::Path::new(&file);
            match media_core::maintenance::MaintenanceEngine::restore_backup(backup_zip, db_path) {
                Ok(_) => println!("Database restored successfully. Please restart any running instances."),
                Err(e) => println!("Error restoring backup: {}", e),
            }
        }
    }

    Ok(())
}
