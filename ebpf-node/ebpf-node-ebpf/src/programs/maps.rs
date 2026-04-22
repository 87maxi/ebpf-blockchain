//! Shared eBPF maps definitions

use aya_ebpf::{
    maps::{HashMap},
    maps::lpm_trie::LpmTrie,
};
use aya_ebpf::macros::map;

/// Whitelist of trusted IPs using longest prefix match.
/// Packets from IPs NOT in this whitelist will be dropped.
/// This is a preventive security measure - only trusted peers are allowed.
/// Format: Key = (prefix_length, ip_address), Value = 1 (trusted)
#[map]
pub static NODES_WHITELIST: LpmTrie<u32, u32> = LpmTrie::with_max_entries(1024, 0);

/// Reactive blacklist for IPs detected as malicious during operation.
/// This complements the whitelist for dynamic threat response.
/// Format: Key = (prefix_length, ip_address), Value = 1 (blocked)
#[map]
pub static NODES_BLACKLIST: LpmTrie<u32, u32> = LpmTrie::with_max_entries(10240, 0);

/// Histogram of latencies.
/// Key is the bucket (power of 2 of the latency in nanoseconds), value is the count.
#[map]
pub static LATENCY_STATS: HashMap<u64, u64> = HashMap::with_max_entries(64, 0);

/// Temporary storage for packet start times, keyed by the skb pointer address.
/// Note: LruHashMap is not available in this version of Aya, so we use a regular HashMap
/// with manual cleanup or other mechanisms as needed.
#[map]
pub static START_TIMES: HashMap<u64, u64> = HashMap::with_max_entries(10240, 0);