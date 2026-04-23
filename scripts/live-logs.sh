#!/bin/bash
# =============================================================================
# live-logs.sh - Real-time log viewer for eBPF node logs via Loki
# =============================================================================
# Similar a 'tail -f' pero para logs de Loki
# Permite filtrar por nivel, evento, nodo, etc.
# =============================================================================

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Default settings
LOKI_URL="${LOKI_URL:-http://localhost:3100}"
PROMTAIL_URL="${PROMTAIL_URL:-http://localhost:9080}"

# Usage
usage() {
    cat << EOF
Usage: $(basename "$0") [OPTIONS]

Real-time log viewer for eBPF node logs via Loki

Options:
    -j, --job JOB         Job name to filter (default: ebpf-nodes)
    -l, --level LEVEL     Log level filter (INFO, WARN, ERROR, etc.)
    -e, --event EVENT     Event name filter (p2p_connected, etc.)
    -n, --lines NUM       Number of initial lines to show (default: 20)
    -f, --follow          Follow mode (continuous streaming)
    -q, --query QUERY     Custom Loki query
    -o, --output FORMAT   Output format: text, json, table (default: text)
    -h, --help            Show this help message

Examples:
    # Show all recent logs
    $(basename "$0")

    # Show only errors
    $(basename "$0") --level ERROR

    # Show P2P connection events
    $(basename "$0") --event p2p_connected

    # Show warnings and errors
    $(basename "$0") --level WARN --level ERROR

    # Custom query
    $(basename "$0") --query '{job="ebpf-nodes"} | json | level="error"'

    # JSON output
    $(basename "$0") --output json

EOF
    exit 0
}

# Parse arguments
JOB="ebpf-nodes"
LEVEL_FILTER=""
EVENT_FILTER=""
LINES=20
FOLLOW=false
CUSTOM_QUERY=""
OUTPUT_FORMAT="text"

while [[ $# -gt 0 ]]; do
    case $1 in
        -j|--job) JOB="$2"; shift 2 ;;
        -l|--level) 
            if [ -n "$LEVEL_FILTER" ]; then
                LEVEL_FILTER="$LEVEL_FILTER|level=\"$2\""
            else
                LEVEL_FILTER="level=\"$2\""
            fi
            shift 2 ;;
        -e|--event) EVENT_FILTER="event=\"$2\""; shift 2 ;;
        -n|--lines) LINES="$2"; shift 2 ;;
        -f|--follow) FOLLOW=true; shift ;;
        -q|--query) CUSTOM_QUERY="$2"; shift 2 ;;
        -o|--output) OUTPUT_FORMAT="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

# Build Loki query
if [ -n "$CUSTOM_QUERY" ]; then
    QUERY="$CUSTOM_QUERY"
else
    QUERY="{job=\"$JOB\"}"
    
    if [ -n "$LEVEL_FILTER" ]; then
        QUERY="$QUERY | json | $LEVEL_FILTER"
    fi
    
    if [ -n "$EVENT_FILTER" ]; then
        QUERY="$QUERY | json | $EVENT_FILTER"
    fi
fi

echo -e "${CYAN}============================================${NC}"
echo -e "${CYAN}eBPF Node Live Logs${NC}"
echo -e "${CYAN}============================================${NC}"
echo -e "Loki URL: ${LOKI_URL}"
echo -e "Query:    ${QUERY}"
echo -e "Lines:    ${LINES}"
echo -e "Follow:   ${FOLLOW}"
echo -e "${CYAN}============================================${NC}"
echo ""

# Function to query Loki
query_loki() {
    local query="$1"
    local limit="$2"
    
    curl -s -G "${LOKI_URL}/loki/api/v1/query" \
        --data-urlencode "query=$query" \
        --data-urlencode "limit=$limit" \
        --data-urlencode "direction=backward" 2>/dev/null
}

# Function to format and display logs
display_logs() {
    local response="$1"
    local format="$2"
    
    case "$format" in
        json)
            echo "$response" | python3 -m json.tool 2>/dev/null || echo "$response"
            ;;
        table)
            echo "$response" | python3 -c "
import sys, json
from datetime import datetime

