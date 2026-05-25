use media_core::db::{Repositories, LibraryReader, MovieReader, TvReader};
use std::sync::Arc;

#[tokio::test]
async fn test_inspect_db() -> anyhow::Result<()> {
    let db_url = "sqlite:../mediavault.db"; 
    println!("Connecting to database: {}", db_url);
    let pool = sqlx::SqlitePool::connect(db_url).await?;
    let repos = Repositories::new(pool);

    println!("--- LIBRARIES ---");
    if let Ok(libs) = repos.library.find_all().await {
        for lib in libs {
            println!("ID: {}, Name: {}, Path: {}, MediaType: {:?}", lib.id, lib.name, lib.path, lib.media_type);
        }
    }

    println!("--- MOVIES ---");
    if let Ok(movies) = repos.movie.find_all(None, None, None).await {
        for m in movies {
            let file_path = repos.movie.get_full_path(m.id).await.ok().flatten();
            println!("ID: {}, Title: {}, Status: {:?}, FullPath: {:?}", m.id, m.title, m.status, file_path);
        }
    }

    println!("--- TV SHOWS ---");
    if let Ok(shows) = repos.tv.find_all_shows(None, None, None).await {
        for s in shows {
            println!("ID: {}, Title: {}, Status: {:?}", s.id, s.title, s.status);
            if let Ok(seasons) = repos.tv.find_seasons_by_show_id(s.id).await {
                for season in seasons {
                    println!("  Season {}: {}", season.season_number, season.name.unwrap_or_default());
                    if let Ok(episodes) = repos.tv.find_episodes_by_season_id(season.id).await {
                        for ep in episodes {
                            let ep_path = repos.tv.get_episode_full_path(ep.id).await.ok().flatten();
                            println!("    Ep {}: {}, FullPath: {:?}", ep.episode_number, ep.original_name, ep_path);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
