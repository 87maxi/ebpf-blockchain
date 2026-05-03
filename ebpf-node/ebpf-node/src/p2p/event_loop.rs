use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use libp2p::{
    gossipsub,
    identify,
    mdns,
    request_response,
    Swarm,
    swarm::SwarmEvent,
};
use libp2p::futures::StreamExt;
use tokio::sync::mpsc;
use aya::maps::lpm_trie::Key;

use crate::config::node::{NodeState, NetworkMessage, Transaction, SyncRequest, Vote, SignedVote, SlashingEvent, CHECKPOINT_INTERVAL, get_current_timestamp};
use ed25519_dalek::Verifier;
use crate::p2p::behaviour::{MyBehaviour, MyBehaviourEvent};
use crate::p2p::swarm::GOSSIPSUB_TOPIC;
use crate::metrics::prometheus::*;
use crate::config::cli::get_ip_from_multiaddr;

use ed25519_dalek::{VerifyingKey, SIGNATURE_LENGTH, Signer, Signature};
use tracing::{info, warn, error};

/// CHANGE 8: Timeout for each consensus round in seconds
/// If a transaction doesn't receive enough votes within this time, the round is reset
pub const CONSENSUS_ROUND_TIMEOUT_SECS: u64 = 30;

/// Handle gossip messages (extracted from event loop for clarity)
async fn handle_gossip_message_inner(
    swarm: &mut Swarm<MyBehaviour>,
    message: gossipsub::Message,
    propagation_source: libp2p::PeerId,
    node_state: &NodeState,
    topic: &gossipsub::IdentTopic,
    ebpf: &mut aya::Ebpf,
) {
    let sender = propagation_source.to_string();
    MESSAGES_RECEIVED.with_label_values(&["gossip"]).inc();
    BANDWIDTH_RECEIVED.inc_by(message.data.len() as u64);
    PACKETS_TRACE.with_label_values(&[&sender, "gossip"]).inc();

    // P1-3: Try to parse as SignedVote first (new format with Ed25519 verification)
    if let Ok(signed_vote) = serde_json::from_slice::<SignedVote>(&message.data) {
        handle_signed_vote(
            swarm,
            signed_vote,
            propagation_source,
            node_state,
            topic,
        ).await;
    } else if let Ok(net_msg) = serde_json::from_slice::<NetworkMessage>(&message.data) {
        match net_msg {
            NetworkMessage::TxProposal(tx) => {
                let sender_clone = sender.clone();
                handle_tx_proposal(
                    swarm,
                    tx,
                    &sender_clone,
                    propagation_source,
                    node_state,
                    topic,
                ).await;
            }
            NetworkMessage::Vote { tx_id, peer_id } => {
                handle_vote(
                    swarm,
                    tx_id,
                    peer_id,
                    propagation_source,
                    node_state,
                    topic,
                ).await;
            }
        }
    } else if message.data.starts_with(b"ATTACK") {
        handle_malicious_message(swarm, ebpf, &sender).await;
    }
}