try:
    data = json.load(sys.stdin)
    results = data.get('data', {}).get('result', [])
    
    if not results:
        print('No log entries found')
        exit(0)
    
    print(f'{\"Timestamp\":<25} {\"Level\":<8} {\"Job\":<15} {\"Message\"}')
    print('-' * 80)
    
    for item in results:
        stream = item.get('stream', {})
        values = item.get('values', [])
        
        for ts, msg in values[-5:]:  # Last 5 entries per stream
            timestamp = datetime.fromtimestamp(int(ts) / 1e9).strftime('%Y-%m-%d %H:%M:%S')
            level = stream.get('level', 'INFO')
            job = stream.get('job', 'unknown')
            message = msg[:60] if isinstance(msg, str) else str(msg)[:60]
            
            # Color by level
            if level == 'ERROR':
                color = RED
            elif level == 'WARN':
                color = YELLOW
            else:
                color = GREEN
            
            print(f'{color}{timestamp:<25}{NC} {color}{level:<8}{NC} {job:<15} {message}')
except Exception as e:
    print(f'Error parsing response: {e}')
    print(response[:500])
" 2>/dev/null || echo "$response"
            ;;
        *)
            # Text format (default)
            echo "$response" | python3 -c "
import sys, json
from datetime import datetime

try:
    data = json.load(sys.stdin)
    results = data.get('data', {}).get('result', [])
    
    if not results:
        print('No log entries found')
        exit(0)
    
    for item in results:
        stream = item.get('stream', {})
        values = item.get('values', [])
        
        for ts, msg in values:
            timestamp = datetime.fromtimestamp(int(ts) / 1e9).strftime('%Y-%m-%d %H:%M:%S.%f')[:-3]
            level = stream.get('level', 'INFO')
            job = stream.get('job', 'unknown')
            instance = stream.get('instance', '')
            
            # Color by level
            if level == 'ERROR':
                color = RED
                icon = '✗'
            elif level == 'WARN':
                color = YELLOW
                icon = '!'
            elif level == 'INFO':
                color = GREEN
                icon = '✓'
            else:
                color = NC
                icon = '?'
            
            print(f'{color}{icon} {timestamp} [{level}] {job}/{instance}{NC} {msg}')
except Exception as e:
    print(f'Error: {e}')
    print(response[:500])
" 2>/dev/null || echo "$response"
            ;;
    esac
}

# Initial query
echo -e "${BLUE}Initial logs:${NC}"
response=$(query_loki "$QUERY" "$LINES")
display_logs "$response" "$OUTPUT_FORMAT"
echo ""

# Follow mode
if [ "$FOLLOW" = true ]; then
    echo -e "${YELLOW}Following logs (Ctrl+C to stop)...${NC}"
    echo ""
    
    last_timestamp=$(echo "$response" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    results = data.get('data', {}).get('result', [])
    if results:
        last_value = results[-1].get('values', [])[-1]
        print(last_value[0])
    else:
        print('0')
except:
    print('0')
" 2>/dev/null || echo "0")
    
    while true; do
        # Query for new logs
        new_response=$(curl -s -G "${LOKI_URL}/loki/api/v1/query_range" \
            --data-urlencode "query=$QUERY" \
            --data-urlencode "start=$last_timestamp" \
            --data-urlencode "end=$(date +%s%N)" \
            --data-urlencode "limit=10" \
            --data-urlencode "step=1s" 2>/dev/null)
        
        new_data=$(echo "$new_response" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    results = data.get('data', {}).get('result', [])
    if results:
        for item in results:
            values = item.get('values', [])
            if values:
                print(values[-1][0])
                print(values[-1][1])
except:
    pass
" 2>/dev/null)
        
        if [ -n "$new_data" ]; then
            echo -e "${GREEN}>>> New logs:${NC}"
            echo "$new_data" | while read -r line; do
                echo "  $line"
            done
            
            # Update last timestamp
            new_ts=$(echo "$new_data" | head -1)
            if [ -n "$new_ts" ] && [ "$new_ts" != "0" ]; then
                last_timestamp="$new_ts"
            fi
            echo ""
        fi
        
        sleep 2
    done
fi
