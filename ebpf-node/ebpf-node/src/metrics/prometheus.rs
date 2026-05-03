use lazy_static::lazy_static;
use prometheus::{
    Encoder, Gauge, GaugeVec, Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, TextEncoder,
    register_int_counter, register_int_counter_vec, register_int_gauge,
    register_int_gauge_vec, register_gauge, register_gauge_vec,
    register_histogram, register_histogram_vec,
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

    // TAREA 3.5: New métricas para laboratorio eBPF completo

    // 1. KProbe hit count
    pub static ref KPROBE_HIT_COUNT: IntCounterVec = register_int_counter_vec!(
        "ebpf_kprobe_hit_count",
        "Number of kprobe hits by probe name",
        &["probe_name"]
    )
    .unwrap();

    // 2. Hot reload success/failure
    pub static ref HOT_RELOAD_SUCCESS_TOTAL: IntCounter = register_int_counter!(
        "ebpf_hot_reload_success_total",
        "Number of successful eBPF hot reloads"
    )
    .unwrap();
    pub static ref HOT_RELOAD_FAILURE_TOTAL: IntCounter = register_int_counter!(
        "ebpf_hot_reload_failure_total",
        "Number of failed eBPF hot reloads"
    )
    .unwrap();

    // 3. Swarm dial errors
    pub static ref SWARM_DIAL_ERRORS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "libp2p_swarm_dial_errors_total",
        "Number of swarm dial errors by error type",
        &["error_type"]
    )
    .unwrap();

    // 4. RocksDB write rate
    pub static ref ROCKSDB_WRITE_RATE_BYTES_TOTAL: IntCounter = register_int_counter!(
        "rocksdb_write_rate_bytes_total",
        "Total bytes written to RocksDB"
    )
    .unwrap();

    // 5. RocksDB DB size
    pub static ref ROCKSDB_DB_SIZE_BYTES: Gauge = register_gauge!(
        "rocksdb_db_size_bytes",
        "Current size of RocksDB in bytes"
    )
    .unwrap();

    // 6. API request duration (using histogram)
    pub static ref API_REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "api_request_duration_seconds",
        "API request duration in seconds",
        &["endpoint", "method", "status"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .unwrap();

    // 7. API requests total
    pub static ref API_REQUESTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "api_requests_total",
        "Total number of API requests",
        &["endpoint", "method"]
    )
    .unwrap();

    // 8. Ringbuf buffer utilization
    pub static ref RINGBUF_BUFFER_UTILIZATION: Gauge = register_gauge!(
        "ebpf_ringbuf_buffer_utilization",
        "Ringbuf buffer utilization percentage"
    )
    .unwrap();

    // =============================================================================
    // SECURITY METRICS - Threat Detection
    // =============================================================================

    pub static ref SECURITY_THREAT_SCORE: GaugeVec = register_gauge_vec!(
        "ebpf_node_security_threat_score",
        "Security threat score (0-100)",
        &["node"]
    )
    .unwrap();

    pub static ref BLACKLIST_SIZE: GaugeVec = register_gauge_vec!(
        "ebpf_node_blacklist_size",
        "Number of peers in blacklist",
        &["node"]
    )
    .unwrap();

    pub static ref VOTE_VALIDATION_FAILURES: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_vote_validation_failures_total",
        "Total vote validation failures",
        &["reason"]
    )
    .unwrap();

    pub static ref DOUBLE_VOTE_ATTEMPTS: IntCounter = register_int_counter!(
        "ebpf_node_double_vote_attempts_total",
        "Total double vote attempts detected"
    )
    .unwrap();

    // =============================================================================
    // CONSENSUS INTEGRITY METRICS
    // =============================================================================

    pub static ref FORK_EVENTS: IntCounter = register_int_counter!(
        "ebpf_node_fork_events_total",
        "Total fork events detected"
    )
    .unwrap();

    pub static ref FINALITY_CHECKPOINTS: IntCounter = register_int_counter!(
        "ebpf_node_finality_checkpoints_total",
        "Total finality checkpoints recorded"
    )
    .unwrap();

    pub static ref VALIDATOR_UPTIME: GaugeVec = register_gauge_vec!(
        "ebpf_node_validator_uptime_ratio",
        "Validator uptime ratio (0-1)",
        &["validator_id"]
    )
    .unwrap();

    pub static ref CONSENSUS_LATENCY_MS: HistogramVec = register_histogram_vec!(
        "ebpf_node_consensus_latency_ms",
        "Consensus round latency in milliseconds",
        &["node", "phase"],
        vec![10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0]
    )
    .unwrap();

    // =============================================================================
    // NETWORK ATTACK SURFACE METRICS
    // =============================================================================

    pub static ref SUSPICIOUS_CONNECTIONS: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_suspicious_connections_total",
        "Total suspicious connections",
        &["reason"]
    )
    .unwrap();

    pub static ref PEER_SCORE: GaugeVec = register_gauge_vec!(
        "ebpf_node_peer_score",
        "Peer reputation score (-100 to 100)",
        &["peer_id"]
    )
    .unwrap();

    pub static ref BANDWIDTH_ABUSE: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_bandwidth_abuse_total",
        "Total bandwidth abuse events",
        &["direction"]
    )
    .unwrap();

    pub static ref MESSAGE_VALIDATION_FAILURES: IntCounterVec = register_int_counter_vec!(
        "ebpf_node_message_validation_failures_total",
        "Total message validation failures",
        &["message_type"]
    )
    .unwrap();

    // =============================================================================
    // P2-2: ECLIPSE ATTACK DETECTION METRICS
    // =============================================================================

    pub static ref ECLIPSE_RISK_SCORE: GaugeVec = register_gauge_vec!(
        "ebpf_node_eclipse_risk_score",
        "Eclipse attack risk score (0-100)",
        &["node_id"]
    )
    .unwrap();

    pub static ref PEER_IP_DIVERSITY: GaugeVec = register_gauge_vec!(
        "ebpf_node_peer_ip_diversity",
        "Number of unique IP prefixes among connected peers",
        &["node_id"]
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
    
    // Initialize TAREA 3.5: New metrics
    KPROBE_HIT_COUNT.with_label_values(&["default"]).inc_by(0);
    HOT_RELOAD_SUCCESS_TOTAL.inc_by(0);
    HOT_RELOAD_FAILURE_TOTAL.inc_by(0);
    SWARM_DIAL_ERRORS_TOTAL.with_label_values(&["timeout"]).inc_by(0);
    ROCKSDB_WRITE_RATE_BYTES_TOTAL.inc_by(0);
    ROCKSDB_DB_SIZE_BYTES.set(0.0);
    API_REQUESTS_TOTAL.with_label_values(&["/health", "GET"]).inc_by(0);
    RINGBUF_BUFFER_UTILIZATION.set(0.0);
    
    // =============================================================================
    // SECURITY METRICS - Threat Detection (Fase A)
    // =============================================================================
    SECURITY_THREAT_SCORE.with_label_values(&["default"]).set(0.0);
    BLACKLIST_SIZE.with_label_values(&["default"]).set(0.0);
    VOTE_VALIDATION_FAILURES.with_label_values(&["invalid_signature"]).inc_by(0);
    VOTE_VALIDATION_FAILURES.with_label_values(&["duplicate"]).inc_by(0);
    DOUBLE_VOTE_ATTEMPTS.inc_by(0);
    
    // =============================================================================
    // CONSENSUS INTEGRITY METRICS (Fase A)
    // =============================================================================
    FORK_EVENTS.inc_by(0);
    FINALITY_CHECKPOINTS.inc_by(0);
    VALIDATOR_UPTIME.with_label_values(&["default"]).set(1.0);
    CONSENSUS_LATENCY_MS.with_label_values(&["default", "proposal"]).observe(0.0);
    CONSENSUS_LATENCY_MS.with_label_values(&["default", "vote"]).observe(0.0);
    CONSENSUS_LATENCY_MS.with_label_values(&["default", "finality"]).observe(0.0);
    
    // =============================================================================
    // NETWORK ATTACK SURFACE METRICS (Fase A)
    // =============================================================================
    SUSPICIOUS_CONNECTIONS.with_label_values(&["unknown"]).inc_by(0);
    PEER_SCORE.with_label_values(&["default"]).set(0.0);
    BANDWIDTH_ABUSE.with_label_values(&["inbound"]).inc_by(0);
    BANDWIDTH_ABUSE.with_label_values(&["outbound"]).inc_by(0);
    MESSAGE_VALIDATION_FAILURES.with_label_values(&["unknown"]).inc_by(0);
    
    // =============================================================================
    // P2-2: ECLIPSE ATTACK DETECTION METRICS
    // =============================================================================
    ECLIPSE_RISK_SCORE.with_label_values(&["default"]).set(0.0);
    PEER_IP_DIVERSITY.with_label_values(&["default"]).set(0.0);
}

/// Gather all metrics for Prometheus export
pub fn gather_metrics() -> Vec<prometheus::proto::MetricFamily> {
    prometheus::gather()
}
