use std::sync::Arc;

use libp2p::PeerId;
use rocksdb::DB;
use tracing::{warn, info};

use crate::config::node::{NONCE_KEY_PREFIX, PROCESSED_TX_PREFIX};

/// Replay protection using nonce tracking and timestamp validation
/// Stores nonces per sender in RocksDB to prevent replay attacks
#[derive(Clone)]
pub struct ReplayProtection {
    db: Arc<DB>,
}

impl ReplayProtection {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
    
    /// Validate nonce: must be incremental per sender, not previously used
    /// Returns the expected next nonce for the sender after validation
    pub fn validate_nonce(&self, sender: &str, nonce: u64) -> Result<u64, String> {
        let key = format!("{}:{}", NONCE_KEY_PREFIX, sender);
        
        match self.db.get(key.as_bytes()) {
            Ok(Some(last_nonce_bytes)) => {
                if let Ok(last_nonce) = String::from_utf8(last_nonce_bytes.to_vec()) {
                    let last_nonce: u64 = last_nonce.parse()
                        .map_err(|e| format!("Invalid stored nonce for {}: {}", sender, e))?;
                    
                    if nonce <= last_nonce {
                        // Nonce is not incremental - possible replay attack
                        warn!(
                            event = "nonce_replay_detected",
                            sender = %sender,
                            nonce = nonce,
                            last_nonce = last_nonce,
                            "Transaction rejected: nonce not incremental"
                        );
                        return Err(format!("Nonce {} <= last nonce {}", nonce, last_nonce));
                    }
                    
                    // Nonce is valid, return expected next nonce
                    Ok(nonce + 1)
                } else {
                    Err("Invalid UTF-8 in stored nonce".to_string())
                }
            }
            Ok(None) => {
                // First transaction from this sender - accept any initial nonce
                Ok(nonce + 1)
            }
            Err(e) => {
                Err(format!("DB error reading nonce for {}: {}", sender, e))
            }
        }
    }
    
    /// Update the last seen nonce for a sender
    pub fn update_nonce(&self, sender: &str, nonce: u64) -> anyhow::Result<()> {
        let key = format!("{}:{}", NONCE_KEY_PREFIX, sender);
        self.db.put(key.as_bytes(), nonce.to_string().as_bytes())?;
        Ok(())
    }
    
    /// Mark a transaction as processed (for deduplication)
    /// Uses transaction ID as key with timestamp as value
    pub fn mark_processed(&self, tx_id: &str, timestamp: u64) -> anyhow::Result<()> {
        let key = format!("{}{}", PROCESSED_TX_PREFIX, tx_id);
        self.db.put(key.as_bytes(), timestamp.to_string().as_bytes())?;
        Ok(())
    }
    
    /// Check if a transaction has already been processed
    pub fn is_processed(&self, tx_id: &str) -> bool {
        let key = format!("{}{}", PROCESSED_TX_PREFIX, tx_id);
        self.db.get(key.as_bytes()).ok().flatten().is_some()
    }
    
    /// Clean up old processed transactions (older than 24 hours)
    /// This prevents unbounded growth of the database
    pub fn cleanup_old_processed(&self, max_age_secs: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| 0);
        
        let cutoff = now.saturating_sub(max_age_secs);
        let mut to_delete = Vec::new();
        
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            if let Ok((key, value)) = item {
                if let (Ok(key_str), Ok(value_str)) =
                    (String::from_utf8(key.to_vec()), String::from_utf8(value.to_vec()))
                {
                    if key_str.starts_with(PROCESSED_TX_PREFIX) {
                        if let Ok(ts) = value_str.parse::<u64>() {
                            if ts < cutoff {
                                to_delete.push(key_str);
                            }
                        }
                    }
                }
            }
        }
        
        let removed_count = to_delete.len();
        for key in to_delete {
            let _ = self.db.delete(key.as_bytes());
        }
        
        if removed_count > 0 {
            info!(removed = removed_count, "Cleaned up old processed transactions");
        }
    }
}
