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
    Regex::new(r"(?i)^(?P<title>.+?)[.\s_-]+[Ss](?P<season>\d{1,2})[Ee](?P<episode>\d{1,2})").unwrap()
});

/// Matches a bare SxxExx at the very start of a stem (e.g. "S01E01 - Episode Title")
static BARE_SXXEXX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^[Ss](?P<season>\d{1,2})[Ee](?P<episode>\d{1,2})").unwrap()
});

static RES_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?P<res>2160p|1080p|720p|480p|576p|iMax)").unwrap()
});

/// Noise tokens that indicate a folder name is a raw release group name, not a clean show title.
static RELEASE_NOISE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(2160p|1080p|720p|480p|576p|x264|x265|h264|h265|10bit|bluray|web[- ]?dl|webrip|hdtv|brrip|hdrip|proper|repack|hevc|avc|aac|ddp|nf|amzn|complete|galaxy|tv|mkvcage|eztv|yts|rarbg|1337x|tgx|psarips|kontrast|minx|memento)\b"
    ).unwrap()
});

pub fn parse_filename(name: &str) -> ParsedMedia {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);

    let resolution = RES_RE.captures(stem)
        .and_then(|c| c.name("res"))
        .and_then(|m| m.as_str().parse().ok());

    // Check for bare SxxExx at the very start — title must come from directory
    if let Some(caps) = BARE_SXXEXX_RE.captures(stem) {
        return ParsedMedia {
            title: String::new(), // signal to caller: look at directory
            year: None,
            season: caps.name("season").and_then(|s| s.as_str().parse().ok()),
            episode: caps.name("episode").and_then(|e| e.as_str().parse().ok()),
            resolution,
            is_tv: true,
        };
    }

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

/// Parse a file path with awareness of the directory structure.
///
/// When the filename starts with `SxxExx` (bare episode pattern) or the parsed title
/// looks like release noise, we walk up the directory tree from the file to find the
/// first folder that is NOT a "Season XX" folder and use that as the show title.
///
/// `library_root` is used to bound the upward walk.
pub fn parse_file_path(path: &Path, library_root: &Path, is_tv: bool) -> ParsedMedia {
    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let mut parsed = parse_filename(filename);

    if is_tv {
        // If we already got a clean title with no release noise, we're done.
        if !parsed.title.is_empty() && !RELEASE_NOISE_RE.is_match(&parsed.title) {
            return parsed;
        }

        // Walk up the directory tree looking for the show folder.
        let show_title = resolve_show_title_from_path(path, library_root, &mut parsed.season);

        if let Some(title) = show_title {
            parsed.title = title;
            parsed.is_tv = true;
        }
    } else {
        parsed.is_tv = false;

        let cleaned_title = clean_dir_name_as_show_title(&parsed.title);
        if !cleaned_title.is_empty() {
            parsed.title = cleaned_title;
        }

        // If we already have a clean title with a year, we're done.
        if !parsed.title.is_empty() && parsed.year.is_some() && !RELEASE_NOISE_RE.is_match(&parsed.title) {
            return parsed;
        }

        // Check the immediate parent folder name
        if let Some(parent) = path.parent() {
            if parent != library_root && parent.starts_with(library_root) {
                if let Some(dir_name) = parent.file_name().and_then(|s| s.to_str()) {
                    // Strip leading bracket prefix like "[TorrentCouch net] "
                    static RE_BRACKET_PREFIX: Lazy<Regex> = Lazy::new(|| {
                        Regex::new(r"(?i)^\s*\[[^\]]*\]\s*").unwrap()
                    });
                    let cleaned_dir = RE_BRACKET_PREFIX.replace(dir_name.trim(), "");

                    // Parse with MOVIE_RE
                    if let Some(caps) = MOVIE_RE.captures(&cleaned_dir) {
                        let title = clean_title(caps.name("title").unwrap().as_str());
                        let year = caps.name("year").and_then(|y| y.as_str().parse::<i32>().ok());

                        // Strip release noise from the parsed title
                        let final_title = clean_dir_name_as_show_title(&title);
                        if !final_title.is_empty() {
                            parsed.title = final_title;
                        } else {
                            parsed.title = title;
                        }

                        if year.is_some() {
                            parsed.year = year;
                        }
                    }
                }
            }
        }
    }

    parsed
}

