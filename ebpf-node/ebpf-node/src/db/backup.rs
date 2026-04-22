use std::fs;
use std::time::SystemTime;

use tokio::time;
use tracing::{info, warn, debug};

use crate::config::cli::hostname_from_path;

/// Create a backup of the RocksDB data
pub fn create_backup(db_path: &str) -> anyhow::Result<()> {
    let backup_dir = format!("/var/lib/ebpf-blockchain/backups/{}", hostname_from_path(db_path));
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string());
    let backup_path = format!("{}/{}", backup_dir, timestamp);
    
    fs::create_dir_all(&backup_dir)?;
    
    // Use RocksDB snapshot for backup
    if let Ok(db) = rocksdb::DB::open_default(db_path) {
        let _snapshot = db.snapshot();
        info!("Created snapshot for backup");
        
        // Copy data files to backup location using simple file copy
        let data_dst = format!("{}/data", backup_path);
        if std::path::Path::new(db_path).exists() {
            fs::create_dir_all(&data_dst)?;
            // Copy key files for backup verification
            for entry in std::fs::read_dir(db_path)? {
                if let Ok(e) = entry {
                    let file_path = e.path();
                    if let Some(file_name) = file_path.file_name() {
                        let dst_path = format!("{}/{}", data_dst, file_name.to_string_lossy());
                        if file_path.is_file() {
                            if let Err(e) = fs::copy(&file_path, &dst_path) {
                                debug!("Failed to copy {:?}: {}", file_path, e);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Create backup marker file
    fs::write(format!("{}/backup_marker.txt", backup_path),
        format!("Backup created at {}\nPath: {}\n", timestamp, db_path))?;
    
    info!("Backup created at: {}", backup_path);
    Ok(())
}

/// Cleanup old backups (keep only last 5)
pub fn cleanup_backups(base_dir: &str) {
    let backup_dir = format!("{}/backups", base_dir);
    if !std::path::Path::new(&backup_dir).exists() {
        return;
    }
    
    let mut backups: Vec<_> = std::fs::read_dir(&backup_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();
    
    backups.sort_by(|a, b| a.cmp(b));
    
    while backups.len() > 5 {
        if let Some(oldest) = backups.pop() {
            warn!("Removing old backup: {:?}", oldest);
            let _ = std::fs::remove_dir_all(&oldest);
        }
    }
}

/// Schedule periodic backups (every hour)
pub fn schedule_backups(db_path: String) {
    let db_path_clone = db_path.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(std::time::Duration::from_secs(3600)); // Every hour
        loop {
            interval.tick().await;
            if let Err(e) = create_backup(&db_path_clone) {
                warn!("Backup failed: {}", e);
            }
        }
    });
}
