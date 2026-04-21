#!/bin/bash
# deploy.sh - Automated deployment script for eBPF Blockchain PoC
# Usage: ./scripts/deploy.sh [node_count] [bootstrap_peer_address]

set -euo pipefail

# Configuration
NODE_COUNT=${1:-3}
BOOTSTRAP_PEER=${2:-""}
WORK_DIR="/root/ebpf-blockchain"
DATA_DIR="/var/lib/ebpf-blockchain"
LOG_DIR="/var/log/ebpf-blockchain"
INSTALL_DIR="${WORK_DIR}/ebpf-node/target/release"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
}

check_dependencies() {
    log_info "Checking dependencies..."
    
    local deps=("rustc" "cargo" "rsync" "systemctl")
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            log_warn "Dependency not found: $dep"
        else
            log_success "Found: $dep"
        fi
    done
}

setup_directories() {
    log_info "Setting up directory structure..."
    
    mkdir -p "${DATA_DIR}/data"
    mkdir -p "${DATA_DIR}/backups"
    mkdir -p "${LOG_DIR}"
    mkdir -p "${WORK_DIR}"
    
    log_success "Directory structure created"
}

build_project() {
    log_info "Building project..."
    
    cd "${WORK_DIR}/ebpf-node"
    
    if [ "${2:-release}" = "debug" ]; then
        cargo build
    else
        cargo build --release
    fi
    
    log_success "Build completed"
}

install_service() {
    log_info "Installing systemd service..."
    
    local service_file="/etc/systemd/system/ebpf-blockchain.service"
    
    cat > "${service_file}" << 'EOF'
[Unit]
Description=eBPF Blockchain Node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
WorkingDirectory=/root/ebpf-blockchain
ExecStart=/root/ebpf-blockchain/ebpf-node/target/release/ebpf-node --iface eth0
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=ebpf-blockchain

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

# Resources
LimitNOFILE=65535
LimitMEMLOCK=Infinity

# Environment
Environment=RUST_LOG=info
Environment=PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

[Install]
WantedBy=multi-user.target
EOF
    
    systemctl daemon-reload
    systemctl enable ebpf-blockchain.service
    
    log_success "Systemd service installed"
}

start_node() {
    local node_id=$1
    local bootstrap=$2
    
    log_info "Starting node ${node_id}..."
    
    # Set environment for bootstrap peers
    if [ -n "${bootstrap}" ]; then
        export BOOTSTRAP_PEERS="${bootstrap}"
    fi
    
    systemctl start ebpf-blockchain.service
    
    sleep 2
    
    if systemctl is-active --quiet ebpf-blockchain.service; then
        log_success "Node ${node_id} started successfully"
    else
        log_error "Node ${node_id} failed to start"
        journalctl -u ebpf-blockchain.service --no-pager -n 20
        return 1
    fi
}

stop_node() {
    log_info "Stopping ebpf-blockchain service..."
    systemctl stop ebpf-blockchain.service
    log_success "Service stopped"
}

restart_node() {
    log_info "Restarting ebpf-blockchain service..."
    systemctl restart ebpf-blockchain.service
    log_success "Service restarted"
}

check_health() {
    log_info "Checking node health..."
    
    # Check service status
    if ! systemctl is-active --quiet ebpf-blockchain.service; then
        log_error "Service is not running"
        return 1
    fi
    
    # Check metrics endpoint
    local metrics_url="http://localhost:9090/metrics"
    if curl -sf "${metrics_url}" &> /dev/null; then
        log_success "Metrics endpoint responding"
        
        # Check key metrics
        local peers=$(curl -sf "${metrics_url}" | grep "ebpf_node_peers_connected" | grep -v "#")
        if [ -n "${peers}" ]; then
            log_info "Peer metrics: ${peers}"
        fi
    else
        log_warn "Metrics endpoint not responding on port 9090"
    fi
    
    # Check logs
    log_info "Recent logs:"
    journalctl -u ebpf-blockchain.service --no-pager -n 10
    
    log_success "Health check completed"
}

create_backup() {
    log_info "Creating backup..."
    
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local backup_dir="${DATA_DIR}/backups/${timestamp}"
    
    mkdir -p "${backup_dir}"
    
    if [ -d "${DATA_DIR}/data" ]; then
        rsync -a "${DATA_DIR}/data/" "${backup_dir}/data/"
        log_success "Backup created at ${backup_dir}"
    else
        log_warn "No data directory found to backup"
    fi
}

cleanup_backups() {
    log_info "Cleaning up old backups (keeping last 5)..."
    
    local backup_dir="${DATA_DIR}/backups"
    local backups=($(ls -1d "${backup_dir}"/*/ 2>/dev/null | sort))
    
    while [ ${#backups[@]} -gt 5 ]; do
        local oldest=${backups[0]}
        log_warn "Removing old backup: ${oldest}"
        rm -rf "${oldest}"
        backups=("${backups[@]:1}")
    done
    
    log_success "Backup cleanup completed"
}

show_status() {
    echo ""
    echo "=================================="
    echo "  eBPF Blockchain Node Status"
    echo "=================================="
    echo ""
    
    # Service status
    echo -e "Service: $(systemctl is-active ebpf-blockchain.service)"
    echo ""
    
    # Peer ID
    if [ -f "/tmp/peer_id.txt" ]; then
        echo -e "Peer ID: $(cat /tmp/peer_id.txt)"
    else
        echo "Peer ID: Not available"
    fi
    echo ""
    
    # Data directory
    echo "Data Directory: ${DATA_DIR}/data"
    if [ -d "${DATA_DIR}/data" ]; then
        local db_size=$(du -sh "${DATA_DIR}/data" 2>/dev/null | cut -f1)
        echo "DB Size: ${db_size}"
    fi
    echo ""
    
    # Backups
    echo "Backups:"
    if [ -d "${DATA_DIR}/backups" ]; then
        ls -1d "${DATA_DIR}/backups"/*/ 2>/dev/null | wc -l | xargs echo "  Count:"
    else
        echo "  No backups found"
    fi
    echo ""
    
    # Metrics
    echo "Metrics Endpoint: http://localhost:9090/metrics"
    echo ""
    
    echo "=================================="
}

# Main execution
case "${1:-status}" in
    deploy)
        check_root
        check_dependencies
        setup_directories
        build_project "${WORK_DIR}" "${2:-release}"
        install_service
        start_node 1 "${BOOTSTRAP_PEER}"
        check_health
        ;;
    start)
        start_node 1 "${BOOTSTRAP_PEER}"
        ;;
    stop)
        stop_node
        ;;
    restart)
        restart_node
        ;;
    health|status)
        check_health
        show_status
        ;;
    backup)
        create_backup
        ;;
    cleanup)
        cleanup_backups
        ;;
    *)
        echo "Usage: $0 {deploy|start|stop|restart|health|status|backup|cleanup}"
        echo ""
        echo "Commands:"
        echo "  deploy [build_type]  - Full deployment (build + install + start)"
        echo "  start [bootstrap]    - Start the node"
        echo "  stop                 - Stop the node"
        echo "  restart              - Restart the node"
        echo "  health               - Check node health"
        echo "  status               - Show node status"
        echo "  backup               - Create backup"
        echo "  cleanup              - Cleanup old backups"
        exit 1
        ;;
esac
