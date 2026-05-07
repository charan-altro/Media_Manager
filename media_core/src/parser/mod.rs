// core/src/parser/mod.rs
use regex::Regex;
use once_cell::sync::Lazy;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct ParsedMedia {
    pub title: String,
    pub year: Option<i32>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub resolution: Option<crate::models::Resolution>,
    pub is_tv: bool,
}

static MOVIE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?P<title>.+?)[. \(\[]*(?P<year>(?:19|20)\d{2})[. \)\]]*").unwrap()
});

static TV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?P<title>.+?)[.\s_-]+[Ss](?P<season>\d{2})[Ee](?P<episode>\d{2})").unwrap()
});

static RES_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?P<res>2160p|1080p|720p|480p|576p|iMax)").unwrap()
});

pub fn parse_filename(name: &str) -> ParsedMedia {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);

    let resolution = RES_RE.captures(stem)
        .and_then(|c| c.name("res"))
        .and_then(|m| m.as_str().parse().ok());

    if let Some(caps) = TV_RE.captures(stem) {
        return ParsedMedia {
            title: clean_title(caps.name("title").unwrap().as_str()),
            year: None,
            season: caps.name("season").and_then(|s| s.as_str().parse().ok()),
            episode: caps.name("episode").and_then(|e| e.as_str().parse().ok()),
            resolution,
            is_tv: true,
        };
    }
    
    if let Some(caps) = MOVIE_RE.captures(stem) {
        return ParsedMedia {
            title: clean_title(caps.name("title").unwrap().as_str()),
            year: caps.name("year").and_then(|y| y.as_str().parse().ok()),
            season: None,
            episode: None,
            resolution,
            is_tv: false,
        };
    }
    
    ParsedMedia {
        title: clean_title(stem),
        resolution,
        ..Default::default()
    }
}

fn clean_title(raw: &str) -> String {
    raw.replace('.', " ")
       .replace('_', " ")
       .trim()
       .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_movie() {
        let p = parse_filename("Inception.2010.1080p.mkv");
        assert_eq!(p.title, "Inception");
        assert_eq!(p.year, Some(2010));
        assert!(!p.is_tv);
    }

    #[test]
    fn test_parse_tv() {
        let p = parse_filename("The.Office.S01E01.720p.mkv");
        assert_eq!(p.title, "The Office");
        assert_eq!(p.season, Some(1));
        assert_eq!(p.episode, Some(1));
        assert!(p.is_tv);
    }
}
