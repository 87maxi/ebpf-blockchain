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

use crate::config::node::{NodeState, NetworkMessage, Transaction, SyncRequest};
use crate::p2p::behaviour::{MyBehaviour, MyBehaviourEvent};
use crate::metrics::prometheus::*;
use crate::config::cli::get_ip_from_multiaddr;

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

    if let Ok(net_msg) = serde_json::from_slice::<NetworkMessage>(&message.data) {
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

    // Quorum Logic: Retrieve current state and add voter
    let mut voters = std::collections::HashSet::new();
    if let Ok(Some(existing)) = node_state.db.get(tx_id.as_bytes()) {
        if let Ok(existing_str) = String::from_utf8(existing.to_vec()) {
            if let Ok(existing_voters) = serde_json::from_str::<std::collections::HashSet<String>>(&existing_str) {
                voters = existing_voters;
            }
        }
    }

    if voters.insert(peer_id.clone()) {
        DB_OPERATIONS.with_label_values(&["put"]).inc();
        if let Ok(voters_json) = serde_json::to_string(&voters) {
            let _ = node_state.db.put(tx_id.as_bytes(), voters_json.as_bytes());

            // QUORUM THRESHOLD: 2/3 (or 2 for a 3-node lab)
            if voters.len() == 2 {
                TRANSACTIONS_CONFIRMED.inc();
                TRANSACTIONS_PROCESSED.inc();
                BLOCKS_PROPOSED.inc();
                CONSENSUS_ROUNDS.inc();
                tracing::info!(event = "quorum_reached", tx_id = %tx_id, "Quorum reached! Transaction confirmed.");
                let alert = serde_json::json!({
                    "event": "BlockConfirmed",
                    "tx_id": tx_id,
                    "voters": voters
                }).to_string();
                let _ = node_state.tx_ws.send(alert);
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
            if let Err(e) = blacklist.insert(&key, 1, 0) {
                tracing::warn!("Failed to block IP: {}", e);
            } else {
                tracing::info!("IP {} blocked in reactive blacklist", ip_to_block);
            }
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

    // Main event loop
    loop {
        tokio::select! {
            _ = stats_interval.tick() => {
                UPTIME.inc();
                
                // Update eBPF XDP metrics
                if let Some(map) = ebpf.map("LATENCY_STATS") {
                    if let Ok(latency_stats) = aya::maps::HashMap::<_, u32, u64>::try_from(map) {
                        let mut total_packets: u64 = 0;
                        for entry in latency_stats.iter() {
                            if let Ok((_, count)) = entry {
                                total_packets = total_packets.saturating_add(count);
                            }
                        }
                        XDP_PACKETS_PROCESSED.set(total_packets as i64);
                        
                        for i in 0..64 {
                            if let Ok(count) = latency_stats.get(&i, 0u64) {
                                LATENCY_BUCKETS.with_label_values(&[&i.to_string()]).set(count as i64);
                            }
                        }
                    }
                }
                
                // Update whitelist/blacklist sizes
                if let Some(map) = ebpf.map("NODES_BLACKLIST") {
                    if let Ok(blacklist) = aya::maps::LpmTrie::<_, u32, u32>::try_from(map) {
                        let blacklist_size = blacklist.iter().count();
                        XDP_BLACKLIST_SIZE.set(blacklist_size as i64);
                    }
                }
                if let Some(map) = ebpf.map("NODES_WHITELIST") {
                    if let Ok(whitelist) = aya::maps::LpmTrie::<_, u32, u32>::try_from(map) {
                        let whitelist_size = whitelist.iter().count();
                        XDP_WHITELIST_SIZE.set(whitelist_size as i64);
                    }
                }
                
                // Update system metrics
                crate::metrics::system::update_system_metrics();
                
                // Update peer count
                VALIDATOR_COUNT.set(PEERS_CONNECTED.with_label_values(&["connected"]).get());
            }
            Some(tx) = rx_rpc.recv() => {
                tracing::info!(event = "rpc_tx_received", tx_id = %tx.id, data = %tx.data, "Received RPC Transaction");
                let msg = NetworkMessage::TxProposal(tx);
                if let Ok(payload) = serde_json::to_vec(&msg) {
                    let payload_size = payload.len();
                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(gossipsub::IdentTopic::new("gossip"), payload) {
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
                                &gossipsub::IdentTopic::new("gossip"),
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
                            if let Some(addr) = node_state.peer_store.get_peer(peer_id) {
                                if let Some(ip) = get_ip_from_multiaddr(&addr) {
                                    if let Err(e) = node_state.sybil_protection.register_connection(peer_id, &ip) {
                                        tracing::warn!(peer_id = %peer_id, ip = %ip, error = %e, "Failed to register connection for Sybil protection");
                                    }
                                    
                                    if let Err(sybil_err) = node_state.sybil_protection.check_ip_limit(peer_id, &ip) {
                                        tracing::warn!(peer_id = %peer_id, ip = %ip, error = %sybil_err, "Sybil protection limit exceeded for peer");
                                    }
                                }
                            }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
                            PEERS_CONNECTED.with_label_values(&["connected"]).dec();
                            P2P_CONNECTIONS_CLOSED.inc();
                            tracing::info!("Decremented PEERS_CONNECTED, incremented P2P_CONNECTIONS_TOTAL. Remaining: {}", num_established);
                            
                            // SECURITY: Unregister connection for Sybil protection tracking
                            if let Some(addr) = node_state.peer_store.get_peer(peer_id) {
                                if let Some(ip) = get_ip_from_multiaddr(&addr) {
                                    if let Err(e) = node_state.sybil_protection.unregister_connection(peer_id, &ip) {
                                        tracing::debug!(peer_id = %peer_id, ip = %ip, error = %e, "Failed to unregister connection");
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
