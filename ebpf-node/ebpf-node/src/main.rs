use anyhow::Context as _;
use aya::{
    maps::{HashMap, LpmTrie, lpm_trie::Key},
    programs::{KProbe, Xdp, XdpFlags},
};
use clap::Parser;
use libp2p::{
    Multiaddr,
    futures::StreamExt,
    gossipsub, identify, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use log::{debug, info, warn};
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::{signal, time};

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long, default_value = "eth0")]
    iface: String,

    #[clap(short, long)]
    listen_address: Option<Multiaddr>,
}

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    env_logger::init();

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
    xdp_program
        .attach(&opt.iface, XdpFlags::default())
        .context("failed to attach XDP program")?;

    // Attach Kprobes for latency observability
    let kprobe_in: &mut KProbe = ebpf.program_mut("netif_receive_skb").unwrap().try_into()?;
    kprobe_in.load()?;
    kprobe_in.attach("netif_receive_skb", 0)?;

    let kprobe_out: &mut KProbe = ebpf.program_mut("napi_consume_skb").unwrap().try_into()?;
    kprobe_out.load()?;
    kprobe_out.attach("napi_consume_skb", 0)?;

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

            Ok(MyBehaviour {
                gossipsub,
                identify,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let listen_addr = opt
        .listen_address
        .unwrap_or("/ip4/0.0.0.0/udp/0/quic-v1".parse()?);
    swarm.listen_on(listen_addr)?;

    info!("Local Peer ID: {}", swarm.local_peer_id());

    let mut stats_interval = time::interval(Duration::from_secs(10));

    // Main event loop
    loop {
        tokio::select! {
            _ = stats_interval.tick() => {
                // Get map reference locally to avoid long-lived borrow of ebpf
                if let Ok(latency_stats) = HashMap::<_, u64, u64>::try_from(ebpf.map("LATENCY_STATS").unwrap()) {
                    println!("--- Latency Histogram (nanoseconds, power of 2 buckets) ---");
                    for i in 0..64 {
                        if let Ok(count) = latency_stats.get(&i, 0) {
                            if count > 0 {
                                println!("Bucket 2^{}: {} packets", i, count);
                            }
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
                SwarmEvent::IncomingConnection { send_back_addr, .. } => {
                    if let Some(ip) = get_ip_from_multiaddr(&send_back_addr) {
                        debug!("Incoming connection from IP: {}", ip);
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
