// core/src/scraper/kodi.rs
// Kodi XML Scraper - discovers local Kodi installations and lists their metadata scrapers.
// Note: Only XML-based scrapers are supported; Python scrapers are NOT compatible.
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct KodiScraperXml {
    pub name: String,
    pub id: String,
    pub content: String, // "movies", "tvshows", "musicvideos", etc.
    pub version: Option<String>,
    pub provider_name: Option<String>,
}

pub struct KodiScraper {
    pub scrapers_path: PathBuf,
}

impl KodiScraper {
    /// Search all known Kodi installation paths across Windows, macOS, and Linux
    pub fn find_local_instances() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        
        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").unwrap_or_default();
            let kodi_appdata = PathBuf::from(&appdata).join("Kodi/addons");
            if kodi_appdata.exists() { paths.push(kodi_appdata); }

            let pf = std::env::var("ProgramFiles").unwrap_or_default();
            let kodi_pf = PathBuf::from(&pf).join("Kodi/addons");
            if kodi_pf.exists() { paths.push(kodi_pf); }

            let pf86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();
            let kodi_pf86 = PathBuf::from(&pf86).join("Kodi/addons");
            if kodi_pf86.exists() { paths.push(kodi_pf86); }

            // ProgramData
            let pd = std::env::var("ProgramData").unwrap_or_default();
            let kodi_pd = PathBuf::from(&pd).join("Kodi/addons");
            if kodi_pd.exists() { paths.push(kodi_pd); }

            // Home folder variations
            let home = std::env::var("USERPROFILE").unwrap_or_default();
            for folder in &["Kodi", ".kodi", "kodi", "XBMC", ".xbmc", "xbmc"] {
                let kodi_home = PathBuf::from(&home).join(folder).join("addons");
                if kodi_home.exists() { paths.push(kodi_home); }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS standard Kodi paths
            let app_resources = PathBuf::from("/Applications/Kodi.app/Contents/Resources/addons");
            if app_resources.exists() { paths.push(app_resources); }

            let xbmc_resources = PathBuf::from("/Applications/XBMC.app/Contents/Resources/addons");
            if xbmc_resources.exists() { paths.push(xbmc_resources); }

            if let Some(home) = std::env::var("HOME").map(PathBuf::from).ok() {
                for folder in &["Kodi", ".kodi", "kodi", "XBMC", ".xbmc", "xbmc"] {
                    let kodi_home = home.join(folder).join("addons");
                    if kodi_home.exists() { paths.push(kodi_home); }
                }
                let app_support = home.join("Library/Application Support/Kodi/addons");
                if app_support.exists() { paths.push(app_support); }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // System-wide installations
            for prefix in &["/usr/share", "/usr/lib"] {
                let kodi_sys = PathBuf::from(prefix).join("kodi/addons");
                if kodi_sys.exists() { paths.push(kodi_sys); }
            }

            if let Some(home) = std::env::var("HOME").map(PathBuf::from).ok() {
                for folder in &["Kodi", ".kodi", "kodi", "XBMC", ".xbmc", "xbmc"] {
                    let kodi_home = home.join(folder).join("addons");
                    if kodi_home.exists() { paths.push(kodi_home); }
                }
            }
        }

        // Also check for a local kodi_scraper folder in the app's working directory
        let local_scraper_dir = PathBuf::from("kodi_scraper");
        if local_scraper_dir.exists() { paths.push(local_scraper_dir); }

        paths
    }

    /// List all metadata scraper addons found in the given addon directory
    pub fn list_scrapers(addon_dir: &Path) -> Result<Vec<KodiScraperXml>> {
        let mut scrapers = Vec::new();
        if let Ok(entries) = std::fs::read_dir(addon_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with("metadata."))
                    .unwrap_or(false)
                {
                    let addon_xml = path.join("addon.xml");
                    if addon_xml.exists() {
                        if let Ok(content) = std::fs::read_to_string(&addon_xml) {
                            let scraper = Self::parse_addon_xml(&content, &path);
                            scrapers.push(scraper);
                        }
                    }
                }
            }
        }
        Ok(scrapers)
    }

    /// Parse a Kodi addon.xml to extract scraper metadata
    fn parse_addon_xml(content: &str, path: &Path) -> KodiScraperXml {
        let folder_name = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Extract id attribute from <addon> tag
        let id = Self::extract_attr(content, "addon", "id")
            .unwrap_or_else(|| folder_name.clone());

        // Extract name attribute from <addon> tag
        let name = Self::extract_attr(content, "addon", "name")
            .unwrap_or_else(|| folder_name.clone());

        // Extract version attribute
        let version = Self::extract_attr(content, "addon", "version");

        // Extract provider-name attribute
        let provider_name = Self::extract_attr(content, "addon", "provider-name");

        // Determine content type from extension point
        let content_type = if content.contains("xbmc.metadata.scraper.movies") {
            "movies".to_string()
        } else if content.contains("xbmc.metadata.scraper.tvshows") {
            "tvshows".to_string()
        } else if content.contains("xbmc.metadata.scraper.musicvideos") {
            "musicvideos".to_string()
        } else {
            "unknown".to_string()
        };

        KodiScraperXml {
            name,
            id,
            content: content_type,
            version,
            provider_name,
        }
    }

    /// Simple attribute extraction from XML content (avoids full XML parser dependency)
    fn extract_attr(content: &str, tag: &str, attr: &str) -> Option<String> {
        let tag_start = content.find(&format!("<{}", tag))?;
        let tag_end = content[tag_start..].find('>')? + tag_start;
        let tag_content = &content[tag_start..tag_end];

        let attr_pattern = format!("{}=\"", attr);
        let attr_start = tag_content.find(&attr_pattern)?;
        let value_start = attr_start + attr_pattern.len();
        let value_end = tag_content[value_start..].find('"')? + value_start;

        Some(tag_content[value_start..value_end].to_string())
    }

    /// Check if a scraper is Python-based (not supported)
    pub fn is_python_scraper(addon_dir: &Path) -> bool {
        // Python scrapers typically have .py files at the root
        if let Ok(entries) = std::fs::read_dir(addon_dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "py" {
                        return true;
                    }
                }
            }
        }
        false
    }
}
