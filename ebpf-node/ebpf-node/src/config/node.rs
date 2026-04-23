use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use libp2p::{Multiaddr, PeerId};
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

use crate::security::{peer_store::PeerStore, replay::ReplayProtection, sybil::SybilProtection};

// Import Digest trait for SHA-256 hashing
use sha2::Digest;

/// Maximum allowed nonce age in seconds (5 minutes)
pub const NONCE_MAX_AGE_SECS: u64 = 300;

/// Prefix for nonce tracking keys in RocksDB
pub const NONCE_KEY_PREFIX: &str = "nonce:";

/// Prefix for processed transaction IDs (for deduplication)
pub const PROCESSED_TX_PREFIX: &str = "processed_tx:";

// ============================================================================
// TAREA 2.3: Signed Vote Types
// ============================================================================

use ed25519_dalek::{Signature, VerifyingKey};

/// Vote structure for consensus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vote {
    pub tx_id: String,
    pub voter_id: String,
    pub timestamp: u64,
    pub validator_id: String,
}

impl Vote {
    /// Create byte representation for signing/verification
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_string(self).unwrap_or_default().into_bytes()
    }
    
    /// Create a new vote
    pub fn new(tx_id: String, voter_id: String, validator_id: String) -> Self {
        Self {
            tx_id,
            voter_id,
            validator_id,
            timestamp: get_current_timestamp(),
        }
    }
}

/// Signed vote for consensus with cryptographic signature
#[derive(Clone, Debug)]
pub struct SignedVote {
    pub vote: Vote,
    pub signature: Signature,
}

impl Serialize for SignedVote {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("SignedVote", 2)?;
        state.serialize_field("vote", &self.vote)?;
        let sig_bytes: Vec<u8> = self.signature.to_bytes().to_vec();
        state.serialize_field("signature", &sig_bytes)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SignedVote {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess};
        struct SignedVoteVisitor;
        
        impl<'de> serde::de::Visitor<'de> for SignedVoteVisitor {
            type Value = SignedVote;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct SignedVote")
            }
            
            fn visit_map<V>(self, mut map: V) -> Result<SignedVote, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut vote: Option<Vote> = None;
                let mut signature_bytes: Option<Vec<u8>> = None;
                
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "vote" => {
                            if vote.is_some() {
                                return Err(serde::de::Error::duplicate_field("vote"));
                            }
                            vote = Some(map.next_value()?);
                        }
                        "signature" => {
                            if signature_bytes.is_some() {
                                return Err(serde::de::Error::duplicate_field("signature"));
                            }
                            signature_bytes = Some(map.next_value()?);
                        }
                        _ => {
                            let _ = map.next_value::<serde::de::IgnoredAny>();
                        }
                    }
                }
                
                let vote = vote.ok_or_else(|| serde::de::Error::missing_field("vote"))?;
                let signature_bytes = signature_bytes.ok_or_else(|| serde::de::Error::missing_field("signature"))?;
                let signature = Signature::from_slice(&signature_bytes)
                    .map_err(serde::de::Error::custom)?;
                
                Ok(SignedVote { vote, signature })
            }
        }
        
        deserializer.deserialize_struct("SignedVote", &["vote", "signature"], SignedVoteVisitor)
    }
}

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
    pub public_key: String,
    pub blocks_proposed: u64,
    pub transactions_processed: u64,
    #[allow(dead_code)]
    pub hot_reload_manager: Arc<crate::ebpf::hot_reload::EbpfHotReloadManager>,
    // TAREA 2.1: Proposer rotation
    pub proposer_rotation_index: Arc<Mutex<u64>>,
    pub validator_peers: Arc<Mutex<Vec<String>>>,
    // TAREA 4: Ed25519 signing/verification keys for real vote signatures
    pub signing_key: Arc<Mutex<Option<ed25519_dalek::SigningKey>>>,
    pub verifying_key: Arc<ed25519_dalek::VerifyingKey>,
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
// TAREA 2.4: Slashing Structures
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlashingEvent {
    pub validator_id: String,
    pub block_height: u64,
    pub reason: String,
    pub timestamp: u64,
    pub evidence: String,
}

// ============================================================================
// TAREA 2.7: Checkpoint Structures
// ============================================================================

pub const CHECKPOINT_INTERVAL: u64 = 100;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub height: u64,
    pub state_root: String,
    pub timestamp: u64,
    pub validators: Vec<String>,
    pub signature: Vec<u8>,
}

// ============================================================================
// TAREA 2.1 & 2.2 & 2.4 & 2.7: NodeState Methods
// ============================================================================

