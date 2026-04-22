use std::fs;
use std::sync::Arc;

use rocksdb::{Options, DB};
use tracing::{info, warn, error, debug};

use crate::config::cli::hostname_from_path;

/// Get the persistent data directory for the node
pub fn get_data_dir() -> String {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();
    
    // Use a consistent, persistent path based on hostname
    format!("/var/lib/ebpf-blockchain/data/{}", hostname)
}

/// Create the data directory structure with proper permissions
pub fn setup_data_dir(path: &str) -> anyhow::Result<()> {
    // Create main directory if it doesn't exist
    let base_dir = "/var/lib/ebpf-blockchain";
    if !std::path::Path::new(base_dir).exists() {
        fs::create_dir_all(base_dir)?;
        info!("Created base data directory: {}", base_dir);
    }
    
    // Create node-specific directory
    fs::create_dir_all(path)?;
    info!("Data directory ready: {}", path);
    
    // Create symlinks for common paths (for compatibility)
    let root_links = [
        ("/root/ebpf-blockchain", base_dir),
        ("/root/ebpf-blockchain/data", path),
    ];
    
    for (link, target) in &root_links {
        let link_path = std::path::Path::new(link);
        if !link_path.exists() && !link_path.is_symlink() {
            if let Err(e) = std::os::unix::fs::symlink(target, link_path) {
                debug!("Symlink creation skipped for {}: {}", link, e);
            }
        }
    }
    
    Ok(())
}

/// Initialize the RocksDB database with error handling and recovery
pub fn init_db() -> anyhow::Result<Arc<DB>> {
    let db_path = get_data_dir();
    info!("Initializing RocksDB at {}", db_path);
    
    // Setup persistent data directory with symlinks for compatibility
    if let Err(e) = setup_data_dir(&db_path) {
        warn!("Failed to setup data directory, using fallback: {}", e);
        // Fallback to old path
        let hostname = std::fs::read_to_string("/etc/hostname")
            .unwrap_or_else(|_| "unknown".to_string())
            .trim()
            .to_string();
        let _ = std::fs::create_dir_all(&db_path);
    }
    
    // Create initial backup marker
    let backup_marker = format!("{}/.backup_marker", db_path);
    if !std::path::Path::new(&backup_marker).exists() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());
        fs::write(&backup_marker, format!("First run at {}\n", timestamp)).ok();
    }
    
    // Open RocksDB with proper options for persistence
    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    db_opts.set_max_open_files(1000);
    
    match DB::open(&db_opts, &db_path) {
        Ok(db) => {
            info!("RocksDB opened successfully at {}", db_path);
            Ok(Arc::new(db))
        }
        Err(e) => {
            error!("Failed to open RocksDB at {}: {}", db_path, e);
            error!("Attempting recovery...");
            // Try to recover from backup
            let backup_path = format!("/var/lib/ebpf-blockchain/backups/{}/latest",
                hostname_from_path(&db_path));
            if std::path::Path::new(&backup_path).exists() {
                warn!("Found backup at {}, attempting recovery", backup_path);
                // Copy from backup
                if let Ok(_output) = std::process::Command::new("rsync")
                    .args(&["-a", &format!("{}/data/", backup_path), &db_path])
                    .output()
                {
                    info!("Recovery from backup successful");
                    Ok(Arc::new(DB::open(&Options::default(), &db_path).unwrap()))
                } else {
                    error!("Recovery failed. Using empty database.");
                    Ok(Arc::new(DB::open_default(&db_path).unwrap()))
                }
            } else {
                error!("No backup found. Using empty database.");
                Ok(Arc::new(DB::open_default(&db_path).unwrap()))
            }
        }
    }
}
