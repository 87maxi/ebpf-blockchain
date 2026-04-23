#!/usr/bin/env python3
"""
eBPF Log Forwarder - Collects logs from remote eBPF nodes via SSH and pushes them to Loki.

This script solves the problem of collecting logs from remote LXC nodes
that are not accessible via file-based Promtail scraping.

Usage:
    python3 ebpf-log-forwarder.py

Configuration:
    Edit the NODES dictionary below to add/remove nodes.
"""

import json
import logging
import re
import time
import subprocess
import sys
from datetime import datetime, timezone
from typing import Dict, List, Optional

import requests

# ===================== Configuration =====================

LOKI_URL = "http://localhost:3100"
LOKI_PUSH_ENDPOINT = f"{LOKI_URL}/loki/api/v1/push"

# Remote eBPF nodes configuration
NODES: Dict[str, Dict[str, str]] = {
    "ebpf-node-1": {
        "host": "192.168.2.210",
        "user": "maxi",
        "log_path": "/var/log/ebpf-node/ebpf-node.log",
    },
    "ebpf-node-2": {
        "host": "192.168.2.211",
        "user": "maxi",
        "log_path": "/var/log/ebpf-node/ebpf-node.log",
    },
    "ebpf-node-3": {
        "host": "192.168.2.212",
        "user": "maxi",
        "log_path": "/var/log/ebpf-node/ebpf-node.log",
    },
}

# SSH options for connecting to remote nodes
SSH_OPTS = "-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5"

# Polling interval in seconds
POLL_INTERVAL = 10

# Stream name for all eBPF logs
STREAM_LABELS = {
    "job": "ebpf-blockchain",
    "cluster": "ebpf-blockchain-lab",
}

# ===================== Logging Setup =====================

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%dT%H:%M:%S",
)
logger = logging.getLogger(__name__)


# ===================== SSH Connection =====================

def ssh_execute(node_user: str, node_host: str, command: str, tail_lines: int = 100) -> Optional[str]:
    """Execute a command on a remote node via SSH."""
    try:
        result = subprocess.run(
            [
                "ssh",
                *SSH_OPTS.split(),
                f"{node_user}@{node_host}",
                f"tail -n {tail_lines} {command}",
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode == 0:
            return result.stdout
        else:
            logger.warning(f"SSH command failed for {node_user}@{node_host}: {result.stderr.strip()}")
            return None
    except subprocess.TimeoutExpired:
        logger.error(f"SSH timeout for {node_user}@{node_host}")
        return None
    except Exception as e:
        logger.error(f"SSH error for {node_user}@{node_host}: {e}")
        return None


# ===================== Log Parsing =====================

def parse_log_line(line: str) -> Optional[dict]:
    """Parse a JSON log line from eBPF node."""
    line = line.strip()
    if not line:
        return None
    
    try:
        log_entry = json.loads(line)
        return log_entry
    except json.JSONDecodeError:
        # If not JSON, create a simple log entry
        return {
            "timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ"),
            "level": "INFO",
            "message": line,
        }


def extract_fields(log_entry: dict) -> dict:
    """Extract relevant fields from a log entry."""
    fields = {
        "level": log_entry.get("level", "INFO"),
        "message": log_entry.get("fields", {}).get("message", log_entry.get("message", "")),
        "target": log_entry.get("target", ""),
        "file": log_entry.get("file", ""),
        "line": str(log_entry.get("line", "")),
        "timestamp": log_entry.get("timestamp", datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")),
    }
    
    # Extract custom event from fields
    if "fields" in log_entry:
        fields["event"] = log_entry["fields"].get("event", "")
    
    return fields


# ===================== Loki Push =====================

def push_to_loki(entries: List[dict], stream_labels: dict) -> bool:
    """Push log entries to Loki."""
    if not entries:
        return True
    
    # Build Loki payload
    payload = {
        "streams": [
            {
                "stream": {**stream_labels},
                "values": [
                    [
                        str(int(entry["timestamp"].replace("Z", "+00:00").timestamp() * 1e9)),
                        json.dumps(extract_fields(entry) if "timestamp" in entry else {"message": entry}),
                    ]
                ],
            }
            for entry in entries
        ],
    }
    
    try:
        response = requests.post(
            LOKI_PUSH_ENDPOINT,
            json=payload,
            timeout=10,
        )
        if response.status_code in (200, 204):
            return True
        else:
            logger.warning(f"Loki push failed: {response.status_code} - {response.text}")
            return False
    except requests.exceptions.RequestException as e:
        logger.error(f"Loki push error: {e}")
        return False


# ===================== Main Loop =====================

class LogForwarder:
    """Forward logs from remote eBPF nodes to Loki."""
    
    def __init__(self):
        # Track last position for each node to avoid duplicate logs
        self.positions: Dict[str, int] = {}
        self.line_counts: Dict[str, int] = {}
    
    def collect_logs(self) -> bool:
        """Collect logs from all nodes and push to Loki."""
        all_entries = []
        
        for node_name, node_config in NODES.items():
            node_key = f"{node_config['user']}@{node_config['host']}"
            
            # Read logs via SSH
            log_content = ssh_execute(
                node_config["user"],
                node_config["host"],
                node_config["log_path"],
                tail_lines=50,
            )
            
            if log_content is None:
                logger.warning(f"Failed to collect logs from {node_name}")
                continue
            
            # Count lines
            lines = log_content.strip().split("\n")
            self.line_counts[node_name] = len(lines)
            
            # Parse and add node label
            for line in lines:
                if line.strip():
                    entry = parse_log_line(line)
                    if entry:
                        entry["node"] = node_name
                        all_entries.append(entry)
        
        if all_entries:
            # Push to Loki
            success = push_to_loki(all_entries, STREAM_LABELS)
            if success:
                logger.info(f"Pushed {len(all_entries)} log entries to Loki")
            return success
        else:
            logger.debug("No new log entries to push")
            return True
    
    def run(self):
        """Run the forwarder loop."""
        logger.info("eBPF Log Forwarder started")
        logger.info(f"Monitoring {len(NODES)} nodes: {', '.join(NODES.keys())}")
        logger.info(f"Pushing to Loki at {LOKI_URL}")
        logger.info(f"Poll interval: {POLL_INTERVAL}s")
        
        while True:
            try:
                self.collect_logs()
            except Exception as e:
                logger.error(f"Error in collect loop: {e}", exc_info=True)
            
            time.sleep(POLL_INTERVAL)


def main():
    """Main entry point."""
    forwarder = LogForwarder()
    forwarder.run()


if __name__ == "__main__":
    main()
