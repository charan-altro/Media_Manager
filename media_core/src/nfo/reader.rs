// media_core/src/nfo/reader.rs
use serde::Deserialize;
use quick_xml::de::from_str;
use anyhow::Result;
use std::path::Path;
use regex::Regex;
use once_cell::sync::Lazy;

static TMDB_ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<tmdbid>(?P<id>\d+)</tmdbid>").unwrap());
static IMDB_ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<imdbid>(?P<id>tt\d+)</imdbid>").unwrap());
static UNIQUE_ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"<uniqueid type="(?P<type>[^"]+)"[^>]*>(?P<id>[^<]+)</uniqueid>"#).unwrap());

#[derive(Debug, Deserialize, Default)]
pub struct MovieNfo {
    #[serde(rename = "title", default)]
    pub title: Vec<String>,
    #[serde(rename = "year", default)]
    pub year: Vec<String>,
    #[serde(rename = "plot", default)]
    pub plot: Vec<String>,
    #[serde(rename = "tagline", default)]
    pub tagline: Vec<String>,
    #[serde(rename = "runtime", default)]
    pub runtime: Vec<i32>,
    #[serde(rename = "rating", default)]
    pub rating: Vec<f32>,
    #[serde(rename = "genre", default)]
    pub genre: Vec<String>,
    #[serde(rename = "language", default)]
    pub language: Vec<String>,
    #[serde(rename = "actor", default)]
    pub actor: Vec<Actor>,
    #[serde(skip)]
    pub tmdb_id: Option<String>,
    #[serde(skip)]
    pub imdb_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum MovieNfoRoot {
    Movie(MovieNfo),
    Videodb(MovieNfo),
}

#[derive(Debug, Deserialize, Default)]
pub struct TvShowNfo {
    #[serde(rename = "title", default)]
    pub title: Vec<String>,
    #[serde(rename = "plot", default)]
    pub plot: Vec<String>,
    #[serde(rename = "rating", default)]
    pub rating: Vec<f32>,
    #[serde(rename = "genre", default)]
    pub genre: Vec<String>,
    #[serde(rename = "language", default)]
    pub language: Vec<String>,
    #[serde(rename = "actor", default)]
    pub actor: Vec<Actor>,
    #[serde(skip)]
    pub tmdb_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TvShowNfoRoot {
    Tvshow(TvShowNfo),
}

#[derive(Debug, Deserialize)]
pub struct Actor {
    pub name: String,
    pub role: Option<String>,
    pub thumb: Option<String>,
}

pub fn read_nfo_movie(path: &Path) -> Result<MovieNfo> {
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() { return Ok(MovieNfo::default()); }
    
    // 1. Try standard XML parse with root wrappers
    let mut nfo = if let Ok(root) = from_str::<MovieNfoRoot>(&content) {
        match root {
            MovieNfoRoot::Movie(m) => m,
            MovieNfoRoot::Videodb(m) => m,
        }
    } else {
        from_str::<MovieNfo>(&content).unwrap_or_default()
    };

    // 2. Use Regex for IDs (most robust against duplicate field errors)
    if let Some(caps) = TMDB_ID_RE.captures(&content) {
        nfo.tmdb_id = Some(caps["id"].to_string());
    }
    if let Some(caps) = IMDB_ID_RE.captures(&content) {
        nfo.imdb_id = Some(caps["id"].to_string());
    }

    // Also look through <uniqueid> tags
    for caps in UNIQUE_ID_RE.captures_iter(&content) {
        let id_type = &caps["type"];
        let id_val = &caps["id"];
        if id_type == "tmdb" && nfo.tmdb_id.is_none() {
            nfo.tmdb_id = Some(id_val.to_string());
        } else if id_type == "imdb" && nfo.imdb_id.is_none() {
            nfo.imdb_id = Some(id_val.to_string());
        }
    }

    Ok(nfo)
}

pub fn read_nfo_tv(path: &Path) -> Result<TvShowNfo> {
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() { return Ok(TvShowNfo::default()); }
    
    let mut nfo = if let Ok(root) = from_str::<TvShowNfoRoot>(&content) {
        match root {
            TvShowNfoRoot::Tvshow(t) => t,
        }
    } else {
        from_str::<TvShowNfo>(&content).unwrap_or_default()
    };

    if let Some(caps) = TMDB_ID_RE.captures(&content) {
        nfo.tmdb_id = Some(caps["id"].to_string());
    }
    for caps in UNIQUE_ID_RE.captures_iter(&content) {
        if &caps["type"] == "tmdb" && nfo.tmdb_id.is_none() {
            nfo.tmdb_id = Some(caps["id"].to_string());
        }
    }

    Ok(nfo)
}

pub struct NfoMetadata {
    pub nfo: Option<MovieNfo>,
    pub tv_nfo: Option<TvShowNfo>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

pub fn detect_metadata(video_path: &Path) -> NfoMetadata {
    let target_dir = video_path.parent().unwrap_or(Path::new("."));
    let base_name = video_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    
    // 1. NFO Detection - Scan all .nfo files in the directory
    let mut best_movie_nfo: Option<MovieNfo> = None;
    let mut best_tv_nfo: Option<TvShowNfo> = None;

    if let Ok(entries) = std::fs::read_dir(target_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("nfo") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.contains("<movie") {
                        if let Ok(nfo) = read_nfo_movie(&path) {
                            if best_movie_nfo.is_none() || nfo.actor.len() > best_movie_nfo.as_ref().unwrap().actor.len() {
                                best_movie_nfo = Some(nfo);
                            }
                        }
                    } else if content.contains("<tvshow") {
                        if let Ok(nfo) = read_nfo_tv(&path) {
                            if best_tv_nfo.is_none() || nfo.actor.len() > best_tv_nfo.as_ref().unwrap().actor.len() {
                                best_tv_nfo = Some(nfo);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Local Image Detection
    let poster_candidates = ["poster.jpg", "poster.png", &format!("{}-poster.jpg", base_name), "folder.jpg"];
    let fanart_candidates = ["fanart.jpg", "backdrop.jpg", &format!("{}-fanart.jpg", base_name)];
    
    let mut poster_path = None;
    for cand in &poster_candidates {
        let full = target_dir.join(cand);
        if full.exists() {
            poster_path = Some(full.to_str().unwrap_or_default().to_string());
            break;
        }
    }

    let mut backdrop_path = None;
    for cand in &fanart_candidates {
        let full = target_dir.join(cand);
        if full.exists() {
            backdrop_path = Some(full.to_str().unwrap_or_default().to_string());
            break;
        }
    }

    NfoMetadata {
        nfo: best_movie_nfo,
        tv_nfo: best_tv_nfo,
        poster_path,
        backdrop_path,
    }
}

