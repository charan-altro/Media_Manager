// media_core/src/scanner/hash.rs
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use anyhow::Result;

/// Calculates a fingerprint for a video file.
/// For large files, it uses the fast OSHash (Size + First/Last 64KB).
/// For small files (<128KB), it uses a full MD5 hash to prevent collisions.
pub fn calculate_oshash<P: AsRef<Path>>(path: P) -> Result<String> {
    let mut file = File::open(&path)?;
    let len = file.metadata()?.len();
    
    if len < 65536 * 2 {
        // For small files, compute full MD5 hash
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let digest = md5::compute(&buffer);
        return Ok(format!("{:x}", digest));
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
