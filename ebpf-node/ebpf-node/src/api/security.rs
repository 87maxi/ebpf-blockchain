use axum::{extract::State, Json, http::StatusCode, response::IntoResponse, extract::Json as ExtractJson};
use std::sync::Arc;

use crate::config::node::{SecurityListResponse, SecurityEntry, SecurityActionResponse, get_current_timestamp, security_action_error};
use crate::config::node::NodeState;

/// GET /api/v1/security/blacklist - Get blacklist
pub async fn security_blacklist_get_handler(
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
pub async fn security_blacklist_put_handler(
    State(state): State<Arc<NodeState>>,
    ExtractJson(payload): ExtractJson<serde_json::Value>,
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
pub async fn security_whitelist_get_handler(
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
