use axum::{extract::State, Json, response::IntoResponse};
use std::sync::Arc;

use crate::config::node::NodeState;
use crate::config::node::Transaction;

pub async fn rpc_handler(
    State(state): State<Arc<NodeState>>,
    Json(payload): Json<Transaction>,
) -> impl IntoResponse {
    let _ = state.tx_rpc.send(payload).await;
    axum::http::StatusCode::ACCEPTED
}
