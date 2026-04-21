# eBPF Blockchain - Architecture Documentation

## Table of Contents

- [Overview](#overview)
- [System Architecture](#system-architecture)
- [Component Details](#component-details)
- [Data Flow](#data-flow)
- [Architecture Decision Records](#architecture-decision-records)
- [Security Architecture](#security-architecture)
- [Scalability](#scalability)

## Overview

The eBPF Blockchain architecture combines kernel-level network observability (eBPF) with decentralized blockchain consensus (libp2p) to create a secure, observable, and performant distributed system.

### Design Principles

1. **Separation of Concerns** - Clear boundaries between kernel and user space
2. **Defense in Depth** - Multiple security layers
3. **Observability by Default** - Metrics, logs, and traces from day one
4. **Zero Trust** - Verify everything, trust nothing
5. **Graceful Degradation** - System continues operating under partial failure

## System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        CLIENT LAYER                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │    CLI       │  │   Web UI     │  │   External Integrations  │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       API LAYER (Axum)                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  HTTP API    │  │ WebSocket    │  │  Prometheus Exporter     │  │
│  │  (:9091)     │  │  (:9092)     │  │  (:9090)                 │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       CORE LAYER (Rust/Tokio)                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │   Consensus  │  │  Transaction │  │     State Manager        │  │
│  │   Module     │  │     Pool     │  │                          │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │   P2P Net-   │  │   Security   │  │      Metrics             │  │
│  │  working     │  │   Module     │  │   Collector              │  │
│  │  (libp2p)    │  │              │  │                          │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      STORAGE LAYER                                  │
│  ┌──────────────┐  ┌──────────────┐                                │
│  │   RocksDB    │  │   Cache      │                                │
│  │  (Persistent)│  │   Layer      │                                │
│  └──────────────┘  └──────────────┘                                │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   KERNEL SPACE (eBPF)                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │    XDP       │  │   KProbes    │  │      Tracepoints         │  │
│  │  Filtering   │  │  Latency     │  │      Monitoring          │  │
│  │  (Security)  │  │  (Monitor)   │  │      (Security)          │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   HARDWARE / NETWORK                                │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    NETWORK HARDWARE                          │  │
│  │              (NIC, Switches, Routers)                        │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### Observability Stack

```
┌─────────────────────────────────────────────────────────────────────┐
│                       GRAFANA (:3000)                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  Health      │  │   Network    │  │     Consensus            │  │
│  │  Dashboard   │  │   Dashboard  │  │     Dashboard            │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐                               │
│  │ Transactions │  │   Custom     │                               │
│  │  Dashboard   │  │   Dashboards │                               │
│  └──────────────┘  └──────────────┘                               │
└─────────────────────────────────────────────────────────────────────┘
         ▲                    ▲                    ▲
         │                    │                    │
         ▼                    ▼                    ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐
│  Prometheus  │  │     Loki     │  │        Tempo             │
│   (:9090)    │  │    (:3100)   │  │       (:3200)            │
│              │  │              │  │                          │
│  Metrics     │  │    Logs      │  │      Traces              │
└──────────────┘  └──────────────┘  └──────────────────────────┘
         ▲                    ▲                    ▲
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       eBPF NODE (User Space)                        │
│  - Structured JSON logs → Promtail → Loki                          │
│  - Prometheus metrics → Prometheus                                 │
│  - OpenTelemetry traces → Tempo                                    │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. eBPF Module (`ebpf-node-ebpf`)

The eBPF module runs in kernel space and provides:

#### XDP Filtering (`try_ebpf_node()`)
- **Purpose**: Highest-performance packet filtering
- **Location**: Network device driver (ingress path)
- **Actions**: `XDP_PASS`, `XDP_DROP`, `XDP_TX`
- **Security**: Blacklist/whitelist IP filtering
- **Performance**: Microsecond-level processing

```rust
// Simplified XDP flow
pub fn ebpf_node(ctx: XdpContext) -> u32 {
    match try_ebpf_node(ctx) {
        Ok(action) => action,
        Err(_) => XDP_PASS,
    }
}
```

#### KProbes (`netif_receive_skb`, `napi_consume_skb`)
- **Purpose**: Latency measurement and performance monitoring
- **Trace Points**: Network receive and skb consumption
- **Metrics**: Latency histograms, throughput counters

#### Tracepoints
- **Purpose**: Security event monitoring
- **Events**: Connection tracking, socket operations

### 2. P2P Networking Module (`libp2p`)

#### Architecture
```
┌─────────────────────────────────────────────────────────────────┐
│                      libp2p Swarm                               │
├──────────────┬──────────────┬──────────────┬───────────────────┤
│   Transport  │  Routing     │  Protocol    │   Multiplexing    │
│              │              │              │                   │
│  QUIC/TCP    │  Kademlia    │  Gossipsub   │  mplex/yamux      │
│  + TLS       │  (DHT)       │  1.1         │                   │
└──────────────┴──────────────┴──────────────┴───────────────────┘
```

#### Key Components
- **Swarm**: Connection management and protocol routing
- **Gossipsub 1.1**: Message propagation (mesh + random mesh)
- **mDNS**: Local peer discovery
- **QUIC**: Secure, low-latency transport
- **Peer Store**: Persistent peer address book

### 3. Consensus Module

#### Algorithm: Proof of Stake (PoS)
- **Quorum**: 2/3 majority required
- **Validator Selection**: Based on stake weight
- **Block Propagation**: Via Gossipsub
- **Finality**: Probabilistic (N confirmations)

#### Components
```rust
pub struct ConsensusEngine {
    stake_manager: StakeManager,
    block_pool: BlockPool,
    validator_set: ValidatorSet,
    quorum_checker: QuorumChecker,
}
```

### 4. Security Module

#### Replay Protection
```rust
pub struct ReplayProtection {
    db: Arc<RocksDB>,
    nonce_cache: HashMap<String, Vec<u64>>,  // sender -> nonces
    processed_txs: HashMap<String, u64>,     // tx_id -> timestamp
}
```
- Nonce-based deduplication
- Timestamp validation (configurable window)
- RocksDB-backed persistent state

#### Sybil Protection
```rust
pub struct SybilProtection {
    db: Arc<RocksDB>,
    max_connections_per_ip: u32,
    whitelist: HashSet<PeerId>,
}
```
- Connection limit per IP (default: 3)
- Peer whitelist for trusted nodes
- Automatic connection pruning

#### XDP Whitelist
```rust
// Proactive blocking via eBPF XDP
// IPs in blacklist are dropped at kernel level
```

### 5. Storage Module (RocksDB)

#### Data Model
```
┌─────────────────────────────────────────────────────────────────┐
│                      RocksDB Keyspace                           │
├──────────────┬──────────────┬───────────────────────────────────┤
│  blocks/     │  transactions/ │  state/                          │
│              │                │                                  │
│  blocks/{id} │  txs/{id}      │  stake/{peer_id}                │
│  blocks/head │  txs/pending   │  reputation/{peer_id}           │
│              │                │  nonce/{sender}                  │
└──────────────┴──────────────┴───────────────────────────────────┘
```

#### Configuration
```toml
[storage]
path = "/var/lib/ebpf-blockchain/data"
cache_size_mb = 1024
compression = "snappy"
max_open_files = 100
```

### 6. Metrics Module (Prometheus)

#### Metric Categories

| Category | Metrics | Type |
|----------|---------|------|
| Network | `peers_connected`, `messages_sent`, `messages_received` | Counter/Gauge |
| Consensus | `blocks_proposed`, `blocks_validated`, `quorum_reached` | Counter |
| Transactions | `transactions_processed`, `transactions_failed` | Counter |
| eBPF | `xdp_packets_processed`, `xdp_packets_dropped`, `ebpf_latency_us` | Histogram/Counter |
| System | `cpu_usage`, `memory_usage`, `disk_usage` | Gauge |

### 7. API Layer (Axum)

#### Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/node/info` | GET | Node information (version, uptime, peers) |
| `/api/v1/network/peers` | GET | Connected peers list |
| `/api/v1/network/config` | GET/PUT | Network configuration |
| `/api/v1/transactions` | POST | Submit new transaction |
| `/api/v1/transactions/{id}` | GET | Get transaction by ID |
| `/api/v1/blocks/{height}` | GET | Get block by height |
| `/api/v1/blocks/latest` | GET | Get latest block |
| `/api/v1/security/blacklist` | GET/PUT | Security blacklist management |
| `/api/v1/security/whitelist` | GET/PUT | Security whitelist management |
| `/metrics` | GET | Prometheus metrics |
| `/health` | GET | Health check endpoint |
| `/ws` | WebSocket | WebSocket for real-time events |

## Data Flow

### Transaction Flow

```
┌─────────┐     ┌──────────┐     ┌───────────┐     ┌────────────┐
│  Client  │────▶│   API    │────▶│ Transaction│────▶│  Consensus │
│          │     │  Layer   │     │    Pool    │     │   Engine   │
└─────────┘     └──────────┘     └───────────┘     └──────┬─────┘
                                                          │
                                                          ▼
┌─────────┐     ┌──────────┐     ┌───────────┐     ┌────────────┐
│  Client  │◀────│   API    │◀────│  Storage  │◀────│  Quorum    │
│          │     │  Layer   │     │  (RocksDB)│     │  Reached   │
└─────────┘     └──────────┘     └───────────┘     └────────────┘
```

### Network Message Flow

```
┌─────────────┐     ┌──────────┐     ┌───────────┐     ┌────────────┐
│  Remote     │────▶│   XDP    │────▶│   libp2p  │────▶│  Consensus │
│  Peer       │     │ Filtering│     │   Swarm    │     │   Handler  │
└─────────────┘     └──────────┘     └───────────┘     └────────────┘
        ▲                                                  │
        │                                                  ▼
        │                                          ┌────────────┐
        │                                          │  Gossipsub │
        │                                          │  Broadcast │
        │                                          └──────┬─────┘
        │                                                 │
        ▼                                                 ▼
┌─────────────┐     ┌──────────┐     ┌───────────┐     ┌────────────┐
│  Local      │◀────│   XDP    │◀────│   libp2p  │◀────│  Response  │
│  Peer       │     │  (egress)│     │   Swarm    │     │  Message   │
└─────────────┘     └──────────┘     └───────────┘     └────────────┘
```

### Observability Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                      eBPF Node                                  │
│                                                                 │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐                 │
│  │  eBPF    │    │  Rust    │    │  Rust    │                 │
│  │ Metrics  │    │  Logs    │    │  Traces  │                 │
│  └────┬─────┘    └────┬─────┘    └────┬─────┘                 │
│       │               │               │                        │
│       ▼               ▼               ▼                        │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐                 │
│  │Prometheus│    │Promtail  │    │OpenTeley-│                 │
│  │Exporter  │    │          │    │  try     │                 │
│  └────┬─────┘    └────┬─────┘    └────┬─────┘                 │
└───────┼───────────────┼───────────────┼────────────────────────┘
        │               │               │
        ▼               ▼               ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  Prometheus  │  │     Loki     │  │     Tempo    │
│  (:9090)     │  │    (:3100)   │  │    (:3200)   │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       ▼                 ▼                 ▼
┌──────────────────────────────────────────────────────┐
│                   Grafana (:3000)                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │  Metrics │  │   Logs   │  │  Traces  │          │
│  │Dashboard │  │ Dashboard│  │ Dashboard│          │
│  └──────────┘  └──────────┘  └──────────┘          │
└──────────────────────────────────────────────────────┘
```

## Architecture Decision Records

See [docs/adr/](docs/adr/) for detailed ADRs.

### Summary of Key Decisions

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [001](docs/adr/001-rust-implementation.md) | Choice of Rust | Accepted | 2026-01-15 |
| [002](docs/adr/002-consensus-algorithm.md) | Consensus Algorithm | Accepted | 2026-01-16 |
| [003](docs/adr/003-ebpf-for-security.md) | eBPF for Security | Accepted | 2026-01-17 |
| [004](docs/adr/004-rocksdb-storage.md) | Storage Choice | Accepted | 2026-01-18 |
| [005](docs/adr/005-libp2p-networking.md) | P2P Networking | Accepted | 2026-01-19 |
| [006](docs/adr/006-observability-stack.md) | Observability Stack | Accepted | 2026-01-20 |

## Security Architecture

### Defense in Depth

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 5: Application Security                                  │
│  - Transaction validation                                       │
│  - Consensus rules enforcement                                   │
│  - Replay protection                                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 4: Network Security                                      │
│  - libp2p encryption (TLS/QUIC)                                 │
│  - Peer authentication                                          │
│  - Sybil protection                                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 3: Host Security                                         │
│  - XDP blacklist/whitelist                                      │
│  - Firewall rules                                               │
│  - Rate limiting                                                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 2: Kernel Security (eBPF)                                │
│  - XDP packet filtering                                         │
│  - KProbe integrity monitoring                                  │
│  - Tracepoint security events                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 1: Hardware/Network Security                             │
│  - Physical security                                            │
│  - Network segmentation                                         │
│  - DDoS protection                                              │
└─────────────────────────────────────────────────────────────────┘
```

### Security Controls

| Control | Implementation | Location |
|---------|----------------|----------|
| Replay Protection | Nonce-based deduplication | User Space |
| Sybil Protection | IP-based connection limits | User Space |
| XDP Blacklist | Kernel-level packet drop | Kernel Space |
| XDP Whitelist | Kernel-level trusted IPs | Kernel Space |
| Peer Authentication | libp2p handshake | Network Layer |
| Transaction Signing | Cryptographic signatures | Application Layer |
| Data Integrity | RocksDB checksums | Storage Layer |

## Scalability

### Horizontal Scaling

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Node 1     │    │  Node 2     │    │  Node 3     │
│  (Bootstrap)│    │  (Validator)│    │  (Validator)│
└──────┬──────┘    └──────┬──────┘    └──────┬──────┘
       │                  │                  │
       └──────────────────┼──────────────────┘
                          │
                    Gossipsub Mesh
                          │
       ┌──────────────────┼──────────────────┐
       │                  │                  │
┌──────┴──────┐    ┌──────┴──────┐    ┌──────┴──────┐
│  Node 4     │    │  Node 5     │    │  Node 6     │
│  (Validator)│    │  (Validator)│    │  (Observer) │
└─────────────┘    └─────────────┘    └─────────────┘
```

### Vertical Scaling

| Resource | Scaling Strategy |
|----------|------------------|
| CPU | Multi-threaded Tokio runtime, parallel eBPF programs |
| Memory | RocksDB block cache, L1 transaction pool |
| Storage | RocksDB compaction, partitioning by height |
| Network | QUIC connection pooling, message batching |

### Configuration for Scaling

```toml
# High-performance configuration
[storage]
cache_size_mb = 4096
max_open_files = 500

[network]
max_connections = 500
message_queue_size = 10000

[consensus]
validator_timeout_ms = 2000
block_proposal_interval_ms = 1000
```

## File Structure Reference

```
ebpf-blockchain/
├── ebpf-node/                    # Main Rust project
│   ├── ebpf-node/               # User space binary
│   │   ├── src/
│   │   │   ├── main.rs          # Entry point
│   │   │   ├── consensus/       # Consensus module
│   │   │   ├── network/         # P2P networking
│   │   │   ├── security/        # Security modules
│   │   │   ├── storage/         # RocksDB storage
│   │   │   └── metrics/         # Prometheus metrics
│   │   └── Cargo.toml
│   ├── ebpf-node-ebpf/          # eBPF programs
│   │   ├── src/
│   │   │   ├── main.rs          # XDP program
│   │   │   └── lib.rs           # KProbes/Tracepoints
│   │   └── Cargo.toml
│   └── ebpf-node-common/        # Shared types
├── monitoring/                   # Observability stack
│   ├── docker-compose.yml
│   ├── prometheus/
│   ├── grafana/
│   ├── loki/
│   └── tempo/
├── ansible/                      # Deployment automation
│   ├── playbooks/
│   ├── roles/
│   └── inventory/
├── docs/                         # Documentation
│   ├── ARCHITECTURE.md
│   ├── API.md
│   ├── CONTRIBUTING.md
│   ├── OPERATIONS.md
│   ├── adr/
│   └── diagrams/
├── scripts/                      # Utility scripts
│   ├── backup.sh
│   └── restore.sh
├── tests/                        # Test suite
│   ├── integration/
│   └── unit/
└── .github/                      # CI/CD
    └── workflows/
```

## Conclusion

The eBPF Blockchain architecture provides a solid foundation for a secure, observable, and performant blockchain system. The separation between kernel space (eBPF) and user space (Rust) enables both high-performance packet processing and complex application logic, while the comprehensive observability stack ensures full visibility into system operation.
