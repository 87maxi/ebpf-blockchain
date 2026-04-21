use std::{net::Ipv4Addr, sync::Arc, time::Duration};


use axum::{
    Router,
    extract::{State, ws::{WebSocket, WebSocketUpgrade, Message}, Json, Path},
    routing::{get, post, put},
    response::IntoResponse,
    http::StatusCode,
};
use std::collections::HashSet;
use std::time::SystemTime;
use aya::{
    maps::{HashMap, LpmTrie, lpm_trie::Key},
    programs::{KProbe, Xdp, XdpFlags},
};
use clap::Parser;
use lazy_static::lazy_static;
use libp2p::{
    Multiaddr,
    PeerId,
    futures::{StreamExt, SinkExt},
    gossipsub, identify, mdns, noise,
    request_response,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use prometheus::{
    Encoder, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, TextEncoder,
    register_int_counter, register_int_counter_vec, register_int_gauge,
    register_int_gauge_vec,
};
use std::fs;
use std::os::unix::fs::symlink;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use tokio::{signal, sync::{broadcast, mpsc}, time};
use tracing::{info, warn, error, debug};

lazy_static! {
    static ref LATENCY_BUCKETS: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_node_latency_buckets",
        "Current values of latency buckets",
        &["bucket"]
    )
    .unwrap();
    static ref MESSAGES_RECEIVED: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_messages_received_total",
        "Total number of gossiped messages received",
        &["type"]
    )
    .unwrap();
    static ref PEERS_CONNECTED: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_node_peers_connected",
        "Number of connected peers",
        &["status"]
    )
    .unwrap();
    static ref PACKETS_TRACE: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_gossip_packets_trace_total",
        "Detailed packet trace count by sender and type",
        &["source_peer", "protocol"]
    )
    .unwrap();
    static ref UPTIME: IntCounter = register_int_counter!(
        "ebpf_node_uptime",
        "Uptime of the node in seconds"
    )
    .unwrap();
    static ref TRANSACTIONS_CONFIRMED: IntCounter = register_int_counter!(
        "ebpf_node_transactions_confirmed_total",
        "Total number of transactions confirmed by consensus"
    )
    .unwrap();
    static ref TRANSACTIONS_REJECTED: IntCounter = register_int_counter!(
        "ebpf_node_transactions_rejected_total",
        "Total number of transactions rejected (e.g., replay attacks)"
    )
    .unwrap();
    static ref TRANSACTIONS_REPLAY_REJECTED: IntCounter = register_int_counter!(
        "ebpf_node_transactions_replay_rejected_total",
        "Total number of transactions rejected due to replay protection (duplicate nonce/timestamp)"
    )
    .unwrap();
    static ref SYBIL_ATTEMPTS_DETECTED: IntCounter = register_int_counter!(
        "ebpf_node_sybil_attempts_total",
        "Total number of potential Sybil attack attempts detected (multiple connections per IP)"
    )
    .unwrap();
    static ref DB_OPERATIONS: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_db_operations_total",
        "Total number of database operations",
        &["operation"]
    )
    .unwrap();
    static ref P2P_CONNECTIONS_TOTAL: IntCounter = register_int_counter!(
        "ebpf_node_p2p_connections_total",
        "Total number of P2P connections established"
    )
    .unwrap();
    static ref P2P_CONNECTIONS_CLOSED: IntCounter = register_int_counter!(
        "ebpf_node_p2p_connections_closed_total",
        "Total number of P2P connections closed"
    )
    .unwrap();
    static ref PEERS_IDENTIFIED: IntCounter = register_int_counter!(
        "ebpf_node_peers_identified_total",
        "Total number of peers identified via libp2p identify protocol"
    )
    .unwrap();
    static ref PEERS_SAVED: IntCounter = register_int_counter!(
        "ebpf_node_peers_saved_total",
        "Total number of peer addresses saved to peer store"
    )
    .unwrap();
    
    // Network metrics
    static ref MESSAGES_SENT: IntCounter = register_int_counter!(
        "ebpf_node_messages_sent_total",
        "Total number of messages sent via gossip"
    )
    .unwrap();
    static ref MESSAGES_SENT_BY_TYPE: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_messages_sent_by_type_total",
        "Total number of messages sent by type",
        &["type"]
    )
    .unwrap();
    static ref NETWORK_LATENCY: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_node_network_latency_ms",
        "Network latency in milliseconds by peer",
        &["peer_id"]
    )
    .unwrap();
    static ref BANDWIDTH_SENT: IntCounter = register_int_counter!(
        "ebpf_node_bandwidth_sent_bytes_total",
        "Total bytes sent over the network"
    )
    .unwrap();
    static ref BANDWIDTH_RECEIVED: IntCounter = register_int_counter!(
        "ebpf_node_bandwidth_received_bytes_total",
        "Total bytes received from the network"
    )
    .unwrap();
    
    // Consensus metrics
    static ref BLOCKS_PROPOSED: IntCounter = register_int_counter!(
        "ebpf_node_blocks_proposed_total",
        "Total number of blocks proposed"
    )
    .unwrap();
    static ref CONSENSUS_ROUNDS: IntCounter = register_int_counter!(
        "ebpf_node_consensus_rounds_total",
        "Total number of consensus rounds"
    )
    .unwrap();
    static ref CONSENSUS_DURATION: IntGauge = register_int_gauge!(
        "ebpf_node_consensus_duration_ms",
        "Current consensus round duration in milliseconds"
    )
    .unwrap();
    static ref VALIDATOR_COUNT: IntGauge = register_int_gauge!(
        "ebpf_node_validator_count",
        "Number of active validators"
    )
    .unwrap();
    static ref SLASHING_EVENTS: IntCounter = register_int_counter!(
        "ebpf_node_slashing_events_total",
        "Total number of slashing events"
    )
    .unwrap();
    
    // Transaction metrics
    static ref TRANSACTIONS_PROCESSED: IntCounter = register_int_counter!(
        "ebpf_node_transactions_processed_total",
        "Total number of transactions processed"
    )
    .unwrap();
    static ref TRANSACTIONS_BY_TYPE: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_transactions_by_type_total",
        "Total number of transactions by type",
        &["type"]
    )
    .unwrap();
    static ref TRANSACTION_QUEUE_SIZE: IntGauge = register_int_gauge!(
        "ebpf_node_transaction_queue_size",
        "Current size of the transaction queue"
    )
    .unwrap();
    static ref TRANSACTION_FAILURES: IntCounter = register_int_counter!(
        "ebpf_node_transactions_failures_total",
        "Total number of transaction processing failures"
    )
    .unwrap();
    
    // eBPF metrics
    static ref XDP_PACKETS_PROCESSED: IntGauge = register_int_gauge!(
        "ebpf_node_xdp_packets_processed_total",
        "Total number of packets processed by XDP"
    )
    .unwrap();
    static ref XDP_PACKETS_DROPPED: IntGauge = register_int_gauge!(
        "ebpf_node_xdp_packets_dropped_total",
        "Total number of packets dropped by XDP"
    )
    .unwrap();
    static ref XDP_BLACKLIST_SIZE: IntGauge = register_int_gauge!(
        "ebpf_node_xdp_blacklist_size",
        "Current size of the XDP blacklist"
    )
    .unwrap();
    static ref XDP_WHITELIST_SIZE: IntGauge = register_int_gauge!(
        "ebpf_node_xdp_whitelist_size",
        "Current size of the XDP whitelist"
    )
    .unwrap();
    static ref EBPF_ERRORS: IntCounter = register_int_counter!(
        "ebpf_node_errors_total",
        "Total number of eBPF errors"
    )
    .unwrap();
    
    // System metrics
    static ref MEMORY_USAGE_BYTES: IntGauge = register_int_gauge!(
        "ebpf_node_memory_usage_bytes",
        "Current memory usage in bytes"
    )
    .unwrap();
    static ref UPTIME_SECONDS: IntGauge = register_int_gauge!(
        "ebpf_node_uptime_seconds",
        "Uptime in seconds"
    )
    .unwrap();
    static ref THREAD_COUNT: IntGauge = register_int_gauge!(
        "ebpf_node_thread_count",
        "Current number of threads"
    )
    .unwrap();
}

