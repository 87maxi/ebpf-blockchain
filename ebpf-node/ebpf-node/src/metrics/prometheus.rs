use lazy_static::lazy_static;
use prometheus::{
    Encoder, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, TextEncoder,
    register_int_counter, register_int_counter_vec, register_int_gauge,
    register_int_gauge_vec,
};

lazy_static! {
    // eBPF metrics
    pub static ref XDP_PACKETS_PROCESSED: IntGauge = register_int_gauge!(
        "ebpf_node_xdp_packets_processed_total",
        "Total number of packets processed by XDP"
    )
    .unwrap();
    pub static ref XDP_PACKETS_DROPPED: IntGauge = register_int_gauge!(
        "ebpf_node_xdp_packets_dropped_total",
        "Total number of packets dropped by XDP"
    )
    .unwrap();
    pub static ref XDP_BLACKLIST_SIZE: IntGauge = register_int_gauge!(
        "ebpf_node_xdp_blacklist_size",
        "Current size of the XDP blacklist"
    )
    .unwrap();
    pub static ref XDP_WHITELIST_SIZE: IntGauge = register_int_gauge!(
        "ebpf_node_xdp_whitelist_size",
        "Current size of the XDP whitelist"
    )
    .unwrap();
    pub static ref EBPF_ERRORS: IntCounter = register_int_counter!(
        "ebpf_node_errors_total",
        "Total number of eBPF errors"
    )
    .unwrap();

    // Network metrics
    pub static ref LATENCY_BUCKETS: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_node_latency_buckets",
        "Current values of latency buckets",
        &["bucket"]
    )
    .unwrap();
    pub static ref MESSAGES_RECEIVED: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_messages_received_total",
        "Total number of gossiped messages received",
        &["type"]
    )
    .unwrap();
    pub static ref PEERS_CONNECTED: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_node_peers_connected",
        "Number of connected peers",
        &["status"]
    )
    .unwrap();
    pub static ref PACKETS_TRACE: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_gossip_packets_trace_total",
        "Detailed packet trace count by sender and type",
        &["source_peer", "protocol"]
    )
    .unwrap();
    pub static ref MESSAGES_SENT: IntCounter = register_int_counter!(
        "ebpf_node_messages_sent_total",
        "Total number of messages sent via gossip"
    )
    .unwrap();
    pub static ref MESSAGES_SENT_BY_TYPE: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_messages_sent_by_type_total",
        "Total number of messages sent by type",
        &["type"]
    )
    .unwrap();
    pub static ref NETWORK_LATENCY: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_node_network_latency_ms",
        "Network latency in milliseconds by peer",
        &["peer_id"]
    )
    .unwrap();
    pub static ref BANDWIDTH_SENT: IntCounter = register_int_counter!(
        "ebpf_node_bandwidth_sent_bytes_total",
        "Total bytes sent over the network"
    )
    .unwrap();
    pub static ref BANDWIDTH_RECEIVED: IntCounter = register_int_counter!(
        "ebpf_node_bandwidth_received_bytes_total",
        "Total bytes received from the network"
    )
    .unwrap();

    // Consensus metrics
    pub static ref BLOCKS_PROPOSED: IntCounter = register_int_counter!(
        "ebpf_node_blocks_proposed_total",
        "Total number of blocks proposed"
    )
    .unwrap();
    pub static ref CONSENSUS_ROUNDS: IntCounter = register_int_counter!(
        "ebpf_node_consensus_rounds_total",
        "Total number of consensus rounds"
    )
    .unwrap();
    pub static ref CONSENSUS_DURATION: IntGauge = register_int_gauge!(
        "ebpf_node_consensus_duration_ms",
        "Current consensus round duration in milliseconds"
    )
    .unwrap();
    pub static ref VALIDATOR_COUNT: IntGauge = register_int_gauge!(
        "ebpf_node_validator_count",
        "Number of active validators"
    )
    .unwrap();
    pub static ref SLASHING_EVENTS: IntCounter = register_int_counter!(
        "ebpf_node_slashing_events_total",
        "Total number of slashing events"
    )
    .unwrap();

    // Transaction metrics
    pub static ref TRANSACTIONS_PROCESSED: IntCounter = register_int_counter!(
        "ebpf_node_transactions_processed_total",
        "Total number of transactions processed"
    )
    .unwrap();
    pub static ref TRANSACTIONS_BY_TYPE: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_transactions_by_type_total",
        "Total number of transactions by type",
        &["type"]
    )
    .unwrap();
    pub static ref TRANSACTION_QUEUE_SIZE: IntGauge = register_int_gauge!(
        "ebpf_node_transaction_queue_size",
        "Current size of the transaction queue"
    )
    .unwrap();
    pub static ref TRANSACTION_FAILURES: IntCounter = register_int_counter!(
        "ebpf_node_transactions_failures_total",
        "Total number of transaction processing failures"
    )
    .unwrap();
    pub static ref TRANSACTIONS_CONFIRMED: IntCounter = register_int_counter!(
        "ebpf_node_transactions_confirmed_total",
        "Total number of transactions confirmed by consensus"
    )
    .unwrap();
    pub static ref TRANSACTIONS_REJECTED: IntCounter = register_int_counter!(
        "ebpf_node_transactions_rejected_total",
        "Total number of transactions rejected (e.g., replay attacks)"
    )
    .unwrap();
    pub static ref TRANSACTIONS_REPLAY_REJECTED: IntCounter = register_int_counter!(
        "ebpf_node_transactions_replay_rejected_total",
        "Total number of transactions rejected due to replay protection (duplicate nonce/timestamp)"
    )
    .unwrap();

    // Sybil attack metrics
    pub static ref SYBIL_ATTEMPTS_DETECTED: IntCounter = register_int_counter!(
        "ebpf_node_sybil_attempts_total",
        "Total number of potential Sybil attack attempts detected (multiple connections per IP)"
    )
    .unwrap();

    // P2P metrics
    pub static ref P2P_CONNECTIONS_TOTAL: IntCounter = register_int_counter!(
        "ebpf_node_p2p_connections_total",
        "Total number of P2P connections established"
    )
    .unwrap();
    pub static ref P2P_CONNECTIONS_CLOSED: IntCounter = register_int_counter!(
        "ebpf_node_p2p_connections_closed_total",
        "Total number of P2P connections closed"
    )
    .unwrap();
    pub static ref PEERS_IDENTIFIED: IntCounter = register_int_counter!(
        "ebpf_node_peers_identified_total",
        "Total number of peers identified via libp2p identify protocol"
    )
    .unwrap();
    pub static ref PEERS_SAVED: IntCounter = register_int_counter!(
        "ebpf_node_peers_saved_total",
        "Total number of peer addresses saved to peer store"
    )
    .unwrap();

    // Database metrics
    pub static ref DB_OPERATIONS: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_db_operations_total",
        "Total number of database operations",
        &["operation"]
    )
    .unwrap();

    // Uptime
    pub static ref UPTIME: IntCounter = register_int_counter!(
        "ebpf_node_uptime",
        "Uptime of the node in seconds"
    )
    .unwrap();

    // System metrics
    pub static ref MEMORY_USAGE_BYTES: IntGauge = register_int_gauge!(
        "ebpf_node_memory_usage_bytes",
        "Current memory usage in bytes"
    )
    .unwrap();
    pub static ref UPTIME_SECONDS: IntGauge = register_int_gauge!(
        "ebpf_node_uptime_seconds",
        "Uptime in seconds"
    )
    .unwrap();
    pub static ref THREAD_COUNT: IntGauge = register_int_gauge!(
        "ebpf_node_thread_count",
        "Current number of threads"
    )
    .unwrap();
}

