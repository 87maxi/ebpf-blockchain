# eBPF Blockchain - Operations Runbook

## Table of Contents

- [Overview](#overview)
- [Daily Operations](#daily-operations)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)
- [Scaling Procedures](#scaling-procedures)
- [Backup and Recovery](#backup-and-recovery)
- [Disaster Recovery](#disaster-recovery)
- [Incident Response](#incident-response)
- [Maintenance Windows](#maintenance-windows)

## Overview

This runbook provides procedures for day-to-day operations of the eBPF Blockchain node infrastructure. It is intended for DevOps engineers, SREs, and system administrators.

### Infrastructure Components

| Component | Purpose | Port | Service |
|-----------|---------|------|---------|
| eBPF Node | Main blockchain node | 9090-9092 | systemd |
| Prometheus | Metrics collection | 9090 | docker |
| Grafana | Dashboards | 3000 | docker |
| Loki | Log storage | 3100 | docker |
| Tempo | Distributed tracing | 3200 | docker |

### Environment Variables

```bash
# Source these before manual operations
source /etc/default/ebpf-blockchain

# Or set manually
export DATA_DIR="/var/lib/ebpf-blockchain/data"
export NETWORK_P2P_PORT=9000
export METRICS_PORT=9090
export LOG_LEVEL="info"
```

## Daily Operations

### Morning Health Check

```bash
#!/bin/bash
# /var/lib/ebpf-blockchain/bin/daily-check.sh

echo "=== eBPF Blockchain Daily Health Check ==="
echo "Date: $(date)"
echo ""

# 1. Service status
echo "--- Service Status ---"
systemctl status ebpf-blockchain --no-pager
echo ""

# 2. Health endpoint
echo "--- Health Check ---"
curl -s http://localhost:9091/health | jq .
echo ""

# 3. Peer count
echo "--- Peer Count ---"
curl -s http://localhost:9091/api/v1/network/peers | jq '.total'
echo ""

# 4. Blockchain height
echo "--- Blockchain Height ---"
curl -s http://localhost:9091/api/v1/blocks/latest | jq '.height'
echo ""

# 5. Disk usage
echo "--- Disk Usage ---"
df -h /var/lib/ebpf-blockchain
echo ""

# 6. Memory usage
echo "--- Memory Usage ---"
ps -o pid,vsz,rss,user,comm -p $(pgrep ebpf-node) 2>/dev/null || echo "Node not running"
echo ""

# 7. Recent logs
echo "--- Recent Errors (last 20) ---"
journalctl -u ebpf-blockchain --since "24 hours ago" -p err --no-pager -n 20
echo ""

# 8. eBPF programs
echo "--- eBPF Programs ---"
sudo bpftool prog list | grep ebpf || echo "No eBPF programs loaded"
```

### Weekly Tasks

| Task | Day | Command |
|------|-----|---------|
| Backup verification | Monday | `ansible-playbook ansible/playbooks/backup.yml` |
| Log review | Wednesday | `grep -c ERROR /var/log/ebpf-blockchain/app.log` |
| Security audit | Friday | `cargo audit` |
| Performance report | Friday | Grafana export |

## Monitoring

### Key Metrics to Monitor

#### Critical Metrics

| Metric | Threshold | Action |
|--------|-----------|--------|
| `peers_connected < 3` | Warning | Check network connectivity |
| `ebpf_xdp_packets_dropped > 1000/min` | Warning | Review blacklist rules |
| `node_up == 0` | Critical | Restart service immediately |
| `disk_usage > 90%` | Critical | Free space immediately |

#### Warning Metrics

| Metric | Threshold | Action |
|--------|-----------|--------|
| `peers_connected < 5` | Warning | Check bootstrap peers |
| `ebpf_latency_us p99 > 1000` | Warning | Profile eBPF programs |
| `consensus_block_interval > 30s` | Warning | Check validator connectivity |
| `transaction_pool_size > 1000` | Warning | Review transaction rate |

### Grafana Dashboards

| Dashboard | URL | Purpose |
|-----------|-----|---------|
| Health Overview | `http://localhost:3000/d/health` | Overall system health |
| Network P2P | `http://localhost:3000/d/network` | P2P network metrics |
| Consensus | `http://localhost:3000/d/consensus` | Consensus operation |
| Transactions | `http://localhost:3000/d/transactions` | Transaction processing |

### Prometheus Alerts

Alerts are defined in [monitoring/prometheus/alerts.yml](../monitoring/prometheus/alerts.yml):

```yaml
groups:
  - name: ebpf-blockchain
    rules:
      - alert: NodeDown
        expr: up{job="ebpf-node"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "eBPF Blockchain node is down"
          
      - alert: HighCPUUsage
        expr: process_cpu_seconds_total * 100 > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High CPU usage on eBPF node"
```

### Log Monitoring

```bash
# View structured logs
journalctl -u ebpf-blockchain -f --output json

# Search for errors
journalctl -u ebpf-blockchain -p err --since "1 hour ago"

# Filter by component
journalctl -u ebpf-blockchain -g "component=consensus"

# Loki query (via Grafana)
{job="ebpf-node"} | json | level="error"
```

## Troubleshooting

### Common Issues

#### Issue 1: Node Not Connecting to Peers

**Symptoms:**
- `peers_connected` shows 0
- No peer connections in logs

**Diagnosis:**
```bash
# Check firewall
sudo firewall-cmd --list-ports
# Expected: 9000/tcp, 9001/udp

# Check network
ss -tlnp | grep 9000
ss -unp | grep 9001

# Check bootstrap peers
cat /etc/ebpf-blockchain/config.toml | grep bootstrap

# Check logs
journalctl -u ebpf-blockchain -g "dial\|connect" --since "1 hour ago"
```

**Resolution:**
```bash
# Fix firewall
sudo firewall-cmd --add-port=9000/tcp --permanent
sudo firewall-cmd --add-port=9001/udp --permanent
sudo firewall-cmd --reload

# Verify connectivity
telnet <peer-ip> 9000

# Restart if needed
sudo systemctl restart ebpf-blockchain
```

---

#### Issue 2: High CPU Usage

**Symptoms:**
- CPU > 80% sustained
- Slow block proposal

**Diagnosis:**
```bash
# Identify process
top -p $(pgrep ebpf-node) -b -n 1

# Check which thread is using CPU
ps -T -p $(pgrep ebpf-node) -o pid,tid,pcpu,comm

# Check consensus metrics
curl http://localhost:9091/api/v1/node/info | jq .

# Check eBPF programs
sudo bpftool prog list
```

**Resolution:**
```bash
# Option 1: Reduce validator timeout
cat >> /etc/ebpf-blockchain/config.toml << EOF
[consensus]
validator_timeout_ms = 5000
EOF
sudo systemctl restart ebpf-blockchain

# Option 2: Reduce max connections
cat >> /etc/ebpf-blockchain/config.toml << EOF
[network]
max_connections = 50
EOF
sudo systemctl restart ebpf-blockchain

# Option 3: Profile and optimize
cargo install flamegraph
sudo flamegraph -p $(pgrep ebpf-node) -o profile.svg
```

---

#### Issue 3: High eBPF Packet Drop Rate

**Symptoms:**
- `ebpf_xdp_packets_dropped` increasing rapidly
- Network latency spikes

**Diagnosis:**
```bash
# Check XDP drop rate
curl -s http://localhost:9090/metrics | grep xdp_packets_dropped

# Check blacklist
curl -s http://localhost:9091/api/v1/security/blacklist | jq .

# Check kernel logs
dmesg | grep -i xdp | tail -20
```

**Resolution:**
```bash
# Review and clean blacklist
curl -s http://localhost:9091/api/v1/security/blacklist

# Remove stale entries
curl -X PUT http://localhost:9091/api/v1/security/blacklist \
  -H "Content-Type: application/json" \
  -d '{"action": "remove", "ip": "192.168.1.200"}'

# Reload eBPF program
sudo systemctl restart ebpf-blockchain
```

---

#### Issue 4: RocksDB Performance Degradation

**Symptoms:**
- Slow block validation
- High disk I/O

**Diagnosis:**
```bash
# Check disk I/O
iostat -x 1 5

# Check RocksDB stats
curl -s http://localhost:9090/metrics | grep rocksdb

# Check database size
du -sh /var/lib/ebpf-blockchain/data/
```

**Resolution:**
```bash
# Trigger manual compaction
curl -X POST http://localhost:9091/api/v1/storage/compact

# Increase cache size
cat >> /etc/ebpf-blockchain/config.toml << EOF
[storage]
cache_size_mb = 2048
EOF
sudo systemctl restart ebpf-blockchain

# Clean old blocks (if configured)
/var/lib/ebpf-blockchain/bin/backup.sh --cleanup
```

---

#### Issue 5: Disk Space Critical

**Symptoms:**
- `disk_usage > 90%`
- Write failures

**Diagnosis:**
```bash
# Find large files
du -sh /var/lib/ebpf-blockchain/* | sort -rh | head -10

# Check journal size
journalctl --disk-usage

# Find old logs
find /var/log/ebpf-blockchain -name "*.log" -mtime +7
```

**Resolution:**
```bash
# Clean old logs
find /var/log/ebpf-blockchain -name "*.log" -mtime +7 -delete
journalctl --vacuum-time=3d

# Clean old backups
/var/lib/ebpf-blockchain/bin/backup.sh --cleanup-only

# Compress RocksDB
sudo systemctl stop ebpf-blockchain
# RocksDB compaction happens on startup
sudo systemctl start ebpf-blockchain
```

## Scaling Procedures

### Horizontal Scaling - Adding a New Node

#### Step 1: Prepare New Node

```bash
# On new server
sudo apt update
sudo apt install -y lxd ansible

# Configure LXC
lxc launch ubuntu:22.04 ebpf-node-<N>
lxc config set ebpf-node-<N> security.privileged true

# Fix network
ansible-playbook ansible/playbooks/fix_network.yml \
  -i ansible/inventory/new_node.yml
```

#### Step 2: Deploy eBPF Node

```bash
# Deploy
ansible-playbook ansible/playbooks/deploy.yml \
  -i ansible/inventory/new_node.yml

# Verify
ansible-playbook ansible/playbooks/health_check.yml \
  -i ansible/inventory/new_node.yml
```

#### Step 3: Verify Integration

```bash
# Check peer connections
curl http://localhost:9091/api/v1/network/peers

# Verify consensus participation
curl http://localhost:9091/api/v1/node/info | jq .is_validator
```

### Vertical Scaling - Increasing Resources

#### Increase Memory

```bash
# Edit RocksDB configuration
cat > /etc/ebpf-blockchain/config.toml << EOF
[storage]
cache_size_mb = 4096
max_open_files = 500
EOF

# Restart
sudo systemctl restart ebpf-blockchain
```

#### Increase Network Connections

```bash
# Edit network configuration
cat > /etc/ebpf-blockchain/config.toml << EOF
[network]
max_connections = 200
message_queue_size = 10000
EOF

# Restart
sudo systemctl restart ebpf-blockchain
```

## Backup and Recovery

### Backup Procedures

#### Automated Backup

Backups run automatically via cron:

```bash
# Check cron job
crontab -l | grep backup
# Expected: 0 2 * * * /var/lib/ebpf-blockchain/bin/backup.sh
```

#### Manual Backup

```bash
# Create backup
/var/lib/ebpf-blockchain/bin/backup.sh

# Backup with specific retention
RETENTION_DAYS=7 /var/lib/ebpf-blockchain/bin/backup.sh

# Dry run (test mode)
DRY_RUN=true /var/lib/ebpf-blockchain/bin/backup.sh
```

#### Verify Backups

```bash
# List recent backups
ls -la /var/lib/ebpf-blockchain/backups/

# Verify integrity
cd /var/lib/ebpf-blockchain/backups
tar -tzf ebpf-blockchain-backup-*.tar.gz | head -20
```

### Restore Procedures

#### Full Restore

```bash
# Stop service
sudo systemctl stop ebpf-blockchain

# Restore from backup
/var/lib/ebpf-blockchain/bin/restore.sh \
  /var/lib/ebpf-blockchain/backups/ebpf-blockchain-backup-20260421.tar.gz \
  --force

# Start service
sudo systemctl start ebpf-blockchain

# Verify
curl http://localhost:9091/health
```

#### Partial Restore

```bash
# Restore only RocksDB
tar -xzf backup.tar.gz -C /var/lib/ebpf-blockchain/ data/rocksdb/

# Restore only config
tar -xzf backup.tar.gz -C / etc/ebpf-blockchain/

# Restore only logs
tar -xzf backup.tar.gz -C /var/log/ebpf-blockchain/
```

## Disaster Recovery

### DR Overview

Disaster recovery follows a 6-phase approach:

1. **Stop** - Halt all services
2. **Assess** - Determine scope of damage
3. **Restore** - Restore from latest known-good backup
4. **Rebuild** - Rebuild from source if needed
5. **Start** - Start services in order
6. **Validate** - Post-recovery validation

### DR Playbook

```bash
# Execute full disaster recovery
ansible-playbook ansible/playbooks/disaster_recovery.yml \
  -i ansible/inventory/production.yml

# Or execute phases manually:
# Phase 1: Stop
sudo systemctl stop ebpf-blockchain

# Phase 2: Assess
ansible-playbook ansible/playbooks/health_check.yml

# Phase 3: Restore
/var/lib/ebpf-blockchain/bin/restore.sh <latest-backup> --force

# Phase 4: Rebuild (if needed)
cd /opt/ebpf-blockchain/ebpf-node
git pull
cargo build --release
sudo cp target/release/ebpf-node /usr/local/bin/

# Phase 5: Start
sudo systemctl start ebpf-blockchain

# Phase 6: Validate
ansible-playbook ansible/playbooks/health_check.yml
```

### RTO and RPO

| Metric | Target | Current |
|--------|--------|---------|
| RTO (Recovery Time) | 4 hours | ~3 hours |
| RPO (Recovery Point) | 24 hours | ~24 hours (daily backups) |
| DR Test Frequency | Quarterly | Annual |

## Incident Response

### Severity Levels

| Severity | Description | Response Time | Update Frequency |
|----------|-------------|---------------|------------------|
| SEV-1 | Complete outage | 15 min | 30 min |
| SEV-2 | Major degradation | 30 min | 1 hour |
| SEV-3 | Minor degradation | 2 hours | 4 hours |
| SEV-4 | Cosmetic/annoyance | 24 hours | Weekly |

### Incident Response Procedure

```
1. DETECT    - Alert triggered or reported
2. TRIAGE    - Assess severity and impact
3. CONTAIN   - Prevent further damage
4. RESOLVE   - Fix the issue
5. VERIFY    - Confirm resolution
6. POSTMORTEM - Document lessons learned
```

### Contact Escalation

| Level | Role | Contact |
|-------|------|---------|
| L1 | On-call Engineer | PagerDuty |
| L2 | Team Lead | Phone |
| L3 | CTO | Phone |

## Maintenance Windows

### Scheduled Maintenance

| Type | Frequency | Duration | Window |
|------|-----------|----------|--------|
| Security updates | Monthly | 1 hour | Sunday 02:00-03:00 |
| Software updates | Quarterly | 2 hours | Saturday 02:00-04:00 |
| DR test | Quarterly | 4 hours | Scheduled |
| Capacity review | Monthly | 1 hour | First Monday |

### Maintenance Checklist

```bash
# Pre-maintenance
sudo systemctl status ebpf-blockchain
/var/lib/ebpf-blockchain/bin/backup.sh

# Post-maintenance
sudo systemctl restart ebpf-blockchain
ansible-playbook ansible/playbooks/health_check.yml
curl http://localhost:9091/health
```

---

## Appendix

### Useful Commands Quick Reference

```bash
# Service management
sudo systemctl status ebpf-blockchain
sudo systemctl restart ebpf-blockchain
sudo systemctl stop ebpf-blockchain

# Logs
journalctl -u ebpf-blockchain -f
journalctl -u ebpf-blockchain -p err --since "1 hour ago"

# Health
curl http://localhost:9091/health
curl http://localhost:9091/api/v1/node/info

# Network
curl http://localhost:9091/api/v1/network/peers

# Metrics
curl http://localhost:9090/metrics

# Backup
/var/lib/ebpf-blockchain/bin/backup.sh
/var/lib/ebpf-blockchain/bin/restore.sh <file>

# eBPF
sudo bpftool prog list
sudo bpftool map list

# Ansible
ansible-playbook ansible/playbooks/deploy.yml
ansible-playbook ansible/playbooks/health_check.yml
ansible-playbook ansible/playbooks/rollback.yml
```

### File Locations

| Path | Purpose |
|------|---------|
| `/var/lib/ebpf-blockchain/data/` | RocksDB data |
| `/var/lib/ebpf-blockchain/backups/` | Backup files |
| `/etc/ebpf-blockchain/config.toml` | Configuration |
| `/var/log/ebpf-blockchain/` | Log files |
| `/usr/local/bin/ebpf-node` | Binary |
| `/etc/default/ebpf-blockchain` | Environment variables |
| `/etc/systemd/system/ebpf-blockchain.service` | Systemd unit |
