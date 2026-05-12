// media_core/src/scanner/hash.rs
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use anyhow::Result;

/// Calculates the OpenSubtitles hash for a video file.
/// This algorithm is extremely fast as it only reads the first and last 64KB of the file.
pub fn calculate_oshash<P: AsRef<Path>>(path: P) -> Result<String> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    
    if len < 65536 * 2 {
        return Ok(format!("{:016x}", len)); // Fallback for tiny files
    }

    let mut hash: u64 = len;

    let mut buffer = [0u8; 8];
    
    // First 64KB
    for _ in 0..8192 {
        file.read_exact(&mut buffer)?;
        hash = hash.wrapping_add(u64::from_le_bytes(buffer));
    }

    // Last 64KB
    file.seek(SeekFrom::Start(len - 65536))?;
    for _ in 0..8192 {
        file.read_exact(&mut buffer)?;
        hash = hash.wrapping_add(u64::from_le_bytes(buffer));
    }

    Ok(format!("{:016x}", hash))
}