/// Maximum allowed nonce age in seconds (5 minutes)
const NONCE_MAX_AGE_SECS: u64 = 300;

/// Prefix for nonce tracking keys in RocksDB
const NONCE_KEY_PREFIX: &str = "nonce:";

/// Prefix for processed transaction IDs (for deduplication)
const PROCESSED_TX_PREFIX: &str = "processed_tx:";

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
    fn protocol() -> &'static str {
        "/ebpf-blockchain/sync/1.0.0"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub transactions: Vec<Transaction>,
}

type AppState = (mpsc::Sender<Transaction>, broadcast::Sender<String>);

// ============================================================================
// P2 - Fase 1: NodeState y API Response Types (Estructuras compartidas)
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
}

// --- Block Structure (P1) ---

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

// --- API Response Types ---

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

// --- Helper Functions ---

fn get_port_from_env(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_else(|_| 0)
}

fn get_current_timestamp_iso() -> String {
    format_iso_timestamp(get_current_timestamp())
}

fn format_iso_timestamp(secs: u64) -> String {
    // Simplified ISO format (in production use chrono)
    format!("1970-01-01T00:00:00Z+{}", secs)
}

fn error_response(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, Json<ErrorResponse>) {
    let resp = ErrorResponse {
        error: error.to_string(),
        message: message.to_string(),
        code: code.to_string(),
        timestamp: get_current_timestamp_iso(),
    };
    (status, Json(resp))
}

fn tx_create_error(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, Json<TransactionCreateResponse>) {
    let resp = TransactionCreateResponse {
        hash: String::new(),
        status: error.to_string(),
        block_number: None,
        timestamp: get_current_timestamp_iso(),
        nonce: 0,
    };
    (status, Json(resp))
}

fn tx_get_error(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, Json<TransactionGetResponse>) {
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
    (status, Json(resp))
}

fn block_error(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({
        "error": error,
        "message": message,
        "code": code,
    })))
}

fn security_action_error(status: StatusCode, error: &str, message: &str, code: &str) -> (StatusCode, Json<SecurityActionResponse>) {
    let resp = SecurityActionResponse {
        success: false,
        ip: String::new(),
        action: error.to_string(),
    };
    (status, Json(resp))
}

// --- API Handlers ---

/// GET /health - Health check endpoint
async fn health_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    let db_status = if state.db.get(b"health_check").is_ok() {
        "ok".to_string()
    } else {
        "degraded".to_string()
    };
    
    let network_status = if state.local_peer_id.len() > 10 {
        "ok".to_string()
    } else {
        "ok".to_string()
    };
    
    let consensus_status = "ok".to_string();
    
    let status_str = if db_status == "ok" && network_status == "ok" {
        "healthy".to_string()
    } else {
        "unhealthy".to_string()
    };
    
    let response = HealthResponse {
        status: status_str.clone(),
        uptime_seconds: uptime,
        version: "1.0.0".to_string(),
        checks: HealthChecks {
            service: "ok".to_string(),
            database: db_status,
            network: network_status,
            consensus: consensus_status,
        },
    };
    
    if status_str == "unhealthy" {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    } else {
        (StatusCode::OK, Json(response))
    }
}

/// GET /api/v1/node/info - Node information
async fn node_info_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    
    // Get metrics values
    let peers_connected = PEERS_CONNECTED.with_label_values(&["connected"]).get() as usize;
    let blocks_proposed = BLOCKS_PROPOSED.get();
    let transactions_processed = TRANSACTIONS_PROCESSED.get();
    
    let response = NodeInfoResponse {
        node_id: state.local_peer_id.clone(),
        version: "1.0.0".to_string(),
        uptime_seconds: uptime,
        peers_connected,
        blocks_proposed,
        blocks_validated: blocks_proposed * 3, // Estimated for POC
        transactions_processed,
        current_height: blocks_proposed,
        is_validator: true, // All nodes are validators in POC
        stake: 0, // Placeholder - to be implemented with StakeManager
        reputation_score: 1.0, // Default for POC
    };
    
    (StatusCode::OK, Json(response))
}

/// GET /api/v1/network/peers - Connected peers list
async fn network_peers_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
    let mut peers = Vec::new();
    
    // Get peers from peer store
    let all_peers = state.peer_store.all_peers();
    for (peer_id, addr) in &all_peers {
        let transport = if addr.to_string().contains("quic") {
            "QUIC".to_string()
        } else {
            "TCP".to_string()
        };
        
        peers.push(PeerDetail {
            peer_id: peer_id.to_string(),
            address: addr.to_string(),
            transport,
            latency_ms: 0.0, // Not tracked in POC
            reputation: 1.0, // Default
            is_validator: true,
            connected_since: format_iso_timestamp(get_current_timestamp()),
            messages_sent: 0, // Not tracked per-peer in POC
            messages_received: 0,
        });
    }
    
    let response = PeerListResponse {
        peers,
        total: all_peers.len(),
    };
    
    (StatusCode::OK, Json(response))
}

/// GET /api/v1/network/config - Get network configuration
async fn network_config_get_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
    let response = NetworkConfigResponse {
        p2p_port: state.config.network_p2p_port,
        max_connections: 100, // Default for POC
        bootstrap_peers: vec![], // Would need to store configured peers
        mdns_enabled: true,
        gossipsub_params: GossipsubParams {
            mesh_size: 12,
            random_mesh_size: 4,
        },
    };
    
    (StatusCode::OK, Json(response))
}

/// PUT /api/v1/network/config - Update network configuration
async fn network_config_put_handler(
    State(state): State<Arc<NodeState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let max_connections = payload.get("max_connections").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    
    // Note: In production, this would update runtime config
    // For POC, we just acknowledge the request
    
    let response = serde_json::json!({
        "success": true,
        "config": {
            "max_connections": max_connections,
        }
    });
    
    (StatusCode::OK, Json(response))
}

/// POST /api/v1/transactions - Create transaction (replaces /rpc)
async fn transactions_create_handler(
    State(state): State<Arc<NodeState>>,
    Json(tx): Json<Transaction>,
) -> (StatusCode, Json<TransactionCreateResponse>) {
    // Validate required fields
    if tx.id.is_empty() || tx.data.is_empty() {
        return tx_create_error(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Transaction must have id and data fields",
            "MISSING_FIELDS",
        );
    }
    
    // Check if transaction already processed
    if state.replay_protection.is_processed(&tx.id) {
        return tx_create_error(
            StatusCode::CONFLICT,
            "Conflict",
            "Transaction already processed",
            "DUPLICATE_TX",
        );
    }
    
    // Validate nonce
    let sender = "api-submitter".to_string(); // API submissions use a placeholder sender
    match state.replay_protection.validate_nonce(&sender, tx.nonce) {
        Ok(next_nonce) => {
            // Valid transaction - record nonce and mark as processed
            if let Err(e) = state.replay_protection.update_nonce(&sender, next_nonce) {
                warn!(event = "nonce_update_failed", sender = %sender, error = %e, "Failed to update nonce");
            }
            if let Err(e) = state.replay_protection.mark_processed(&tx.id, tx.timestamp) {
                warn!(event = "process_mark_failed", tx_id = %tx.id, error = %e, "Failed to mark transaction as processed");
            }
        }
        Err(e) => {
            TRANSACTIONS_REPLAY_REJECTED.inc();
            return tx_create_error(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                &format!("Invalid nonce: {}", e),
                "INVALID_NONCE",
            );
        }
    }
    
    // Send to gossip via channel
    let _ = state.tx_rpc.send(tx.clone()).await;
    
    TRANSACTIONS_PROCESSED.inc();
    TRANSACTIONS_BY_TYPE.with_label_values(&["transfer"]).inc();
    
    let response = TransactionCreateResponse {
        hash: format!("0x{:?}", tx.id),
        status: "pending".to_string(),
        block_number: None,
        timestamp: format_iso_timestamp(tx.timestamp),
        nonce: tx.nonce,
    };
    
    (StatusCode::CREATED, Json(response))
}

