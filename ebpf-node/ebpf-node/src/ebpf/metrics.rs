use crate::ebpf::maps::EbpfMaps;
use crate::metrics::prometheus::{
    XDP_PACKETS_PROCESSED, XDP_BLACKLIST_SIZE, XDP_WHITELIST_SIZE,
    LATENCY_BUCKETS,
};

/// Update eBPF metrics from the eBPF maps
pub fn update_ebpf_metrics(ebpf_maps: &mut EbpfMaps) {
    // Update total packets processed
    let total_packets = ebpf_maps.total_packets_processed();
    XDP_PACKETS_PROCESSED.set(total_packets as i64);
    
    // Update latency buckets
    if let Ok(latency_stats) = ebpf_maps.latency_stats() {
        for i in 0..64 {
            if let Ok(count) = latency_stats.get(&i, 0u64) {
                LATENCY_BUCKETS.with_label_values(&[&i.to_string()]).set(count as i64);
            }
        }
    }
    
    // Update blacklist/whitelist sizes
    if let Ok(blacklist_size) = ebpf_maps.blacklist_size() {
        XDP_BLACKLIST_SIZE.set(blacklist_size as i64);
    }
    if let Ok(whitelist_size) = ebpf_maps.whitelist_size() {
        XDP_WHITELIST_SIZE.set(whitelist_size as i64);
    }
}