/// Initialize all metrics
pub fn initialize_metrics() {
    // Initialize all existing metrics
    for i in 0..64 {
        LATENCY_BUCKETS.with_label_values(&[&i.to_string()]).set(0);
    }
    PEERS_CONNECTED.with_label_values(&["connected"]).set(0);
    MESSAGES_RECEIVED.with_label_values(&["gossip"]).inc_by(0);
    UPTIME.inc_by(0);
    TRANSACTIONS_CONFIRMED.inc_by(0);
    TRANSACTIONS_REJECTED.inc_by(0);
    DB_OPERATIONS.with_label_values(&["put"]).inc_by(0);
    DB_OPERATIONS.with_label_values(&["get"]).inc_by(0);
    P2P_CONNECTIONS_TOTAL.inc_by(0);
    P2P_CONNECTIONS_CLOSED.inc_by(0);
    PEERS_IDENTIFIED.inc_by(0);
    PEERS_SAVED.inc_by(0);
    TRANSACTIONS_REPLAY_REJECTED.inc_by(0);
    SYBIL_ATTEMPTS_DETECTED.inc_by(0);
    
    // Initialize new network metrics
    MESSAGES_SENT.inc_by(0);
    MESSAGES_SENT_BY_TYPE.with_label_values(&["tx"]).inc_by(0);
    MESSAGES_SENT_BY_TYPE.with_label_values(&["vote"]).inc_by(0);
    MESSAGES_SENT_BY_TYPE.with_label_values(&["sync"]).inc_by(0);
    NETWORK_LATENCY.with_label_values(&["average"]).set(0);
    BANDWIDTH_SENT.inc_by(0);
    BANDWIDTH_RECEIVED.inc_by(0);
    
    // Initialize new consensus metrics
    BLOCKS_PROPOSED.inc_by(0);
    CONSENSUS_ROUNDS.inc_by(0);
    CONSENSUS_DURATION.set(0);
    VALIDATOR_COUNT.set(0);
    SLASHING_EVENTS.inc_by(0);
    
    // Initialize new transaction metrics
    TRANSACTIONS_PROCESSED.inc_by(0);
    TRANSACTIONS_BY_TYPE.with_label_values(&["transfer"]).inc_by(0);
    TRANSACTIONS_BY_TYPE.with_label_values(&["vote"]).inc_by(0);
    TRANSACTION_QUEUE_SIZE.set(0);
    TRANSACTION_FAILURES.inc_by(0);
    
    // Initialize new eBPF metrics
    XDP_PACKETS_PROCESSED.set(0);
    XDP_PACKETS_DROPPED.set(0);
    XDP_BLACKLIST_SIZE.set(0);
    XDP_WHITELIST_SIZE.set(0);
    EBPF_ERRORS.inc_by(0);
    
    // Initialize system metrics
    MEMORY_USAGE_BYTES.set(0);
    UPTIME_SECONDS.set(0);
    THREAD_COUNT.set(0);
}

/// Gather all metrics for Prometheus export
pub fn gather_metrics() -> Vec<prometheus::proto::MetricFamily> {
    prometheus::gather()
}
