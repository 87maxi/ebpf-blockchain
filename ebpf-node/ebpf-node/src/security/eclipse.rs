use std::collections::HashSet;
use std::sync::Arc;

use libp2p::PeerId;
use rocksdb::DB;
use tracing::warn;

use crate::metrics::prometheus::{ECLIPSE_RISK_SCORE, PEER_IP_DIVERSITY};

/// Eclipse attack detection
/// Monitors peer IP diversity to detect when a node might be isolated
/// from the main network by an attacker controlling most connections.
#[derive(Clone)]
pub struct EclipseProtection {
    db: Arc<DB>,
    /// Minimum number of unique /24 prefixes considered safe
    min_safe_prefixes: u32,
    /// Minimum number of connected peers considered safe
    min_safe_peers: u32,
}

impl EclipseProtection {
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            db,
            min_safe_prefixes: 2,
            min_safe_peers: 3,
        }
    }

    /// Register a connected peer with its IP address
    pub fn register_peer(&self, peer_id: PeerId, ip: &str) -> anyhow::Result<()> {
        let key = format!("eclipse_peer:{}", peer_id);
        self.db.put(key.as_bytes(), ip.as_bytes())?;
        Ok(())
    }

    /// Unregister a disconnected peer
    pub fn unregister_peer(&self, peer_id: PeerId) -> anyhow::Result<()> {
        let key = format!("eclipse_peer:{}", peer_id);
        self.db.delete(key.as_bytes())?;
        Ok(())
    }

    /// Extract /24 prefix from an IP address (e.g., "192.168.2.x" → "192.168.2")
    fn extract_prefix_24(ip: &str) -> Option<String> {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() >= 3 {
            Some(format!("{}.{}.{}", parts[0], parts[1], parts[2]))
        } else {
            None
        }
    }

    /// Calculate the eclipse risk score (0-100)
    /// 
    /// Score factors:
    /// - Low peer count increases risk
    /// - Low IP diversity (all peers from same /24) increases risk
    /// - Single prefix dominance increases risk
    /// 
    /// Returns: (risk_score, unique_prefixes, total_peers)
    pub fn calculate_risk_score(&self) -> (f64, u32, u32) {
        let mut prefixes = HashSet::new();
        let mut total_peers: u32 = 0;
        let mut prefix_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            if let Ok((key, value)) = item {
                if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                    if key_str.starts_with("eclipse_peer:") {
                        if let Ok(ip_str) = String::from_utf8(value.to_vec()) {
                            total_peers += 1;
                            if let Some(prefix) = Self::extract_prefix_24(&ip_str) {
                                prefixes.insert(prefix.clone());
                                *prefix_counts.entry(prefix).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }

        let unique_prefixes = prefixes.len() as u32;

        // Calculate risk score
        let mut risk_score: f64 = 0.0;

        // Factor 1: Low peer count (0-30 points)
        if total_peers < self.min_safe_peers {
            risk_score += 30.0 * (1.0 - (total_peers as f64 / self.min_safe_peers as f64));
        }

        // Factor 2: Low IP diversity (0-40 points)
        if unique_prefixes < self.min_safe_prefixes {
            risk_score += 40.0 * (1.0 - (unique_prefixes as f64 / self.min_safe_prefixes as f64));
        }

        // Factor 3: Single prefix dominance (0-30 points)
        if total_peers > 0 {
            let max_dominance = prefix_counts.values().cloned().reduce(|a, b| a.max(b)).unwrap_or(0);
            let dominance_ratio = max_dominance as f64 / total_peers as f64;
            if dominance_ratio > 0.8 {
                risk_score += 30.0 * ((dominance_ratio - 0.8) / 0.2);
            }
        }

        let final_score = risk_score.min(100.0).max(0.0);

        // Update metrics
        ECLIPSE_RISK_SCORE
            .with_label_values(&["default"])
            .set(final_score);
        PEER_IP_DIVERSITY
            .with_label_values(&["default"])
            .set(unique_prefixes as f64);

        // Warn if risk is high
        if final_score > 50.0 {
            warn!(
                event = "eclipse_risk_high",
                risk_score = final_score,
                unique_prefixes,
                total_peers,
                "High eclipse attack risk detected"
            );
        }

        (final_score, unique_prefixes, total_peers)
    }

    /// Check if eclipse attack is likely
    pub fn is_eclipse_likely(&self) -> bool {
        let (score, _, _) = self.calculate_risk_score();
        score > 50.0
    }
}