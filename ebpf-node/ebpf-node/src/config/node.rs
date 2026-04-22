use std::sync::Arc;

use axum::http::StatusCode;
use libp2p::{Multiaddr, PeerId};
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

use crate::security::{peer_store::PeerStore, replay::ReplayProtection, sybil::SybilProtection};

/// Maximum allowed nonce age in seconds (5 minutes)
pub const NONCE_MAX_AGE_SECS: u64 = 300;

/// Prefix for nonce tracking keys in RocksDB
pub const NONCE_KEY_PREFIX: &str = "nonce:";

/// Prefix for processed transaction IDs (for deduplication)
pub const PROCESSED_TX_PREFIX: &str = "processed_tx:";

// ============================================================================
// Core Types
// ============================================================================

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub id: String,
    pub data: String,
    /// Nonce is a monotonically increasing counter per sender to prevent replay attacks
    #[serde(default)]
    pub nonce: u64,
    /// Unix timestamp when the transaction was created (seconds since epoch)
    #[serde(default)]
    pub timestamp: u64,
}

impl Transaction {
    /// Create a new transaction with current timestamp
    pub fn new(id: String, data: String, nonce: u64) -> Self {
        Self {
            id,
            data,
            nonce,
            timestamp: Self::current_timestamp(),
        }
    }
    
    /// Get current Unix timestamp in seconds
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| 0)
    }
    
    /// Validate timestamp is within acceptable window
    pub fn is_timestamp_valid(&self) -> bool {
        let now = Self::current_timestamp();
        now.saturating_sub(self.timestamp) <= NONCE_MAX_AGE_SECS
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NetworkMessage {
    TxProposal(Transaction),
    Vote { tx_id: String, peer_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest;

impl SyncRequest {
    pub fn protocol() -> &'static str {
        "/ebpf-blockchain/sync/1.0.0"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub transactions: Vec<Transaction>,
}

// ============================================================================
// Node Configuration
// ============================================================================

/// Node configuration with ports from environment variables
#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub iface: String,
    pub network_p2p_port: u16,
    pub metrics_port: u16,
    pub rpc_port: u16,
    pub ws_port: u16,
}

/// Shared application state for all API handlers
pub struct NodeState {
    pub start_time: std::time::Instant,
    pub db: Arc<DB>,
    pub peer_store: PeerStore,
    pub replay_protection: ReplayProtection,
    pub sybil_protection: SybilProtection,
    pub tx_rpc: mpsc::Sender<Transaction>,
    pub tx_ws: broadcast::Sender<String>,
    pub config: NodeConfig,
    pub local_peer_id: String,
    pub blocks_proposed: u64,
    pub transactions_processed: u64,
    #[allow(dead_code)]
    pub hot_reload_manager: Arc<crate::ebpf::hot_reload::EbpfHotReloadManager>,
}

// --- Block Structure ---

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Block {
    pub height: u64,
    pub hash: String,
    pub parent_hash: String,
    pub proposer: String,
    pub timestamp: u64,
    pub transactions: Vec<String>,
    pub quorum_votes: u64,
    pub total_validators: u64,
}

impl Block {
    pub fn compute_hash(&self) -> String {
        let content = format!(
            "{}{}{}{}{}{}{}",
            self.height, self.parent_hash, self.proposer, self.timestamp,
            self.transactions.join(","), self.quorum_votes, self.total_validators
        );
        // Simple hash for POC (in production use sha256)
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&content, &mut hasher);
        format!("0x{:016x}", std::hash::Hasher::finish(&mut hasher))
    }
}

// ============================================================================
// API Response Types
// ============================================================================

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NodeInfoResponse {
    pub node_id: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub peers_connected: usize,
    pub blocks_proposed: u64,
    pub blocks_validated: u64,
    pub transactions_processed: u64,
    pub current_height: u64,
    pub is_validator: bool,
    pub stake: u64,
    pub reputation_score: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerListResponse {
    pub peers: Vec<PeerDetail>,
    pub total: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerDetail {
    pub peer_id: String,
    pub address: String,
    pub transport: String,
    pub latency_ms: f64,
    pub reputation: f64,
    pub is_validator: bool,
    pub connected_since: String,
    pub messages_sent: u64,
    pub messages_received: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkConfigResponse {
    pub p2p_port: u16,
    pub max_connections: usize,
    pub bootstrap_peers: Vec<String>,
    pub mdns_enabled: bool,
    pub gossipsub_params: GossipsubParams,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GossipsubParams {
    pub mesh_size: usize,
    pub random_mesh_size: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransactionCreateResponse {
    pub hash: String,
    pub status: String,
    pub block_number: Option<u64>,
    pub timestamp: String,
    pub nonce: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransactionGetResponse {
    pub id: String,
    pub hash: String,
    pub data: String,
    pub nonce: u64,
    pub status: String,
    pub block_number: Option<u64>,
    pub confirmations: u64,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BlockListResponse {
    pub blocks: Vec<BlockSummary>,
    pub total: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BlockSummary {
    pub height: u64,
    pub hash: String,
    pub proposer: String,
    pub timestamp: u64,
    pub transactions: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SecurityListResponse {
    pub entries: Vec<SecurityEntry>,
    pub total: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SecurityEntry {
    pub ip: String,
    pub peer_id: Option<String>,
    pub reason: String,
    pub added_at: u64,
    pub duration_hours: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SecurityActionResponse {
    pub success: bool,
    pub ip: String,
    pub action: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
    pub version: String,
    pub checks: HealthChecks,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthChecks {
    pub service: String,
    pub database: String,
    pub network: String,
    pub consensus: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub code: String,
    pub timestamp: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

pub fn get_port_from_env(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_else(|_| 0)
}

pub fn get_current_timestamp_iso() -> String {
    format_iso_timestamp(get_current_timestamp())
}

pub fn format_iso_timestamp(secs: u64) -> String {
    // Simplified ISO format (in production use chrono)
    format!("1970-01-01T00:00:00Z+{}", secs)
}

pub fn error_response(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, axum::Json<ErrorResponse>) {
    let resp = ErrorResponse {
        error: error.to_string(),
        message: message.to_string(),
        code: code.to_string(),
        timestamp: get_current_timestamp_iso(),
    };
    (status, axum::Json(resp))
}

pub fn tx_create_error(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, axum::Json<TransactionCreateResponse>) {
    let resp = TransactionCreateResponse {
        hash: String::new(),
        status: error.to_string(),
        block_number: None,
        timestamp: get_current_timestamp_iso(),
        nonce: 0,
    };
    (status, axum::Json(resp))
}

pub fn tx_get_error(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, axum::Json<TransactionGetResponse>) {
    let resp = TransactionGetResponse {
        id: String::new(),
        hash: String::new(),
        data: message.to_string(),
        nonce: 0,
        status: error.to_string(),
        block_number: None,
        confirmations: 0,
        timestamp: get_current_timestamp(),
    };
    (status, axum::Json(resp))
}

pub fn block_error(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, axum::Json<serde_json::Value>) {
    (status, axum::Json(serde_json::json!({
        "error": error,
        "message": message,
        "code": code,
    })))
}

pub fn security_action_error(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, axum::Json<SecurityActionResponse>) {
    let resp = SecurityActionResponse {
        success: false,
        ip: String::new(),
        action: error.to_string(),
    };
    (status, axum::Json(resp))
}
