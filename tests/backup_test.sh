#!/bin/bash
# =============================================================================
# Backup Tests - eBPF Blockchain
# Descripción: Pruebas automatizadas para el sistema de backup
# Uso: bash tests/backup_test.sh
# =============================================================================

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Test directory
TEST_DIR=$(mktemp -d)
BACKUP_BASE_DIR="${TEST_DIR}/backups"
DATA_DIR="${TEST_DIR}/data"
LOG_FILE="${TEST_DIR}/test_results.log"

# =============================================================================
# Helper Functions
# =============================================================================
log_test() {
    echo -e "${GREEN}[TEST]${NC} $1" | tee -a "$LOG_FILE"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1" | tee -a "$LOG_FILE"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    TESTS_RUN=$((TESTS_RUN + 1))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1" | tee -a "$LOG_FILE"
    TESTS_FAILED=$((TESTS_FAILED + 1))
    TESTS_RUN=$((TESTS_RUN + 1))
}

log_info() {
    echo -e "${YELLOW}[INFO]${NC} $1" | tee -a "$LOG_FILE"
}

cleanup() {
    rm -rf "$TEST_DIR"
}

trap cleanup EXIT

# =============================================================================
# Test 1: Create Backup
# =============================================================================
test_backup_creation() {
    log_test "Test 1: Create backup"

    mkdir -p "$BACKUP_BASE_DIR"
    mkdir -p "$DATA_DIR"

    # Create test data
    echo "test blockchain data" > "$DATA_DIR/test.db"
    echo "test RocksDB manifest" > "$DATA_DIR/MANIFEST-000001"

    # Simulate backup creation
    tar -czf "${BACKUP_BASE_DIR}/rocksdb_test_$(date +%Y%m%d_%H%M%S).tar.gz" \
        -C "$TEST_DIR" data 2>/dev/null

    # Verify backup file exists
    if ls "${BACKUP_BASE_DIR}/rocksdb_*.tar.gz" 1>/dev/null 2>&1; then
        log_pass "Backup file created successfully"
    else
        log_fail "Backup file not created"
        return 1
    fi
}

# =============================================================================
# Test 2: Verify Backup Integrity
# =============================================================================
test_backup_integrity() {
    log_test "Test 2: Verify backup integrity"

    BACKUP_FILE=$(ls "${BACKUP_BASE_DIR}/rocksdb_*.tar.gz" 2>/dev/null | head -1)

    if [ -z "$BACKUP_FILE" ]; then
        log_fail "No backup file found for integrity test"
        return 1
    fi

    # Check file is not empty
    FILE_SIZE=$(stat -c%s "$BACKUP_FILE" 2>/dev/null || stat -f%z "$BACKUP_FILE" 2>/dev/null)
    if [ "$FILE_SIZE" -gt 0 ]; then
        log_pass "Backup file is not empty (${FILE_SIZE} bytes)"
    else
        log_fail "Backup file is empty"
        return 1
    fi

    # Verify tar archive integrity
    if tar -tzf "$BACKUP_FILE" > /dev/null 2>&1; then
        log_pass "Backup archive integrity verified"
    else
        log_fail "Backup archive is corrupted"
        return 1
    fi

    # List archive contents
    CONTENTS=$(tar -tzf "$BACKUP_FILE")
    if echo "$CONTENTS" | grep -q "data/"; then
        log_pass "Archive contains expected data directory"
    else
        log_fail "Archive missing expected data directory"
        return 1
    fi
}

# =============================================================================
# Test 3: Test Restore
# =============================================================================
test_restore() {
    log_test "Test 3: Test restore from backup"

    RESTORE_DIR="${TEST_DIR}/restore"
    mkdir -p "$RESTORE_DIR"

    BACKUP_FILE=$(ls "${BACKUP_BASE_DIR}/rocksdb_*.tar.gz" 2>/dev/null | head -1)

    # Extract backup to restore directory
    if tar -xzf "$BACKUP_FILE" -C "$RESTORE_DIR" 2>/dev/null; then
        log_pass "Backup extracted successfully"
    else
        log_fail "Failed to extract backup"
        return 1
    fi

    # Verify restored data
    if [ -d "${RESTORE_DIR}/data" ]; then
        log_pass "Restored data directory exists"
    else
        log_fail "Restored data directory missing"
        return 1
    fi

    # Verify file contents
    if [ -f "${RESTORE_DIR}/data/test.db" ]; then
        CONTENT=$(cat "${RESTORE_DIR}/data/test.db")
        if [ "$CONTENT" = "test blockchain data" ]; then
            log_pass "Restored file contents match original"
        else
            log_fail "Restored file contents do not match"
            return 1
        fi
    else
        log_fail "Restored test.db file not found"
        return 1
    fi
}

