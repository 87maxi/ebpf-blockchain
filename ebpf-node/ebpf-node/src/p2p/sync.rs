use libp2p::{
    request_response,
    Swarm,
};

use crate::config::node::{SyncRequest, SyncResponse, Transaction};
use crate::p2p::behaviour::MyBehaviour;

/// Handle sync request-response messages
pub async fn handle_sync_message(
    swarm: &mut Swarm<MyBehaviour>,
    peer: libp2p::PeerId,
    message: request_response::Message<SyncRequest, SyncResponse>,
    db: &std::sync::Arc<rocksdb::DB>,
    tx_ws: &tokio::sync::broadcast::Sender<String>,
) {
    match message {
        request_response::Message::Request { request: SyncRequest, channel, .. } => {
            tracing::debug!(event = "sync_request_received", from = %peer, "Received sync request, scanning RocksDB");
            let mut transactions = Vec::new();
            let iter = db.iterator(rocksdb::IteratorMode::Start);
            for item in iter {
                if let Ok((id, data)) = item {
                    if let (Ok(id_str), Ok(data_str)) = (String::from_utf8(id.to_vec()), String::from_utf8(data.to_vec())) {
                        // Parse nonce and timestamp from data if present, otherwise use defaults
                        let (nonce, timestamp) = if let Some(nonce_str) = data_str.strip_prefix("nonce:") {
                            if let Some((nonce_part, timestamp_part)) = nonce_str.split_once(":ts:") {
                                if let Ok(n) = nonce_part.parse::<u64>() {
                                    if let Ok(t) = timestamp_part.parse::<u64>() {
                                        (n, t)
                                    } else {
                                        (0, 0)
                                    }
                                } else {
                                    (0, 0)
                                }
                            } else {
                                (0, 0)
                            }
                        } else {
                            (0, 0)
                        };
                        transactions.push(Transaction {
                            id: id_str,
                            data: data_str,
                            nonce,
                            timestamp
                        });
                    }
                }
            }
            tracing::info!(event = "sync_response_sent", target = %peer, count = transactions.len(), "Sending historical sync response");
            let _ = swarm.behaviour_mut().sync.send_response(channel, SyncResponse { transactions });
        }
        request_response::Message::Response { response, .. } => {
            tracing::info!(event = "sync_response_received", from = %peer, count = response.transactions.len(), "Processing historical sync");
            for tx in response.transactions {
                // Idempotent put: if we already have it, it's fine.
                // We don't want to overwrite "Approved by" with basic data if we already approved it.
                crate::metrics::prometheus::DB_OPERATIONS.with_label_values(&["get"]).inc();
                if db.get(tx.id.as_bytes()).unwrap_or(None).is_none() {
                    crate::metrics::prometheus::DB_OPERATIONS.with_label_values(&["put"]).inc();
                    let _ = db.put(tx.id.as_bytes(), tx.data.as_bytes());
                    let approval_alert = serde_json::json!({
                        "event": "BlockSynced",
                        "tx_id": tx.id,
                        "data": tx.data
                    }).to_string();
                    let _ = tx_ws.send(approval_alert);
                }
            }
        }
        _ => {}
    }
}