/// P1-3: Handle signed vote with Ed25519 signature verification
async fn handle_signed_vote(
    swarm: &mut Swarm<MyBehaviour>,
    signed_vote: SignedVote,
    propagation_source: libp2p::PeerId,
    node_state: &NodeState,
    topic: &gossipsub::IdentTopic,
) {
    let voter_id = signed_vote.vote.voter_id.clone();
    let tx_id = signed_vote.vote.tx_id.clone();

    tracing::info!(
        event = "signed_vote_received",
        tx_id = %tx_id,
        voter = %voter_id,
        "Signed Vote Received with Ed25519 signature"
    );

    // P1-3: Verify Ed25519 signature
    // Get the verifying key for the voter from peer store
    // For now, we use a simplified approach: store verifying keys in DB
    let vk_key = format!("verifying_key:{}", voter_id);
    
    let verification_result = (|| -> Result<(), String> {
        // Retrieve the voter's verifying key from DB
        let vk_bytes = node_state.db.get(vk_key.as_bytes())
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or_else(|| format!("Verifying key not found for voter {}", voter_id))?;
        
        if vk_bytes.len() != 32 {
            return Err("Invalid verifying key size".to_string());
        }
        
        let mut vk_arr = [0u8; 32];
        vk_arr.copy_from_slice(&vk_bytes[..32]);
        
        let verifying_key = VerifyingKey::from_bytes(&vk_arr)
            .map_err(|e| format!("Invalid verifying key: {}", e))?;
        
        // Verify the signature
        verifying_key.verify_strict(
            &signed_vote.vote.to_bytes(),
            &signed_vote.signature
        ).map_err(|e| {
            VOTE_VALIDATION_FAILURES.with_label_values(&["invalid_signature"]).inc();
            format!("Signature verification failed: {}", e)
        })?;
        
        Ok(())
    })();
    
    match verification_result {
        Ok(()) => {
            tracing::info!(
                event = "signature_verified",
                voter = %voter_id,
                tx_id = %tx_id,
                "Ed25519 signature verified successfully"
            );
            
            // Forward to regular vote handling after signature verification
            handle_vote(
                swarm,
                tx_id,
                voter_id,
                propagation_source,
                node_state,
                topic,
            ).await;
        }
        Err(e) => {
            warn!(
                event = "signature_verification_failed",
                voter = %voter_id,
                tx_id = %tx_id,
                error = %e,
                "Rejected vote: signature verification failed"
            );
            TRANSACTIONS_REJECTED.inc();
            TRANSACTION_FAILURES.inc();
        }
    }
}

