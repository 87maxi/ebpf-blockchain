use std::sync::Arc;

use libp2p::{Multiaddr, PeerId};
use rocksdb::DB;
use tracing::debug;

/// Persistent peer store using RocksDB
#[derive(Clone)]
pub struct PeerStore {
    pub db: Arc<DB>,
}

impl PeerStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
    
    pub fn save_peer(&self, peer_id: PeerId, addr: &Multiaddr) -> anyhow::Result<()> {
        let key = format!("peer:{}", peer_id);
        let value = addr.to_string();
        self.db.put(key.as_bytes(), value.as_bytes())?;
        Ok(())
    }
    
    pub fn get_peer(&self, peer_id: PeerId) -> Option<Multiaddr> {
        let key = format!("peer:{}", peer_id);
        self.db.get(key.as_bytes())
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v.to_vec()).ok())
            .and_then(|s| s.parse().ok())
    }
    
    pub fn all_peers(&self) -> Vec<(PeerId, Multiaddr)> {
        let mut result = Vec::new();
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            if let Ok((key, value)) = item {
                if let (Ok(key_str), Ok(value_str)) =
                    (String::from_utf8(key.to_vec()), String::from_utf8(value.to_vec()))
                {
                    if key_str.starts_with("peer:") {
                        if let Some(peer_id_str) = key_str.strip_prefix("peer:") {
                            if let Ok(peer_id) = peer_id_str.parse() {
                                if let Ok(addr) = value_str.parse() {
                                    result.push((peer_id, addr));
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }
    
    pub fn remove_peer(&self, peer_id: PeerId) -> anyhow::Result<()> {
        let key = format!("peer:{}", peer_id);
        self.db.delete(key.as_bytes())?;
        Ok(())
    }
}
