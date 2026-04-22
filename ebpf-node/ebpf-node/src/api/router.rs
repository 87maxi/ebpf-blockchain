use axum::{Router, routing::{get, post, put}};
use std::sync::Arc;

use crate::config::node::NodeState;
use crate::config::node::Transaction;
use tokio::sync::{broadcast, mpsc};

pub fn create_router(
    state: Arc<NodeState>,
    tx_rpc: mpsc::Sender<Transaction>,
    tx_ws: broadcast::Sender<String>,
) -> Router {
    let tx_ws_clone = tx_ws.clone();
    let node_state_clone = state.clone();
    
    Router::new()
        // Health check
        .route("/health", get(crate::api::health::health_handler))
        // Prometheus metrics
        .route("/metrics", get(crate::api::metrics::metrics_handler))
        // REST API v1 - Node
        .route("/api/v1/node/info", get(crate::api::node::node_info_handler))
        // REST API v1 - Network
        .route("/api/v1/network/peers", get(crate::api::network::network_peers_handler))
        .route("/api/v1/network/config", get(crate::api::network::network_config_get_handler))
        .route("/api/v1/network/config", put(crate::api::network::network_config_put_handler))
        // REST API v1 - Transactions
        .route("/api/v1/transactions", post(crate::api::transactions::transactions_create_handler))
        .route("/api/v1/transactions/:id", get(crate::api::transactions::transactions_get_handler))
        // REST API v1 - Blocks
        .route("/api/v1/blocks/latest", get(crate::api::blocks::blocks_latest_handler))
        .route("/api/v1/blocks/:height", get(crate::api::blocks::blocks_by_height_handler))
        // REST API v1 - Security
        .route("/api/v1/security/blacklist", get(crate::api::security::security_blacklist_get_handler))
        .route("/api/v1/security/blacklist", put(crate::api::security::security_blacklist_put_handler))
        .route("/api/v1/security/whitelist", get(crate::api::security::security_whitelist_get_handler))
        // Legacy endpoints (compatibility)
        .route("/rpc", post(crate::api::rpc::rpc_handler))
        .route("/ws", get(crate::api::ws::ws_handler))
        .with_state(node_state_clone)
}
