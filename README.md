# eBPF Blockchain

![CI/CD Pipeline](https://github.com/ebpf-blockchain/ebpf-blockchain/workflows/CI/CD%20Pipeline/badge.svg)
![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)
![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)

## 📋 Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [API Documentation](#api-documentation)
- [Observability](#observability)
- [Deployment](#deployment)
- [Contributing](#contributing)
- [Project Structure](#project-structure)
- [Archive](#archive)
- [License](#license)

## Overview

[eBPF Blockchain](https://github.com/ebpf-blockchain/ebpf-blockchain) is an experimental blockchain system implemented in Rust that combines:

- **eBPF** for kernel-level network observability and security
- **libp2p** for decentralized P2P networking (Gossipsub 1.1)
- **Rust** for memory safety and high performance
- **RocksDB** for persistent data storage
- **Prometheus + Grafana + Loki** for complete observability

This POC demonstrates how eBPF capabilities can be integrated with blockchain decentralization to create a robust, secure, and highly observable distributed system.

## Features

### Security
- ✅ **eBPF XDP Filtering** - Proactive IP blocking at kernel level
- ✅ **Replay Protection** - Nonce-based transaction deduplication
- ✅ **Sybil Protection** - Peer connection limits per IP
- ✅ **Whitelist XDP** - Preventive IP blocking
- ✅ **KProbes & Tracepoints** - Kernel-level monitoring

### Consensus & Networking
- ✅ **Proof of Stake** - Consensus mechanism with 2/3 quorum
- ✅ **P2P Networking** - libp2p with Gossipsub 1.1
- ✅ **mDNS Discovery** - Automatic peer discovery
- ✅ **QUIC Transport** - Secure, low-latency communication

### Observability
- ✅ **Prometheus Metrics** - Network, consensus, transaction, eBPF, system metrics
- ✅ **Grafana Dashboards** - Health overview, network P2P, consensus, transactions
- ✅ **Structured Logging** - JSON format logs with Loki integration
- ✅ **Alerts** - Prometheus alerting rules

### Automation
- ✅ **CI/CD Pipeline** - GitHub Actions with 6 stages
- ✅ **Ansible Playbooks** - Deploy, rollback, health check, backup, disaster recovery
- ✅ **Automated Backups** - With retention policy and integrity verification
- ✅ **Disaster Recovery** - 6-phase recovery process

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      CLIENT/USER                                │
│                    (CLI / Web Interface)                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        USER SPACE                               │
│  ┌───────────────┐  ┌─────────────────┐  ┌─────────────────┐   │
│  │    P2P        │  │   CONSENSUS     │  │    STORAGE      │   │
│  │  Networking   │  │  Mechanism      │  │   (RocksDB)     │   │
│  │  (libp2p)     │  │  (PoS 2/3)      │  │                 │   │
│  └───────────────┘  └─────────────────┘  └─────────────────┘   │
│           │                │                  │             │
│           ▼                ▼                  ▼             │
│    ┌───────────────┐  ┌─────────────────┐  ┌─────────────────┐   │
│    │   METRICS     │  │   SECURITY      │  │    API          │   │
│    │ (Prometheus)  │  │  (Detector)     │  │   (Axum)        │   │
│    └───────────────┘  └─────────────────┘  └─────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       KERNEL SPACE (eBPF)                       │
│  ┌───────────────┐  ┌─────────────────┐  ┌─────────────────┐   │
│  │    XDP        │  │    KPROBES      │  │   TRACEPOINTS   │   │
│  │  Filtering    │  │  Latency        │  │   Monitoring    │   │
│  │  (Security)   │  │  (Monitoring)   │  │  (Security)     │   │
│  └───────────────┘  └─────────────────┘  └─────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

For detailed architecture documentation, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Quick Start

### Prerequisites

- **Linux Kernel** ≥ 5.10 with BTF enabled
- **Rust Nightly** (latest)
- **Docker** ≥ 20.10 (for monitoring stack)
- **Ansible** ≥ 2.12 (for deployment)

### Build and Run

```bash
# Clone the repository
git clone https://github.com/ebpf-blockchain/ebpf-blockchain.git
cd ebpf-blockchain

# Build the node
cd ebpf-node
cargo build --release

# Run the node
./target/release/ebpf-node
```

### Start Monitoring Stack

```bash
# From the monitoring directory
cd monitoring

# Start all services
docker-compose up -d

# Access Grafana at http://localhost:3000
# Default credentials: admin/admin
```

## Installation

### From Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/ebpf-blockchain/ebpf-blockchain.git
cd ebpf-blockchain/ebpf-node
cargo build --release

# Install binary
sudo cp target/release/ebpf-node /usr/local/bin/
```

### Deploy with Ansible

```bash
# Configure inventory
cp ansible/inventory/hosts.yml.example ansible/inventory/hosts.yml
# Edit ansible/inventory/hosts.yml with your server details

# Run deployment
ansible-playbook ansible/playbooks/deploy.yml -i ansible/inventory/hosts.yml
```

### System Requirements

| Resource | Minimum | Recommended |
|----------|---------|-------------|
| CPU | 2 cores | 4+ cores |
| RAM | 4 GB | 8+ GB |
| Storage | 20 GB | 50+ GB SSD |
| Network | 100 Mbps | 1 Gbps |
| Kernel | 5.10 (with BTF) | 6.1+ (with BTF) |

## Configuration

### Environment Variables

```bash
# Node configuration
export DATA_DIR="/var/lib/ebpf-blockchain/data"
export NETWORK_P2P_PORT=9000
export NETWORK_QUIC_PORT=9001
export METRICS_PORT=9090
export RPC_PORT=9091
export WS_PORT=9092

# Security
export SECURITY_MODE="strict"
export REPLAY_PROTECTION="true"
export SYBIL_PROTECTION="true"

# Logging
export LOG_LEVEL="info"
export LOG_FORMAT="json"

# Bootstrap peers
export BOOTSTRAP_PEERS="/ip4/192.168.1.100/tcp/9000"
```

### Configuration File

```toml
# config/config.toml
[consensus]
mode = "proof_of_stake"
minimum_stake = 10000
validator_timeout_ms = 5000

[storage]
path = "/var/lib/ebpf-blockchain/data"
cache_size_mb = 1024

[network]
p2p_port = 9000
quic_port = 9001
max_connections = 100

[security]
mode = "strict"
replay_protection = true
sybil_protection = true
blacklist_enabled = true

[metrics]
enabled = true
port = 9090

[logging]
level = "info"
format = "json"
```

## Usage

### Command Line Interface

```bash
# Start node
ebpf-node --config config/config.toml

# View node information
curl http://localhost:9091/api/v1/node/info

# View connected peers
curl http://localhost:9091/api/v1/network/peers

# Submit transaction
curl -X POST http://localhost:9091/api/v1/transactions \
  -H "Content-Type: application/json" \
  -d '{"id": "tx-001", "data": "hello", "nonce": 1}'

# View metrics
curl http://localhost:9090/metrics
```

### Backup and Restore

```bash
# Create backup
/var/lib/ebpf-blockchain/bin/backup.sh

# Restore from backup
/var/lib/ebpf-blockchain/bin/restore.sh /path/to/backup.tar.gz --force

# List recent backups
ls -la /var/lib/ebpf-blockchain/backups/
```

## API Documentation

### Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/node/info` | Node information |
| GET | `/api/v1/network/peers` | Connected peers list |
| POST | `/api/v1/transactions` | Create transaction |
| GET | `/api/v1/blocks/{height}` | Get block by height |
| GET | `/metrics` | Prometheus metrics |
| GET | `/health` | Health check |
| GET | `/ws` | WebSocket connection |

For complete API documentation, see [docs/API.md](docs/API.md) and [docs/openapi.yml](docs/openapi.yml).

## Observability

### Grafana Dashboards

| Dashboard | URL | Description |
|-----------|-----|-------------|
| Health Overview | `http://localhost:3000/health` | System health status |
| Network P2P | `http://localhost:3000/network` | P2P network metrics |
| Consensus | `http://localhost:3000/consensus` | Consensus metrics |
| Transactions | `http://localhost:3000/transactions` | Transaction metrics |

### Prometheus Alerts

Alerts are configured in [monitoring/prometheus/alerts.yml](monitoring/prometheus/alerts.yml):

| Alert | Condition | Severity |
|-------|-----------|----------|
| NodeDown | Service stopped | Critical |
| HighCPUUsage | CPU > 80% | Warning |
| LowPeerCount | Peers < 3 | Warning |
| HighLatency | Latency > 100ms | Warning |
| DiskSpaceLow | Disk > 90% | Critical |

### Structured Logs

Logs are emitted in JSON format for Loki ingestion:

```json
{
  "timestamp": "2026-04-21T10:00:00Z",
  "level": "INFO",
  "component": "consensus",
  "message": "Block proposed",
  "block_height": 42,
  "peer_id": "12D3Koo..."
}
```

## Deployment

### Ansible Playbooks

| Playbook | Description | Usage |
|----------|-------------|-------|
| `deploy.yml` | Deploy node with rollback | `ansible-playbook ansible/playbooks/deploy.yml` |
| `rollback.yml` | Rollback to previous version | `ansible-playbook ansible/playbooks/rollback.yml` |
| `health_check.yml` | Post-deployment health check | `ansible-playbook ansible/playbooks/health_check.yml` |
| `backup.yml` | Execute backup | `ansible-playbook ansible/playbooks/backup.yml` |
| `disaster_recovery.yml` | Full disaster recovery | `ansible-playbook ansible/playbooks/disaster_recovery.yml` |

### CI/CD Pipeline

The project uses GitHub Actions with 6 stages:

1. **Lint** - cargo fmt, clippy, security audit
2. **Test** - Unit and integration tests
3. **Build** - Release binary, package creation
4. **Deploy Staging** - Auto-deploy to staging (develop branch)
5. **Deploy Production** - Auto-deploy to production (main branch)
6. **Backup Verification** - Verify backup scripts

## Contributing

Please read [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for details on how to contribute to this project.

### Quick Contribution Steps

```bash
# 1. Fork the repository
# 2. Create a feature branch
git checkout -b feature/my-feature

# 3. Make changes
# 4. Run tests
cargo test

# 5. Run linter
cargo fmt
cargo clippy --all-targets

# 6. Commit and push
git commit -m "feat: add my feature"
git push origin feature/my-feature

# 7. Open a Pull Request
```

## License

This project is licensed under the MIT License - see the [MIT-LICENSE](ebpf-node/LICENSE-MIT) file for details.

## Project Status

| Phase | Status | Description |
|-------|--------|-------------|
| Fase 1: Security | ✅ Complete | Replay protection, Sybil protection, XDP whitelist |
| Fase 2: Observability | ✅ Complete | Prometheus, Grafana, Loki, structured logging |
| Fase 3: Automation | ✅ Complete | CI/CD, Ansible, backups, disaster recovery |
| Fase 4: Documentation | ✅ Complete | ADRs, API docs, Architecture, Operations, Deployment, Contributing |

## Project Structure

```
ebpf-blockchain/
├── README.md                    # This file (project overview)
├── docs/                        # Active documentation (implemented)
│   ├── ARCHITECTURE.md          # System architecture & design
│   ├── API.md                   # REST API documentation
│   ├── CONTRIBUTING.md          # Contribution guidelines
│   ├── DEPLOYMENT.md            # Deployment procedures
│   ├── OPERATIONS.md            # Operations runbook
│   ├── openapi.yml              # OpenAPI 3.0.3 specification
│   └── adr/                     # Architecture Decision Records
│       ├── 001-rust-implementation.md
│       ├── 002-consensus-algorithm.md
│       ├── 003-ebpf-for-security.md
│       ├── 004-rocksdb-storage.md
│       ├── 005-libp2p-networking.md
│       └── 006-observability-stack.md
├── archive/                     # Historical/archived documentation
│   ├── README.md                # Archive organization guide
│   ├── plans/                   # Implementation plans (completed)
│   ├── specs/                   # Technical specifications (implemented)
│   ├── legacy/                  # Obsolete documentation
│   └── references/              # Reference materials
├── ebpf-node/                   # Rust node implementation
│   ├── ebpf-node/               # User space (libp2p, API, metrics)
│   ├── ebpf-node-ebpf/          # Kernel space (eBPF programs)
│   └── ebpf-node-common/        # Shared code
├── ansible/                     # Deployment automation
│   ├── playbooks/               # Deploy, rollback, health check
│   ├── roles/                   # Ansible roles
│   └── inventory/               # Host configuration
├── monitoring/                  # Observability stack
│   ├── prometheus/              # Metrics collection & alerts
│   ├── grafana/                 # Dashboards
│   ├── loki/                    # Log aggregation
│   ├── tempo/                   # Distributed tracing
│   └── promtail/                # Log shipping
├── scripts/                     # Utility scripts
├── tests/                       # Integration tests
└── tools/                       # Development tools
```

## Archive

The [`archive/`](archive/) directory contains historical documentation that has been superseded by the active documentation in [`docs/`](docs/). See [`archive/README.md`](archive/README.md) for:

- **What was archived and why**
- **Justification for implementation decisions** (eBPF, Rust, PoS, etc.)
- **Where to find information** that was moved from obsolete documents

| Archive Subdirectory | Contents |
|---------------------|----------|
| [`archive/plans/`](archive/plans/) | Implementation plans and phase summaries |
| [`archive/specs/`](archive/specs/) | Technical specifications by phase |
| [`archive/legacy/`](archive/legacy/) | Obsolete documentation replaced by active docs |
| [`archive/references/`](archive/references/) | Reference materials (RFC, tutorials, configs) |

## Contact

- **Project Documentation**: [docs/](docs/)
- **Issue Tracker**: [GitHub Issues](https://github.com/ebpf-blockchain/ebpf-blockchain/issues)
- **Architecture**: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- **Archive**: [archive/README.md](archive/README.md)

## Acknowledgments

- **Aya** - eBPF framework for Rust
- **libp2p** - P2P networking library
- **Tokio** - Async runtime
- **RocksDB** - Embedded database
- **Prometheus & Grafana** - Observability stack