/// GET /api/v1/transactions/{id} - Get transaction by ID
async fn transactions_get_handler(
    State(state): State<Arc<NodeState>>,
    Path(tx_id): Path<String>,
) -> (StatusCode, Json<TransactionGetResponse>) {
    // Look up in RocksDB
    match state.db.get(tx_id.as_bytes()) {
        Ok(Some(data)) => {
            if let Ok(data_str) = String::from_utf8(data.to_vec()) {
                let tx_hash = format!("0x{:?}", tx_id);
                // Check if it's a voter set (confirmed) or raw data
                if let Ok(voters) = serde_json::from_str::<HashSet<String>>(&data_str) {
                    // It's a confirmed transaction with voters
                    let response = TransactionGetResponse {
                        id: tx_id,
                        hash: tx_hash,
                        data: data_str.clone(),
                        nonce: 0, // Not stored in voter set
                        status: "confirmed".to_string(),
                        block_number: Some(BLOCKS_PROPOSED.get()),
                        confirmations: voters.len() as u64,
                        timestamp: get_current_timestamp(),
                    };
                    return (StatusCode::OK, Json(response));
                } else {
                    // Raw transaction data
                    let response = TransactionGetResponse {
                        id: tx_id,
                        hash: tx_hash,
                        data: data_str,
                        nonce: 0,
                        status: "pending".to_string(),
                        block_number: None,
                        confirmations: 0,
                        timestamp: get_current_timestamp(),
                    };
                    return (StatusCode::OK, Json(response));
                }
            } else {
                return tx_get_error(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                    &format!("Transaction {} not found", tx_id),
                    "TX_NOT_FOUND",
                );
            }
        }
        Ok(None) => {
            return tx_get_error(
                StatusCode::NOT_FOUND,
                "Not Found",
                &format!("Transaction {} not found", tx_id),
                "TX_NOT_FOUND",
            );
        }
        Err(e) => {
            return tx_get_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                &format!("Database error: {}", e),
                "DB_ERROR",
            );
        }
    }
}

/// GET /api/v1/blocks/latest - Latest block
async fn blocks_latest_handler(State(state): State<Arc<NodeState>>) -> (StatusCode, Json<serde_json::Value>) {
    let height = BLOCKS_PROPOSED.get();
    
    if height == 0 {
        return block_error(
            StatusCode::NOT_FOUND,
            "Not Found",
            "No blocks yet",
            "NO_BLOCKS",
        );
    }
    
    // Create a synthetic block from consensus data
    let block = Block {
        height,
        hash: format!("0x{:016x}", height * 0xdeadbeef),
        parent_hash: format!("0x{:016x}", (height - 1) * 0xdeadbeef),
        proposer: state.local_peer_id.clone(),
        timestamp: get_current_timestamp(),
        transactions: vec![], // Would need to track txs per block
        quorum_votes: 2, // Default quorum for POC
        total_validators: 3, // Default validators for POC
    };
    
    let response = serde_json::json!({
        "height": block.height,
        "hash": block.hash,
        "parent_hash": block.parent_hash,
        "proposer": block.proposer,
        "timestamp": format_iso_timestamp(block.timestamp),
        "transactions": block.transactions,
        "quorum_votes": block.quorum_votes,
        "total_validators": block.total_validators,
    });
    
    (StatusCode::OK, Json(response))
}

/// GET /api/v1/blocks/{height} - Block by height
async fn blocks_by_height_handler(
    State(state): State<Arc<NodeState>>,
    Path(height): Path<u64>,
) -> (StatusCode, Json<serde_json::Value>) {
    let current_height = BLOCKS_PROPOSED.get();
    
    if height > current_height {
        return block_error(
            StatusCode::NOT_FOUND,
            "Not Found",
            &format!("Block at height {} not found", height),
            "BLOCK_NOT_FOUND",
        );
    }
    
    if current_height == 0 {
        return block_error(
            StatusCode::NOT_FOUND,
            "Not Found",
            "No blocks yet",
            "NO_BLOCKS",
        );
    }
    
    // Create synthetic block
    let block = Block {
        height,
        hash: format!("0x{:016x}", height * 0xdeadbeef),
        parent_hash: if height > 1 { format!("0x{:016x}", (height - 1) * 0xdeadbeef) } else { "0x0000000000000000".to_string() },
        proposer: state.local_peer_id.clone(),
        timestamp: get_current_timestamp(),
        transactions: vec![],
        quorum_votes: 2,
        total_validators: 3,
    };
    
    let response = serde_json::json!({
        "height": block.height,
        "hash": block.hash,
        "parent_hash": block.parent_hash,
        "proposer": block.proposer,
        "timestamp": format_iso_timestamp(block.timestamp),
        "transactions": block.transactions,
        "quorum_votes": block.quorum_votes,
        "total_validators": block.total_validators,
    });
    
    (StatusCode::OK, Json(response))
}

/// GET /api/v1/security/blacklist - Get blacklist
async fn security_blacklist_get_handler(
    State(state): State<Arc<NodeState>>,
) -> (StatusCode, Json<SecurityListResponse>) {
    // Get blacklist from eBPF map (would need access to ebpf instance)
    // For now, return empty list as the eBPF program is managed separately
    let response = SecurityListResponse {
        entries: vec![],
        total: 0,
    };
    
    (StatusCode::OK, Json(response))
}

/// PUT /api/v1/security/blacklist - Modify blacklist
async fn security_blacklist_put_handler(
    State(state): State<Arc<NodeState>>,
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, Json<SecurityActionResponse>) {
    let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("add");
    let ip = payload.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let reason = payload.get("reason").and_then(|v| v.as_str()).unwrap_or("manual");
    
    if ip.is_empty() {
        return security_action_error(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "IP address is required",
            "MISSING_IP",
        );
    }
    
    // Parse duration
    let duration_hours = payload.get("duration_hours").and_then(|v| v.as_u64()).unwrap_or(24);
    
    // Note: In production, this would modify the eBPF XDP blacklist map
    // For POC, we acknowledge the request
    let action_str = if action == "remove" { "removed" } else { "added" };
    
    let response = SecurityActionResponse {
        success: true,
        ip: ip.to_string(),
        action: action_str.to_string(),
    };
    
    (StatusCode::OK, Json(response))
}

/// GET /api/v1/security/whitelist - Get whitelist
async fn security_whitelist_get_handler(
    State(state): State<Arc<NodeState>>,
) -> (StatusCode, Json<SecurityListResponse>) {
    let whitelist_peers = state.sybil_protection.get_whitelisted_peers();
    
    let mut entries = Vec::new();
    for peer_id in &whitelist_peers {
        entries.push(SecurityEntry {
            ip: "0.0.0.0".to_string(), // Would need to look up from connection tracking
            peer_id: Some(peer_id.to_string()),
            reason: "whitelisted".to_string(),
            added_at: get_current_timestamp(),
            duration_hours: 0,
        });
    }
    
    let response = SecurityListResponse {
        entries,
        total: whitelist_peers.len(),
    };
    
    (StatusCode::OK, Json(response))
}

async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    let metric_families = prometheus::gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

async fn rpc_handler(
    State(state): State<Arc<NodeState>>,
    Json(payload): Json<Transaction>,
) -> impl IntoResponse {
    let _ = state.tx_rpc.send(payload).await;
    axum::http::StatusCode::ACCEPTED
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<NodeState>>,
) -> impl IntoResponse {
    let rx = state.tx_ws.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}

