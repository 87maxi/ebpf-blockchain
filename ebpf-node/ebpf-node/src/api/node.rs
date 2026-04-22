use axum::{extract::State, Json, http::StatusCode, response::IntoResponse};
use std::sync::Arc;

use crate::config::node::{NodeInfoResponse, NodeState};
use crate::metrics::prometheus::{PEERS_CONNECTED, BLOCKS_PROPOSED, TRANSACTIONS_PROCESSED};

/// GET /api/v1/node/info - Node information
pub async fn node_info_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
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
