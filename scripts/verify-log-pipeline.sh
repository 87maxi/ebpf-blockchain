#!/bin/bash
# =============================================================================
# verify-log-pipeline.sh - Debug script for eBPF log collection pipeline
# =============================================================================
# Verifica que el pipeline completo funcione:
# journald → Promtail → Loki → Grafana
# =============================================================================

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Variables
PROMTAIL_URL="http://localhost:9080"
LOKI_URL="http://localhost:3100"
GRAFANA_URL="http://localhost:3000"

echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}eBPF Log Collection Pipeline Verification${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""

# =============================================================================
# Step 1: Check Docker services status
# =============================================================================
echo -e "${YELLOW}[1/7] Checking Docker services status...${NC}"
echo ""

SERVICES=("ebpf-promtail" "ebpf-loki" "ebpf-grafana" "ebpf-prometheus")
for service in "${SERVICES[@]}"; do
    if docker ps --filter "name=$service" --format "{{.Names}}: {{.Status}}" | grep -q "$service"; then
        status=$(docker ps --filter "name=$service" --format "{{.Status}}" | grep "$service" || echo "NOT RUNNING")
        echo -e "  ${GREEN}✓${NC} $service: $status"
    else
        echo -e "  ${RED}✗${NC} $service: NOT RUNNING"
    fi
done
echo ""

# =============================================================================
# Step 2: Check Promtail configuration
# =============================================================================
echo -e "${YELLOW}[2/7] Checking Promtail configuration...${NC}"
echo ""

# Check if promtail container is running
if docker ps --filter "name=ebpf-promtail" --format "{{.Names}}" | grep -q "ebpf-promtail"; then
    echo -e "  ${GREEN}✓${NC} Promtail container is running"
    
    # Check journal volume mount
    volumes=$(docker inspect ebpf-promtail --format '{{range .Mounts}}{{.Source}} -> {{.Destination}}{{"\n"}}{{end}}')
    if echo "$volumes" | grep -q "/var/log/journal"; then
        echo -e "  ${GREEN}✓${NC} Journal volume mounted: /var/log/journal"
    else
        echo -e "  ${RED}✗${NC} Journal volume NOT mounted! Check docker-compose.yml"
    fi
    
    # Check promtail config
    config_check=$(docker exec ebpf-promtail cat /etc/promtail/promtail-config.yml 2>/dev/null || echo "CANNOT READ CONFIG")
    if echo "$config_check" | grep -q "ebpf-nodes"; then
        echo -e "  ${GREEN}✓${NC} Promtail config has 'ebpf-nodes' job"
    else
        echo -e "  ${RED}✗${NC} Promtail config MISSING 'ebpf-nodes' job"
    fi
    
    if echo "$config_check" | grep -q "journal:"; then
        echo -e "  ${GREEN}✓${NC} Promtail has journal input configured"
    else
        echo -e "  ${RED}✗${NC} Promtail MISSING journal input"
    fi
else
    echo -e "  ${RED}✗${NC} Promtail container is NOT running"
fi
echo ""

# =============================================================================
# Step 3: Check Promtail logs
# =============================================================================
echo -e "${YELLOW}[3/7] Checking Promtail logs...${NC}"
echo ""

promtail_logs=$(docker logs ebpf-promtail --tail 50 2>&1)

if echo "$promtail_logs" | grep -qi "error\|failed\|panic"; then
    echo -e "  ${RED}✗${NC} Promtail has errors:"
    echo "$promtail_logs" | grep -i "error\|failed\|panic" | head -10 | sed 's/^/    /'
else
    echo -e "  ${GREEN}✓${NC} Promtail logs look clean (no errors in last 50 lines)"
fi

# Check if promtail is trying to read journal
if echo "$promtail_logs" | grep -q "journal"; then
    echo -e "  ${GREEN}✓${NC} Promtail is attempting to read journal"
else
    echo -e "  ${YELLOW}!${NC} No journal activity in Promtail logs"
fi
echo ""

# =============================================================================
# Step 4: Check Loki API
# =============================================================================
echo -e "${YELLOW}[4/7] Checking Loki API...${NC}"
echo ""

# Test Loki health
loki_health=$(curl -s "$LOKI_URL/ready" 2>/dev/null || echo "UNREACHABLE")
if [ "$loki_health" = "ready" ]; then
    echo -e "  ${GREEN}✓${NC} Loki API is healthy: $loki_health"
else
    echo -e "  ${RED}✗${NC} Loki API status: $loki_health"
fi

# Check Loki metrics
loki_metrics=$(curl -s "$LOKI_URL/metrics" 2>/dev/null || echo "")
if [ -n "$loki_metrics" ]; then
    # Get ingester metrics
    ingester_active_streams=$(echo "$loki_metrics" | grep "loki_storage_ingester_streams_active" || echo "NOT FOUND")
    echo -e "  ${GREEN}✓${NC} Loki metrics available"
    echo "    Active streams: $(echo "$loki_metrics" | grep "loki_storage_ingester_streams_active" | head -1 || echo "0")"
    
    # Get sample count
    samples_ingested=$(echo "$loki_metrics" | grep "loki_ingester_samples_ingested_total" | head -1 || echo "NOT FOUND")
    echo "    Samples ingested: $samples_ingested"
else
    echo -e "  ${RED}✗${NC} Cannot get Loki metrics"
fi
echo ""

# =============================================================================
# Step 5: Query Loki for eBPF logs
# =============================================================================
echo -e "${YELLOW}[5/7] Querying Loki for eBPF node logs...${NC}"
echo ""

# Query Loki for any logs with job=ebpf-nodes
query_result=$(curl -s -G "$LOKI_URL/loki/api/v1/query" \
    --data-urlencode 'query={job="ebpf-nodes"}' \
    --data-urlencode 'limit=5' 2>/dev/null || echo "QUERY FAILED")

if echo "$query_result" | grep -q '"status":"success"'; then
    echo -e "  ${GREEN}✓${NC} Loki query successful"
    
    # Extract data
    data=$(echo "$query_result" | python3 -c "
import sys, json
try:
    result = json.load(sys.stdin)
    if result.get('data', {}).get('result'):
        for item in result['data']['result']:
            stream = item.get('stream', {})
            values = item.get('values', [])
            print(f\"  Stream: {stream}\")
            print(f\"  Log entries: {len(values)}\")
            if values:
                last_value = values[-1]
                timestamp = last_value[0]
                message = last_value[1]
                print(f\"  Latest: {timestamp} -> {message[:200]}\")
    else:
        print('  No log entries found for job=ebpf-nodes')
except Exception as e:
    print(f'  Error parsing response: {e}')
    print(result[:500])
" 2>/dev/null || echo "  Error parsing JSON response")
    
    echo "$data"
else
    echo -e "  ${YELLOW}!${NC} No logs found in Loki for job=ebpf-nodes"
    echo "  This could mean:"
    echo "    - Promtail is not reading the journal correctly"
    echo "    - The eBPF node service is not writing to journal"
    echo "    - The match expression is filtering out all logs"
fi
echo ""

# =============================================================================
# Step 6: Check systemd journal directly
# =============================================================================
echo -e "${YELLOW}[6/7] Checking systemd journal for eBPF logs...${NC}"
echo ""

# Check if journal has ebpf-blockchain logs
journal_count=$(journalctl --user _SYSTEMD_UNIT=ebpf-blockchain.service --no-pager -n 0 2>/dev/null | tail -1 || echo "0")
if echo "$journal_count" | grep -q "lines"; then
    count=$(echo "$journal_count" | grep -o '[0-9]*' | head -1)
    echo -e "  ${GREEN}✓${NC} Journal has $count entries for ebpf-blockchain.service"
else
    # Try system journal
    journal_count=$(journalctl _SYSTEMD_UNIT=ebpf-blockchain.service --no-pager -n 0 2>/dev/null | tail -1 || echo "0")
    count=$(echo "$journal_count" | grep -o '[0-9]*' | head -1)
    if [ -n "$count" ] && [ "$count" != "0" ]; then
        echo -e "  ${GREEN}✓${NC} Journal has $count entries for ebpf-blockchain.service (system)"
    else
        echo -e "  ${YELLOW}!${NC} No journal entries found for ebpf-blockchain.service"
        echo "    Check if the service is running: systemctl status ebpf-blockchain"
    fi
fi

# Show latest journal entries
echo ""
echo "  Latest journal entries:"
journalctl _SYSTEMD_UNIT=ebpf-blockchain.service --no-pager -n 3 --output=short-iso 2>/dev/null | sed 's/^/    /' || echo "    Cannot read journal"
echo ""

# =============================================================================
# Step 7: Test Promtail scrape config
# =============================================================================
echo -e "${YELLOW}[7/7] Testing Promtail scrape endpoints...${NC}"
echo ""

# Get Promtail config status
config_status=$(curl -s "$PROMTAIL_URL/config" 2>/dev/null || echo "UNREACHABLE")
if [ "$config_status" != "UNREACHABLE" ]; then
    echo -e "  ${GREEN}✓${NC} Promtail API reachable at $PROMTAIL_URL"
else
    echo -e "  ${RED}✗${NC} Promtail API NOT reachable"
fi

# Check Promtail targets
targets=$(curl -s "$PROMTAIL_URL/services" 2>/dev/null || echo "")
if [ -n "$targets" ]; then
    echo -e "  ${GREEN}✓${NC} Promtail targets:"
    echo "$targets" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    for service in data.get('services', []):
        name = service.get('Name', 'unknown')
        status = service.get('Status', 'unknown')
        print(f\"    - {name}: {status}\")
except:
    print(f\"    {targets[:200]}\")
" 2>/dev/null || echo "    Cannot parse targets"
else
    echo -e "  ${YELLOW}!${NC} No targets found via Promtail API"
fi
echo ""

# =============================================================================
# Summary & Recommendations
# =============================================================================
echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}Summary & Recommendations${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""

# Check if we found any logs
if echo "$query_result" | grep -q '"status":"success"'; then
    has_data=$(echo "$query_result" | python3 -c "
import sys, json
try:
    result = json.load(sys.stdin)
    results = result.get('data', {}).get('result', [])
    print('yes' if results else 'no')
except:
    print('no')
" 2>/dev/null || echo "no")
    
    if [ "$has_data" = "yes" ]; then
        echo -e "  ${GREEN}✓${NC} Pipeline is working! Logs are flowing from journald → Promtail → Loki → Grafana"
        echo ""
        echo "  Next steps:"
        echo "    1. Open Grafana: http://localhost:3000"
        echo "    2. Navigate to: Dashboard → eBPF Network Activity & Debug"
        echo "    3. Select node/level filters"
    else
        echo -e "  ${YELLOW}!${NC} Loki is reachable but no logs found"
        echo ""
        echo "  Possible causes:"
        echo "    1. Promtail cannot read /var/log/journal (permission issue)"
        echo "    2. eBPF node service not writing to systemd journal"
        echo "    3. Match expression filtering out all logs"
        echo ""
        echo "  Troubleshooting:"
        echo "    docker logs ebpf-promtail --tail 100"
        echo "    journalctl _SYSTEMD_UNIT=ebpf-blockchain.service -n 20"
        echo "    Check volume mount: docker inspect ebpf-promtail | grep -A 5 Mounts"
    fi
else
    echo -e "  ${RED}✗${NC} Pipeline is BROKEN"
    echo ""
    echo "  Troubleshooting steps:"
    echo "    1. Restart services: docker-compose down && docker-compose up -d"
    echo "    2. Check Promtail logs: docker logs ebpf-promtail --tail 100"
    echo "    3. Check Loki logs: docker logs ebpf-loki --tail 50"
    echo "    4. Verify journal access: sudo journalctl -u ebpf-blockchain -n 10"
fi
echo ""
echo -e "${BLUE}============================================${NC}"
