#!/bin/bash
# =============================================================================
# Restore Script - eBPF Blockchain
# Descripción: Restaura desde backup con verificación de integridad
# Uso: /var/lib/ebpf-blockchain/bin/restore.sh <backup_file> [--force]
# =============================================================================

set -euo pipefail

# Configuration
DATA_DIR="${DATA_DIR:-/var/lib/ebpf-blockchain/data}"
CONFIG_DIR="${CONFIG_DIR:-/etc/ebpf-blockchain}"
LOG_DIR="${LOG_DIR:-/var/log/ebpf-blockchain}"
BACKUP_BASE_DIR="${BACKUP_BASE_DIR:-/var/lib/ebpf-blockchain/backups}"

# Logging
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_FILE="${LOG_DIR}/restore_${TIMESTAMP}.log"

# Force restore mode
FORCE=false
BACKUP_FILE=""

# =============================================================================
# Helper Functions
# =============================================================================
log() {
    local level="$1"
    shift
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] [$level] $*" | tee -a "$LOG_FILE"
}

log_info() { log "INFO" "$@"; }
log_warn() { log "WARN" "$@"; }
log_error() { log "ERROR" "$@"; }

usage() {
    echo "Usage: $0 <backup_file> [--force]"
    echo ""
    echo "Arguments:"
    echo "  backup_file    Path to backup file to restore"
    echo "  --force        Skip confirmation prompt"
    echo ""
    echo "Examples:"
    echo "  $0 /var/lib/ebpf-blockchain/backups/rocksdb_host_20260126_020000.tar.gz"
    echo "  $0 /var/lib/ebpf-blockchain/backups/config_host_20260126_020000.tar.gz --force"
    exit 1
}