fn initialize_metrics() {
    // Initialize all existing metrics
    for i in 0..64 {
        LATENCY_BUCKETS.with_label_values(&[&i.to_string()]).set(0);
    }
    PEERS_CONNECTED.with_label_values(&["connected"]).set(0);
    MESSAGES_RECEIVED.with_label_values(&["gossip"]).inc_by(0);
    UPTIME.inc_by(0);
    TRANSACTIONS_CONFIRMED.inc_by(0);
    TRANSACTIONS_REJECTED.inc_by(0);
    DB_OPERATIONS.with_label_values(&["put"]).inc_by(0);
    DB_OPERATIONS.with_label_values(&["get"]).inc_by(0);
    P2P_CONNECTIONS_TOTAL.inc_by(0);
    P2P_CONNECTIONS_CLOSED.inc_by(0);
    PEERS_IDENTIFIED.inc_by(0);
    PEERS_SAVED.inc_by(0);
    TRANSACTIONS_REPLAY_REJECTED.inc_by(0);
    SYBIL_ATTEMPTS_DETECTED.inc_by(0);
    
    // Initialize new network metrics
    MESSAGES_SENT.inc_by(0);
    MESSAGES_SENT_BY_TYPE.with_label_values(&["tx"]).inc_by(0);
    MESSAGES_SENT_BY_TYPE.with_label_values(&["vote"]).inc_by(0);
    MESSAGES_SENT_BY_TYPE.with_label_values(&["sync"]).inc_by(0);
    NETWORK_LATENCY.with_label_values(&["average"]).set(0);
    BANDWIDTH_SENT.inc_by(0);
    BANDWIDTH_RECEIVED.inc_by(0);
    
    // Initialize new consensus metrics
    BLOCKS_PROPOSED.inc_by(0);
    CONSENSUS_ROUNDS.inc_by(0);
    CONSENSUS_DURATION.set(0);
    VALIDATOR_COUNT.set(0);
    SLASHING_EVENTS.inc_by(0);
    
    // Initialize new transaction metrics
    TRANSACTIONS_PROCESSED.inc_by(0);
    TRANSACTIONS_BY_TYPE.with_label_values(&["transfer"]).inc_by(0);
    TRANSACTIONS_BY_TYPE.with_label_values(&["vote"]).inc_by(0);
    TRANSACTION_QUEUE_SIZE.set(0);
    TRANSACTION_FAILURES.inc_by(0);
    
    // Initialize new eBPF metrics
    XDP_PACKETS_PROCESSED.set(0);
    XDP_PACKETS_DROPPED.set(0);
    XDP_BLACKLIST_SIZE.set(0);
    XDP_WHITELIST_SIZE.set(0);
    EBPF_ERRORS.inc_by(0);
    
    // Initialize system metrics
    MEMORY_USAGE_BYTES.set(0);
    UPTIME_SECONDS.set(0);
    THREAD_COUNT.set(0);
    
    info!("All metrics initialized successfully");
}

/// Update system metrics (memory, threads, etc.)
fn update_system_metrics() {
    // Read memory usage from /proc/self/status
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(bytes_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = bytes_str.parse::<u64>() {
                        MEMORY_USAGE_BYTES.set((kb * 1024) as i64);
                    }
                }
                break;
            }
        }
    }
    
    // Update uptime
    UPTIME_SECONDS.set(UPTIME.get() as i64);
    
    // Thread count (approximate)
    THREAD_COUNT.set(0); // Will be updated by tokio runtime
}

/// Get the persistent data directory for the node
fn get_data_dir() -> String {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();
    
    // Use a consistent, persistent path based on hostname
    format!("/var/lib/ebpf-blockchain/data/{}", hostname)
}

/// Create the data directory structure with proper permissions
fn setup_data_dir(path: &str) -> anyhow::Result<()> {
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
            if let Err(e) = symlink(target, link_path) {
                debug!("Symlink creation skipped for {}: {}", link, e);
            }
        }
    }
    
    Ok(())
}

/// Create a backup of the RocksDB data
fn create_backup(db_path: &str) -> anyhow::Result<()> {
    let backup_dir = format!("/var/lib/ebpf-blockchain/backups/{}", hostname_from_path(db_path));
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string());
    let backup_path = format!("{}/{}", backup_dir, timestamp);
    
    fs::create_dir_all(&backup_dir)?;
    
    // Use RocksDB snapshot for backup
    if let Ok(db) = DB::open_default(db_path) {
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

fn hostname_from_path(path: &str) -> String {
    path.rsplit('/').next().unwrap_or("unknown").to_string()
}

/// Cleanup old backups (keep only last 5)
fn cleanup_backups(base_dir: &str) {
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

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long, default_value = "eth0")]
    iface: String,

    #[clap(short, long, value_delimiter = ',')]
    listen_addresses: Vec<Multiaddr>,

    #[clap(long, value_delimiter = ',')]
    bootstrap_peers: Vec<Multiaddr>,
    
    #[clap(long, default_value = "10")]
    connection_retries: u32,
    
    #[clap(long, default_value = "30")]
    retry_interval_secs: u64,
}

/// Load saved peers from file
fn load_saved_peers(path: &str) -> Vec<Multiaddr> {
    if let Ok(content) = std::fs::read_to_string(path) {
        content
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.trim().to_string())
            .filter_map(|addr| addr.parse().ok())
            .collect()
    } else {
        Vec::new()
    }
}

/// Save peers to file for persistence
fn save_peers(peers: &[(PeerId, Multiaddr)], path: &str) -> anyhow::Result<()> {
    let mut lines = Vec::new();
    for (peer_id, addr) in peers {
        lines.push(format!("{} {}", peer_id, addr));
    }
    std::fs::write(path, lines.join("\n"))?;
    Ok(())
}