/// Walk up `path`'s parent directories (bounded by `library_root`) and return the first
/// directory name that is not a season folder and does not look like release noise.
/// Also updates `season_hint` if a season folder is found along the way.
fn resolve_show_title_from_path(
    path: &Path,
    library_root: &Path,
    season_hint: &mut Option<i32>,
) -> Option<String> {
    static RE_SEASON_FOLDER: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)^(?:season|series|s)\s*(\d+)$").unwrap()
    });

    let mut current = path.parent();
    while let Some(dir) = current {
        // Don't walk above the library root
        if dir == library_root || !dir.starts_with(library_root) {
            break;
        }

        let dir_name = match dir.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => {
                current = dir.parent();
                continue;
            }
        };

        // If it looks like a "Season 01" folder, record the season number and keep walking
        if let Some(caps) = RE_SEASON_FOLDER.captures(dir_name) {
            if season_hint.is_none() {
                *season_hint = caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
            current = dir.parent();
            continue;
        }

        // Clean the directory name and use it as the show title
        let cleaned = clean_dir_name_as_show_title(dir_name);
        if !cleaned.is_empty() {
            return Some(cleaned);
        }

        current = dir.parent();
    }

    None
}

/// Convert a raw directory name (e.g. `Better.Call.Saul.S04.1080p.BluRay.x265-KONTRAST`)
/// into a clean show title (`Better Call Saul`).
///
/// Also handles:
/// - Bracket site prefixes: `[TorrentCouch net] Game of Thrones` → `Game of Thrones`
/// - Standalone years: `Game of Thrones 2012` → `Game of Thrones`
pub fn clean_dir_name_as_show_title(raw: &str) -> String {
    // Strip leading bracket prefix like "[TorrentCouch net] " or "[www.site.com]"
    static RE_BRACKET_PREFIX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)^\s*\[[^\]]*\]\s*").unwrap()
    });
    let raw = RE_BRACKET_PREFIX.replace(raw.trim(), "");

    // Replace dots/underscores with spaces (hyphens only between words, not at end)
    let spaced = raw.replace('.', " ").replace('_', " ");

    // Strip everything from the first Sxx/SxxExx marker onwards
    static RE_SEASON_STRIP: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\s*\b[Ss]\d{1,2}(?:[Ee]\d{1,2})?\b.*").unwrap()
    });
    let stripped = RE_SEASON_STRIP.replace(&spaced, "");

    // Strip quality / release noise tokens (everything from first noise token onwards)
    static RE_QUALITY_STRIP: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?i)\s+\b(2160p|1080p|720p|480p|576p|x264|x265|h264|h265|10bit|hdtv|web[- ]?dl|webdl|bluray|brrip|hdrip|webrip|hevc|avc|complete|galaxy|tv|mkvcage|eztv|yts|rarbg|1337x|tgx|psarips|kontrast|minx|memento)\b.*"
        ).unwrap()
    });
    let stripped = RE_QUALITY_STRIP.replace(&stripped, "");

    // Strip trailing standalone year (e.g. "Game Of Thrones 2012" → "Game Of Thrones")
    static RE_YEAR_SUFFIX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\s+\b(?:19|20)\d{2}\b\s*$").unwrap()
    });
    let stripped = RE_YEAR_SUFFIX.replace(stripped.trim_end(), "");

    // Collapse whitespace and trim
    static RE_SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());
    let result = RE_SPACES.replace_all(stripped.trim(), " ").trim().to_string();

    // If the result is empty or still all release tokens, return empty to signal "keep walking"
    if result.is_empty() || RELEASE_NOISE_RE.is_match(&result) {
        return String::new();
    }

    result
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

    #[test]
    fn test_parse_bare_sxxexx() {
        let p = parse_filename("S01E01 - Winter Is Coming.mkv");
        assert_eq!(p.title, "");
        assert_eq!(p.season, Some(1));
        assert_eq!(p.episode, Some(1));
        assert!(p.is_tv);
    }

    #[test]
    fn test_clean_dir_name_as_show_title() {
        assert_eq!(
            clean_dir_name_as_show_title("Better.Call.Saul.S04.1080p.BluRay.x265-KONTRAST"),
            "Better Call Saul"
        );
        assert_eq!(
            clean_dir_name_as_show_title("Band.Of.Brothers.S01.720p.BRRip.MkvCage"),
            "Band Of Brothers"
        );
        assert_eq!(clean_dir_name_as_show_title("Better Call Saul"), "Better Call Saul");
        assert_eq!(
            clean_dir_name_as_show_title("Better.Call.Saul.S05.COMPLETE.720p"),
            "Better Call Saul"
        );
        // Year stripping
        assert_eq!(
            clean_dir_name_as_show_title("Game Of Thrones 2012"),
            "Game Of Thrones"
        );
        assert_eq!(
            clean_dir_name_as_show_title("Game.Of.Thrones.2012"),
            "Game Of Thrones"
        );
        // Bracket site prefix stripping
        assert_eq!(
            clean_dir_name_as_show_title("[TorrentCouch net] Game of Thrones"),
            "Game of Thrones"
        );
        assert_eq!(
            clean_dir_name_as_show_title("[www.1337x.to] Better.Call.Saul.S04.1080p"),
            "Better Call Saul"
        );
    }

    #[test]
    fn test_parse_file_path_bare_sxxexx() {
        let lib = Path::new("/media/tv");
        let file = Path::new("/media/tv/Game of Thrones/Season 01/S01E01 - Winter Is Coming.mkv");
        let p = parse_file_path(file, lib, true);
        assert_eq!(p.title, "Game of Thrones");
        assert_eq!(p.season, Some(1));
        assert_eq!(p.episode, Some(1));
        assert!(p.is_tv);
    }

    #[test]
    fn test_parse_file_path_release_folder() {
        let lib = Path::new("/media/tv");
        let file = Path::new(
            "/media/tv/Better.Call.Saul.S04.1080p.BluRay.x265-KONTRAST/S04E01.mkv",
        );
        let p = parse_file_path(file, lib, true);
        assert_eq!(p.title, "Better Call Saul");
        assert_eq!(p.season, Some(4));
        assert_eq!(p.episode, Some(1));
        assert!(p.is_tv);
    }

    #[test]
    fn test_parse_file_path_torrent_prefix() {
        let lib = Path::new("/media/tv");
        let file = Path::new(
            "/media/tv/[TorrentCouch net] Game of Thrones/Season 08/S08E01.mkv",
        );
        let p = parse_file_path(file, lib, true);
        assert_eq!(p.title, "Game of Thrones");
        assert_eq!(p.season, Some(8));
        assert_eq!(p.episode, Some(1));
    }

    #[test]
    fn test_parse_file_path_year_in_folder() {
        let lib = Path::new("/media/tv");
        let file = Path::new("/media/tv/Game Of Thrones 2012/Season 02/S02E01.mkv");
        let p = parse_file_path(file, lib, true);
        // Year should be stripped so title normalizes cleanly
        assert_eq!(p.title, "Game Of Thrones");
        assert_eq!(p.season, Some(2));
        assert_eq!(p.episode, Some(1));
    }

    #[test]
    fn test_parse_file_path_movie() {
        let lib = Path::new("/media/movies");
        let file = Path::new("/media/movies/Inception.2010.1080p.BluRay/inception.mkv");
        let p = parse_file_path(file, lib, false);
        assert_eq!(p.title, "Inception");
        assert_eq!(p.year, Some(2010));
        assert!(!p.is_tv);
    }
}
