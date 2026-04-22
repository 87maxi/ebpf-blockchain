use std::net::Ipv4Addr;

use clap::Parser;
use libp2p::{Multiaddr, PeerId};
use tracing::{debug, warn};

use crate::config::node::get_port_from_env;

/// CLI options for the eBPF node
#[derive(Debug, Parser)]
pub struct Opt {
    #[clap(short, long, default_value = "eth0")]
    pub iface: String,

    #[clap(short, long, value_delimiter = ',')]
    pub listen_addresses: Vec<Multiaddr>,

    #[clap(long, value_delimiter = ',')]
    pub bootstrap_peers: Vec<Multiaddr>,
    
    #[clap(long, default_value = "10")]
    pub connection_retries: u32,
    
    #[clap(long, default_value = "30")]
    pub retry_interval_secs: u64,
}

/// Load saved peers from file
pub fn load_saved_peers(path: &str) -> Vec<Multiaddr> {
    if let Ok(content) = std::fs::read_to_string(path) {
        content
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.trim().to_string())
            .filter_map(|addr| addr.parse().ok())
            .collect()
    } else {
        Vec::new()
    }
}

/// Save peers to file for persistence
pub fn save_peers(peers: &[(PeerId, Multiaddr)], path: &str) -> anyhow::Result<()> {
    let mut lines = Vec::new();
    for (peer_id, addr) in peers {
        lines.push(format!("{} {}", peer_id, addr));
    }
    std::fs::write(path, lines.join("\n"))?;
    Ok(())
}

/// Get bootstrap peers from environment or config
pub fn get_bootstrap_peers_from_env() -> Vec<Multiaddr> {
    std::env::var("BOOTSTRAP_PEERS")
        .ok()
        .map(|val| {
            val.split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Get current Unix timestamp in seconds
pub fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_else(|_| 0)
}

/// Format timestamp as ISO string
pub fn format_iso_timestamp(secs: u64) -> String {
    format!("1970-01-01T00:00:00Z+{}", secs)
}

/// Get IP from multiaddr
pub fn get_ip_from_multiaddr(addr: &Multiaddr) -> Option<Ipv4Addr> {
    for proto in addr.iter() {
        if let libp2p::multiaddr::Protocol::Ip4(ip) = proto {
            return Some(ip);
        }
    }
    None
}

/// Extract hostname from a db path
pub fn hostname_from_path(path: &str) -> String {
    path.split('/').find(|p| !p.is_empty()).unwrap_or("unknown").to_string()
}
