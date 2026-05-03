#!/bin/bash
# =============================================================================
# Start Local eBPF Blockchain Nodes via LXD Bridge
# =============================================================================
# Usage: ./services/start-local-nodes.sh
# =============================================================================
# This script starts all nodes (normal, victim, attacker) on the lxdbr1 bridge
# All nodes are accessible via LAN at 192.168.2.x subnet
# =============================================================================

set -e

PROJECT_DIR="/home/maxi/Documentos/source/ebpf-blockchain"
LXC_BRIDGE="lxdbr1"
SUBNET="192.168.2"
GATEWAY="192.168.2.1"
LOG_DIR="$PROJECT_DIR/logs"

# Node configuration: NAME:IP:P2P_PORT:RPC_PORT:METRICS_PORT
NODES=(
    "ebpf-node-1:192.168.2.210:50000:8080:9090"
    "ebpf-node-2:192.168.2.211:50001:8080:9090"
    "ebpf-node-3:192.168.2.212:50002:8080:9090"
    "ebpf-victim-1:192.168.2.220:50003:8080:9090"
    "ebpf-victim-2:192.168.2.221:50004:8080:9090"
    "ebpf-attacker-1:192.168.2.230:50005:8080:9090"
    "ebpf-attacker-2:192.168.2.231:50006:8080:9090"
)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  eBPF Blockchain - LXD Bridge Lab${NC}"
echo -e "${GREEN}============================================${NC}"
echo -e "${BLUE}  Network: ${LXC_BRIDGE} (${SUBNET}.0/24)${NC}"
echo -e "${BLUE}  Gateway: ${GATEWAY}${NC}"
echo ""

# Verify LXD bridge exists
if ! lxc network show "$LXC_BRIDGE" >/dev/null 2>&1; then
    echo -e "${RED}ERROR: LXD bridge $LXC_BRIDGE not found${NC}"
    echo ""
    echo "Create it with:"
    echo "  ansible-playbook ansible/playbooks/deploy_cluster.yml"
    echo ""
    echo "Or manually:"
    echo "  lxc network create $LXC_BRIDGE ipv4.address=${SUBNET}.1/24 ipv4.nat=true ipv6.address=none"
    exit 1
fi

# Verify all required containers exist
MISSING_CONTAINERS=()
for NODE_CONFIG in "${NODES[@]}"; do
    NODE_NAME=$(echo "$NODE_CONFIG" | cut -d: -f1)
    if ! lxc info "$NODE_NAME" >/dev/null 2>&1; then
        MISSING_CONTAINERS+=("$NODE_NAME")
    fi
done

