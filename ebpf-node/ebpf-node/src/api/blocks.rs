use axum::{extract::State, Json, http::StatusCode, response::IntoResponse, extract::Path};
use std::sync::Arc;

use crate::config::node::{NodeState, Block, BlockSummary, BlockListResponse, format_iso_timestamp, get_current_timestamp};
use crate::metrics::prometheus::BLOCKS_PROPOSED;

/// GET /api/v1/blocks/latest - Latest block
pub async fn blocks_latest_handler(State(state): State<Arc<NodeState>>) -> (StatusCode, Json<serde_json::Value>) {
    let height = BLOCKS_PROPOSED.get();
    
    if height == 0 {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "Not Found",
            "message": "No blocks yet",
            "code": "NO_BLOCKS",
        })));
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
pub async fn blocks_by_height_handler(
    State(state): State<Arc<NodeState>>,
    Path(height): Path<u64>,
) -> (StatusCode, Json<serde_json::Value>) {
    let current_height = BLOCKS_PROPOSED.get();
    
    if height > current_height {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "Not Found",
            "message": format!("Block at height {} not found", height),
            "code": "BLOCK_NOT_FOUND",
        })));
    }
    
    if current_height == 0 {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "Not Found",
            "message": "No blocks yet",
            "code": "NO_BLOCKS",
        })));
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
