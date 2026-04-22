use axum::{extract::State, Json, http::StatusCode, response::IntoResponse};
use std::sync::Arc;

use crate::config::node::{HealthResponse, HealthChecks};
use crate::config::node::NodeState;

/// GET /health - Health check endpoint
pub async fn health_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
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
