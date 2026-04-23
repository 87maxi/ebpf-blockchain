#!/bin/bash
# SSH Log Scraper for Promtail
# Collects logs from remote eBPF nodes via SSH and outputs them in a format
# that can be consumed by a external log receiver that sends to Loki.
#
# Usage: ./ssh-log-scraper.sh
# Output: JSON lines to stdout (can be piped to llogtail or other receivers)

set -euo pipefail

# eBPF nodes configuration
declare -A NODES=(
    ["ebpf-node-1"]="maxi@192.168.2.210"
    ["ebpf-node-2"]="maxi@192.168.2.211"
    ["ebpf-node-3"]="maxi@192.168.2.212"
)

SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5"

for node_name in "${!NODES[@]}"; do
    node_user="${NODES[$node_name]%%@*}"
    node_host="${NODES[$node_name]##*@}"
    
    # Try to read the log file via SSH
    log_content=$(ssh $SSH_OPTS "${NODES[$node_name]}" \
        "tail -n 100 /var/log/ebpf-node/ebpf-node.log 2>/dev/null" 2>/dev/null) || {
        echo "{\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%S.000Z)\",\"level\":\"WARN\",\"message\":\"Cannot connect to $node_name\",\"target\":\"$node_name\",\"event\":\"ssh_connect_failed\"}"
        continue
    }
    
    # Output each line with node label
    while IFS= read -r line; do
        if [ -n "$line" ]; then
            # Add node label to the JSON log
            echo "$line" | jq -c --arg node "$node_name" '. + {"node": $node}' 2>/dev/null || \
            echo "{\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%S.000Z)\",\"level\":\"INFO\",\"message\":\"$line\",\"target\":\"$node_name\",\"event\":\"log_line\"}"
        fi
    done <<< "$log_content"
done