if [ ${#MISSING_CONTAINERS[@]} -gt 0 ]; then
    echo -e "${YELLOW}WARNING: Missing containers: ${MISSING_CONTAINERS[*]}${NC}"
    echo "These nodes will be skipped. To create them:"
    echo "  ansible-playbook ansible/playbooks/deploy_cluster.yml"
    echo ""
fi

# Verify binary exists (for local build reference)
BINARY="$PROJECT_DIR/ebpf-node/target/release/ebpf-node"
if [ ! -f "$BINARY" ]; then
    echo -e "${YELLOW}WARNING: Local binary not found at $BINARY${NC}"
    echo "Nodes will use binaries installed inside LXC containers."
fi

echo -e "${YELLOW}Starting nodes on $LXC_BRIDGE ($SUBNET.0/24)...${NC}"
echo ""

# Start each node via LXC exec
STARTED_NODES=()
FAILED_NODES=()

for NODE_CONFIG in "${NODES[@]}"; do
    IFS=':' read -r NODE_NAME NODE_IP P2P_PORT RPC_PORT METRICS_PORT <<< "$NODE_CONFIG"
    
    # Skip missing containers
    if ! lxc info "$NODE_NAME" >/dev/null 2>&1; then
        echo -e "${YELLOW}SKIP  $NODE_NAME ($NODE_IP) - Container not found${NC}"
        continue
    fi
    
    echo -e "${GREEN}START $NODE_NAME ($NODE_IP)...${NC}"
    echo "  P2P: $P2P_PORT | RPC: $RPC_PORT | Metrics: $METRICS_PORT"
    
    # Create log directory
    mkdir -p "$LOG_DIR/$NODE_NAME"
    
    # Check if node is already running
    if lxc exec "$NODE_NAME" -- pgrep -f "ebpf-node" >/dev/null 2>&1; then
        echo -e "${YELLOW}WARN  $NODE_NAME is already running${NC}"
        STARTED_NODES+=("$NODE_NAME")
        continue
    fi
    
    # Start node in LXC container
    if lxc exec "$NODE_NAME" -- bash -c "
        cd /root/ebpf-blockchain/ebpf-node 2>/dev/null || cd /root/ebpf-blockchain/ebpf-node-ebpf 2>/dev/null || exit 1
        source /root/.cargo/env 2>/dev/null || true
        
        # Try release binary first, then debug
        if [ -f ./target/release/ebpf-node ]; then
            BINARY=./target/release/ebpf-node
        elif [ -f ./target/debug/ebpf-node ]; then
            BINARY=./target/debug/ebpf-node
        else
            echo 'Binary not found'
            exit 1
        fi
        
        nohup \$BINARY \
            --iface eth0 \
            --listen-addresses '/ip4/0.0.0.0/tcp/$P2P_PORT,/ip4/0.0.0.0/udp/$P2P_PORT/quic-v1' \
            --rpc-port $RPC_PORT \
            --metrics-port $METRICS_PORT \
            > /tmp/${NODE_NAME}.log 2>&1 &
        echo \$! > /tmp/${NODE_NAME}.pid
        echo 'Started'
    " | grep -q "Started"; then
        STARTED_NODES+=("$NODE_NAME")
        echo -e "${GREEN}OK    $NODE_NAME started successfully${NC}"
    else
        FAILED_NODES+=("$NODE_NAME")
        echo -e "${RED}FAIL  $NODE_NAME failed to start${NC}"
    fi
done

# Wait a moment for nodes to initialize
sleep 3

echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  Lab Status Summary${NC}"
echo -e "${GREEN}============================================${NC}"
echo ""
printf "  %-20s %-18s %-10s %-10s %-12s %-10s\n" "NODE" "IP" "P2P" "RPC" "METRICS" "STATUS"
printf "  %-20s %-18s %-10s %-10s %-12s %-10s\n" "----" "--" "---" "---" "-------" "------"

for NODE_CONFIG in "${NODES[@]}"; do
    IFS=':' read -r NODE_NAME NODE_IP P2P_PORT RPC_PORT METRICS_PORT <<< "$NODE_CONFIG"
    
    if lxc info "$NODE_NAME" >/dev/null 2>&1; then
        if lxc exec "$NODE_NAME" -- pgrep -f "ebpf-node" >/dev/null 2>&1; then
            STATUS="${GREEN}RUNNING${NC}"
        else
            STATUS="${YELLOW}STOPPED${NC}"
        fi
    else
        STATUS="${RED}MISSING${NC}"
    fi
    
    printf "  %-20s %-18s %-10s %-10s %-12s ${STATUS}\n" "$NODE_NAME" "$NODE_IP" ":$P2P_PORT" ":$RPC_PORT" ":$METRICS_PORT"
done

echo ""
echo -e "  ${GREEN}Started: ${#STARTED_NODES[@]}${NC} | ${RED}Failed: ${#FAILED_NODES[@]}${NC}"

if [ ${#FAILED_NODES[@]} -gt 0 ]; then
    echo ""
    echo -e "${RED}Failed nodes: ${FAILED_NODES[*]}${NC}"
    echo "Check logs: lxc exec ${FAILED_NODES[0]} -- cat /tmp/${FAILED_NODES[0]}.log"
fi

echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  Access Points${NC}"
echo -e "${GREEN}============================================${NC}"
echo ""
echo -e "  Grafana:    http://localhost:3000"
echo -e "  Prometheus: http://localhost:9090"
echo -e "  Block-Monitor: http://localhost:8082"
echo ""
echo -e "${YELLOW}To validate network:${NC}"
echo -e "  ansible-playbook ansible/playbooks/validate_network.yml -i ansible/inventory/hosts.yml"
echo ""
echo -e "${YELLOW}To validate lab:${NC}"
echo -e "  ansible-playbook ansible/playbooks/validate_lab.yml -i ansible/inventory/hosts.yml"
echo ""
echo -e "${YELLOW}To stop nodes:${NC}"
echo -e "  ./services/stop-local-nodes.sh"
echo -e "${YELLOW}To view logs:${NC}"
echo -e "  lxc exec ebpf-node-1 -- tail -f /tmp/ebpf-node-1.log"
echo -e "${YELLOW}Or:${NC}"
echo -e "  ./scripts/live-logs.sh"
echo ""
