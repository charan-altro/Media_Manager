// core/src/cleanup/mod.rs
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;
use rayon::prelude::*;
use anyhow::Result;
use std::io::Read;
use std::fs;

pub struct CleanupService {
    pub root: PathBuf,
}

impl CleanupService {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn remove_duplicate_artwork(&self) -> Result<Vec<PathBuf>> {
        let mut removed = Vec::new();
        let mut size_groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();

        // 1. Group by size
        for entry in WalkDir::new(&self.root).into_iter().flatten() {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ["jpg", "jpeg", "png", "webp"].contains(&ext_lower.as_str()) {
                        if let Ok(meta) = entry.metadata() {
                            size_groups.entry(meta.len()).or_default().push(entry.path().to_path_buf());
                        }
                    }
                }
            }
        }

        // 2. Hash files with same size
        let potential_dupes: Vec<PathBuf> = size_groups.into_iter()
            .filter(|(_, paths)| paths.len() >= 2)
            .flat_map(|(_, paths)| paths)
            .collect();

        let path_to_hash: HashMap<PathBuf, String> = potential_dupes.par_iter()
            .filter_map(|p| {
                self.hash_file(p).ok().map(|h| (p.clone(), h))
            })
            .collect();

        // 3. Remove duplicates in the same directory
        let mut dir_groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        for (path, _) in &path_to_hash {
            if let Some(parent) = path.parent() {
                dir_groups.entry(parent.to_path_buf()).or_default().push(path.clone());
            }
        }

        for (_, paths) in dir_groups {
            let mut hash_to_paths: HashMap<String, Vec<PathBuf>> = HashMap::new();
            for p in paths {
                if let Some(h) = path_to_hash.get(&p) {
                    hash_to_paths.entry(h.clone()).or_default().push(p);
                }
            }

            for (_, hashed_paths) in hash_to_paths {
                if hashed_paths.len() >= 2 {
                    let mut sorted = hashed_paths.clone();
                    sorted.sort_by_key(|p| p.file_name().unwrap().len());
                    
                    for dupe in sorted.into_iter().skip(1) {
                        if let Ok(_) = fs::remove_file(&dupe) {
                            removed.push(dupe);
                        }
                    }
                }
            }
        }

        Ok(removed)
    }

    pub fn cleanup_metadata_for_movie(&self, dir: &Path, standard_stem: &str) -> Result<Vec<PathBuf>> {
        let mut removed = Vec::new();
        if !dir.exists() { return Ok(removed); }

        let standard_nfo = format!("{}.nfo", standard_stem);
        let standard_poster = format!("{}-poster.jpg", standard_stem);
        let standard_fanart = format!("{}-fanart.jpg", standard_stem);

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() { continue; }

                let file_name = p.file_name().and_then(|s| s.to_str()).unwrap_or_default();
                if file_name == standard_nfo || file_name == standard_poster || file_name == standard_fanart {
                    continue;
                }

                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
                
                // 1. Remove redundant NFOs
                if ext == "nfo" {
                    if let Ok(_) = fs::remove_file(&p) {
                        removed.push(p);
                    }
                    continue;
                }

                // 2. Remove redundant Posters/Backdrops
                let name_lower = file_name.to_lowercase();
                if ["jpg", "jpeg", "png"].contains(&ext.as_str()) {
                    let is_garbage_name = name_lower == "poster.jpg" || 
                                         name_lower == "backdrop.jpg" || 
                                         name_lower == "fanart.jpg" ||
                                         name_lower.contains("_poster.jpg") ||
                                         name_lower.contains("-fanart1.jpg") ||
                                         name_lower.contains("-fanart2.jpg");
                                         
                    if is_garbage_name {
                        if let Ok(_) = fs::remove_file(&p) {
                            removed.push(p);
                        }
                    }
                }
            }
        }

        Ok(removed)
    }

    fn hash_file(&self, path: &Path) -> Result<String> {
        let mut file = fs::File::open(path)?;
        let mut buffer = Vec::new();
        // Read only first 64KB for speed (matching Python logic)
        let mut chunk = vec![0u8; 65536];
        let n = file.read(&mut chunk)?;
        buffer.extend_from_slice(&chunk[..n]);
        
        let digest = md5::compute(buffer);
        Ok(format!("{:x}", digest))
    }

    pub fn remove_empty_folders(&self) -> Result<Vec<PathBuf>> {
        let mut removed = Vec::new();
        let metadata_exts: HashSet<&str> = ["jpg", "jpeg", "png", "webp", "nfo", "txt", "xml", "json"].into_iter().collect();

        let mut dirs: Vec<PathBuf> = WalkDir::new(&self.root)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_dir())
            .map(|e| e.path().to_path_buf())
            .collect();
        
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));

        for dir in dirs {
            if dir == self.root { continue; }
            
            if self.is_safe_to_delete(&dir, &metadata_exts) {
                if let Ok(_) = fs::remove_dir_all(&dir) {
                    removed.push(dir);
                }
            }
        }

        Ok(removed)
    }

    fn is_safe_to_delete(&self, dir: &Path, metadata_exts: &HashSet<&str>) -> bool {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() { return false; }
                
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    if !metadata_exts.contains(ext.to_lowercase().as_str()) {
                        return false;
                    }
                } else {
                    return false;
                }
                
                if let Ok(meta) = p.metadata() {
                    if meta.len() > 20 * 1024 * 1024 { return false; }
                }
            }
            true
        } else {
            false
        }
    }
}
