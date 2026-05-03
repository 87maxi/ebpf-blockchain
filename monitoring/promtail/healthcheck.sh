#!/bin/bash
# Simple HTTP healthcheck for Promtail
# Uses bash built-in /dev/tcp to check if promtail is responding
set -e

# Copy to writable location and execute
cp /app/healthcheck.sh /tmp/healthcheck.sh
chmod +x /tmp/healthcheck.sh
/bin/bash /tmp/healthcheck.sh
