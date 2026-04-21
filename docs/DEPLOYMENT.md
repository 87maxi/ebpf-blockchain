# eBPF Blockchain - Deployment Guide

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Deployment Options](#deployment-options)
- [Ansible Deployment](#ansible-deployment)
- [Docker Deployment](#docker-deployment)
- [Manual Deployment](#manual-deployment)
- [CI/CD Deployment](#cicd-deployment)
- [Post-Deployment](#post-deployment)
- [Rollback Procedures](#rollback-procedures)
- [Environment Configuration](#environment-configuration)

## Overview

This guide covers all deployment options for the eBPF Blockchain node:

1. **Ansible** - Recommended for production (automated)
2. **Docker** - For development and testing
3. **Manual** - For custom environments
4. **CI/CD** - Automated pipeline deployment

## Prerequisites

### System Requirements

| Resource | Minimum | Recommended | Production |
|----------|---------|-------------|------------|
| CPU | 2 cores | 4 cores | 8+ cores |
| RAM | 4 GB | 8 GB | 16+ GB |
| Storage | 20 GB | 50 GB SSD | 100+ GB SSD |
| Network | 100 Mbps | 1 Gbps | 10 Gbps |
| Kernel | 5.10 (BTF) | 6.1 (BTF) | 6.5+ (BTF) |

### Software Requirements

| Software | Version | Purpose |
|----------|---------|---------|
| Rust | Nightly | Build from source |
| Ansible | ≥ 2.12 | Automation |
| Docker | ≥ 20.10 | Monitoring stack |
| Git | ≥ 2.30 | Version control |

### Network Requirements

| Port | Protocol | Purpose |
|------|----------|---------|
| 9000 | TCP | P2P networking |
| 9001 | UDP (QUIC) | Secure P2P |
| 9090 | TCP | Prometheus metrics |
| 9091 | TCP | HTTP API |
| 9092 | TCP | WebSocket |
| 3000 | TCP | Grafana (monitoring) |
| 3100 | TCP | Loki (monitoring) |
| 3200 | TCP | Tempo (monitoring) |

## Deployment Options

### Option 1: Ansible Deployment (Recommended)

#### Step 1: Configure Inventory

```bash
# Create inventory file
cat > ansible/inventory/production.yml << EOF
[ebpf_nodes]
node1 ansible_host=192.168.1.100 ansible_user=deploy
node2 ansible_host=192.168.1.101 ansible_user=deploy
node3 ansible_host=192.168.1.102 ansible_user=deploy

[ebpf_nodes:vars]
ansible_python_interpreter=/usr/bin/python3
DATA_DIR=/var/lib/ebpf-blockchain/data
NETWORK_P2P_PORT=9000
METRICS_PORT=9090
EOF
```

#### Step 2: Configure Group Variables

```bash
cat > ansible/inventory/group_vars/all.yml << EOF
---
# General variables
ansible_become: true
ansible_become_method: sudo

# Node configuration
ebpf_data_dir: /var/lib/ebpf-blockchain/data
ebpf_config_dir: /etc/ebpf-blockchain
ebpf_log_dir: /var/log/ebpf-blockchain

# Network configuration
ebpf_p2p_port: 9000
ebpf_quic_port: 9001
ebpf_metrics_port: 9090
ebpf_rpc_port: 9091
ebpf_ws_port: 9092

# Security
ebpf_security_mode: strict
ebpf_replay_protection: true
ebpf_sybil_protection: true

# Monitoring
ebpf_monitoring_enabled: true
ebpf_prometheus_port: 9090
EOF
```

#### Step 3: Run Deployment

```bash
# Pre-deployment check
ansible-playbook ansible/playbooks/deploy.yml \
  -i ansible/inventory/production.yml \
  --check

# Deploy
ansible-playbook ansible/playbooks/deploy.yml \
  -i ansible/inventory/production.yml

# Post-deployment health check
ansible-playbook ansible/playbooks/health_check.yml \
  -i ansible/inventory/production.yml
```

#### Step 4: Verify Deployment

```bash
# Check all nodes
ansible ebpf_nodes -m ping

# Check service status
ansible ebpf_nodes -a "systemctl status ebpf-blockchain --no-pager"

# Check API
curl http://node1:9091/health
curl http://node2:9091/health
curl http://node3:9091/health
```

### Option 2: Docker Deployment

#### Step 1: Configure Monitoring Stack

```bash
cd monitoring

# Customize configuration if needed
cat > .env << EOF
GRAFANA_ADMIN_PASSWORD=your-secure-password
PROMETHEUS_RETENTION=30d
EOF
```

#### Step 2: Build and Run

```bash
# Build node image (optional, uses latest release by default)
cd ../ebpf-node
docker build -t ebpf-node:latest .
cd ../monitoring

# Start all services
docker-compose up -d

# Verify
docker-compose ps
```

#### Step 3: Access Services

| Service | URL | Credentials |
|---------|-----|-------------|
| Grafana | `http://localhost:3000` | admin/admin |
| Prometheus | `http://localhost:9090` | N/A |
| Loki | `http://localhost:3100` | N/A |
| Tempo | `http://localhost:3200` | N/A |

### Option 3: Manual Deployment

#### Step 1: Install Dependencies

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install dependencies
sudo apt install -y \
  curl \
  git \
  build-essential \
  pkg-config \
  libssl-dev \
  llvm \
  libelf-dev \
  python3 \
  python3-pip

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Ansible
pip3 install ansible
```

#### Step 2: Build from Source

```bash
# Clone repository
git clone https://github.com/ebpf-blockchain/ebpf-blockchain.git
cd ebpf-blockchain/ebpf-node

# Build release
cargo build --release

# Install binary
sudo cp target/release/ebpf-node /usr/local/bin/
sudo chmod +x /usr/local/bin/ebpf-node
```

#### Step 3: Configure

```bash
# Create directories
sudo mkdir -p /var/lib/ebpf-blockchain/data
sudo mkdir -p /var/lib/ebpf-blockchain/backups
sudo mkdir -p /etc/ebpf-blockchain
sudo mkdir -p /var/log/ebpf-blockchain

# Create configuration
sudo tee /etc/ebpf-blockchain/config.toml > /dev/null << EOF
[consensus]
mode = "proof_of_stake"
minimum_stake = 10000

[storage]
path = "/var/lib/ebpf-blockchain/data"

[network]
p2p_port = 9000
quic_port = 9001
max_connections = 100

[security]
mode = "strict"
replay_protection = true
sybil_protection = true

[metrics]
enabled = true
port = 9090

[logging]
level = "info"
format = "json"
EOF
```

#### Step 4: Create Systemd Service

```bash
sudo tee /etc/systemd/system/ebpf-blockchain.service > /dev/null << EOF
[Unit]
Description=eBPF Blockchain Node
After=network.target

[Service]
Type=simple
User=ebpf
Group=ebpf
ExecStart=/usr/local/bin/ebpf-node --config /etc/ebpf-blockchain/config.toml
Restart=on-failure
RestartSec=10
LimitNOFILE=65536
Environment=DATA_DIR=/var/lib/ebpf-blockchain/data
Environment=LOG_LEVEL=info
Environment=LOG_FORMAT=json

[Install]
WantedBy=multi-user.target
EOF

# Create service user
sudo useradd -r -s /bin/false ebpf
sudo chown -R ebpf:ebpf /var/lib/ebpf-blockchain
sudo chown -R ebpf:ebpf /var/log/ebpf-blockchain

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable ebpf-blockchain
sudo systemctl start ebpf-blockchain
```

#### Step 5: Configure Firewall

```bash
# Allow necessary ports
sudo ufw allow 9000/tcp   # P2P
sudo ufw allow 9001/udp   # QUIC
sudo ufw allow 9090/tcp   # Metrics
sudo ufw allow 9091/tcp   # API
sudo ufw allow 22/tcp     # SSH

# Enable firewall
sudo ufw enable
```

## CI/CD Deployment

### GitHub Actions

The CI/CD pipeline automatically deploys based on branch:

| Branch | Environment | Trigger |
|--------|-------------|---------|
| `develop` | Staging | Auto-push on merge |
| `main` | Production | Auto-push on merge |
| `tag` | Production | Manual trigger |

### Required GitHub Secrets

```yaml
# Staging environment
STAGING_SSH_PRIVATE_KEY: ${{ secrets.STAGING_SSH_PRIVATE_KEY }}
STAGING_HOST: ${{ secrets.STAGING_HOST }}
STAGING_USER: ${{ secrets.STAGING_USER }}

# Production environment
PRODUCTION_SSH_PRIVATE_KEY: ${{ secrets.PRODUCTION_SSH_PRIVATE_KEY }}
PRODUCTION_HOST: ${{ secrets.PRODUCTION_HOST }}
PRODUCTION_USER: ${{ secrets.PRODUCTION_USER }}
```

### Manual CI/CD Trigger

```bash
# Trigger deployment workflow
gh workflow run ci-cd.yml \
  -f environment=staging

# Or with production
gh workflow run ci-cd.yml \
  -f environment=production
```

## Post-Deployment

### Verification Checklist

```bash
# 1. Service running
systemctl status ebpf-blockchain

# 2. Health check
curl http://localhost:9091/health

# 3. Node info
curl http://localhost:9091/api/v1/node/info

# 4. Peer connections
curl http://localhost:9091/api/v1/network/peers

# 5. Metrics available
curl http://localhost:9090/metrics | grep ebpf

# 6. eBPF programs loaded
bpftool prog list | grep ebpf

# 7. Disk space
df -h /var/lib/ebpf-blockchain

# 8. Logs
journalctl -u ebpf-blockchain --since "5 minutes ago"
```

### Monitoring Setup

```bash
# Verify Prometheus scraping
curl http://localhost:9090/api/v1/targets

# Verify Grafana dashboards
curl -u admin:admin http://localhost:3000/api/dashboards/health

# Verify Loki receiving logs
curl http://localhost:3100/loki/api/v1/query \
  -d 'query={job="ebpf-node"}'
```

### Initial Configuration

```bash
# Set bootstrap peers (first node only)
cat >> /etc/ebpf-blockchain/config.toml << EOF
[network]
bootstrap_peers = ["/ip4/192.168.1.100/tcp/9000"]
EOF

# Reload
sudo systemctl reload ebpf-blockchain
```

## Rollback Procedures

### Ansible Rollback

```bash
# Rollback to previous version
ansible-playbook ansible/playbooks/rollback.yml \
  -i ansible/inventory/production.yml

# Verify rollback
ansible-playbook ansible/playbooks/health_check.yml \
  -i ansible/inventory/production.yml
```

### Manual Rollback

```bash
# Stop current version
sudo systemctl stop ebpf-blockchain

# Restore previous binary
sudo cp /usr/local/bin/ebpf-node.bak /usr/local/bin/ebpf-node

# Restore data from backup
/var/lib/ebpf-blockchain/bin/restore.sh \
  /var/lib/ebpf-blockchain/backups/ebpf-blockchain-backup-$(date -d yesterday +%Y%m%d).tar.gz

# Start
sudo systemctl start ebpf-blockchain
```

### Emergency Rollback

```bash
# Full disaster recovery
ansible-playbook ansible/playbooks/disaster_recovery.yml \
  -i ansible/inventory/production.yml
```

## Environment Configuration

### Development Environment

```bash
# .env.development
export DATA_DIR="/tmp/ebpf-blockchain-dev"
export LOG_LEVEL="debug"
export LOG_FORMAT="text"
export NETWORK_P2P_PORT=9000
export METRICS_PORT=9090
export SECURITY_MODE="permissive"
```

### Staging Environment

```bash
# .env.staging
export DATA_DIR="/var/lib/ebpf-blockchain/data"
export LOG_LEVEL="info"
export LOG_FORMAT="json"
export NETWORK_P2P_PORT=9000
export METRICS_PORT=9090
export SECURITY_MODE="moderate"
```

### Production Environment

```bash
# /etc/default/ebpf-blockchain
export DATA_DIR="/var/lib/ebpf-blockchain/data"
export LOG_LEVEL="warn"
export LOG_FORMAT="json"
export NETWORK_P2P_PORT=9000
export METRICS_PORT=9090
export SECURITY_MODE="strict"
export REPLAY_PROTECTION="true"
export SYBIL_PROTECTION="true"
```

### Environment Variables Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `DATA_DIR` | `/var/lib/ebpf-blockchain/data` | RocksDB data directory |
| `CONFIG_DIR` | `/etc/ebpf-blockchain` | Configuration directory |
| `LOG_DIR` | `/var/log/ebpf-blockchain` | Log directory |
| `LOG_LEVEL` | `info` | Logging level |
| `LOG_FORMAT` | `json` | Log format (json/text) |
| `NETWORK_P2P_PORT` | `9000` | P2P TCP port |
| `NETWORK_QUIC_PORT` | `9001` | QUIC UDP port |
| `METRICS_PORT` | `9090` | Prometheus port |
| `RPC_PORT` | `9091` | HTTP API port |
| `WS_PORT` | `9092` | WebSocket port |
| `SECURITY_MODE` | `strict` | Security level |
| `REPLAY_PROTECTION` | `true` | Enable replay protection |
| `SYBIL_PROTECTION` | `true` | Enable Sybil protection |
| `BOOTSTRAP_PEERS` | `` | Comma-separated bootstrap peers |
| `BACKUP_BASE_DIR` | `/var/lib/ebpf-blockchain/backups` | Backup directory |
| `RETENTION_DAYS` | `30` | Backup retention days |

---

## Troubleshooting Deployment

### Common Issues

#### Issue: Ansible Connection Failed

```bash
# Check SSH connectivity
ssh deploy@<host>

# Verify SSH keys
ssh-copy-id deploy@<host>

# Test Ansible connection
ansible <host> -m ping -i inventory/production.yml
```

#### Issue: eBPF Program Loading Failed

```bash
# Check kernel BTF
cat /sys/kernel/btf/vmlinux

# Check kernel version
uname -r

# Check eBPF support
ls /sys/fs/bpf/
```

#### Issue: Service Won't Start

```bash
# Check logs
journalctl -u ebpf-blockchain -n 100 --no-pager

# Check configuration
cat /etc/ebpf-blockchain/config.toml

# Check permissions
ls -la /var/lib/ebpf-blockchain/

# Test binary
/usr/local/bin/ebpf-node --help
```
