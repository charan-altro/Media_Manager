// media_core/src/paths.rs
use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use unicode_normalization::UnicodeNormalization;

/// Normalizes a path string to use forward slashes (/) as separators
/// and NFC normalization for cross-platform character consistency.
pub fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
        .nfc()
        .collect::<String>()
}

/// Converts an absolute path to a relative path based on a library root.
/// Returns the relative path as a normalized string with forward slashes.
pub fn make_relative(full_path: &Path, library_root: &Path) -> Result<String> {
    let relative = full_path.strip_prefix(library_root)
        .map_err(|_| anyhow!("Path {:?} is not within library root {:?}", full_path, library_root))?;
    
    Ok(normalize_slashes(&relative.to_string_lossy()))
}

/// Converts a relative path (from the DB) back to an absolute path for the current OS.
pub fn make_absolute(relative_path: &str, library_root: &Path) -> PathBuf {
    // Path::join handles both \ and / correctly on Windows, and / on Linux.
    library_root.join(relative_path)
}

/// Returns a "canonicalized" string for comparison, but without resolving symlinks
/// if they are not wanted. Just ensures consistent separators and no trailing slash.
pub fn canonicalize_string(path: &str) -> String {
    let mut normalized = normalize_slashes(path);
    if normalized.ends_with('/') && normalized.len() > 1 {
        normalized.pop();
    }
    normalized
}
