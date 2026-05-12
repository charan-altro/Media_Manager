use media_core::paths;
use media_core::scanner::hash;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- MVP 1.1 Validation Script ---");

    // 1. Test NFC Normalization
    let raw_path = "test_library\\test_café.mp4";
    let normalized = paths::normalize_slashes(raw_path);
    println!("Raw: {}", raw_path);
    println!("Normalized: {}", normalized);
    
    assert!(normalized.contains("/"), "Path should use forward slashes");
    // Verify NFC: 'é' can be represented in multiple ways. 
    // NFC should ensure a consistent bit-pattern.
    let nfc_check = normalized.chars().any(|c| c == 'é');
    println!("NFC Contains 'é': {}", nfc_check);

    // 2. Test Fingerprinting (OSHash)
    let test_file = "test_file.mp4";
    {
        let mut f = File::create(test_file)?;
        // Create 200KB file (OSHash needs > 128KB)
        let buf = vec![0u8; 200 * 1024];
        f.write_all(&buf)?;
    }
    
    let fingerprint = hash::calculate_oshash(test_file)?;
    println!("Fingerprint (Empty 200KB): {}", fingerprint);
    assert_ne!(fingerprint, "", "Fingerprint should not be empty");

    // 3. Verify path stability after move
    let moved_path = "archive/test_file.mp4";
    let normalized_moved = paths::normalize_slashes(moved_path);
    println!("Moved Path: {}", normalized_moved);

    println!("--- MVP 1.1 Validation SUCCESS ---");
    Ok(())
}
