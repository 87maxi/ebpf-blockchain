use axum::{extract::State, Json, http::StatusCode, response::IntoResponse, extract::Json as ExtractJson};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::node::{NodeState, PeerListResponse, PeerDetail, NetworkConfigResponse, GossipsubParams, format_iso_timestamp, get_current_timestamp};
use crate::config::cli::get_ip_from_multiaddr;

/// GET /api/v1/network/peers - Connected peers list
pub async fn network_peers_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
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
pub async fn network_config_get_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
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
pub async fn network_config_put_handler(
    State(state): State<Arc<NodeState>>,
    ExtractJson(payload): ExtractJson<serde_json::Value>,
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
