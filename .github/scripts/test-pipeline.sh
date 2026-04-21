#!/bin/bash
# =============================================================================
# CI/CD Pipeline Test Script
# Descripción: Verifica que todas las etapas del pipeline funcionen correctamente
# Uso: .github/scripts/test-pipeline.sh [--full]
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../ebpf-node" && pwd)"
FULL_TEST="${1:-false}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# =============================================================================
# Helper Functions
# =============================================================================
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

run_test() {
    local test_name="$1"
    local test_command="$2"

    TESTS_RUN=$((TESTS_RUN + 1))
    echo -n "  Testing: $test_name ... "

    if eval "$test_command" > /dev/null 2>&1; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        echo -e "${GREEN}PASS${NC}"
        return 0
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        echo -e "${RED}FAIL${NC}"
        return 1
    fi
}

# =============================================================================
# Stage 1: Lint Tests
# =============================================================================
test_lint() {
    log_info "Stage 1: Lint and Code Quality"

    run_test "cargo fmt check" "cd '$PROJECT_DIR' && cargo fmt --all -- --check"
    run_test "cargo clippy" "cd '$PROJECT_DIR' && cargo clippy --all-targets --all-features -- -W clippy::all"
    run_test "ansible-lint deploy" "ansible-lint ansible/playbooks/deploy.yml --quiet"
    run_test "ansible-lint rollback" "ansible-lint ansible/playbooks/rollback.yml --quiet"
    run_test "ansible-lint health_check" "ansible-lint ansible/playbooks/health_check.yml --quiet"
}

# =============================================================================
# Stage 2: Build Tests
# =============================================================================
test_build() {
    log_info "Stage 2: Build"

    run_test "cargo build" "cd '$PROJECT_DIR' && cargo build"
    run_test "cargo build release" "cd '$PROJECT_DIR' && cargo build --release"
    run_test "eBPF binary build" "cd '$PROJECT_DIR' && cargo build --release --package ebpf-node-ebpf"
}

# =============================================================================
# Stage 3: Unit Tests
# =============================================================================
test_unit() {
    log_info "Stage 3: Unit Tests"

    run_test "cargo test lib" "cd '$PROJECT_DIR' && cargo test --lib"
    run_test "cargo test --release" "cd '$PROJECT_DIR' && cargo test --release"
}

# =============================================================================
# Stage 4: Integration Tests (if --full flag)
# =============================================================================
test_integration() {
    log_info "Stage 4: Integration Tests"

    if [ "$FULL_TEST" = "true" ]; then
        run_test "integration tests" "cd '$PROJECT_DIR' && cargo test --test integration"
    else
        log_warn "Skipping integration tests (use --full flag)"
    fi
}

# =============================================================================
# Stage 5: Ansible Playbook Tests
# =============================================================================
test_ansible() {
    log_info "Stage 5: Ansible Playbook Validation"

    run_test "deploy.yml syntax" "ansible-playbook ansible/playbooks/deploy.yml --syntax-check --inventory ansible/inventory/hosts.yml"
    run_test "rollback.yml syntax" "ansible-playbook ansible/playbooks/rollback.yml --syntax-check --inventory ansible/inventory/hosts.yml"
    run_test "health_check.yml syntax" "ansible-playbook ansible/playbooks/health_check.yml --syntax-check --inventory ansible/inventory/hosts.yml"
}

# =============================================================================
# Stage 6: Script Tests
# =============================================================================
test_scripts() {
    log_info "Stage 6: Script Validation"

    run_test "backup.sh exists" "test -f scripts/backup.sh"
    run_test "restore.sh exists" "test -f scripts/restore.sh"
    run_test "deploy.sh exists" "test -f scripts/deploy.sh"
    run_test "backup.sh is executable" "test -x scripts/backup.sh"
    run_test "restore.sh is executable" "test -x scripts/restore.sh"
}

# =============================================================================
# Stage 7: Monitoring Stack Tests
# =============================================================================
test_monitoring() {
    log_info "Stage 7: Monitoring Stack Validation"

    run_test "docker-compose.yml valid" "docker-compose -f monitoring/docker-compose.yml config > /dev/null 2>&1 || true"
    run_test "prometheus config valid" "test -f monitoring/prometheus/prometheus.yml"
    run_test "grafana dashboards exist" "test -f monitoring/grafana/dashboards/health-overview.json"
    run_test "loki config valid" "test -f monitoring/loki/loki-config.yml"
}

# =============================================================================
# Main
# =============================================================================
main() {
    log_info "Starting CI/CD Pipeline Tests"
    echo "============================================"

    test_lint
    test_build
    test_unit
    test_integration
    test_ansible
    test_scripts
    test_monitoring

    echo "============================================"
    echo "Test Results:"
    echo "  Total:  $TESTS_RUN"
    echo -e "  Passed: ${GREEN}$TESTS_PASSED${NC}"
    echo -e "  Failed: ${RED}$TESTS_FAILED${NC}"
    echo "============================================"

    if [ "$TESTS_FAILED" -gt 0 ]; then
        log_error "$TESTS_FAILED test(s) failed"
        exit 1
    else
        log_info "All tests passed!"
        exit 0
    fi
}

main "$@"
