use std::{net::Ipv4Addr, time::Duration};

use log::{debug, info, warn};
use anyhow::Context as _;
use axum::{Router, routing::get};
use aya::{
    maps::{HashMap, LpmTrie, lpm_trie::Key},
    programs::{KProbe, Xdp, XdpFlags},
};
use clap::Parser;
use lazy_static::lazy_static;
use libp2p::{
    Multiaddr,
    futures::StreamExt,
    gossipsub, identify, mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use prometheus::{
    Encoder, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, TextEncoder,
    register_int_counter, register_int_counter_vec, register_int_gauge,
    register_int_gauge_vec,
};
use tokio::{signal, time};

lazy_static! {
    static ref LATENCY_BUCKETS: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_node_latency_buckets",
        "Current values of latency buckets",
        &["bucket"]
    )
    .unwrap();
    static ref MESSAGES_RECEIVED: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_messages_received_total",
        "Total number of gossiped messages received",
        &["type"]
    )
    .unwrap();
    static ref PEERS_CONNECTED: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_node_peers_connected",
        "Number of connected peers",
        &["status"]
    )
    .unwrap();
    static ref PACKETS_TRACE: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_gossip_packets_trace_total",
        "Detailed packet trace count by sender and type",
        &["source_peer", "protocol"]
    )
    .unwrap();
    static ref UPTIME: IntCounter = register_int_counter!(
        "ebpf_node_uptime",
        "Uptime of the node in seconds"
    )
    .unwrap();
}

async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    let metric_families = prometheus::gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

