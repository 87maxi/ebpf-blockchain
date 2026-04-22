use axum::{
    extract::State,
    extract::ws::{WebSocket, WebSocketUpgrade, Message},
    response::IntoResponse,
};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::node::NodeState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<NodeState>>,
) -> impl IntoResponse {
    let rx = state.tx_ws.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

pub async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}
