#!/bin/bash
# =============================================================================
# Stop Local eBPF Blockchain Nodes
# =============================================================================
# Usage: ./services/stop-local-nodes.sh
# =============================================================================

echo "Stopping all eBPF nodes..."
pkill -f "ebpf-node.*--iface" 2>/dev/null || true
sleep 2

echo "Checking for remaining processes..."
if pgrep -f "ebpf-node.*--iface" > /dev/null; then
    echo "Force killing..."
    pkill -9 -f "ebpf-node.*--iface" 2>/dev/null || true
fi

echo "All nodes stopped."
