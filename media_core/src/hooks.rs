// core/src/hooks.rs
use std::process::Command;
use std::collections::HashMap;
use tracing::{info, error};

pub async fn run_post_processing(script_path: &str, event: &str, context: HashMap<String, String>) {
    if script_path.is_empty() {
        return;
    }

    info!("Executing post-processing script: {} for event: {}", script_path, event);

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("powershell.exe");
        c.arg("-Command").arg(script_path);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg(script_path);
        c
    };

    cmd.env("MEDIA_EVENT", event);
    for (key, value) in context {
        cmd.env(format!("MEDIA_{}", key.to_uppercase()), value);
    }

    // Spawn the script in the background
    tokio::task::spawn_blocking(move || {
        match cmd.output() {
            Ok(output) => {
                if !output.status.success() {
                    error!("Post-processing script failed with status: {}", output.status);
                    error!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
                } else {
                    info!("Post-processing script completed successfully.");
                }
            }
            Err(e) => {
                error!("Failed to execute post-processing script: {}", e);
            }
        }
    });
}
