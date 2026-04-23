mod config;
mod db;
mod ebpf;
mod api;
mod p2p;
mod security;
mod metrics;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use config::cli::{Opt, load_saved_peers, save_peers, get_bootstrap_peers_from_env, get_ip_from_multiaddr};
use config::node::{NodeState, NodeConfig};
use clap::Parser;
use db::rocksdb::init_db;
use db::backup::schedule_backups;
use ebpf::loader::{load_binary, load};
use ebpf::programs::attach_all;
use ebpf::hot_reload::EbpfHotReloadManager;
use metrics::prometheus::{initialize_metrics, PEERS_CONNECTED, PEERS_IDENTIFIED, PEERS_SAVED, BLOCKS_PROPOSED, KPROBE_HIT_COUNT, HOT_RELOAD_SUCCESS_TOTAL, HOT_RELOAD_FAILURE_TOTAL, SWARM_DIAL_ERRORS_TOTAL, ROCKSDB_WRITE_RATE_BYTES_TOTAL, ROCKSDB_DB_SIZE_BYTES, API_REQUEST_DURATION, API_REQUESTS_TOTAL, RINGBUF_BUFFER_UTILIZATION};
use p2p::behaviour::MyBehaviour;
use p2p::swarm::{create_swarm, create_gossipsub, setup_listening_and_subscription};
use security::peer_store::PeerStore;
use security::replay::ReplayProtection;
use security::sybil::SybilProtection;
use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn, debug, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let opt = Opt::parse();
    
    // Setup structured logging
    setup_structured_logging();
    
    info!(event = "node_startup", iface = %opt.iface, "eBPF Node starting...");
    
    // Initialize metrics
    initialize_metrics();
    
    // Verify metrics registration
    info!("Verifying metrics registration...");
    let test_gauge = metrics::prometheus::LATENCY_BUCKETS.with_label_values(&["0"]);
    test_gauge.set(1);
    info!("Metrics registration verified");
    
    // Set memory limit for eBPF
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("remove limit on locked memory failed, ret is: {ret}");
    }
    
    // Initialize database
    let db = init_db()?;
    
    // Schedule periodic backups
    let db_path = db::rocksdb::get_data_dir();
    schedule_backups(db_path.clone());
    
    // Initialize channels
    let (tx_rpc, rx_rpc) = mpsc::channel::<config::node::Transaction>(100);
    let (tx_ws, _rx_ws) = broadcast::channel::<String>(100);
    
    // Load eBPF program
    let mut ebpf = load_binary()?;
    
    // Attach all eBPF programs (XDP + KProbes)
    load(&mut ebpf, &opt.iface)?;
    
    // Initialize hot-reload manager for eBPF programs
    let hot_reload_manager = Arc::new(EbpfHotReloadManager::new(opt.iface.clone()));
    hot_reload_manager.init().await?;
    
    // Initialize identity keypair
    const ED25519_PRIVATE_KEY_SIZE: usize = 64;
    let keypair = libp2p::identity::ed25519::Keypair::generate();
    let persistent_keypath = format!("{}/identity.key", db_path);
    
    let keypair = if let Ok(bytes) = std::fs::read(&persistent_keypath) {
        if bytes.len() == ED25519_PRIVATE_KEY_SIZE {
            if let Ok(sk) = libp2p::identity::ed25519::SecretKey::try_from_bytes(bytes) {
                info!("Loaded persistent identity key from {}", persistent_keypath);
                libp2p::identity::ed25519::Keypair::from(sk)
            } else {
                info!("Failed to load persistent key, generating new one");
                keypair
            }
        } else {
            info!("Invalid key file size (expected {}, got {})", ED25519_PRIVATE_KEY_SIZE, bytes.len());
            keypair
        }
    } else {
        info!("Generating new identity keypair");
        keypair
    };
    
    // Save persistent key for future use
    let key_bytes = keypair.to_bytes();
    if key_bytes.len() == ED25519_PRIVATE_KEY_SIZE {
        if let Err(e) = std::fs::write(&persistent_keypath, &key_bytes) {
            warn!("Failed to save identity key: {}", e);
        }
    }
    
    // Create security modules
    let peer_store = PeerStore::new(db.clone());
    let replay_protection = ReplayProtection::new(db.clone());
    let sybil_protection = SybilProtection::new(db.clone(), 3); // Max 3 connections per IP
    
    // Create gossipsub behaviour
    let gossipsub = create_gossipsub(keypair.clone().into())?;
    
    // Create swarm
    let mut swarm = create_swarm(keypair.clone().into(), gossipsub)?;
    
    // Setup listening addresses
    let mut listen_addrs = vec![
        "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
        "/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap(),
    ];
    
    if !opt.listen_addresses.is_empty() {
        listen_addrs = opt.listen_addresses;
    }
    
    let topic = setup_listening_and_subscription(&mut swarm, listen_addrs)?;
    info!("Local Peer ID: {}", swarm.local_peer_id());
    let _ = std::fs::write("/tmp/peer_id.txt", swarm.local_peer_id().to_string());
    
    // =============================================================================
    // CHANGE 5: Initialize Genesis Block if database is empty
    // =============================================================================
    // Moved after swarm creation to have access to local_peer_id for genesis proposer
    {
        let local_peer_id_for_genesis = swarm.local_peer_id().to_string();
        let latest_height: Option<u64> = db.get(b"latest_height".as_ref())
            .ok().flatten()
            .and_then(|v| bincode::deserialize(&v).ok());
            
        if latest_height.is_none() || latest_height.unwrap() == 0 {
            info!("Initializing genesis block...");
            let genesis_timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let genesis_block = crate::config::node::Block {
                height: 0,
                hash: String::new(), // Will be computed
                parent_hash: "0x0".to_string(),
                proposer: local_peer_id_for_genesis.clone(),
                timestamp: genesis_timestamp,
                transactions: Vec::new(),
                quorum_votes: 0,
                total_validators: 0,
            };
            
            // Compute hash for genesis block
            let genesis_hash = genesis_block.compute_hash();
            let genesis_block = crate::config::node::Block {
                hash: genesis_hash.clone(),
                ..genesis_block
            };
            
            let genesis_key = format!("block:0");
            db.put(genesis_key.as_bytes(), bincode::serialize(&genesis_block).unwrap()).unwrap();
            db.put(b"latest_height".as_ref(), bincode::serialize(&0u64).unwrap()).unwrap();
            info!("Genesis block initialized: hash={}", genesis_hash);
        }
    }
    
    // =============================================================================
    // TAREA 4.7: Initialize whitelist with local peer_id as trusted peer
    // =============================================================================
    {
        let local_peer_id_str = swarm.local_peer_id().to_string();
        if let Ok(peer_id) = local_peer_id_str.parse::<libp2p::identity::PeerId>() {
            if sybil_protection.get_whitelisted_peer_count() == 0 {
                match sybil_protection.add_to_whitelist(peer_id) {
                    Ok(_) => {
                        info!("Local peer added to whitelist: {}", peer_id);
                    }
                    Err(e) => {
                        warn!("Failed to add local peer to whitelist: {}", e);
                    }
                }
            }
        }
    }
    
    // Load bootstrap peers
    let mut bootstrap_peers = opt.bootstrap_peers.clone();
    let env_peers = get_bootstrap_peers_from_env();
    for peer in &env_peers {
        if !bootstrap_peers.contains(peer) {
            bootstrap_peers.push(peer.clone());
        }
    }
    
    // =============================================================================
    // CHANGE 7: Populate Whitelist with Bootstrap Peer IDs
    // =============================================================================
    let bootstrap_peer_ids: Vec<String> = bootstrap_peers.iter()
        .filter_map(|addr| {
            for protocol in addr.iter() {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = protocol {
                    return Some(peer_id.to_string());
                }
            }
            None
        })
        .collect();
    
    if !bootstrap_peer_ids.is_empty() {
        if let Err(e) = sybil_protection.init_whitelist(bootstrap_peer_ids.clone()) {
            warn!("Failed to initialize whitelist: {}", e);
        } else {
            info!("Whitelist initialized with {} bootstrap peers", bootstrap_peer_ids.len());
        }
    }
    
    // Load saved peers
    let saved_peers = peer_store.all_peers();
    info!("Loaded {} saved peers from peer store", saved_peers.len());
    
    // Try to reconnect to saved peers
    for (peer_id, addr) in &saved_peers {
        info!("Trying to reconnect to saved peer {} at {}", peer_id, addr);
    }
    
    // =============================================================================
    // CHANGE 6: Kademlia Bootstrap - Populate Routing Table with Bootstrap Peers
    // =============================================================================
    // Add bootstrap peers to Kademlia routing table before dialing
    for addr in &bootstrap_peers {
        if let Some(peer_id) = addr.iter().find_map(|p| {
            if let libp2p::multiaddr::Protocol::P2p(id) = p {
                Some(id)
            } else {
                None
            }
        }) {
            swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
        }
    }
    info!("Kademlia routing table populated with {} bootstrap peers", bootstrap_peers.len());
    
    // Bootstrap with retry logic
    let connection_retries = opt.connection_retries;
    let retry_interval = Duration::from_secs(opt.retry_interval_secs);
    
    for addr in &bootstrap_peers {
        for attempt in 1..=connection_retries {
            info!("Dialing bootstrap peer: {} (attempt {}/{})", addr, attempt, connection_retries);
            match swarm.dial(addr.clone()) {
                Ok(_) => {
                    info!("Successfully initiated dial to {}", addr);
                    break;
                }
                Err(e) => {
                    if attempt < connection_retries {
                        warn!("Failed to dial {}: {}, retrying...", addr, e);
                        tokio::time::sleep(retry_interval).await;
                    } else {
                        warn!("Failed to dial {}: {}, max retries reached", addr, e);
                    }
                }
            }
        }
    }
    
    // Try to reconnect to saved peers with retry
    for (peer_id, addr) in &saved_peers {
        for attempt in 1..=3 {
            info!("Reconnecting to saved peer {} at {} (attempt {}/3)", peer_id, addr, attempt);
            if swarm.dial(addr.clone()).is_ok() {
                info!("Successfully initiated reconnect to {}", peer_id);
                break;
            }
            if attempt < 3 {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
    
    // Setup ports from environment
    let metrics_port = config::node::get_port_from_env("METRICS_PORT", 9090);
    let rpc_port = config::node::get_port_from_env("RPC_PORT", 8080);
    let ws_port = config::node::get_port_from_env("WS_PORT", 9092);
    let network_p2p_port = config::node::get_port_from_env("NETWORK_P2P_PORT", 50000);
    
    info!("Port configuration - Metrics: {}, RPC: {}, WS: {}, P2P: {}",
        metrics_port, rpc_port, ws_port, network_p2p_port);
    
    // Create NodeState
    let local_peer_id = swarm.local_peer_id().to_string();
    let public_key_bytes = keypair.public().to_bytes();
    let public_key = hex::encode(public_key_bytes);
    
    // =============================================================================
    // TAREA 4: Create Ed25519 signing/verification keys for real vote signatures
    // =============================================================================
    // Convert libp2p keypair to ed25519_dalek signing key
    // libp2p ed25519 keypair is 64 bytes (32 private + 32 public), we need only the first 32
    let keypair_bytes = keypair.to_bytes();
    let mut private_key_bytes = [0u8; 32];
    private_key_bytes.copy_from_slice(&keypair_bytes[..32]);
    let signing_key_inner = ed25519_dalek::SigningKey::from_bytes(&private_key_bytes);
    let verifying_key_inner = ed25519_dalek::VerifyingKey::from(&signing_key_inner);
    let signing_key = Arc::new(std::sync::Mutex::new(Some(signing_key_inner)));
    let verifying_key = Arc::new(verifying_key_inner);
    
    let node_state = NodeState {
        start_time: std::time::Instant::now(),
        db: db.clone(),
        peer_store: peer_store.clone(),
        replay_protection: replay_protection.clone(),
        sybil_protection: sybil_protection.clone(),
        tx_rpc: tx_rpc.clone(),
        tx_ws: tx_ws.clone(),
        config: NodeConfig {
            iface: opt.iface.clone(),
            network_p2p_port,
            metrics_port,
            rpc_port,
            ws_port,
        },
        local_peer_id: local_peer_id.clone(),
        public_key: public_key.clone(),
        blocks_proposed: 0,
        transactions_processed: 0,
        hot_reload_manager: hot_reload_manager.clone(),
        // TAREA 2.1: Initialize proposer rotation
        proposer_rotation_index: Arc::new(std::sync::Mutex::new(0u64)),
        validator_peers: Arc::new(std::sync::Mutex::new(Vec::<String>::new())),
        // TAREA 4: Ed25519 signing/verification keys
        signing_key: signing_key.clone(),
        verifying_key: verifying_key.clone(),
    };
    
    // =============================================================================
    // TAREA 2: Populate validator set from saved peers or bootstrap peers
    // =============================================================================
    // Initialize validator set from saved peers
    if !saved_peers.is_empty() {
        info!("Using {} saved peers as initial validators", saved_peers.len());
        for (peer_id, _) in &saved_peers {
            let peer_id_str = peer_id.to_string();
            node_state.register_validator(peer_id_str);
        }
    }
    
    // If no saved peers, use bootstrap peer IDs
    if node_state.validator_peers.lock().unwrap().is_empty() {
        info!("Initializing validator set from bootstrap peers");
        for addr in &bootstrap_peers {
            for protocol in addr.iter() {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = protocol {
                    node_state.register_validator(peer_id.to_string());
                }
            }
        }
    }
    
    let validator_count = node_state.validator_peers.lock().unwrap().len();
    info!("Validator set initialized with {} validators", validator_count);
    
    let node_state_arc = Arc::new(node_state);
    
    // Setup HTTP API
    let app = api::router::create_router(node_state_arc.clone(), tx_rpc, tx_ws);
    
    // Spawn Prometheus metrics server (port 9090)
    let metrics_port_clone = metrics_port;
    let metrics_state = node_state_arc.clone();
    tokio::spawn(async move {
        let bind_addr = format!("0.0.0.0:{}", metrics_port_clone);
        if let Ok(listener) = tokio::net::TcpListener::bind(&bind_addr).await {
            info!("Prometheus metrics server listening on {}", bind_addr);
            if let Err(e) = axum::serve(listener, api::router::create_metrics_router(metrics_state)).await {
                error!("Prometheus metrics server error: {}", e);
            }
        } else {
            warn!("Failed to bind Prometheus metrics to {}. Trying fallback ports...", bind_addr);
            for fallback_port in [9091, 9092, 8080, 3000] {
                let fallback_addr = format!("0.0.0.0:{}", fallback_port);
                if let Ok(listener) = tokio::net::TcpListener::bind(&fallback_addr).await {
                    info!("Prometheus metrics server listening on {} (fallback)", fallback_addr);
                    let _ = axum::serve(listener, api::router::create_metrics_router(metrics_state.clone())).await;
                    break;
                }
            }
        }
    });
    
    // Spawn REST API server (port 9091)
    let rpc_port_clone = rpc_port;
    tokio::spawn(async move {
        let bind_addr = format!("0.0.0.0:{}", rpc_port_clone);
        if let Ok(listener) = tokio::net::TcpListener::bind(&bind_addr).await {
            info!("REST API server listening on {} (health, api, rpc, ws)", bind_addr);
            if let Err(e) = axum::serve(listener, app).await {
                error!("REST API server error: {}", e);
            }
        } else {
            warn!("Failed to bind REST API to {}. Trying fallback ports...", bind_addr);
            for fallback_port in [9092, 8080, 3000, 8081] {
                let fallback_addr = format!("0.0.0.0:{}", fallback_port);
                if let Ok(listener) = tokio::net::TcpListener::bind(&fallback_addr).await {
                    info!("REST API server listening on {} (fallback)", fallback_addr);
                    let _ = axum::serve(listener, app).await;
                    break;
                }
            }
        }
    });
    
    // Run P2P event loop
    p2p::event_loop::run(swarm, node_state_arc, rx_rpc, &mut ebpf).await?;
    
    Ok(())
}

/// Initialize structured JSON logging for Loki integration
fn setup_structured_logging() {
    use tracing_subscriber::prelude::*;
    
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        .add_directive("aya=warn".parse().unwrap())
        .add_directive("libp2p=info".parse().unwrap());
    
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .json()
        .with_writer(std::io::stderr)
        .init();
}
