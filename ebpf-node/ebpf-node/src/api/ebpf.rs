//! eBPF hot-reload API endpoints

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::node::NodeState;
use crate::ebpf::hot_reload::EbpfHotReloadManager;

/// Response for eBPF reload operation
#[derive(Serialize, Deserialize)]
pub struct EbpfReloadResponse {
    pub success: bool,
    pub message: String,
}

/// Reload eBPF programs endpoint
pub async fn ebpf_reload_handler(
    State(state): State<Arc<NodeState>>,
) -> (StatusCode, Json<EbpfReloadResponse>) {
    let hot_reload_manager = &state.hot_reload_manager;
    
    match hot_reload_manager.reload().await {
        Ok(_) => {
            let response = EbpfReloadResponse {
                success: true,
                message: "eBPF programs reloaded successfully".to_string(),
            };
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            let response = EbpfReloadResponse {
                success: false,
                message: format!("Failed to reload eBPF programs: {:?}", e),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
}