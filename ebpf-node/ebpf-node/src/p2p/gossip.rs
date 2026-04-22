use std::net::Ipv4Addr;

use libp2p::{
    gossipsub,
    multiaddr::Protocol,
    Multiaddr,
    PeerId,
    Swarm,
};
use aya::{
    maps::{HashMap, LpmTrie, lpm_trie::Key},
};

use crate::config::node::{NetworkMessage, Transaction};
use crate::p2p::behaviour::MyBehaviour;
use crate::metrics::prometheus::{
    MESSAGES_RECEIVED, PACKETS_TRACE, MESSAGES_SENT, MESSAGES_SENT_BY_TYPE,
    BANDWIDTH_SENT, BANDWIDTH_RECEIVED, TRANSACTIONS_PROCESSED, TRANSACTIONS_BY_TYPE,
    TRANSACTIONS_REPLAY_REJECTED, BLOCKS_PROPOSED, CONSENSUS_ROUNDS,
    TRANSACTIONS_CONFIRMED, DB_OPERATIONS, TRANSACTIONS_REJECTED, TRANSACTION_FAILURES,
};
use crate::security::replay::ReplayProtection;
use std::sync::Arc;
use rocksdb::DB;

/// Handle gossipsub messages
pub async fn handle_gossip_message(
    swarm: &mut Swarm<MyBehaviour>,
    message: gossipsub::Message,
    propagation_source: PeerId,
    db: &Arc<DB>, // Using rocksdb in reality
    replay_protection: &ReplayProtection,
    tx_ws: &tokio::sync::broadcast::Sender<String>,
    topic: &gossipsub::IdentTopic,
) {
    let sender = propagation_source.to_string();
    MESSAGES_RECEIVED.with_label_values(&["gossip"]).inc();
    BANDWIDTH_RECEIVED.inc_by(message.data.len() as u64);

    PACKETS_TRACE.with_label_values(&[&sender, "gossip"]).inc();

    if let Ok(net_msg) = serde_json::from_slice::<NetworkMessage>(&message.data) {
        match net_msg {
            NetworkMessage::TxProposal(tx) => {
                handle_tx_proposal(
                    swarm,
                    tx,
                    &sender,
                    propagation_source,
                    db,
                    replay_protection,
                    topic,
                ).await;
            }
            NetworkMessage::Vote { tx_id, peer_id } => {
                handle_vote(
                    swarm,
                    tx_id,
                    peer_id,
                    propagation_source,
                    db,
                    tx_ws,
                    topic,
                ).await;
            }
        }
    } else if message.data.starts_with(b"ATTACK") {
        handle_malicious_message(swarm, &sender).await;
    }
}

async fn handle_tx_proposal(
    swarm: &mut Swarm<MyBehaviour>,
    tx: Transaction,
    sender: &str,
    propagation_source: PeerId,
    db: &Arc<DB>,
    replay_protection: &ReplayProtection,
    topic: &gossipsub::IdentTopic,
) {
    tracing::info!(
        event = "gossip_tx_proposal",
        sender = %sender,
        tx_id = %tx.id,
        data = %tx.data,
        nonce = tx.nonce,
        timestamp = tx.timestamp,
        "Received Gossip TxProposal"
    );

    // SECURITY: Replay Protection - Validate nonce and timestamp
    let validation_result: Result<u64, (String, String)> = (|| {
        // 1. Check timestamp validity
        if !tx.is_timestamp_valid() {
            return Err(("timestamp_expired".to_string(), "Transaction timestamp is too old".to_string()));
        }

        // 2. Check if transaction was already processed
        if replay_protection.is_processed(&tx.id) {
            return Err(("duplicate_tx".to_string(), "Transaction ID already processed".to_string()));
        }

        // 3. Validate nonce is incremental (sender is already &str, no need for as_str())
        let next_nonce = replay_protection.validate_nonce(sender, tx.nonce)
            .map_err(|e| ("invalid_nonce".to_string(), e))?;

        // Transaction passed all validation
        Ok(next_nonce)
    })();

    match validation_result {
        Ok(next_nonce) => {
            // Transaction is valid - record nonce and mark as processed
            if let Err(e) = replay_protection.update_nonce(sender, next_nonce) {
                tracing::warn!(event = "nonce_update_failed", sender = %sender, error = %e, "Failed to update nonce after valid transaction");
            }
            if let Err(e) = replay_protection.mark_processed(&tx.id, tx.timestamp) {
                tracing::warn!(event = "process_mark_failed", tx_id = %tx.id, error = %e, "Failed to mark transaction as processed");
            }

            // Solana-like Consensus: Validate & Vote via Gossip
            let vote = NetworkMessage::Vote {
                tx_id: tx.id.clone(),
                peer_id: swarm.local_peer_id().to_string()
            };
            if let Ok(payload) = serde_json::to_vec(&vote) {
                let payload_size = payload.len();
                if let Ok(_) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), payload) {
                    MESSAGES_SENT.inc();
                    MESSAGES_SENT_BY_TYPE.with_label_values(&["vote"]).inc();
                    BANDWIDTH_SENT.inc_by(payload_size as u64);
                }
            }
        }
        Err((reason, detail)) => {
            TRANSACTIONS_REPLAY_REJECTED.inc();
            tracing::warn!(
                event = "transaction_rejected_replay_protection",
                tx_id = %tx.id,
                sender = %sender,
                reason = reason,
                detail = detail,
                "Transaction rejected by replay protection"
            );
        }
    }
}

async fn handle_vote(
    swarm: &mut Swarm<MyBehaviour>,
    tx_id: String,
    peer_id: String,
    propagation_source: PeerId,
    db: &Arc<DB>,
    tx_ws: &tokio::sync::broadcast::Sender<String>,
    topic: &gossipsub::IdentTopic,
) {
    tracing::info!(
        event = "gossip_vote_received",
        tx_id = %tx_id,
        voter = %peer_id,
        "Consensus Vote Received"
    );

    // Quorum Logic: Retrieve current state and add voter
    let mut voters: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Note: This would need the actual RocksDB handle
    // For now, placeholder implementation

    // Don't vote for invalid transactions
}

async fn handle_malicious_message(
    swarm: &mut Swarm<MyBehaviour>,
    sender: &str,
) {
    tracing::warn!("Malicious message detected from peer {}. Blocking IP.", sender);
    // Note: In production, you would extract the real IP from the packet/peer
    // For now, this is a placeholder for the dynamic threat detection logic
    let ip_to_block = Ipv4Addr::new(1, 2, 3, 4);
    let ip_u32 = u32::from_be_bytes(ip_to_block.octets());
    let key = Key::new(32, ip_u32);

    // This would need the eBPF map handle
    // For now, placeholder
}
