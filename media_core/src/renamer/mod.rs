// core/src/renamer/mod.rs
use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;
use anyhow::{Result, anyhow};
use crate::models::Movie;

pub struct Renamer {
    pub folder_template: String,
    pub file_template: String,
}

impl Renamer {
    pub fn new(folder_template: Option<String>, file_template: Option<String>) -> Self {
        Self {
            folder_template: folder_template.unwrap_or_else(|| "${title} (${year})".to_string()),
            file_template: file_template.unwrap_or_else(|| "${title} (${year}) [${resolution}]".to_string()),
        }
    }

    pub fn sanitize(&self, name: &str) -> String {
        let re = Regex::new(r#"[\\/*?:"<>|]"#).unwrap();
        re.replace_all(name, "").trim().to_string()
    }

    fn resolution_to_name(&self, res: Option<crate::models::Resolution>) -> String {
        res.map(|r| r.as_str().to_string()).unwrap_or_default()
    }

    pub fn generate_paths(&self, movie: &Movie, original_path: &Path, resolution: Option<crate::models::Resolution>, codec: Option<&str>) -> (String, String) {
        let title = self.sanitize(&movie.title);
        let year = movie.year.map(|y| y.to_string()).unwrap_or_default();
        
        let res_name = self.resolution_to_name(resolution);
        let cod = codec.unwrap_or("");

        let mut folder_name = self.folder_template
            .replace("${title}", &title)
            .replace("${year}", &year);
            
        let mut file_name = self.file_template
            .replace("${title}", &title)
            .replace("${year}", &year)
            .replace("${resolution}", &res_name)
            .replace("${codec}", cod);

        // Clean up
        folder_name = self.clean_placeholders(folder_name);
        file_name = self.clean_placeholders(file_name);

        let ext = original_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("mkv");
            
        (folder_name, format!("{}.{}", file_name, ext))
    }

    fn clean_placeholders(&self, mut name: String) -> String {
        let re_brackets = Regex::new(r"\[\s*\]|\(\s*\)").unwrap();
        name = re_brackets.replace_all(&name, "").to_string();
        let re_spaces = Regex::new(r"\s+").unwrap();
        re_spaces.replace_all(&name, " ").trim().to_string()
    }

    pub fn rename_movie(&self, movie: &Movie, current_path: &Path, library_root: &Path, resolution: Option<crate::models::Resolution>, codec: Option<&str>, script_path: Option<&str>) -> Result<PathBuf> {
        if !current_path.exists() {
            return Err(anyhow!("File not found: {:?}", current_path));
        }

        let (folder_name, file_name) = self.generate_paths(movie, current_path, resolution, codec);
        let dest_folder = library_root.join(folder_name);
        let dest_path = dest_folder.join(file_name);

        if current_path == dest_path {
            return Ok(dest_path);
        }

        fs::create_dir_all(&dest_folder)?;

        let old_dir = current_path.parent().ok_or_else(|| anyhow!("No parent dir"))?;
        let old_stem = current_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let new_stem = dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();

        // Move main file
        self.move_item(current_path, &dest_path)?;

        // Move companions
        if let Ok(entries) = fs::read_dir(old_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() || p == dest_path || p == current_path {
                    continue;
                }

                if let Some(file_name) = p.file_name().and_then(|s| s.to_str()) {
                    if file_name.starts_with(old_stem) {
                        let new_name = file_name.replace(old_stem, new_stem);
                        let _ = self.move_item(&p, &dest_folder.join(new_name));
                    }
                }
            }
        }

        // Trigger post-processing hook
        if let Some(script) = script_path {
            let mut context = std::collections::HashMap::new();
            context.insert("title".to_string(), movie.title.clone());
            context.insert("new_path".to_string(), dest_path.to_string_lossy().to_string());
            context.insert("type".to_string(), "movie".to_string());
            
            let script_clone = script.to_string();
            tokio::spawn(async move {
                crate::hooks::run_post_processing(&script_clone, "on_renamed", context).await;
            });
        }

        // Clean up old dir if empty or only contains metadata we'd delete anyway
        if old_dir != library_root {
            let _ = fs::remove_dir(old_dir);
        }

        Ok(dest_path)
    }

    fn move_item(&self, from: &Path, to: &Path) -> Result<()> {
        match fs::rename(from, to) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::Other || e.to_string().contains("cross-device") => {
                // Fallback for cross-device moves
                let options = fs_extra::file::CopyOptions::new();
                fs_extra::file::move_file(from, to, &options)
                    .map_err(|e| anyhow!("Failed cross-device move: {}", e))?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}
