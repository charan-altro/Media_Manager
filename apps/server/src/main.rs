// apps/server/src/main.rs
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use media_core::db::{self, SettingsRepository};
use media_core::task_manager::TaskManager;
use tower_http::cors::CorsLayer;

use media_core::scanner::streaming::{StreamManager, StreamingService};

pub mod state;
pub mod utils;
pub mod routes;
use state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    
    // Fallback log level if RUST_LOG is not set
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,media_core=debug,server=debug");
    }
    tracing_subscriber::fmt::init();

    println!("=========================================");
    println!("  Media Orchestrator Backend Starting... ");
    println!("=========================================");

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:mediavault.db?mode=rwc".to_string());
    let pool = db::init_pool(&database_url).await.expect("Failed to initialize database pool");
    let repos = Arc::new(db::Repositories::new(pool.clone()));
    let task_manager = Arc::new(TaskManager::new());
    
    // Configurable Transcode Directory (Default to RAM disk /dev/shm on Linux/Pi to save SD card life)
    let transcode_dir = std::env::var("HLS_TRANSCODE_DIR").unwrap_or_else(|_| {
        if cfg!(target_os = "linux") && std::path::Path::new("/dev/shm").exists() {
            "/dev/shm/media_manager_transcodes".to_string()
        } else {
            std::env::temp_dir().join("media_manager_transcodes").to_string_lossy().to_string()
        }
    });
    media_core::config::set_hls_transcode_dir(transcode_dir.clone());
    let stream_manager: Arc<dyn StreamingService> = Arc::new(StreamManager::new(std::path::PathBuf::from(&transcode_dir)));

    let clients = Arc::new(media_core::scraper::ScraperClients::from_settings(&repos).await);
    let scraper_service = Arc::new(media_core::scraper::service::DefaultScraperService::new(
        repos.clone(),
        task_manager.clone(),
        clients,
    ));
    let scanner_service = Arc::new(media_core::scanner::service::DefaultScannerService::new(
        repos.clone(),
        task_manager.clone(),
    ));

    let app_state = Arc::new(AppState {
        pool: pool.clone(),
        repos: repos.clone(),
        task_manager: task_manager.clone(),
        stream_manager: stream_manager.clone(),
        scraper_service: scraper_service.clone(),
        scanner_service: scanner_service.clone(),
    });

    // Start background stream cleanup
    let sm_for_cleanup = stream_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            sm_for_cleanup.cleanup_stale_streams().await;
        }
    });

    // Start background notification monitor
    let task_manager_for_notifications = task_manager.clone();
    let repos_for_notifications = app_state.repos.clone();
    tokio::spawn(async move {
        let mut rx = task_manager_for_notifications.subscribe();
        let notifier = media_core::notifications::Notifier::new();
        
        while let Ok(update) = rx.recv().await {
            // Only notify on completion or error
            if update.status == "completed" || update.status == "error" {
                // Check settings for webhook URL
                if let Ok(settings) = repos_for_notifications.settings.get_all().await {
                    if let Some(url) = settings.get("discord_webhook_url") {
                        if !url.is_empty() {
                            let _ = notifier.send_discord_webhook(url, &update).await;
                        }
                    }
                }
            }
        }
    });

    // Start Real-time Watchdog
    let repos_for_watchdog = repos.clone();
    let scanner_service_for_watchdog = scanner_service.clone();
    tokio::spawn(async move {
        let watchdog = media_core::scanner::watchdog::Watchdog::new(repos_for_watchdog, scanner_service_for_watchdog);
        if let Err(e) = watchdog.start().await {
            tracing::error!("Watchdog failed: {}", e);
        }
    });

    let allowed_origin = std::env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let cors = if allowed_origin == "*" {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
            .allow_origin(allowed_origin.parse::<axum::http::HeaderValue>().unwrap())
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
    };

    let app = Router::new()
        .nest("/api", routes::health::router())
        .nest("/api", routes::webhook::router())
        .nest("/api", routes::library::router())
        .nest("/api", routes::media::router())
        .nest("/api", routes::scraper::router())
        .nest("/api", routes::cleanup::router())
        .nest("/api", routes::assets::router())
        .nest("/api", routes::streaming::router())
        .nest("/api", routes::system::router())
        .nest_service("/transcodes", tower_http::services::ServeDir::new(&transcode_dir))
        .layer(cors)
        .with_state(app_state.clone())
        .fallback_service(
            tower_http::services::ServeDir::new("frontend/dist")
                .fallback(tower_http::services::ServeFile::new("frontend/dist/index.html"))
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 7878));
    tracing::info!("Server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(app_state))
        .await
        .unwrap();
}

async fn shutdown_signal(state: Arc<AppState>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, cleaning up streams...");
    state.stream_manager.stop_all_streams().await;
}