async fn handle_tx_proposal(
    swarm: &mut Swarm<MyBehaviour>,
    tx: Transaction,
    sender: &str,
    _propagation_source: libp2p::PeerId,
    node_state: &NodeState,
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
        if node_state.replay_protection.is_processed(&tx.id) {
            return Err(("duplicate_tx".to_string(), "Transaction ID already processed".to_string()));
        }

        // 3. Validate nonce is incremental
        let next_nonce = node_state.replay_protection.validate_nonce(sender, tx.nonce)
            .map_err(|e| ("invalid_nonce".to_string(), e))?;

        // Transaction passed all validation
        Ok(next_nonce)
    })();

    match validation_result {
        Ok(next_nonce) => {
            // Transaction is valid - record nonce and mark as processed
            if let Err(e) = node_state.replay_protection.update_nonce(sender, next_nonce) {
                tracing::warn!(event = "nonce_update_failed", sender = %sender, error = %e, "Failed to update nonce after valid transaction");
            }
            if let Err(e) = node_state.replay_protection.mark_processed(&tx.id, tx.timestamp) {
                tracing::warn!(event = "process_mark_failed", tx_id = %tx.id, error = %e, "Failed to mark transaction as processed");
            }

            // Solana-like Consensus: Validate & Vote via Gossip
            // TAREA 2.3 & 4: Create and sign the vote with real Ed25519 signature
            let vote = Vote::new(
                tx.id.clone(),
                swarm.local_peer_id().to_string(),
                swarm.local_peer_id().to_string(),
            );
            // Sign the vote using the node's signing key
            let signature = node_state.signing_key.lock().unwrap().as_ref()
                .map(|sk| sk.sign(vote.to_bytes().as_slice()))
                .unwrap_or_else(|| ed25519_dalek::Signature::from_bytes(&[0u8; 64]));
            let signed_vote = SignedVote {
                vote: vote.clone(),
                signature,
            };
            
            // Publish using the namespaced gossip topic
            if let Ok(payload) = serde_json::to_vec(&signed_vote) {
                let payload_size = payload.len();
                if let Ok(_) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), payload) {
                    MESSAGES_SENT.inc();
                    MESSAGES_SENT_BY_TYPE.with_label_values(&["vote"]).inc();
                    BANDWIDTH_SENT.inc_by(payload_size as u64);
                }
            }
            
            // Also send the raw vote format for backward compatibility
            let vote_msg = NetworkMessage::Vote {
                tx_id: tx.id.clone(),
                peer_id: swarm.local_peer_id().to_string()
            };
            if let Ok(payload) = serde_json::to_vec(&vote_msg) {
                let _ = swarm.behaviour_mut().gossipsub.publish(topic.clone(), payload);
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

/// Get current block height from DB
fn get_current_height(node_state: &NodeState) -> u64 {
    node_state.db.get(b"latest_height".as_ref())
        .ok().flatten()
        .and_then(|v| bincode::deserialize(&v).ok())
        .unwrap_or(0)
}

/// CHANGE 8: Helper to check and handle consensus round timeout
/// Resets vote tracking state if the round has timed out
fn check_consensus_timeout(
    node_state: &NodeState,
    tx_id: &str,
) -> bool {
    // Check if there's an active timeout marker for this tx_id
    let timeout_key = format!("consensus_timeout:{}", tx_id);
    if let Ok(Some(timeout_bytes)) = node_state.db.get(timeout_key.as_bytes()) {
        if let Ok(timeout_arr) = <[u8; 8]>::try_from(timeout_bytes.to_vec()) {
            let timeout_timestamp = u64::from_be_bytes(timeout_arr);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            if now.saturating_sub(timeout_timestamp) > CONSENSUS_ROUND_TIMEOUT_SECS {
                info!(
                    event = "consensus_round_timeout",
                    tx_id = %tx_id,
                    timeout_secs = CONSENSUS_ROUND_TIMEOUT_SECS,
                    "Consensus round timed out, resetting vote tracking"
                );
                
                // Clean up stale vote data for this tx_id
                let iter = node_state.db.iterator(rocksdb::IteratorMode::Start);
                let keys_to_delete: Vec<Vec<u8>> = iter
                    .filter_map(|item| {
                        item.ok().and_then(|(k, _)| {
                            if let Ok(key_str) = String::from_utf8(k.to_vec()) {
                                if key_str.starts_with(&format!("votes:{}:", tx_id)) {
                                    Some(k.to_vec())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                
                // Use reference to avoid moving keys_to_delete
                for key in &keys_to_delete {
                    let _ = node_state.db.delete(key);
                }
                
                // Remove timeout marker
                let _ = node_state.db.delete(timeout_key.as_bytes());
                
                return true; // Timeout occurred
            }
        }
    }
    false
}

async fn handle_vote(
    swarm: &mut Swarm<MyBehaviour>,
    tx_id: String,
    peer_id: String,
    _propagation_source: libp2p::PeerId,
    node_state: &NodeState,
    _topic: &gossipsub::IdentTopic,
) {
    tracing::info!(
        event = "gossip_vote_received",
        tx_id = %tx_id,
        voter = %peer_id,
        "Consensus Vote Received"
    );

    // CHANGE 8: Check for consensus round timeout
    if check_consensus_timeout(node_state, &tx_id) {
        tracing::info!(
            event = "consensus_round_reset",
            tx_id = %tx_id,
            "Consensus round reset due to timeout, processing vote"
        );
    }
    
    // Set timeout marker for this consensus round if not already set
    {
        let timeout_key = format!("consensus_timeout:{}", tx_id);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // Only set if not already set (non-overwrite)
        if node_state.db.get(timeout_key.as_bytes()).ok().flatten().is_none() {
            let _ = node_state.db.put(&timeout_key, now.to_be_bytes());
        }
    }

    // TAREA 2.4: Check if voter is slashed
    if node_state.is_slashed(&peer_id) {
        tracing::warn!(
            event = "vote_from_slashed_validator",
            tx_id = %tx_id,
            voter = %peer_id,
            "Vote rejected: validator is slashed"
        );
        return;
    }

    // Quorum Logic: Retrieve current state and add voter
    // Use dynamic quorum based on 2/3 majority formula - calculated from actual validator set
    let total_validators = node_state.validator_peers.lock().unwrap().len();
    if total_validators == 0 {
        warn!("No validators registered, consensus cannot proceed");
        return; // Skip vote processing
    }
    let quorum_threshold = (total_validators * 2 + 2) / 3; // 2/3 majority with rounding up

    let current_height = get_current_height(node_state);

    // TAREA 2.4: Double-voting detection - check if this voter already voted for this tx_id
    let voter_key = format!("votes:{}:{}", peer_id, tx_id);
    if node_state.db.get(voter_key.as_bytes()).ok().flatten().is_some() {
        warn!("Double voting detected from validator {}", peer_id);
        let slashing_event = SlashingEvent {
            validator_id: peer_id.clone(),
            block_height: current_height,
            reason: "double_voting".to_string(),
            timestamp: get_current_timestamp(),
            evidence: format!("voter_key={}", voter_key),
        };
        if let Err(e) = node_state.record_slashing_event(slashing_event) {
            warn!("Failed to record slashing event: {}", e);
        }
        TRANSACTIONS_REJECTED.inc();
        TRANSACTION_FAILURES.inc();
        // P2-1: Increment double vote detection metric
        DOUBLE_VOTE_ATTEMPTS.inc();
        VOTE_VALIDATION_FAILURES.with_label_values(&["duplicate"]).inc();
        return;
    }

    let mut voters = std::collections::HashSet::new();
    if let Ok(Some(existing)) = node_state.db.get(tx_id.as_bytes()) {
        if let Ok(existing_str) = String::from_utf8(existing.to_vec()) {
            if let Ok(existing_voters) = serde_json::from_str::<std::collections::HashSet<String>>(&existing_str) {
                voters = existing_voters;
            }
        }
    }

    if voters.insert(peer_id.clone()) {
        // Record this vote to detect double-voting
        let _ = node_state.db.put(voter_key.as_bytes(), b"voted");
        
        DB_OPERATIONS.with_label_values(&["put"]).inc();
        if let Ok(voters_json) = serde_json::to_string(&voters) {
            let _ = node_state.db.put(tx_id.as_bytes(), voters_json.as_bytes());

            // Update validator count metric with actual total
            VALIDATOR_COUNT.set(total_validators as i64);

            // Dynamic quorum threshold: 2/3 majority with rounding up
            if voters.len() >= quorum_threshold {
                // P0-1: Measure CONSENSUS_DURATION from timeout marker
                {
                    let timeout_key = format!("consensus_timeout:{}", tx_id);
                    if let Ok(Some(timeout_bytes)) = node_state.db.get(timeout_key.as_bytes()) {
                        if let Ok(timeout_arr) = <[u8; 8]>::try_from(timeout_bytes.to_vec()) {
                            let start_timestamp = u64::from_be_bytes(timeout_arr);
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            let duration_ms = (now.saturating_sub(start_timestamp)) * 1000;
                            CONSENSUS_DURATION.set(duration_ms as i64);
                            
                            // P4-1: Record consensus latency histogram
                            CONSENSUS_LATENCY_MS.with_label_values(&["default", "finality"])
                                .observe(duration_ms as f64);
                        }
                    }
                    // Clear timeout marker on successful consensus
                    let _ = node_state.db.delete(timeout_key.as_bytes());
                }
                
                TRANSACTIONS_CONFIRMED.inc();
                TRANSACTIONS_PROCESSED.inc();
                
                // TAREA 2.2 & 2.7: Create real block and checkpoint
                let confirmed_txs: Vec<String> = voters.iter().cloned().collect();
                
                if let Ok(block) = node_state.create_block(confirmed_txs) {
                    BLOCKS_PROPOSED.inc();
                    CONSENSUS_ROUNDS.inc();
                    FINALITY_CHECKPOINTS.inc();
                    info!("Block created: height={}, hash={}", block.height, block.hash);
                    
                    // TAREA 2.7: Create checkpoint at intervals
                    if block.height % CHECKPOINT_INTERVAL == 0 {
                        if let Ok(checkpoint) = node_state.create_checkpoint(&block) {
                            info!("Checkpoint finalized at height {}", checkpoint.height);
                        }
                    }
                } else {
                    BLOCKS_PROPOSED.inc();
                    CONSENSUS_ROUNDS.inc();
                }
                
                tracing::info!(
                    event = "quorum_reached",
                    tx_id = %tx_id,
                    voters = %voters.len(),
                    threshold = %quorum_threshold,
                    total_validators = %total_validators,
                    "Quorum reached! Transaction confirmed."
                );
                let alert = serde_json::json!({
                    "event": "BlockConfirmed",
                    "tx_id": tx_id,
                    "voters": voters,
                    "quorum_threshold": quorum_threshold,
                    "total_validators": total_validators
                }).to_string();
                let _ = node_state.tx_ws.send(alert);
            } else {
                tracing::debug!(
                    event = "vote_recorded",
                    tx_id = %tx_id,
                    voters = %voters.len(),
                    threshold = %quorum_threshold,
                    "Vote recorded, waiting for quorum"
                );
            }
        }
    } else {
        // Duplicate vote - possible replay attack
        TRANSACTIONS_REJECTED.inc();
        TRANSACTION_FAILURES.inc();
        tracing::warn!(event = "duplicate_vote_rejected", tx_id = %tx_id, voter = %peer_id, "Duplicate vote rejected - possible replay attack");
    }
}

async fn handle_malicious_message(
    _swarm: &mut Swarm<MyBehaviour>,
    ebpf: &mut aya::Ebpf,
    sender: &str,
) {
    tracing::warn!("Malicious message detected from peer {}. Blocking IP.", sender);
    let ip_to_block = Ipv4Addr::new(1, 2, 3, 4);
    let ip_u32 = u32::from_be_bytes(ip_to_block.octets());
    let key = Key::new(32, ip_u32);

    if let Some(map) = ebpf.map_mut("NODES_BLACKLIST") {
        if let Ok(mut blacklist) = aya::maps::LpmTrie::<_, u32, u32>::try_from(map) {
            if let Err(e) = blacklist.insert(&key, &1u32, 0) {
                tracing::warn!("Failed to block IP: {}", e);
            } else {
                tracing::info!("IP {} blocked in reactive blacklist", ip_to_block);
            }
        } else {
            tracing::error!("Failed to convert NODES_BLACKLIST to LpmTrie");
        }
    }
}

/// Run the main P2P event loop
pub async fn run(
    mut swarm: Swarm<MyBehaviour>,
    node_state: Arc<NodeState>,
    mut rx_rpc: mpsc::Receiver<Transaction>,
    ebpf: &mut aya::Ebpf,
) -> anyhow::Result<()> {
    let mut stats_interval = tokio::time::interval(Duration::from_secs(10));
    let mut peer_save_interval = tokio::time::interval(Duration::from_secs(60));
    let mut replay_cleanup_interval = tokio::time::interval(Duration::from_secs(3600)); // Every hour
    let mut sync_interval = tokio::time::interval(Duration::from_secs(30)); // TAREA 2.6: Periodic sync every 30 seconds
    let mut consensus_cleanup_interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes

    // Main event loop
    loop {
        tokio::select! {
            _ = stats_interval.tick() => {
                UPTIME.inc();
                
                // P0-1: Update eBPF XDP metrics using maps module
                {
                    let mut ebpf_maps = crate::ebpf::maps::EbpfMaps::new(ebpf);
                    let processed = ebpf_maps.total_packets_processed();
                    XDP_PACKETS_PROCESSED.set(processed);
                    
                    // P0-1: Update XDP_PACKETS_DROPPED from eBPF maps
                    let dropped = ebpf_maps.dropped_packets_count();
                    if dropped >= 0 {
                        XDP_PACKETS_DROPPED.set(dropped);
                    }
                    
                    // Update latency buckets
                    if let Ok(latency_stats) = ebpf_maps.latency_stats() {
                        for i in 0..64 {
                            if let Ok(count) = latency_stats.get(&i, 0) {
                                LATENCY_BUCKETS.with_label_values(&[&i.to_string()]).set(count as i64);
                            }
                        }
                    }
                    
                    // Update whitelist/blacklist sizes
                    if let Ok(bl_size) = ebpf_maps.blacklist_size() {
                        XDP_BLACKLIST_SIZE.set(bl_size as i64);
                    }
                    if let Ok(wl_size) = ebpf_maps.whitelist_size() {
                        XDP_WHITELIST_SIZE.set(wl_size as i64);
                    }
                }
                
                // P0-1: Update TRANSACTION_QUEUE_SIZE from pending votes in DB
                {
                    let iter = node_state.db.iterator(rocksdb::IteratorMode::Start);
                    let mut pending_count: i64 = 0;
                    for item in iter.take(1000) {
                        if let Ok((key, _)) = item {
                            if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                                // Count keys that represent pending consensus rounds
                                if key_str.starts_with("consensus_timeout:") {
                                    pending_count += 1;
                                }
                            }
                        }
                    }
                    TRANSACTION_QUEUE_SIZE.set(pending_count);
                }
                
                // Update system metrics
                crate::metrics::system::update_system_metrics();
                
                // Update peer count
                VALIDATOR_COUNT.set(PEERS_CONNECTED.with_label_values(&["connected"]).get());
                
                // P2-2: Eclipse attack detection - periodic check
                {
                    let (risk_score, prefixes, peers) = node_state.eclipse_protection.calculate_risk_score();
                    if risk_score > 70.0 {
                        tracing::warn!(
                            event = "eclipse_warning",
                            risk_score,
                            unique_prefixes = prefixes,
                            connected_peers = peers,
                            "High eclipse attack risk - limited peer diversity detected"
                        );
                    }
                }
                
                // P2-3: Update global threat score
                {
                    let sybil_attempts = SYBIL_ATTEMPTS_DETECTED.get();
                    let slashing_count = SLASHING_EVENTS.get();
                    let replay_rejections = TRANSACTIONS_REPLAY_REJECTED.get();
                    let blacklist_sz = XDP_BLACKLIST_SIZE.get();
                    let double_votes = DOUBLE_VOTE_ATTEMPTS.get();
                    
                    let threat: f64 = (
                        (sybil_attempts as f64 * 15.0) +
                        (slashing_count as f64 * 25.0) +
                        (replay_rejections as f64 * 10.0) +
                        ((blacklist_sz as f64) / 10.0) +
                        (double_votes as f64 * 20.0)
                    ).min(100.0);
                    
                    SECURITY_THREAT_SCORE.with_label_values(&["default"]).set(threat);
                    BLACKLIST_SIZE.with_label_values(&["default"]).set(blacklist_sz as f64);
                }
            }
            Some(tx) = rx_rpc.recv() => {
                tracing::info!(event = "rpc_tx_received", tx_id = %tx.id, data = %tx.data, "Received RPC Transaction");
                let msg = NetworkMessage::TxProposal(tx);
                if let Ok(payload) = serde_json::to_vec(&msg) {
                    let payload_size = payload.len();
                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC), payload) {
                        tracing::warn!("Failed to publish via RPC: {:?}", e);
                    } else {
                        MESSAGES_SENT.inc();
                        MESSAGES_SENT_BY_TYPE.with_label_values(&["tx"]).inc();
                        BANDWIDTH_SENT.inc_by(payload_size as u64);
                        TRANSACTIONS_PROCESSED.inc();
                        TRANSACTIONS_BY_TYPE.with_label_values(&["transfer"]).inc();
                    }
                }
            }
            event = tokio::time::sleep(Duration::from_millis(100)) => {
                // Poll the swarm manually
                match swarm.next().await {
                    Some(event) => match event {
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                            propagation_source,
                            message_id: _,
                            message,
                        })) => {
                            handle_gossip_message(
                                &mut swarm,
                                message,
                                propagation_source,
                                &node_state,
                                &gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC),
                                ebpf,
                            ).await;
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            tracing::info!("Listening on {:?}", address);
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            tracing::info!(event = "p2p_connected", peer = %peer_id, "New peer connected");
                            PEERS_CONNECTED.with_label_values(&["connected"]).inc();
                            P2P_CONNECTIONS_TOTAL.inc();
                            
                            // SECURITY: Sybil Protection - Register connection with known peer store addresses
                            // P2-2: Eclipse Protection - Register peer IP for eclipse detection
                            if let Some(addr) = node_state.peer_store.get_peer(peer_id) {
                                if let Some(ip) = get_ip_from_multiaddr(&addr) {
                                    if let Err(e) = node_state.sybil_protection.register_connection(peer_id, &ip) {
                                        tracing::warn!(peer_id = %peer_id, ip = %ip, error = %e, "Failed to register connection for Sybil protection");
                                    }
                                    
                                    if let Err(sybil_err) = node_state.sybil_protection.check_ip_limit(peer_id, &ip) {
                                        tracing::warn!(peer_id = %peer_id, ip = %ip, error = %sybil_err, "Sybil protection limit exceeded for peer");
                                    }

                                    // P2-2: Register with eclipse protection
                                    if let Err(e) = node_state.eclipse_protection.register_peer(peer_id, &ip.to_string()) {
                                        tracing::debug!(peer_id = %peer_id, ip = %ip, error = %e, "Failed to register peer for eclipse detection");
                                    }
                                }
                            }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
                            PEERS_CONNECTED.with_label_values(&["connected"]).dec();
                            P2P_CONNECTIONS_CLOSED.inc();
                            tracing::info!("Decremented PEERS_CONNECTED, incremented P2P_CONNECTIONS_TOTAL. Remaining: {}", num_established);
                            
                            // SECURITY: Unregister connection for Sybil protection tracking
                            // P2-2: Eclipse Protection - Unregister peer IP
                            if let Some(addr) = node_state.peer_store.get_peer(peer_id) {
                                if let Some(ip) = get_ip_from_multiaddr(&addr) {
                                    if let Err(e) = node_state.sybil_protection.unregister_connection(peer_id, &ip) {
                                        tracing::debug!(peer_id = %peer_id, ip = %ip, error = %e, "Failed to unregister connection");
                                    }

                                    // P2-2: Unregister from eclipse protection
                                    if let Err(e) = node_state.eclipse_protection.unregister_peer(peer_id) {
                                        tracing::debug!(peer_id = %peer_id, error = %e, "Failed to unregister peer from eclipse detection");
                                    }
                                }
                            }
                        }
                        SwarmEvent::IncomingConnection { send_back_addr, .. } => {
                            if let Some(ip) = get_ip_from_multiaddr(&send_back_addr) {
                                tracing::debug!("Incoming connection from IP: {}", ip);
                            }
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                            for (peer_id, multiaddr) in list {
                                tracing::info!("mDNS discovered a new peer: {} at {}", peer_id, multiaddr);
                                
                                // Save discovered peer
                                let _ = node_state.peer_store.save_peer(peer_id, &multiaddr);
                                
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                if let Err(e) = swarm.dial(multiaddr.clone()) {
                                    tracing::warn!("Failed to dial mDNS discovered peer {}: {}", peer_id, e);
                                }
                            }
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                            for (peer_id, multiaddr) in list {
                                tracing::info!("mDNS discovered peer has expired: {} at {}", peer_id, multiaddr);
                                swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                            }
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. })) => {
                            tracing::info!(event = "p2p_identified", peer = %peer_id, agent = %info.agent_version, "Peer identified");
                            
                            // Save identified peer with its addresses
                            PEERS_IDENTIFIED.inc();
                            for addr in info.listen_addrs {
                                let _ = node_state.peer_store.save_peer(peer_id, &addr);
                            }
                            
                            // Trigger historical sync when a peer is identified
                            tracing::info!(event = "sync_request_sent", target = %peer_id, "Requesting historical sync");
                            swarm.behaviour_mut().sync.send_request(&peer_id, SyncRequest);
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad_event)) => {
                            // TAREA 2.5: Handle Kademlia DHT events
                            swarm.behaviour_mut().on_kad_event(kad_event);
                        }
                        SwarmEvent::Behaviour(MyBehaviourEvent::Sync(request_response::Event::Message {
                            peer,
                            message,
                            ..
                        })) => {
                            crate::p2p::sync::handle_sync_message(
                                &mut swarm,
                                peer,
                                message,
                                &node_state.db,
                                &node_state.tx_ws,
                            ).await;
                        }
                        _ => {}
                    },
                    None => {
                        tracing::warn!("Swarm stream ended unexpectedly");
                        break;
                    }
                }
            }
            _ = peer_save_interval.tick() => {
                // Periodically save connected peers (addresses tracked via identify behavior)
                let connected_peers: Vec<_> = swarm.connected_peers().cloned().collect();
                PEERS_SAVED.inc_by(connected_peers.len() as u64);
            }
            _ = sync_interval.tick() => {
                // TAREA 2.6: Periodic sync - request data from all connected peers
                tracing::info!("Periodic sync triggered");
                let connected_peers: Vec<_> = swarm.connected_peers().cloned().collect();
                for peer in connected_peers {
                    let _ = swarm.behaviour_mut().sync.send_request(&peer, SyncRequest);
                }
            }
            _ = consensus_cleanup_interval.tick() => {
                // CHANGE 8: Periodically clean up stale consensus timeout markers
                tracing::info!("Cleaning up stale consensus timeout markers...");
                let iter = node_state.db.iterator(rocksdb::IteratorMode::Start);
                let keys_to_delete: Vec<Vec<u8>> = iter
                    .filter_map(|item| {
                        item.ok().and_then(|(k, v)| {
                            if let Ok(key_str) = String::from_utf8(k.to_vec()) {
                                if key_str.starts_with("consensus_timeout:") {
                                    // Check if timeout has expired
                                    if let Ok(timeout_bytes) = <[u8; 8]>::try_from(v.to_vec()) {
                                        let timeout_timestamp = u64::from_be_bytes(timeout_bytes);
                                        let now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        
                                        if now.saturating_sub(timeout_timestamp) > CONSENSUS_ROUND_TIMEOUT_SECS {
                                            return Some(k.to_vec());
                                        }
                                    }
                                }
                            }
                            None
                        })
                    })
                    .collect();
                
                // Use reference to avoid moving keys_to_delete
                for key in &keys_to_delete {
                    let _ = node_state.db.delete(key);
                }
                if !keys_to_delete.is_empty() {
                    info!("Cleaned up {} stale consensus timeout markers", keys_to_delete.len());
                }
            }
            _ = replay_cleanup_interval.tick() => {
                // Periodically clean up old processed transactions to prevent unbounded DB growth
                tracing::info!("Cleaning up old processed transactions...");
                node_state.replay_protection.cleanup_old_processed(86400); // 24 hours
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Exiting...");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_gossip_message(
    swarm: &mut Swarm<MyBehaviour>,
    message: gossipsub::Message,
    propagation_source: libp2p::PeerId,
    node_state: &NodeState,
    topic: &gossipsub::IdentTopic,
    ebpf: &mut aya::Ebpf,
) {
    handle_gossip_message_inner(swarm, message, propagation_source, node_state, topic, ebpf).await;
}
