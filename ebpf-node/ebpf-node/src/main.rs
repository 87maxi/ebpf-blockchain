mod config;
mod db;
mod ebpf;
mod api;
mod p2p;
mod security;
mod metrics;

use std::sync::Arc;
use std::time::Duration;

use config::cli::{Opt, load_saved_peers, save_peers, get_bootstrap_peers_from_env, get_ip_from_multiaddr};
use config::node::{NodeState, NodeConfig};
use clap::Parser;
use db::rocksdb::init_db;
use db::backup::schedule_backups;
use ebpf::loader::{load_binary, load};
use ebpf::programs::attach_all;
use ebpf::hot_reload::EbpfHotReloadManager;
use metrics::prometheus::{initialize_metrics, PEERS_CONNECTED, PEERS_IDENTIFIED, PEERS_SAVED, BLOCKS_PROPOSED};
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
    
    // Load bootstrap peers
    let mut bootstrap_peers = opt.bootstrap_peers.clone();
    let env_peers = get_bootstrap_peers_from_env();
    for peer in &env_peers {
        if !bootstrap_peers.contains(peer) {
            bootstrap_peers.push(peer.clone());
        }
    }
    
    // Load saved peers
    let saved_peers = peer_store.all_peers();
    info!("Loaded {} saved peers from peer store", saved_peers.len());
    
    // Try to reconnect to saved peers
    for (peer_id, addr) in &saved_peers {
        info!("Trying to reconnect to saved peer {} at {}", peer_id, addr);
    }
    
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
    let rpc_port = config::node::get_port_from_env("RPC_PORT", 9091);
    let ws_port = config::node::get_port_from_env("WS_PORT", 9092);
    let network_p2p_port = config::node::get_port_from_env("NETWORK_P2P_PORT", 9000);
    
    info!("Port configuration - Metrics: {}, RPC: {}, WS: {}, P2P: {}",
        metrics_port, rpc_port, ws_port, network_p2p_port);
    
    // Create NodeState
    let local_peer_id = swarm.local_peer_id().to_string();
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
        blocks_proposed: 0,
        transactions_processed: 0,
        hot_reload_manager: hot_reload_manager.clone(),
    };
    let node_state_arc = Arc::new(node_state);
    
    // Setup HTTP API
    let app = api::router::create_router(node_state_arc.clone(), tx_rpc, tx_ws);
    
    // Spawn HTTP server
    let metrics_port_clone = metrics_port;
    tokio::spawn(async move {
        let bind_addr = format!("0.0.0.0:{}", metrics_port_clone);
        if let Ok(listener) = tokio::net::TcpListener::bind(&bind_addr).await {
            info!("REST API server listening on {} (health, metrics, api, rpc, ws)", bind_addr);
            if let Err(e) = axum::serve(listener, app).await {
                error!("Axum server error: {}", e);
            }
        } else {
            warn!("Failed to bind REST API to {}. Trying fallback ports...", bind_addr);
            for fallback_port in [9091, 9092, 8080, 3000] {
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