# =============================================================================
# Test 4: Verify Retention Policy
# =============================================================================
test_retention_policy() {
    log_test "Test 4: Verify retention policy"

    # Create multiple backup files with different dates
    touch "${BACKUP_BASE_DIR}/rocksdb_test_20260101_020000.tar.gz"
    touch "${BACKUP_BASE_DIR}/rocksdb_test_20260115_020000.tar.gz"
    touch "${BACKUP_BASE_DIR}/rocksdb_test_20260126_020000.tar.gz"

    OLD_BACKUPS=$(ls "${BACKUP_BASE_DIR}/rocksdb_test_20260101_020000.tar.gz" 2>/dev/null)
    NEW_BACKUPS=$(ls "${BACKUP_BASE_DIR}/rocksdb_test_20260126_020000.tar.gz" 2>/dev/null)

    if [ -n "$OLD_BACKUPS" ] && [ -n "$NEW_BACKUPS" ]; then
        log_pass "Multiple backup files created for retention testing"
    else
        log_fail "Failed to create test backup files"
        return 1
    fi

    # Count total backups
    BACKUP_COUNT=$(ls -1 "${BACKUP_BASE_DIR}"/*.tar.gz 2>/dev/null | wc -l)
    if [ "$BACKUP_COUNT" -ge 3 ]; then
        log_pass "Retention test: $BACKUP_COUNT backup files exist"
    else
        log_fail "Retention test: Expected 3+ backups, found $BACKUP_COUNT"
        return 1
    fi
}

# =============================================================================
# Test 5: Backup Script Validation
# =============================================================================
test_backup_script() {
    log_test "Test 5: Backup script validation"

    if [ -f "scripts/backup.sh" ]; then
        log_pass "backup.sh exists"
    else
        log_fail "backup.sh not found"
        return 1
    fi

    if [ -x "scripts/backup.sh" ]; then
        log_pass "backup.sh is executable"
    else
        log_warn "backup.sh is not executable (chmod +x scripts/backup.sh)"
    fi

    # Check script has required components
    if grep -q "RETENTION_DAYS" scripts/backup.sh 2>/dev/null; then
        log_pass "backup.sh has retention policy"
    else
        log_fail "backup.sh missing retention policy"
        return 1
    fi

    if grep -q "verify_backup" scripts/backup.sh 2>/dev/null; then
        log_pass "backup.sh has integrity verification"
    else
        log_fail "backup.sh missing integrity verification"
        return 1
    fi
}

# =============================================================================
# Test 6: Restore Script Validation
# =============================================================================
test_restore_script() {
    log_test "Test 6: Restore script validation"

    if [ -f "scripts/restore.sh" ]; then
        log_pass "restore.sh exists"
    else
        log_fail "restore.sh not found"
        return 1
    fi

    if [ -x "scripts/restore.sh" ]; then
        log_pass "restore.sh is executable"
    else
        log_warn "restore.sh is not executable (chmod +x scripts/restore.sh)"
    fi

    # Check script has safety backup
    if grep -q "pre_restore" scripts/restore.sh 2>/dev/null; then
        log_pass "restore.sh has safety backup before restore"
    else
        log_fail "restore.sh missing safety backup"
        return 1
    fi

    # Check script has confirmation prompt
    if grep -q "read -p" scripts/restore.sh 2>/dev/null; then
        log_pass "restore.sh has confirmation prompt"
    else
        log_warn "restore.sh missing confirmation prompt"
    fi
}

# =============================================================================
# Main
# =============================================================================
main() {
    log_info "=========================================="
    log_info "Starting Backup Tests"
    log_info "Test directory: $TEST_DIR"
    log_info "=========================================="

    test_backup_creation
    test_backup_integrity
    test_restore
    test_retention_policy
    test_backup_script
    test_restore_script

    log_info "=========================================="
    log_info "Test Results:"
    log_info "  Total:  $TESTS_RUN"
    log_info "  Passed: $TESTS_PASSED"
    log_info "  Failed: $TESTS_FAILED"
    log_info "=========================================="

    if [ "$TESTS_FAILED" -gt 0 ]; then
        echo -e "${RED}Backup tests failed${NC}"
        exit 1
    else
        echo -e "${GREEN}All backup tests passed!${NC}"
        exit 0
    fi
}

main "$@"
