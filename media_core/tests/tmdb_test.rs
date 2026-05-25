use media_core::scraper::tmdb::TmdbClient;
use media_core::scraper::MediaScraper;

#[tokio::test]
async fn test_tmdb_tv_details() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let api_key = std::env::var("TMDB_API_KEY").expect("TMDB_API_KEY not set");
    println!("Using TMDB_API_KEY: {}", &api_key[..10]);
    let client = TmdbClient::new(api_key);
    match client.get_tv_details(4613).await {
        Ok(details) => {
            println!("Success! Show name: {}", details.name);
            assert_eq!(details.id, 4613);
        }
        Err(e) => {
            println!("Failed with error: {:?}", e);
            panic!("TMDB get_tv_details failed: {:?}", e);
        }
    }
    Ok(())
}