/// Get bootstrap peers from environment or config
fn get_bootstrap_peers_from_env() -> Vec<Multiaddr> {
    std::env::var("BOOTSTRAP_PEERS")
        .ok()
        .map(|val| {
            val.split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Persistent peer store using RocksDB
#[derive(Clone)]
struct PeerStore {
    db: Arc<DB>,
}

impl PeerStore {
    fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
    
    fn save_peer(&self, peer_id: PeerId, addr: &Multiaddr) -> anyhow::Result<()> {
        let key = format!("peer:{}", peer_id);
        let value = addr.to_string();
        self.db.put(key.as_bytes(), value.as_bytes())?;
        Ok(())
    }
    
    fn get_peer(&self, peer_id: PeerId) -> Option<Multiaddr> {
        let key = format!("peer:{}", peer_id);
        self.db.get(key.as_bytes())
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v.to_vec()).ok())
            .and_then(|s| s.parse().ok())
    }
    
    fn all_peers(&self) -> Vec<(PeerId, Multiaddr)> {
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
    
    fn remove_peer(&self, peer_id: PeerId) -> anyhow::Result<()> {
        let key = format!("peer:{}", peer_id);
        self.db.delete(key.as_bytes())?;
        Ok(())
    }
}

/// Replay protection using nonce tracking and timestamp validation
/// Stores nonces per sender in RocksDB to prevent replay attacks
#[derive(Clone)]
struct ReplayProtection {
    db: Arc<DB>,
}

impl ReplayProtection {
    fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
    
    /// Validate nonce: must be incremental per sender, not previously used
    /// Returns the expected next nonce for the sender after validation
    fn validate_nonce(&self, sender: &str, nonce: u64) -> Result<u64, String> {
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
    fn update_nonce(&self, sender: &str, nonce: u64) -> anyhow::Result<()> {
        let key = format!("{}:{}", NONCE_KEY_PREFIX, sender);
        self.db.put(key.as_bytes(), nonce.to_string().as_bytes())?;
        Ok(())
    }
    
    /// Mark a transaction as processed (for deduplication)
    /// Uses transaction ID as key with timestamp as value
    fn mark_processed(&self, tx_id: &str, timestamp: u64) -> anyhow::Result<()> {
        let key = format!("{}{}", PROCESSED_TX_PREFIX, tx_id);
        self.db.put(key.as_bytes(), timestamp.to_string().as_bytes())?;
        Ok(())
    }
    
    /// Check if a transaction has already been processed
    fn is_processed(&self, tx_id: &str) -> bool {
        let key = format!("{}{}", PROCESSED_TX_PREFIX, tx_id);
        self.db.get(key.as_bytes()).ok().flatten().is_some()
    }
    
    /// Clean up old processed transactions (older than 24 hours)
    /// This prevents unbounded growth of the database
    fn cleanup_old_processed(&self, max_age_secs: u64) {
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

/// Sybil protection: limits connections per IP and validates peer identity
/// Prevents attackers from creating multiple fake identities
#[derive(Clone)]
struct SybilProtection {
    db: Arc<DB>,
    /// Maximum number of connections allowed per IP address
    max_connections_per_ip: u32,
}

impl SybilProtection {
    fn new(db: Arc<DB>, max_per_ip: u32) -> Self {
        Self { db, max_connections_per_ip: max_per_ip }
    }
    
    /// Count how many connections are currently active from a given IP
    /// Returns the count of peer_ids associated with this IP
    fn count_connections_per_ip(&self, ip: &Ipv4Addr) -> u32 {
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
    fn register_connection(&self, peer_id: PeerId, ip: &Ipv4Addr) -> anyhow::Result<()> {
        let key = format!("ip_conn:{}:{}", ip, peer_id);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| 0);
        self.db.put(key.as_bytes(), now.to_string().as_bytes())?;
        Ok(())
    }
    
    /// Unregister a connection when a peer disconnects
    fn unregister_connection(&self, peer_id: PeerId, ip: &Ipv4Addr) -> anyhow::Result<()> {
        let key = format!("ip_conn:{}:{}", ip, peer_id);
        self.db.delete(key.as_bytes())?;
        Ok(())
    }
    
    /// Check if a new connection from an IP would exceed the limit
    /// Returns Ok(()) if allowed, Err if Sybil attack detected
    fn check_ip_limit(&self, peer_id: PeerId, ip: &Ipv4Addr) -> Result<(), SybilError> {
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
    fn get_whitelisted_peers(&self) -> Vec<PeerId> {
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
    fn add_to_whitelist(&self, peer_id: PeerId) -> anyhow::Result<()> {
        let key = format!("whitelist_peer:{}", peer_id);
        self.db.put(key.as_bytes(), b"trusted")?;
        info!(peer_id = %peer_id, "Peer added to whitelist");
        Ok(())
    }
    
    /// Remove a peer from the whitelist
    fn remove_from_whitelist(&self, peer_id: PeerId) -> anyhow::Result<()> {
        let key = format!("whitelist_peer:{}", peer_id);
        self.db.delete(key.as_bytes())?;
        info!(peer_id = %peer_id, "Peer removed from whitelist");
        Ok(())
    }
}

#[derive(Debug)]
enum SybilError {
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

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    sync: request_response::cbor::Behaviour<SyncRequest, SyncResponse>,
    mdns: mdns::tokio::Behaviour,
}

/// Initialize structured JSON logging for Loki integration
fn setup_structured_logging() {
    use tracing_subscriber::prelude::*;
    
    // Layer 1: EnvFilter for log level filtering
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        // Add eBPF-specific log levels to suppress noisy kernel logs
        .add_directive("aya=warn".parse().unwrap())
        .add_directive("libp2p=info".parse().unwrap());
    
    // Layer 2: JSON formatter with custom fields for Loki
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)           // Suppress target module path
        .with_thread_ids(true)        // Include thread IDs for debugging
        .with_thread_names(true)      // Include thread names
        .with_file(true)              // Include source file
        .with_line_number(true)       // Include line numbers
        .with_level(true)             // Include log level
        .json()
        // Modify JSON output to add Loki-friendly fields
        .with_writer(std::io::stderr)
        .init();
    
    // Log after initialization is complete (use stderr directly before subscriber is ready)
    eprintln!("eBPF Node: Structured JSON logging initialized for Loki integration");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_structured_logging();

    let opt = Opt::parse();

    info!(event = "node_startup", iface = %opt.iface, "eBPF Node starting...");
    initialize_metrics();

    // Add a debug check that metrics are properly registered
    info!("Verifying metrics registration...");
    let test_gauge = LATENCY_BUCKETS.with_label_values(&["0"]);
    test_gauge.set(1);
    info!("Metrics registration verified");

    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("remove limit on locked memory failed, ret is: {ret}");
    }

    // Initialize RocksDB in a unique persistent path per node
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
    use rocksdb::{Options, DB};
    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    db_opts.set_max_open_files(1000);
    
    let db = match DB::open(&db_opts, &db_path) {
        Ok(db) => {
            info!("RocksDB opened successfully at {}", db_path);
            DB_OPERATIONS.with_label_values(&["open"]).inc();
            Arc::new(db)
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
                    Arc::new(DB::open(&Options::default(), &db_path).unwrap())
                } else {
                    error!("Recovery failed. Using empty database.");
                    Arc::new(DB::open_default(&db_path).unwrap())
                }
            } else {
                error!("No backup found. Using empty database.");
                Arc::new(DB::open_default(&db_path).unwrap())
            }
        }
    };
    
    // Schedule periodic backups (every hour in a separate task)
    let db_backup = db.clone();
    let db_path_clone = db_path.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(3600)); // Every hour
        loop {
            interval.tick().await;
            if let Err(e) = create_backup(&db_path_clone) {
                warn!("Backup failed: {}", e);
            }
        }
    });

    // Setup Tokio async channels for Axum-Swarm communication
    let (tx_rpc, mut rx_rpc) = mpsc::channel::<Transaction>(100);
    let (tx_ws, _rx_ws) = broadcast::channel::<String>(100);

    let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/ebpf-node"
    )))?;

    if let Err(e) = aya_log::EbpfLogger::init(&mut ebpf) {
        warn!("failed to initialize eBPF logger: {e}");
    }

    let xdp_program: &mut Xdp = ebpf.program_mut("ebpf_node").unwrap().try_into()?;
    xdp_program.load()?;
    if let Err(e) = xdp_program.attach(&opt.iface, XdpFlags::default()) {
        warn!("Failed to attach XDP program, continuing: {}", e);
    }

    let kprobe_in: &mut KProbe = ebpf.program_mut("netif_receive_skb").unwrap().try_into()?;
    kprobe_in.load()?;
    if let Err(e) = kprobe_in.attach("netif_receive_skb", 0) {
        warn!("Failed to attach KProbe in, continuing: {}", e);
    }

    let kprobe_out: &mut KProbe = ebpf.program_mut("napi_consume_skb").unwrap().try_into()?;
    kprobe_out.load()?;
    if let Err(e) = kprobe_out.attach("napi_consume_skb", 0) {
        warn!("Failed to attach KProbe out, continuing: {}", e);
    }

    // Use identity key for stable peer ID
    const ED25519_PRIVATE_KEY_SIZE: usize = 64;
    let keypair = libp2p::identity::ed25519::Keypair::generate();
    let persistent_keypath = format!("{}/identity.key", get_data_dir());
    
    let keypair = if let Ok(bytes) = std::fs::read(&persistent_keypath) {
        if bytes.len() == ED25519_PRIVATE_KEY_SIZE {
            if let Ok(sk) = libp2p::identity::ed25519::SecretKey::try_from_bytes(bytes) {
                info!("Loaded persistent identity key from {}", persistent_keypath);
                libp2p::identity::ed25519::Keypair::from(sk)
            } else {
                info!("Failed to load persistent key, generating new one");
                keypair
            }
        } else {
            info!("Invalid key file size (expected {}, got {})", ED25519_PRIVATE_KEY_SIZE, bytes.len());
            keypair
        }
    } else {
        info!("Generating new identity keypair");
        keypair
    };
    
    // Save persistent key for future use
    let key_bytes = keypair.to_bytes();
    if key_bytes.len() == ED25519_PRIVATE_KEY_SIZE {
        if let Err(e) = std::fs::write(&persistent_keypath, &key_bytes) {
            warn!("Failed to save identity key: {}", e);
        }
    }
    
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone().into())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|key| {
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(&message.data, &mut s);
                gossipsub::MessageId::from(std::hash::Hasher::finish(&s).to_string())
            };

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(1))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()
                .map_err(|msg| std::io::Error::new(std::io::ErrorKind::Other, msg))?;

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            let identify = identify::Behaviour::new(identify::Config::new(
                "/ebpf-blockchain/1.0.0".into(),
                key.public(),
            ));

            let mdns = mdns::tokio::Behaviour::new(
                mdns::Config::default(),
                key.public().to_peer_id(),
            )?;

            let sync = request_response::cbor::Behaviour::new(
                [(libp2p::StreamProtocol::new(SyncRequest::protocol()), request_response::ProtocolSupport::Full)],
                request_response::Config::default(),
            );

            Ok(MyBehaviour {
                gossipsub,
                identify,
                sync,
                mdns,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(120)))
        .build();

    // Listen on both TCP and QUIC for LXC compatibility
    let mut listen_addrs = vec![
        "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
        "/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap(),
    ];
    
    if !opt.listen_addresses.is_empty() {
        listen_addrs = opt.listen_addresses;
    }

    for addr in &listen_addrs {
        info!("Listening on: {}", addr);
        swarm.listen_on(addr.clone())?;
    }

    // Subscribe to Gossip topic
    let topic = gossipsub::IdentTopic::new("gossip");
    let _ = swarm.behaviour_mut().gossipsub.subscribe(&topic);

    info!("Local Peer ID: {}", swarm.local_peer_id());
    let _ = std::fs::write("/tmp/peer_id.txt", swarm.local_peer_id().to_string());
    
    // Load saved peers from environment and peer store
    let mut bootstrap_peers = opt.bootstrap_peers.clone();
    
    // Add peers from environment variable
    let env_peers = get_bootstrap_peers_from_env();
    for peer in &env_peers {
        if !bootstrap_peers.contains(peer) {
            bootstrap_peers.push(peer.clone());
        }
    }
    
    // Load known peers from peer store
    let peer_store = PeerStore::new(db.clone());
    let saved_peers = peer_store.all_peers();
    info!("Loaded {} saved peers from peer store", saved_peers.len());
    
    // Try to dial saved peers
    for (peer_id, addr) in &saved_peers {
        info!("Trying to reconnect to saved peer {} at {}", peer_id, addr);
    }

    // Bootstrap with retry logic
    let connection_retries = opt.connection_retries;
    let retry_interval = Duration::from_secs(opt.retry_interval_secs);
    
    for addr in &bootstrap_peers {
        for attempt in 1..=connection_retries {
            info!("Dialing bootstrap peer: {} (attempt {}/{})", addr, attempt, connection_retries);
            match swarm.dial(addr.clone()) {
                Ok(_) => {
                    info!("Successfully initiated dial to {}", addr);
                    break;
                }
                Err(e) => {
                    if attempt < connection_retries {
                        warn!("Failed to dial {}: {}, retrying...", addr, e);
                        tokio::time::sleep(retry_interval).await;
                    } else {
                        warn!("Failed to dial {}: {}, max retries reached", addr, e);
                    }
                }
            }
        }
    }
    
    // Try to reconnect to saved peers with retry
    for (peer_id, addr) in &saved_peers {
        for attempt in 1..=3 {
            info!("Reconnecting to saved peer {} at {} (attempt {}/3)", peer_id, addr, attempt);
            if swarm.dial(addr.clone()).is_ok() {
                info!("Successfully initiated reconnect to {}", peer_id);
                break;
            }
            if attempt < 3 {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    let mut stats_interval = time::interval(Duration::from_secs(10));
    let mut peer_save_interval = time::interval(Duration::from_secs(60));
    let mut replay_cleanup_interval = time::interval(Duration::from_secs(3600)); // Every hour
    
    // Create peer store for persistence
    let peer_store = PeerStore::new(db.clone());
    
    // Create security managers
    let replay_protection = ReplayProtection::new(db.clone());
    let sybil_protection = SybilProtection::new(db.clone(), 3); // Max 3 connections per IP

    // ============================================================================
    // P2 - Variables de entorno para puertos
    // ============================================================================
    let metrics_port = get_port_from_env("METRICS_PORT", 9090);
    let rpc_port = get_port_from_env("RPC_PORT", 9091);
    let ws_port = get_port_from_env("WS_PORT", 9092);
    let network_p2p_port = get_port_from_env("NETWORK_P2P_PORT", 9000);

    info!("Port configuration - Metrics: {}, RPC: {}, WS: {}, P2P: {}",
        metrics_port, rpc_port, ws_port, network_p2p_port);

    // Create NodeState for API handlers
    let local_peer_id = swarm.local_peer_id().to_string();
    
    let node_state = NodeState {
        start_time: std::time::Instant::now(),
        db: db.clone(),
        peer_store: peer_store.clone(),
        replay_protection: replay_protection.clone(),
        sybil_protection: sybil_protection.clone(),
        tx_rpc: tx_rpc.clone(),
        tx_ws: tx_ws.clone(),
        config: NodeConfig {
            iface: opt.iface.clone(),
            network_p2p_port,
            metrics_port,
            rpc_port,
            ws_port,
        },
        local_peer_id: local_peer_id.clone(),
        blocks_proposed: 0,
        transactions_processed: 0,
    };
    let node_state_arc = Arc::new(node_state);

    // Initialize Axum server with all endpoints
    let tx_ws_clone = tx_ws.clone();
    let node_state_clone = node_state_arc.clone();
    tokio::spawn(async move {
        let app = Router::new()
            // Health check
            .route("/health", get(health_handler))
            // Prometheus metrics
            .route("/metrics", get(metrics_handler))
            // REST API v1 - Node
            .route("/api/v1/node/info", get(node_info_handler))
            // REST API v1 - Network
            .route("/api/v1/network/peers", get(network_peers_handler))
            .route("/api/v1/network/config", get(network_config_get_handler))
            .route("/api/v1/network/config", put(network_config_put_handler))
            // REST API v1 - Transactions
            .route("/api/v1/transactions", post(transactions_create_handler))
            .route("/api/v1/transactions/:id", get(transactions_get_handler))
            // REST API v1 - Blocks
            .route("/api/v1/blocks/latest", get(blocks_latest_handler))
            .route("/api/v1/blocks/:height", get(blocks_by_height_handler))
            // REST API v1 - Security
            .route("/api/v1/security/blacklist", get(security_blacklist_get_handler))
            .route("/api/v1/security/blacklist", put(security_blacklist_put_handler))
            .route("/api/v1/security/whitelist", get(security_whitelist_get_handler))
            // Legacy endpoints (compatibility)
            .route("/rpc", post(rpc_handler))
            .route("/ws", get(ws_handler))
            .with_state(node_state_clone);

        // Bind to metrics port for REST API
        let bind_addr = format!("0.0.0.0:{}", metrics_port);
        if let Ok(listener) = tokio::net::TcpListener::bind(&bind_addr).await {
            info!("REST API server listening on {} (health, metrics, api, rpc, ws)", bind_addr);
            if let Err(e) = axum::serve(listener, app).await {
                error!("Axum server error: {}", e);
            }
        } else {
            warn!("Failed to bind REST API to {}. Trying fallback ports...", bind_addr);
            // Try fallback ports
            for fallback_port in [9091, 9092, 8080, 3000] {
                let fallback_addr = format!("0.0.0.0:{}", fallback_port);
                if let Ok(listener) = tokio::net::TcpListener::bind(&fallback_addr).await {
                    info!("REST API server listening on {} (fallback)", fallback_addr);
                    let _ = axum::serve(listener, app).await;
                    break;
                }
            }
        }
    });

    // Main event loop
    loop {
        tokio::select! {
            _ = stats_interval.tick() => {
                UPTIME.inc();
                
                // Update eBPF XDP metrics
                if let Ok(latency_stats) = HashMap::<_, u64, u64>::try_from(ebpf.map("LATENCY_STATS").unwrap()) {
                    let mut total_packets: u64 = 0;
                    for entry in latency_stats.iter() {
                        if let Ok((_, count)) = entry {
                            total_packets = total_packets.saturating_add(count);
                        }
                    }
                    XDP_PACKETS_PROCESSED.set(total_packets as i64);
                    
                    for i in 0..64 {
                        if let Ok(count) = latency_stats.get(&i, 0u64) {
                            LATENCY_BUCKETS.with_label_values(&[&i.to_string()]).set(count as i64);
                        }
                    }
                }
                
                // Update whitelist/blacklist sizes
                if let Ok(blacklist) = LpmTrie::<_, u32, u32>::try_from(ebpf.map("NODES_BLACKLIST").unwrap()) {
                    let blacklist_size = blacklist.iter().count();
                    XDP_BLACKLIST_SIZE.set(blacklist_size as i64);
                }
                if let Ok(whitelist) = LpmTrie::<_, u32, u32>::try_from(ebpf.map("NODES_WHITELIST").unwrap()) {
                    let whitelist_size = whitelist.iter().count();
                    XDP_WHITELIST_SIZE.set(whitelist_size as i64);
                }
                
                // Update system metrics
                update_system_metrics();
                
                // Update peer count
                VALIDATOR_COUNT.set(PEERS_CONNECTED.with_label_values(&["connected"]).get());
            }
            Some(tx) = rx_rpc.recv() => {
                info!(event = "rpc_tx_received", tx_id = %tx.id, data = %tx.data, "Received RPC Transaction");
                let msg = NetworkMessage::TxProposal(tx);
                if let Ok(payload) = serde_json::to_vec(&msg) {
                    let payload_size = payload.len();
                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), payload) {
                        warn!("Failed to publish via RPC: {:?}", e);
                    } else {
                        MESSAGES_SENT.inc();
                        MESSAGES_SENT_BY_TYPE.with_label_values(&["tx"]).inc();
                        BANDWIDTH_SENT.inc_by(payload_size as u64);
                        TRANSACTIONS_PROCESSED.inc();
                        TRANSACTIONS_BY_TYPE.with_label_values(&["transfer"]).inc();
                    }
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source,
                    message_id: _,
                    message,
                })) => {
                    info!(
                        event = "gossip_msg_received",
                        msg_type = ?message.topic,
                        data_len = message.data.len(),
                        "Received gossip message"
                    );
                    MESSAGES_RECEIVED.with_label_values(&["gossip"]).inc();
                    BANDWIDTH_RECEIVED.inc_by(message.data.len() as u64);
                    info!("Incremented MESSAGES_RECEIVED gauge");
                    let sender = propagation_source.to_string();
                    PACKETS_TRACE.with_label_values(&[&sender, "gossip"]).inc();
                    info!("Incremented PACKETS_TRACE gauge");

                    if let Ok(net_msg) = serde_json::from_slice::<NetworkMessage>(&message.data) {
                        match net_msg {
                            NetworkMessage::TxProposal(tx) => {
                                info!(
                                    event = "gossip_tx_proposal",
                                    sender = %sender,
                                    tx_id = %tx.id,
                                    data = %tx.data,
                                    nonce = tx.nonce,
                                    timestamp = tx.timestamp,
                                    "Received Gossip TxProposal"
                                );
                                
                                // SECURITY: Replay Protection - Validate nonce and timestamp
                                let validation_result: Result<u64, (String, String)> = (|| {
                                    // 1. Check timestamp validity
                                    if !tx.is_timestamp_valid() {
                                        return Err(("timestamp_expired".to_string(), "Transaction timestamp is too old".to_string()));
                                    }
                                    
                                    // 2. Check if transaction was already processed
                                    if replay_protection.is_processed(&tx.id) {
                                        return Err(("duplicate_tx".to_string(), "Transaction ID already processed".to_string()));
                                    }
                                    
                                    // 3. Validate nonce is incremental
                                    let next_nonce = replay_protection.validate_nonce(sender.as_str(), tx.nonce)
                                        .map_err(|e| ("invalid_nonce".to_string(), e))?;
                                    
                                    // Transaction passed all validation
                                    Ok(next_nonce)
                                })();
                                
                                match validation_result {
                                    Ok(next_nonce) => {
                                        // Transaction is valid - record nonce and mark as processed
                                        if let Err(e) = replay_protection.update_nonce(sender.as_str(), next_nonce) {
                                            warn!(event = "nonce_update_failed", sender = %sender, error = %e, "Failed to update nonce after valid transaction");
                                        }
                                        if let Err(e) = replay_protection.mark_processed(&tx.id, tx.timestamp) {
                                            warn!(event = "process_mark_failed", tx_id = %tx.id, error = %e, "Failed to mark transaction as processed");
                                        }
                                        
                                        // Solana-like Consensus: Validate & Vote via Gossip
                                        let vote = NetworkMessage::Vote {
                                            tx_id: tx.id.clone(),
                                            peer_id: swarm.local_peer_id().to_string()
                                        };
                                        if let Ok(payload) = serde_json::to_vec(&vote) {
                                            let payload_size = payload.len();
                                            if let Ok(_) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), payload) {
                                                MESSAGES_SENT.inc();
                                                MESSAGES_SENT_BY_TYPE.with_label_values(&["vote"]).inc();
                                                BANDWIDTH_SENT.inc_by(payload_size as u64);
                                            }
                                        }
                                    }
                                    Err((reason, detail)) => {
                                        TRANSACTIONS_REPLAY_REJECTED.inc();
                                        warn!(
                                            event = "transaction_rejected_replay_protection",
                                            tx_id = %tx.id,
                                            sender = %sender,
                                            reason = reason,
                                            detail = detail,
                                            "Transaction rejected by replay protection"
                                        );
                                        // Don't vote for invalid transactions
                                        continue;
                                    }
                                }
                            }
                            NetworkMessage::Vote { tx_id, peer_id } => {
                                info!(
                                    event = "gossip_vote_received",
                                    tx_id = %tx_id,
                                    voter = %peer_id,
                                    "Consensus Vote Received"
                                );

                                // Quorum Logic: Retrieve current state and add voter
                                let mut voters = std::collections::HashSet::new();
                                if let Ok(Some(existing)) = db.get(tx_id.as_bytes()) {
                                    if let Ok(existing_str) = String::from_utf8(existing.to_vec()) {
                                        if let Ok(existing_voters) = serde_json::from_str::<std::collections::HashSet<String>>(&existing_str) {
                                            voters = existing_voters;
                                        }
                                    }
                                }

                                if voters.insert(peer_id.clone()) {
                                    DB_OPERATIONS.with_label_values(&["put"]).inc();
                                    if let Ok(voters_json) = serde_json::to_string(&voters) {
                                        let _ = db.put(tx_id.as_bytes(), voters_json.as_bytes());

                                        // QUORUM THRESHOLD: 2/3 (or 2 for a 3-node lab)
                                        if voters.len() == 2 {
                                            TRANSACTIONS_CONFIRMED.inc();
                                            TRANSACTIONS_PROCESSED.inc();
                                            BLOCKS_PROPOSED.inc();
                                            CONSENSUS_ROUNDS.inc();
                                            info!(event = "quorum_reached", tx_id = %tx_id, "Quorum reached! Transaction confirmed.");
                                            let alert = serde_json::json!({
                                                "event": "BlockConfirmed",
                                                "tx_id": tx_id,
                                                "voters": voters
                                            }).to_string();
                                            let _ = tx_ws.send(alert);
                                        }
                                    }
                                } else {
                                    // Duplicate vote - possible replay attack
                                    TRANSACTIONS_REJECTED.inc();
                                    TRANSACTION_FAILURES.inc();
                                    warn!(event = "duplicate_vote_rejected", tx_id = %tx_id, voter = %peer_id, "Duplicate vote rejected - possible replay attack");
                                }
                            }
                        }
                    } else if message.data.starts_with(b"ATTACK") {
                        warn!("Malicious message detected from peer {}. Blocking IP.", sender);
                        // Note: In production, you would extract the real IP from the packet/peer
                        // For now, this is a placeholder for the dynamic threat detection logic
                        let ip_to_block = Ipv4Addr::new(1, 2, 3, 4);
                        let ip_u32 = u32::from_be_bytes(ip_to_block.octets());
                        let key = Key::new(32, ip_u32);

                        if let Ok(mut blacklist) = LpmTrie::<_, u32, u32>::try_from(ebpf.map_mut("NODES_BLACKLIST").unwrap()) {
                            if let Err(e) = blacklist.insert(&key, 1, 0) {
                                warn!("Failed to block IP: {}", e);
                            } else {
                                info!("IP {} blocked in reactive blacklist", ip_to_block);
                            }
                        }
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Listening on {:?}", address);
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    info!(event = "p2p_connected", peer = %peer_id, "New peer connected");
                    PEERS_CONNECTED.with_label_values(&["connected"]).inc();
                    P2P_CONNECTIONS_TOTAL.inc();
                    info!("Incremented PEERS_CONNECTED and P2P_CONNECTIONS_TOTAL gauges");
                    
                    // SECURITY: Sybil Protection - Register connection with known peer store addresses
                    if let Some(addr) = peer_store.get_peer(peer_id) {
                        if let Some(ip) = get_ip_from_multiaddr(&addr) {
                            if let Err(e) = sybil_protection.register_connection(peer_id, &ip) {
                                warn!(peer_id = %peer_id, ip = %ip, error = %e, "Failed to register connection for Sybil protection");
                            }
                            
                            if let Err(sybil_err) = sybil_protection.check_ip_limit(peer_id, &ip) {
                                warn!(peer_id = %peer_id, ip = %ip, error = %sybil_err, "Sybil protection limit exceeded for peer");
                            }
                        }
                    }
                }
                SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
                    PEERS_CONNECTED.with_label_values(&["connected"]).dec();
                    P2P_CONNECTIONS_CLOSED.inc();
                    info!("Decremented PEERS_CONNECTED, incremented P2P_CONNECTIONS_CLOSED. Remaining: {}", num_established);
                    
                    // SECURITY: Unregister connection for Sybil protection tracking
                    if let Some(addr) = peer_store.get_peer(peer_id) {
                        if let Some(ip) = get_ip_from_multiaddr(&addr) {
                            if let Err(e) = sybil_protection.unregister_connection(peer_id, &ip) {
                                debug!(peer_id = %peer_id, ip = %ip, error = %e, "Failed to unregister connection");
                            }
                        }
                    }
                }
                SwarmEvent::IncomingConnection { send_back_addr, .. } => {
                    if let Some(ip) = get_ip_from_multiaddr(&send_back_addr) {
                        debug!("Incoming connection from IP: {}", ip);
                        
                        // SECURITY: Pre-check Sybil limit before accepting connection
                        // Note: We can't check Sybil here since PeerId is not yet known
                        debug!(ip = %ip, "Incoming connection from unknown peer");
                    }
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, multiaddr) in list {
                        info!("mDNS discovered a new peer: {} at {}", peer_id, multiaddr);
                        
                        // Save discovered peer
                        let _ = peer_store.save_peer(peer_id, &multiaddr);
                        
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        if let Err(e) = swarm.dial(multiaddr.clone()) {
                            warn!("Failed to dial mDNS discovered peer {}: {}", peer_id, e);
                        }
                    }
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, multiaddr) in list {
                        info!("mDNS discovered peer has expired: {} at {}", peer_id, multiaddr);
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. })) => {
                    info!(event = "p2p_identified", peer = %peer_id, agent = %info.agent_version, "Peer identified");
                    
                    // Save identified peer with its addresses
                    PEERS_IDENTIFIED.inc();
                    for addr in info.listen_addrs {
                        let _ = peer_store.save_peer(peer_id, &addr);
                    }
                    
                    // Trigger historical sync when a peer is identified
                    info!(event = "sync_request_sent", target = %peer_id, "Requesting historical sync");
                    swarm.behaviour_mut().sync.send_request(&peer_id, SyncRequest);
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Sync(request_response::Event::Message {
                    peer,
                    message,
                    ..
                })) => match message {
                    request_response::Message::Request { request: SyncRequest, channel, .. } => {
                        debug!(event = "sync_request_received", from = %peer, "Received sync request, scanning RocksDB");
                        let mut transactions = Vec::new();
                        let iter = db.iterator(rocksdb::IteratorMode::Start);
                        for item in iter {
                            if let Ok((id, data)) = item {
                                if let (Ok(id_str), Ok(data_str)) = (String::from_utf8(id.to_vec()), String::from_utf8(data.to_vec())) {
                                    // Parse nonce and timestamp from data if present, otherwise use defaults
                                    let (nonce, timestamp) = if let Some(nonce_str) = data_str.strip_prefix("nonce:") {
                                        if let Some((nonce_part, timestamp_part)) = nonce_str.split_once(":ts:") {
                                            if let Ok(n) = nonce_part.parse::<u64>() {
                                                if let Ok(t) = timestamp_part.parse::<u64>() {
                                                    (n, t)
                                                } else {
                                                    (0, 0)
                                                }
                                            } else {
                                                (0, 0)
                                            }
                                        } else {
                                            (0, 0)
                                        }
                                    } else {
                                        (0, 0)
                                    };
                                    transactions.push(Transaction {
                                        id: id_str,
                                        data: data_str,
                                        nonce,
                                        timestamp
                                    });
                                }
                            }
                        }
                        info!(event = "sync_response_sent", target = %peer, count = transactions.len(), "Sending historical sync response");
                        let _ = swarm.behaviour_mut().sync.send_response(channel, SyncResponse { transactions });
                    }
                    request_response::Message::Response { response, .. } => {
                        info!(event = "sync_response_received", from = %peer, count = response.transactions.len(), "Processing historical sync");
                        for tx in response.transactions {
                            // Idempotent put: if we already have it, it's fine.
                            // We don't want to overwrite "Approved by" with basic data if we already approved it.
                            DB_OPERATIONS.with_label_values(&["get"]).inc();
                            if db.get(tx.id.as_bytes()).unwrap_or(None).is_none() {
                                DB_OPERATIONS.with_label_values(&["put"]).inc();
                                let _ = db.put(tx.id.as_bytes(), tx.data.as_bytes());
                                let approval_alert = serde_json::json!({
                                    "event": "BlockSynced",
                                    "tx_id": tx.id,
                                    "data": tx.data
                                }).to_string();
                                let _ = tx_ws.send(approval_alert);
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            _ = peer_save_interval.tick() => {
                // Periodically save connected peers (addresses tracked via identify behavior)
                let connected_peers: Vec<_> = swarm.connected_peers().cloned().collect();
                PEERS_SAVED.inc_by(connected_peers.len() as u64);
            }
            _ = replay_cleanup_interval.tick() => {
                // Periodically clean up old processed transactions to prevent unbounded DB growth
                info!("Cleaning up old processed transactions...");
                replay_protection.cleanup_old_processed(86400); // 24 hours
            }
            _ = signal::ctrl_c() => {
                info!("Exiting...");
                break;
            }
        }
    }

    Ok(())
}

fn get_ip_from_multiaddr(addr: &Multiaddr) -> Option<Ipv4Addr> {
    for proto in addr.iter() {
        if let libp2p::multiaddr::Protocol::Ip4(ip) = proto {
            return Some(ip);
        }
    }
    None
}