impl NodeState {
    /// TAREA 2.1: Get next proposer using round-robin rotation
    pub fn get_next_proposer(&self) -> Option<String> {
        let mut idx = self.proposer_rotation_index.lock().unwrap();
        let validators = self.validator_peers.lock().unwrap();
        if validators.is_empty() {
            return None;
        }
        let proposer = validators[*idx as usize % validators.len()].clone();
        *idx += 1;
        Some(proposer)
    }
    
    /// TAREA 2.1: Register a validator peer
    pub fn register_validator(&self, peer_id: String) {
        let mut validators = self.validator_peers.lock().unwrap();
        if !validators.contains(&peer_id) {
            validators.push(peer_id);
        }
    }
    
    /// TAREA 2.2: Create a real block and persist to RocksDB
    pub fn create_block(&self, transactions: Vec<String>) -> anyhow::Result<Block> {
        let latest_height: u64 = self.db.get(b"latest_height".as_ref())
            .ok().flatten()
            .and_then(|v| bincode::deserialize(&v).ok())
            .unwrap_or(0);
        
        let height = latest_height + 1;
        let parent_hash = if latest_height > 0 {
            let parent_key = format!("block:{}", latest_height);
            self.db.get(parent_key.as_bytes())
                .ok().flatten()
                .map(|v| {
                    let block: Block = bincode::deserialize(&v).unwrap();
                    block.hash
                })
                .unwrap_or_else(|| "genesis".to_string())
        } else {
            "genesis".to_string()
        };
        
        let proposer = self.get_next_proposer().unwrap_or_else(|| self.local_peer_id.clone());
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Compute real hash using SHA256
        let mut hasher = sha2::Sha256::new();
        hasher.update(height.to_string().as_bytes());
        hasher.update(parent_hash.as_bytes());
        hasher.update(proposer.as_bytes());
        hasher.update(timestamp.to_string().as_bytes());
        for tx in &transactions {
            hasher.update(tx.as_bytes());
        }
        let hash = format!("0x{:x}", hasher.finalize());
        
        let block = Block {
            height,
            hash,
            parent_hash,
            proposer,
            timestamp,
            transactions,
            quorum_votes: 0,
            total_validators: 0,
        };
        
        // Persist block
        let block_key = format!("block:{}", height);
        let block_data = bincode::serialize(&block)?;
        self.db.put(block_key.as_bytes(), block_data)?;
        
        // Update latest_height
        self.db.put(b"latest_height".as_ref(), bincode::serialize(&height)?)?;
        
        Ok(block)
    }
    
    /// TAREA 2.4: Record a slashing event
    pub fn record_slashing_event(&self, event: SlashingEvent) -> anyhow::Result<()> {
        let key = format!("slashing:{}", event.validator_id);
        let mut events: Vec<SlashingEvent> = self.db.get(key.as_bytes())
            .ok().flatten()
            .and_then(|v| bincode::deserialize(&v).ok())
            .unwrap_or_default();
        
        events.push(event);
        self.db.put(key.as_bytes(), bincode::serialize(&events)?)?;
        crate::metrics::prometheus::SLASHING_EVENTS.inc();
        Ok(())
    }
    
    /// TAREA 2.4: Check if a validator is slashed (2+ offenses)
    pub fn is_slashed(&self, validator_id: &str) -> bool {
        let key = format!("slashing:{}", validator_id);
        self.db.get(key.as_bytes())
            .ok().flatten()
            .map(|v| {
                let events: Vec<SlashingEvent> = bincode::deserialize(&v).unwrap_or_default();
                events.len() >= 2
            })
            .unwrap_or(false)
    }
    
    /// TAREA 2.7: Create a checkpoint at checkpoint intervals
    pub fn create_checkpoint(&self, block: &Block) -> anyhow::Result<Checkpoint> {
        let checkpoint = Checkpoint {
            height: block.height,
            state_root: block.hash.clone(),
            timestamp: block.timestamp,
            validators: self.validator_peers.lock().unwrap().clone(),
            signature: Vec::new(),
        };
        
        // Persist checkpoint
        let key = format!("checkpoint:{}", checkpoint.height);
        self.db.put(key.as_bytes(), bincode::serialize(&checkpoint)?)?;
        
        Ok(checkpoint)
    }
    
    /// TAREA 2.7: Get the latest checkpoint
    pub fn get_latest_checkpoint(&self) -> Option<Checkpoint> {
        let latest_height: u64 = self.db.get(b"latest_height".as_ref())
            .ok().flatten()
            .and_then(|v| bincode::deserialize(&v).ok())
            .unwrap_or(0);
        
        let checkpoint_height = (latest_height / CHECKPOINT_INTERVAL) * CHECKPOINT_INTERVAL;
        if checkpoint_height > 0 {
            let key = format!("checkpoint:{}", checkpoint_height);
            self.db.get(key.as_bytes())
                .ok().flatten()
                .and_then(|v| bincode::deserialize(&v).ok())
        } else {
            None
        }
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