# =============================================================================
# Parse Arguments
# =============================================================================
if [[ $# -lt 1 ]]; then
    usage
fi

BACKUP_FILE="$1"
shift

if [[ "${1:-}" == "--force" ]]; then
    FORCE=true
fi

# =============================================================================
# Validation
# =============================================================================
log_info "=========================================="
log_info "Starting restore process"
log_info "Backup file: $BACKUP_FILE"
log_info "Force mode: $FORCE"
log_info "=========================================="

# Check backup file exists
if [ ! -f "$BACKUP_FILE" ]; then
    # Try to find in backup base directory
    SEARCH_FILE="${BACKUP_BASE_DIR}/$(basename "$BACKUP_FILE")"
    if [ -f "$SEARCH_FILE" ]; then
        BACKUP_FILE="$SEARCH_FILE"
        log_info "Found backup in base directory: $BACKUP_FILE"
    else
        log_error "Backup file not found: $BACKUP_FILE"
        log_error "Searched: $BACKUP_FILE and $SEARCH_FILE"
        exit 1
    fi
fi

# Verify backup integrity before restore
log_info "Verifying backup integrity..."
if ! tar -tzf "$BACKUP_FILE" > /dev/null 2>&1; then
    log_error "Backup file is corrupted: $BACKUP_FILE"
    exit 1
fi

log_info "Backup integrity verified"

# Get backup type from filename
BACKUP_TYPE=$(basename "$BACKUP_FILE" | sed -E "s/_(rocksdb|config|logs|state)_[^_]+_[0-9]+\.tar.gz$/\1/")
log_info "Backup type detected: $BACKUP_TYPE"

# =============================================================================
# Pre-Restore Checks
# =============================================================================
# Stop service
log_info "Stopping eBPF blockchain service..."
systemctl stop ebpf-blockchain 2>/dev/null || log_warn "Service not running or failed to stop"

# Create backup of current state (safety net)
if [ -d "$DATA_DIR" ] || [ -d "$CONFIG_DIR" ]; then
    SAFETY_BACKUP="${BACKUP_BASE_DIR}/pre_restore_${TIMESTAMP}"
    log_info "Creating safety backup: $SAFETY_BACKUP"
    mkdir -p "$SAFETY_BACKUP"

    if [ -d "$DATA_DIR" ]; then
        cp -r "$DATA_DIR" "$SAFETY_BACKUP/data" 2>/dev/null || true
    fi

    if [ -d "$CONFIG_DIR" ]; then
        cp -r "$CONFIG_DIR" "$SAFETY_BACKUP/config" 2>/dev/null || true
    fi

    log_info "Safety backup created"
fi

# =============================================================================
# Restore Functions
# =============================================================================
restore_rocksdb() {
    log_info "Restoring RocksDB data..."

    # Backup current data
    if [ -d "$DATA_DIR" ]; then
        CURRENT_BACKUP="${DATA_DIR}.backup.${TIMESTAMP}"
        log_info "Moving current data to: $CURRENT_BACKUP"
        mv "$DATA_DIR" "$CURRENT_BACKUP" 2>/dev/null || true
    fi

    # Extract backup
    log_info "Extracting backup to $DATA_DIR..."
    mkdir -p "$(dirname "$DATA_DIR")"
    tar -xzf "$BACKUP_FILE" -C "$(dirname "$DATA_DIR")" 2>/dev/null

    # Verify restore
    if [ -d "$DATA_DIR" ]; then
        log_info "RocksDB restore completed successfully"
        return 0
    else
        log_error "RocksDB restore failed"
        return 1
    fi
}

restore_config() {
    log_info "Restoring configuration..."

    # Backup current config
    if [ -d "$CONFIG_DIR" ]; then
        CURRENT_BACKUP="${CONFIG_DIR}.backup.${TIMESTAMP}"
        log_info "Moving current config to: $CURRENT_BACKUP"
        mv "$CONFIG_DIR" "$CURRENT_BACKUP" 2>/dev/null || true
    fi

    # Extract backup
    log_info "Extracting backup to $CONFIG_DIR..."
    mkdir -p "$(dirname "$CONFIG_DIR")"
    tar -xzf "$BACKUP_FILE" -C "$(dirname "$CONFIG_DIR")" 2>/dev/null

    # Verify restore
    if [ -d "$CONFIG_DIR" ]; then
        log_info "Config restore completed successfully"
        return 0
    else
        log_error "Config restore failed"
        return 1
    fi
}

restore_logs() {
    log_warn "Restoring logs is not recommended - logs are generated at runtime"
    log_warn "Skipping log restore"
}

restore_state() {
    log_info "Restoring node state information..."

    # Extract state to temp directory for inspection
    local state_dir
    state_dir=$(mktemp -d)
    tar -xzf "$BACKUP_FILE" -C "$state_dir" 2>/dev/null

    log_info "State information extracted to: $state_dir"
    log_info "State files:"
    ls -la "$state_dir" 2>/dev/null || true

    # Cleanup
    rm -rf "$state_dir"
}

# =============================================================================
# Post-Restore
# =============================================================================
post_restore() {
    log_info "Running post-restore checks..."

    # Start service
    log_info "Starting eBPF blockchain service..."
    systemctl start ebpf-blockchain 2>/dev/null || log_warn "Service start failed - manual intervention may be required"

    # Wait for service
    sleep 10

    # Check service status
    if systemctl is-active ebpf-blockchain > /dev/null 2>&1; then
        log_info "Service started successfully"
    else
        log_error "Service failed to start after restore"
        log_error "Check: journalctl -u ebpf-blockchain -n 50"
        return 1
    fi

    # Verify data directory
    if [ -d "$DATA_DIR" ]; then
        local data_size
        data_size=$(du -sh "$DATA_DIR" 2>/dev/null | awk '{print $1}')
        log_info "Data directory size: $data_size"
    else
        log_warn "Data directory not found after restore"
    fi

    return 0
}

# =============================================================================
# Main
# =============================================================================
main() {
    # Confirmation prompt (unless --force)
    if [ "$FORCE" = false ]; then
        echo ""
        echo "=========================================="
        echo "RESTORE OPERATION"
        echo "=========================================="
        echo "Backup file: $BACKUP_FILE"
        echo "Backup type: $BACKUP_TYPE"
        echo "Target data dir: $DATA_DIR"
        echo "Target config dir: $CONFIG_DIR"
        echo ""
        echo "WARNING: This will STOP the eBPF blockchain service"
        echo "and REPLACE the current data/configuration."
        echo ""
        read -p "Are you sure? (yes/no): " confirm

        if [[ "$confirm" != "yes" ]]; then
            log_info "Restore cancelled by user"
            exit 0
        fi
    fi

    # Perform restore based on type
    case "$BACKUP_TYPE" in
        rocksdb)
            restore_rocksdb || log_error "RocksDB restore failed"
            ;;
        config)
            restore_config || log_error "Config restore failed"
            ;;
        logs)
            restore_logs
            ;;
        state)
            restore_state
            ;;
        *)
            log_warn "Unknown backup type: $BACKUP_TYPE"
            log_warn "Attempting generic restore..."
            tar -xzf "$BACKUP_FILE" -C / 2>/dev/null || log_error "Generic restore failed"
            ;;
    esac

    # Post-restore checks
    post_restore

    # Summary
    log_info "=========================================="
    log_info "Restore completed"
    log_info "Backup file: $BACKUP_FILE"
    log_info "Restore log: $LOG_FILE"
    log_info "Safety backup: ${BACKUP_BASE_DIR}/pre_restore_${TIMESTAMP}"
    log_info "=========================================="
}

main "$@"
