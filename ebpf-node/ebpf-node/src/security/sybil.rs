use std::net::Ipv4Addr;
use std::sync::Arc;

use libp2p::PeerId;
use rocksdb::DB;
use tracing::{warn, info};

use crate::metrics::prometheus::SYBIL_ATTEMPTS_DETECTED;

/// Sybil protection: limits connections per IP and validates peer identity
/// Prevents attackers from creating multiple fake identities
#[derive(Clone)]
pub struct SybilProtection {
    db: Arc<DB>,
    /// Maximum number of connections allowed per IP address
    pub max_connections_per_ip: u32,
}

#[derive(Debug)]
pub enum SybilError {
    IpLimitExceeded { count: u32, max: u32 },
}

impl std::fmt::Display for SybilError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SybilError::IpLimitExceeded { count, max } => {
                write!(f, "Sybil attack detected: {} connections from IP, max allowed is {}", count, max)
            }
        }
    }
}

impl std::error::Error for SybilError {}

impl SybilProtection {
    pub fn new(db: Arc<DB>, max_per_ip: u32) -> Self {
        Self { db, max_connections_per_ip: max_per_ip }
    }
    
    /// Count how many connections are currently active from a given IP
    /// Returns the count of peer_ids associated with this IP
    pub fn count_connections_per_ip(&self, ip: &Ipv4Addr) -> u32 {
        let ip_str = ip.to_string();
        let prefix = format!("ip_conn:{}", ip_str);
        
        let mut count = 0u32;
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        
        for item in iter {
            if let Ok((key, _)) = item {
                if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                    if key_str.starts_with(&prefix) {
                        count += 1;
                    }
                }
            }
        }
        
        count
    }
    
    /// Register a connection from a peer_id to an IP address
    pub fn register_connection(&self, peer_id: PeerId, ip: &Ipv4Addr) -> anyhow::Result<()> {
        let key = format!("ip_conn:{}:{}", ip, peer_id);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| 0);
        self.db.put(key.as_bytes(), now.to_string().as_bytes())?;
        Ok(())
    }
    
    /// Unregister a connection when a peer disconnects
    pub fn unregister_connection(&self, peer_id: PeerId, ip: &Ipv4Addr) -> anyhow::Result<()> {
        let key = format!("ip_conn:{}:{}", ip, peer_id);
        self.db.delete(key.as_bytes())?;
        Ok(())
    }
    
    /// Check if a new connection from an IP would exceed the limit
    /// Returns Ok(()) if allowed, Err if Sybil attack detected
    pub fn check_ip_limit(&self, peer_id: PeerId, ip: &Ipv4Addr) -> Result<(), SybilError> {
        let count = self.count_connections_per_ip(ip);
        
        if count >= self.max_connections_per_ip {
            warn!(
                event = "sybil_limit_exceeded",
                peer_id = %peer_id,
                ip = %ip,
                current_connections = count,
                max_allowed = self.max_connections_per_ip,
                "Potential Sybil attack: too many connections from same IP"
            );
            SYBIL_ATTEMPTS_DETECTED.inc();
            Err(SybilError::IpLimitExceeded { count, max: self.max_connections_per_ip })
        } else {
            Ok(())
        }
    }
    
    /// Get the list of whitelisted peer IDs (trusted peers)
    pub fn get_whitelisted_peers(&self) -> Vec<PeerId> {
        let mut result = Vec::new();
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        
        for item in iter {
            if let Ok((key, value)) = item {
                if let (Ok(key_str), Ok(value_str)) =
                    (String::from_utf8(key.to_vec()), String::from_utf8(value.to_vec()))
                {
                    if key_str.starts_with("whitelist_peer:") {
                        if let Some(peer_id_str) = key_str.strip_prefix("whitelist_peer:") {
                            if let Ok(peer_id) = peer_id_str.parse() {
                                // Value should be "trusted" or "verified"
                                if value_str == "trusted" || value_str == "verified" {
                                    result.push(peer_id);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        result
    }
    
    /// Add a peer to the whitelist (via admin API or manual config)
    pub fn add_to_whitelist(&self, peer_id: PeerId) -> anyhow::Result<()> {
        let key = format!("whitelist_peer:{}", peer_id);
        self.db.put(key.as_bytes(), b"trusted")?;
        info!(peer_id = %peer_id, "Peer added to whitelist");
        Ok(())
    }
    
    /// Remove a peer from the whitelist
    pub fn remove_from_whitelist(&self, peer_id: PeerId) -> anyhow::Result<()> {
        let key = format!("whitelist_peer:{}", peer_id);
        self.db.delete(key.as_bytes())?;
        info!(peer_id = %peer_id, "Peer removed from whitelist");
        Ok(())
    }
    
    // =============================================================================
    // TAREA 4.7: Initialize whitelist with bootstrap peers
    // =============================================================================
    
    /// Initialize whitelist with bootstrap peers
    pub fn init_whitelist(&self, bootstrap_peers: Vec<String>) -> anyhow::Result<()> {
        use libp2p::identity::PeerId as Libp2pPeerId;
        
        for peer_str in bootstrap_peers {
            if let Ok(peer_id) = peer_str.parse::<Libp2pPeerId>() {
                self.add_to_whitelist(peer_id)?;
            } else {
                warn!(peer_str = %peer_str, "Invalid peer ID format, skipping whitelist addition");
            }
        }
        let count = self.get_whitelisted_peer_count();
        info!(whitelist_count = count, "Whitelist initialized with {} peers", count);
        Ok(())
    }
    
    /// Get the count of whitelisted peers
    pub fn get_whitelisted_peer_count(&self) -> usize {
        let mut count = 0;
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        
        for item in iter {
            if let Ok((key, value)) = item {
                if let (Ok(key_str), Ok(value_str)) =
                    (String::from_utf8(key.to_vec()), String::from_utf8(value.to_vec()))
                {
                    if key_str.starts_with("whitelist_peer:") {
                        if value_str == "trusted" || value_str == "verified" {
                            count += 1;
                        }
                    }
                }
            }
        }
        
        count
    }
}
