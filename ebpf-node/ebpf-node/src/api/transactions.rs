use axum::{extract::State, Json, http::StatusCode, response::IntoResponse, extract::Json as ExtractJson, extract::Path};
use std::sync::Arc;
use std::collections::HashSet;

use crate::config::node::{NodeState, Transaction, TransactionCreateResponse, TransactionGetResponse, tx_create_error, tx_get_error, format_iso_timestamp, get_current_timestamp};
use crate::metrics::prometheus::{TRANSACTIONS_REPLAY_REJECTED, TRANSACTIONS_PROCESSED, TRANSACTIONS_BY_TYPE};
use tracing::{warn, info};

/// POST /api/v1/transactions - Create transaction (replaces /rpc)
pub async fn transactions_create_handler(
    State(state): State<Arc<NodeState>>,
    ExtractJson(tx): ExtractJson<Transaction>,
) -> (StatusCode, Json<TransactionCreateResponse>) {
    // Validate required fields
    if tx.id.is_empty() || tx.data.is_empty() {
        return tx_create_error(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Transaction must have id and data fields",
            "MISSING_FIELDS",
        );
    }
    
    // Check if transaction already processed
    if state.replay_protection.is_processed(&tx.id) {
        return tx_create_error(
            StatusCode::CONFLICT,
            "Conflict",
            "Transaction already processed",
            "DUPLICATE_TX",
        );
    }
    
    // Validate nonce
    let sender = "api-submitter".to_string(); // API submissions use a placeholder sender
    match state.replay_protection.validate_nonce(&sender, tx.nonce) {
        Ok(next_nonce) => {
            // Valid transaction - record nonce and mark as processed
            if let Err(e) = state.replay_protection.update_nonce(&sender, next_nonce) {
                warn!(event = "nonce_update_failed", sender = %sender, error = %e, "Failed to update nonce");
            }
            if let Err(e) = state.replay_protection.mark_processed(&tx.id, tx.timestamp) {
                warn!(event = "process_mark_failed", tx_id = %tx.id, error = %e, "Failed to mark transaction as processed");
            }
        }
        Err(e) => {
            TRANSACTIONS_REPLAY_REJECTED.inc();
            return tx_create_error(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                &format!("Invalid nonce: {}", e),
                "INVALID_NONCE",
            );
        }
    }
    
    // Send to gossip via channel
    let _ = state.tx_rpc.send(tx.clone()).await;
    
    TRANSACTIONS_PROCESSED.inc();
    TRANSACTIONS_BY_TYPE.with_label_values(&["transfer"]).inc();
    
    let response = TransactionCreateResponse {
        hash: format!("0x{:?}", tx.id),
        status: "pending".to_string(),
        block_number: None,
        timestamp: format_iso_timestamp(tx.timestamp),
        nonce: tx.nonce,
    };
    
    (StatusCode::CREATED, Json(response))
}

/// GET /api/v1/transactions/{id} - Get transaction by ID
pub async fn transactions_get_handler(
    State(state): State<Arc<NodeState>>,
    Path(tx_id): Path<String>,
) -> (StatusCode, Json<TransactionGetResponse>) {
    // Look up in RocksDB
    match state.db.get(tx_id.as_bytes()) {
        Ok(Some(data)) => {
            if let Ok(data_str) = String::from_utf8(data.to_vec()) {
                let tx_hash = format!("0x{:?}", tx_id);
                // Check if it's a voter set (confirmed) or raw data
                if let Ok(voters) = serde_json::from_str::<HashSet<String>>(&data_str) {
                    // It's a confirmed transaction with voters
                    let response = TransactionGetResponse {
                        id: tx_id,
                        hash: tx_hash,
                        data: data_str.clone(),
                        nonce: 0, // Not stored in voter set
                        status: "confirmed".to_string(),
                        block_number: Some(crate::metrics::prometheus::BLOCKS_PROPOSED.get()),
                        confirmations: voters.len() as u64,
                        timestamp: get_current_timestamp(),
                    };
                    return (StatusCode::OK, Json(response));
                } else {
                    // Raw transaction data
                    let response = TransactionGetResponse {
                        id: tx_id,
                        hash: tx_hash,
                        data: data_str,
                        nonce: 0,
                        status: "pending".to_string(),
                        block_number: None,
                        confirmations: 0,
                        timestamp: get_current_timestamp(),
                    };
                    return (StatusCode::OK, Json(response));
                }
            } else {
                return tx_get_error(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                    &format!("Transaction {} not found", tx_id),
                    "TX_NOT_FOUND",
                );
            }
        }
        Ok(None) => {
            return tx_get_error(
                StatusCode::NOT_FOUND,
                "Not Found",
                &format!("Transaction {} not found", tx_id),
                "TX_NOT_FOUND",
            );
        }
        Err(e) => {
            return tx_get_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                &format!("Database error: {}", e),
                "DB_ERROR",
            );
        }
    }
}
