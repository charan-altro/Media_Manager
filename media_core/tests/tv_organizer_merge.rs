use media_core::{db, scanner, models, AppConfig, CoreContext};
use media_core::scanner::service::ScannerService;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use media_core::db::{LibraryReader, LibraryWriter, TvReader, Repositories};

#[tokio::test]
async fn test_tv_organizer_merge() -> anyhow::Result<()> {
    // 1. Setup fresh DB (runs migrations and tv show deduplication on init)
    let db_url = "sqlite::memory:"; 
    let pool = db::init_pool(db_url).await?;
    let repos = Arc::new(Repositories::new(pool.clone()));

    // 2. Setup test media directory
    let test_dir = PathBuf::from("test_media_tv_merge");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir)?;
    }
    fs::create_dir_all(&test_dir)?;

    // Create duplicate folders:
    // A. "Better.Call.Saul.S04.720p.BRRip.MkvCage" containing Season 04
    let dup_dir_a = test_dir.join("Better.Call.Saul.S04.720p.BRRip.MkvCage");
    let season_4_dir = dup_dir_a.join("Season 04");
    fs::create_dir_all(&season_4_dir)?;
    let episode_4_file = season_4_dir.join("S04E04.mkv");
    {
        let mut f = File::create(&episode_4_file)?;
        f.write_all(b"dummy video content for episode 4")?;
    }

    // B. "[TorrentCouch net] Better Call Saul" containing Season 05
    let dup_dir_b = test_dir.join("[TorrentCouch net] Better Call Saul");
    let season_5_dir = dup_dir_b.join("Season 05");
    fs::create_dir_all(&season_5_dir)?;
    let episode_5_file = season_5_dir.join("S05E05.mkv");
    {
        let mut f = File::create(&episode_5_file)?;
        f.write_all(b"dummy video content for episode 5")?;
    }

    // C. Write some metadata files that should be cleaned up (e.g. .nfo and .jpg)
    let nfo_file_a = dup_dir_a.join("Better.Call.Saul.nfo");
    {
        let mut f = File::create(&nfo_file_a)?;
        f.write_all(b"dummy nfo")?;
    }
    let poster_file_b = dup_dir_b.join("poster.jpg");
    {
        let mut f = File::create(&poster_file_b)?;
        f.write_all(b"dummy poster")?;
    }

    // 3. Add TV Library
    let lib_id = repos.library.insert(
        "TV Lib",
        test_dir.to_str().unwrap(),
        models::MediaType::Tv
    ).await?;
    let libraries = repos.library.find_all().await?;
    let lib = libraries.into_iter().find(|l| l.id == lib_id).unwrap();

    let task_manager = Arc::new(media_core::task_manager::TaskManager::new());
    let config = AppConfig {
        ffmpeg_path: "ffmpeg".to_string(),
        ffprobe_path: "ffprobe".to_string(),
        hls_transcode_dir: "tmp".to_string(),
    };
    let ctx = CoreContext::new(config, repos.clone(), task_manager.clone());
    let scanner_service = scanner::service::DefaultScannerService::new(ctx, task_manager.clone());

    // 4. Run the first scan
    scanner_service.scan_library(&lib, "task_1".into()).await?;

    // 5. Verify database representation
    // Verify we have only ONE show "Better Call Saul"
    let shows = repos.tv.find_all_shows(Some(lib_id), None, None).await?;
    assert_eq!(shows.len(), 1, "Should have deduplicated the show to a single record");
    assert_eq!(shows[0].title, "Better Call Saul");

    let show_id = shows[0].id;
    let seasons = repos.tv.find_seasons_by_show_id(show_id).await?;
    assert_eq!(seasons.len(), 2, "Should have 2 seasons under the single show");

    // 6. Verify filesystem organization
    // Organized files should be under "Better Call Saul/Season 04" and "Better Call Saul/Season 05"
    let organized_show_dir = test_dir.join("Better Call Saul");
    let organized_ep_4 = organized_show_dir.join("Season 04").join("Better Call Saul - S04E04.mkv");
    let organized_ep_5 = organized_show_dir.join("Season 05").join("Better Call Saul - S05E05.mkv");

    assert!(organized_ep_4.exists(), "Episode 4 must be organized to destination");
    assert!(organized_ep_5.exists(), "Episode 5 must be organized to destination");

    // 7. Verify folder cleanup: old duplicate directories must be deleted
    assert!(!dup_dir_a.exists(), "Old duplicate directory A must be deleted");
    assert!(!dup_dir_b.exists(), "Old duplicate directory B must be deleted");

    // Cleanup test directory
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir)?;
    }

    Ok(())
}