fn initialize_metrics() {
    // Initialize latency buckets (0 to 63)
    for i in 0..64 {
        LATENCY_BUCKETS.with_label_values(&[&i.to_string()]).set(0);
    }
    // Initialize connected peers
    PEERS_CONNECTED.with_label_values(&["connected"]).set(0);
    // Initialize messages received
    MESSAGES_RECEIVED.with_label_values(&["gossip"]).inc_by(0);
    // Initialize uptime
    UPTIME.inc_by(0);
}

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long, default_value = "eth0")]
    iface: String,

    #[clap(short, long, value_delimiter = ',')]
    listen_addresses: Vec<Multiaddr>,

    #[clap(long, value_delimiter = ',')]
    bootstrap_peers: Vec<Multiaddr>,
}

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    env_logger::init();
    initialize_metrics();

    // Bump the memlock rlimit.
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("remove limit on locked memory failed, ret is: {ret}");
    }

    // Load eBPF programs
    let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/ebpf-node"
    )))?;

    // Initialize eBPF logger
    if let Err(e) = aya_log::EbpfLogger::init(&mut ebpf) {
        warn!("failed to initialize eBPF logger: {e}");
    }

    // Attach XDP program
    let xdp_program: &mut Xdp = ebpf.program_mut("ebpf_node").unwrap().try_into()?;
    xdp_program.load()?;
    if let Err(e) = xdp_program.attach(&opt.iface, XdpFlags::default()) {
        warn!("Failed to attach XDP program, continuing: {}", e);
    }

    // Attach Kprobes for latency observability
    let kprobe_in: &mut KProbe = ebpf.program_mut("netif_receive_skb").unwrap().try_into()?;
    kprobe_in.load()?;
    if let Err(e) = kprobe_in.attach("netif_receive_skb", 0) {
        warn!("Failed to attach KProbe in, continuing: {}", e);
    }

    let kprobe_out: &mut KProbe = ebpf.program_mut("napi_consume_skb").unwrap().try_into()?;
    kprobe_out.load()?;
    if let Err(e) = kprobe_out.attach("napi_consume_skb", 0) {
        warn!("Failed to attach KProbe out, continuing: {}", e);
    }

    // Setup libp2p
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|key| {
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(&message.data, &mut s);
                gossipsub::MessageId::from(std::hash::Hasher::finish(&s).to_string())
            };

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(1))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()
                .map_err(|msg| std::io::Error::new(std::io::ErrorKind::Other, msg))?;

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            let identify = identify::Behaviour::new(identify::Config::new(
                "/ebpf-blockchain/1.0.0".into(),
                key.public(),
            ));

            let mdns = mdns::tokio::Behaviour::new(
                mdns::Config::default(),
                key.public().to_peer_id(),
            )?;

            Ok(MyBehaviour {
                gossipsub,
                identify,
                mdns,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let listen_addrs = if opt.listen_addresses.is_empty() {
        vec!["/ip4/0.0.0.0/udp/0/quic-v1".parse()?]
    } else {
        opt.listen_addresses
    };

    for addr in &listen_addrs {
        swarm.listen_on(addr.clone())?;
    }

    info!("Local Peer ID: {}", swarm.local_peer_id());
    let _ = std::fs::write("/tmp/peer_id.txt", swarm.local_peer_id().to_string());

    // Dial bootstrap peers
    for addr in &opt.bootstrap_peers {
        info!("Dialing bootstrap peer: {}", addr);
        if let Err(e) = swarm.dial(addr.clone()) {
            warn!("Failed to dial {}: {}", addr, e);
        }
    }

    // Spawn Prometheus metrics server
    tokio::spawn(async move {
        let app = Router::new().route("/metrics", get(metrics_handler));
        if let Ok(listener) = tokio::net::TcpListener::bind("0.0.0.0:9090").await {
            info!("Prometheus metrics server listening on 0.0.0.0:9090/metrics");
            let _ = axum::serve(listener, app).await;
        } else {
            warn!("Failed to bind metrics server to 0.0.0.0:9090");
        }
    });

    let mut stats_interval = time::interval(Duration::from_secs(10));

    // Main event loop
    loop {
        tokio::select! {
            _ = stats_interval.tick() => {
                UPTIME.inc();
                // Get map reference locally to avoid long-lived borrow of ebpf
                if let Ok(latency_stats) = HashMap::<_, u64, u64>::try_from(ebpf.map("LATENCY_STATS").unwrap()) {
                    for i in 0..64 {
                        if let Ok(count) = latency_stats.get(&i, 0) {
                            LATENCY_BUCKETS.with_label_values(&[&i.to_string()]).set(count as i64);
                        }
                    }
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source,
                    message_id,
                    message,
                })) => {
                    MESSAGES_RECEIVED.with_label_values(&["gossip"]).inc();
                    let sender = propagation_source.to_string();
                    PACKETS_TRACE.with_label_values(&[&sender, "gossip"]).inc();
                    info!("Got message: '{}' with id: {} from peer: {}",
                        String::from_utf8_lossy(&message.data), message_id, propagation_source);

                    if message.data.starts_with(b"ATTACK") {
                        warn!("Malicious message detected from peer {}. Blocking IP.", propagation_source);

                        // Simulation of blocking an IP (1.2.3.4)
                        let ip_to_block = Ipv4Addr::new(1, 2, 3, 4);
                        let ip_u32 = u32::from_be_bytes(ip_to_block.octets());
                        let key = Key::new(32, ip_u32);

                        // Get map reference locally as LpmTrie
                        if let Ok(mut blacklist) = LpmTrie::<_, u32, u32>::try_from(ebpf.map_mut("NODES_BLACKLIST").unwrap()) {
                            if let Err(e) = blacklist.insert(&key, 1, 0) {
                                warn!("Failed to block IP: {}", e);
                            } else {
                                info!("IP {} blocked successfully", ip_to_block);
                            }
                        }
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Listening on {:?}", address);
                }
                SwarmEvent::ConnectionEstablished { .. } => {
                    PEERS_CONNECTED.with_label_values(&["connected"]).inc();
                }
                SwarmEvent::ConnectionClosed { .. } => {
                    PEERS_CONNECTED.with_label_values(&["connected"]).dec();
                }
                SwarmEvent::IncomingConnection { send_back_addr, .. } => {
                    if let Some(ip) = get_ip_from_multiaddr(&send_back_addr) {
                        debug!("Incoming connection from IP: {}", ip);
                    }
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, multiaddr) in list {
                        info!("mDNS discovered a new peer: {} at {}", peer_id, multiaddr);
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        if let Err(e) = swarm.dial(multiaddr.clone()) {
                            warn!("Failed to dial mDNS discovered peer {}: {}", peer_id, e);
                        }
                    }
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, multiaddr) in list {
                        info!("mDNS discovered peer has expired: {} at {}", peer_id, multiaddr);
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                }
                _ => {}
            },
            _ = signal::ctrl_c() => {
                info!("Exiting...");
                break;
            }
        }
    }

    Ok(())
}

fn get_ip_from_multiaddr(addr: &Multiaddr) -> Option<Ipv4Addr> {
    for proto in addr.iter() {
        if let libp2p::multiaddr::Protocol::Ip4(ip) = proto {
            return Some(ip);
        }
    }
    None
}
