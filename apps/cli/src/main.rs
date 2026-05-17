use clap::{Parser, Subcommand};
use media_core::db::{self, Repositories, LibraryReader};
use media_core::models::LibraryId;
use media_core::task_manager::TaskManager;
use media_core::scanner::service::{ScannerService, DefaultScannerService};
use media_core::scraper::service::{ScraperService, DefaultScraperService};
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
    let repos = Arc::new(Repositories::new(pool.clone()));
    let task_manager = Arc::new(TaskManager::new());
    let scanner_service = DefaultScannerService::new(repos.clone(), task_manager.clone());
    let scraper_clients = Arc::new(media_core::scraper::ScraperClients::from_settings(&repos).await);
    let scraper_service = DefaultScraperService::new(repos.clone(), task_manager.clone(), scraper_clients);

    match cli.command {
        Commands::Scan { library_id } => {
            println!("Starting scan for library ID: {}", library_id);
            let libraries = repos.library.find_all().await?;
            if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(library_id)) {
                let task_id = uuid::Uuid::new_v4().to_string();
                scanner_service.scan_library(&lib, task_id).await?;
                println!("Scan completed.");
            } else {
                println!("Error: Library not found.");
            }
        }
        Commands::Scrape { library_id, media_type } => {
            println!("Starting bulk scrape for library ID: {} (Type: {})", library_id, media_type);
            
            let task_id = uuid::Uuid::new_v4().to_string();
            scraper_service.bulk_scrape_library(LibraryId(library_id), task_id).await?;
            
            println!("Bulk scrape completed.");
        }
        Commands::Cleanup { library_id } => {
            println!("Starting cleanup for library ID: {}", library_id);
            let libraries = repos.library.find_all().await?;
            if let Some(lib) = libraries.into_iter().find(|l| l.id == LibraryId(library_id)) {
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
            let backup_dir = dest.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("backups"));
            match media_core::maintenance::MaintenanceEngine::create_backup(&pool, &backup_dir).await {
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
