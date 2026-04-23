use axum::{extract::State, Json, http::StatusCode, response::IntoResponse, extract::Path};
use std::sync::Arc;

use crate::config::node::{NodeState, Block, BlockSummary, BlockListResponse, format_iso_timestamp, get_current_timestamp};
use crate::metrics::prometheus::BLOCKS_PROPOSED;

/// GET /api/v1/blocks/latest - Latest block
pub async fn blocks_latest_handler(State(state): State<Arc<NodeState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Query RocksDB for the latest block height
    let latest_height = state.db.get(b"latest_height".as_ref())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    
    let height: u64 = latest_height
        .and_then(|v| bincode::deserialize(&v).ok())
        .unwrap_or(0);
    
    if height == 0 {
        return Ok(Json(serde_json::json!({
            "error": "Not Found",
            "message": "No blocks yet",
            "code": "NO_BLOCKS",
        })));
    }
    
    // Retrieve block from RocksDB by key
    let block_key = format!("block:{}", height);
    let block_data = state.db.get(block_key.as_bytes())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    
    match block_data {
        Some(data) => {
            let block: Block = bincode::deserialize(&data)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Deserialize error: {}", e)))?;
            
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
            
            Ok(Json(response))
        }
        None => Err((StatusCode::NOT_FOUND, "Block not found".to_string())),
    }
}

/// GET /api/v1/blocks/{height} - Block by height
pub async fn blocks_by_height_handler(
    State(state): State<Arc<NodeState>>,
    Path(height): Path<u64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Query RocksDB for the block at the given height
    let block_key = format!("block:{}", height);
    let block_data = state.db.get(block_key.as_bytes())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    
    match block_data {
        Some(data) => {
            let block: Block = bincode::deserialize(&data)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Deserialize error: {}", e)))?;
            
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
            
            Ok(Json(response))
        }
        None => Err((StatusCode::NOT_FOUND, format!("Block at height {} not found", height))),
    }
}
