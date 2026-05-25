use sqlx::sqlite::SqlitePool;

#[tokio::test]
async fn test_db_dump() -> anyhow::Result<()> {
    let db_url = "sqlite:c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/mediavault.db?mode=rwc";
    println!("Connecting to database: {}", db_url);
    
    let pool = SqlitePool::connect(&db_url).await?;
    
    println!("--- LIBRARIES ---");
    let libraries: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, name, path, media_type FROM libraries"
    )
    .fetch_all(&pool)
    .await?;
    
    for lib in &libraries {
        println!("ID: {}, Name: {}, Path: {}, Media Type: {}", lib.0, lib.1, lib.2, lib.3);
    }
    
    println!("--- MOVIES & FILES ---");
    let movie_files: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT m.id, m.title, mf.file_path FROM movies m JOIN movie_files mf ON m.id = mf.movie_id"
    )
    .fetch_all(&pool)
    .await?;
    
    for mf in &movie_files {
        println!("Movie ID: {}, Title: {}, File Path: {}", mf.0, mf.1, mf.2);
    }
    
    println!("--- TV SHOWS & EPISODES ---");
    let episodes: Vec<(i64, String, i64, String)> = sqlx::query_as(
        "SELECT s.id, s.title, e.episode_number, e.file_path FROM tv_shows s JOIN seasons se ON s.id = se.show_id JOIN episodes e ON se.id = e.season_id"
    )
    .fetch_all(&pool)
    .await?;
    
    for ep in &episodes {
        println!("Show ID: {}, Title: {}, Episode: {}, File Path: {}", ep.0, ep.1, ep.2, ep.3);
    }
    
    Ok(())
}
