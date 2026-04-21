#!/bin/bash
# =============================================================================
# Backup Script - eBPF Blockchain
# Descripción: Backup automatizado con retention policy
# Uso: /var/lib/ebpf-blockchain/bin/backup.sh [--dry-run]
# Cron: 0 2 * * * /var/lib/ebpf-blockchain/bin/backup.sh >> /var/log/ebpf-blockchain/backup.log 2>&1
# =============================================================================

set -euo pipefail

# Configuration
BACKUP_BASE_DIR="${BACKUP_BASE_DIR:-/var/lib/ebpf-blockchain/backups}"
DATA_DIR="${DATA_DIR:-/var/lib/ebpf-blockchain/data}"
CONFIG_DIR="${CONFIG_DIR:-/etc/ebpf-blockchain}"
LOG_DIR="${LOG_DIR:-/var/log/ebpf-blockchain}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
DATE=$(date +%Y%m%d_%H%M%S)
HOSTNAME=$(hostname -s)

# Backup components
BACKUP_ROCKSDB="rocksdb"
BACKUP_CONFIG="config"
BACKUP_LOGS="logs"
BACKUP_STATE="state"

# Logging
LOG_FILE="${LOG_DIR}/backup_${DATE}.log"

# Dry run mode
DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
    echo "[DRY RUN] Backup script in dry-run mode"
fi

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

cleanup_old_backups() {
    log_info "Cleaning up backups older than $RETENTION_DAYS days..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would delete backups older than $RETENTION_DAYS days from $BACKUP_BASE_DIR"
        find "$BACKUP_BASE_DIR" -name "*.tar.gz" -mtime +"$RETENTION_DAYS" -print 2>/dev/null || true
        return 0
    fi

    local deleted_count
    deleted_count=$(find "$BACKUP_BASE_DIR" -name "*.tar.gz" -mtime +"$RETENTION_DAYS" -delete -print 2>/dev/null | wc -l)
    log_info "Deleted $deleted_count old backup files"
}

verify_backup() {
    local backup_file="$1"

    log_info "Verifying backup integrity: $backup_file"

    if [ ! -f "$backup_file" ]; then
        log_error "Backup file not found: $backup_file"
        return 1
    fi

    # Check file size (should be > 0)
    local file_size
    file_size=$(stat -c%s "$backup_file" 2>/dev/null || stat -f%z "$backup_file" 2>/dev/null)
    if [ "$file_size" -eq 0 ]; then
        log_error "Backup file is empty: $backup_file"
        return 1
    fi

    # Verify tar archive integrity
    if tar -tzf "$backup_file" > /dev/null 2>&1; then
        log_info "Backup integrity verified: $backup_file ($file_size bytes)"
        return 0
    else
        log_error "Backup integrity check failed: $backup_file"
        return 1
    fi
}

# =============================================================================
# Backup Functions
# =============================================================================
backup_rocksdb() {
    local backup_file="${BACKUP_BASE_DIR}/${BACKUP_ROCKSDB}_${HOSTNAME}_${DATE}.tar.gz"

    log_info "Backing up RocksDB data..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would backup $DATA_DIR to $backup_file"
        return 0
    fi

    if [ ! -d "$DATA_DIR" ]; then
        log_warn "RocksDB data directory not found: $DATA_DIR"
        return 0
    fi

    # Create compressed backup
    tar -czf "$backup_file" -C "$(dirname "$DATA_DIR")" "$(basename "$DATA_DIR")" 2>/dev/null

    if verify_backup "$backup_file"; then
        log_info "RocksDB backup completed: $backup_file"
    else
        log_error "RocksDB backup failed"
        return 1
    fi
}

backup_config() {
    local backup_file="${BACKUP_BASE_DIR}/${BACKUP_CONFIG}_${HOSTNAME}_${DATE}.tar.gz"

    log_info "Backing up configuration..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would backup $CONFIG_DIR to $backup_file"
        return 0
    fi

    if [ ! -d "$CONFIG_DIR" ]; then
        log_warn "Config directory not found: $CONFIG_DIR"
        return 0
    fi

    tar -czf "$backup_file" -C "$(dirname "$CONFIG_DIR")" "$(basename "$CONFIG_DIR")" 2>/dev/null

    if verify_backup "$backup_file"; then
        log_info "Config backup completed: $backup_file"
    else
        log_error "Config backup failed"
        return 1
    fi
}

backup_logs() {
    local backup_file="${BACKUP_BASE_DIR}/${BACKUP_LOGS}_${HOSTNAME}_${DATE}.tar.gz"

    log_info "Backing up recent logs (last 24h)..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would backup logs to $backup_file"
        return 0
    fi

    # Only backup logs from last 24 hours
    find "$LOG_DIR" -name "*.log" -mtime -1 -type f 2>/dev/null | head -20 | \
        tar -czf "$backup_file" -T - 2>/dev/null || true

    if [ -f "$backup_file" ] && verify_backup "$backup_file"; then
        log_info "Logs backup completed: $backup_file"
    else
        log_warn "Logs backup skipped or failed"
    fi
}

backup_state() {
    local backup_file="${BACKUP_BASE_DIR}/${BACKUP_STATE}_${HOSTNAME}_${DATE}.tar.gz"

    log_info "Backing up node state..."

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would backup node state to $backup_file"
        return 0
    fi

    # Collect state information
    local state_dir
    state_dir=$(mktemp -d)

    # Service status
    systemctl status ebpf-blockchain --no-pager > "$state_dir/service_status.txt" 2>&1 || true

    # Peer information from metrics
    curl -s http://localhost:9090/metrics > "$state_dir/metrics.txt" 2>&1 || true

    # Network connections
    ss -tlnp > "$state_dir/network_connections.txt" 2>&1 || true

    # System info
    uname -a > "$state_dir/system_info.txt" 2>&1
    df -h > "$state_dir/disk_usage.txt" 2>&1
    free -m > "$state_dir/memory_info.txt" 2>&1

    # Create backup archive
    tar -czf "$backup_file" -C "$state_dir" . 2>/dev/null

    # Cleanup
    rm -rf "$state_dir"

    if verify_backup "$backup_file"; then
        log_info "State backup completed: $backup_file"
    else
        log_warn "State backup skipped or failed"
    fi
}

# =============================================================================
# Main
# =============================================================================
main() {
    log_info "=========================================="
    log_info "Starting backup process"
    log_info "Hostname: $HOSTNAME"
    log_info "Date: $DATE"
    log_info "Retention: $RETENTION_DAYS days"
    log_info "=========================================="

    # Create backup directory
    mkdir -p "$BACKUP_BASE_DIR"
    mkdir -p "$LOG_DIR"

    local errors=0

    # Run backups
    backup_rocksdb || errors=$((errors + 1))
    backup_config || errors=$((errors + 1))
    backup_logs || errors=$((errors + 1))
    backup_state || errors=$((errors + 1))

    # Cleanup old backups
    cleanup_old_backups

    # Summary
    log_info "=========================================="
    if [ "$errors" -gt 0 ]; then
        log_error "Backup completed with $errors error(s)"
        exit 1
    else
        log_info "Backup completed successfully"
        log_info "Backups stored in: $BACKUP_BASE_DIR"
        ls -lh "$BACKUP_BASE_DIR"/*_"$DATE".tar.gz 2>/dev/null || true
        log_info "=========================================="
        exit 0
    fi
}

main "$@"
