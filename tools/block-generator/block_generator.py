#!/usr/bin/env python3
"""
Block Generator Service for eBPF Blockchain Network

Generates realistic transaction patterns simulating production network activity.
Features:
- Burst patterns (high activity periods alternating with calm)
- Transaction type distribution (70% transfers, 15% contracts, 10% votes, 5% swaps)
- Variable latency simulation
- Multiple senders rotation
- Realistic address patterns
- Logarithmic amount distribution
- Occasional simulated failures (2-5%)

Exposes Prometheus metrics on /metrics endpoint (configurable port).

Usage:
    python3 block_generator.py --nodes 192.168.2.210,192.168.2.211,192.168.2.212 --interval 5

Configuration:
    - Nodes: Comma-separated list of eBPF node IPs
    - Interval: Seconds between transaction batches (default: 5)
    - Batch size: Transactions per batch (default: 3)
    - Sender: Unique sender ID for nonce tracking (default: "block-generator")
"""

import argparse
import hashlib
import json
import logging
import math
import os
import random
import socket
import string
import sys
import threading
import time
from datetime import datetime, timezone
from http.server import HTTPServer, BaseHTTPRequestHandler
from pathlib import Path
from typing import Any

import requests

# Default configuration file path
DEFAULT_CONFIG_PATH = os.path.expanduser("~/.ebpf-blockchain/block-generator.conf")

# Logging setup
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
logger = logging.getLogger("ebpf-block-generator")


# =============================================================================
# Prometheus Metrics Collector
# =============================================================================

class PrometheusMetrics:
    """Collects and exposes Prometheus metrics."""

    def __init__(self, node_id: str = "unknown"):
        self.node_id = node_id
        self.lock = threading.Lock()

        # Counters
        self.transactions_total: dict[str, int] = {}  # {sender: {type: count}}
        self.total_transactions = 0
        self.successful_transactions = 0
        self.failed_transactions = 0
        self.total_batches = 0

        # Histogram data (latency buckets)
        self.latency_samples: list[float] = []
        self.batch_durations: list[float] = []

        # Gauge values
        self.active_senders: set[str] = set()
        self.current_batch_size = 0

        # Tracking
        self.start_time = time.time()
        self.sender_tx_counts: dict[str, int] = {}

    def record_transaction(self, sender: str, tx_type: str, success: bool, latency: float):
        """Record a transaction event."""
        with self.lock:
            self.total_transactions += 1
            self.latency_samples.append(latency)

            # Keep only last 1000 latency samples
            if len(self.latency_samples) > 1000:
                self.latency_samples = self.latency_samples[-1000:]

            if success:
                self.successful_transactions += 1
            else:
                self.failed_transactions += 1

            # Track by sender and type
            if sender not in self.transactions_total:
                self.transactions_total[sender] = {}
            if tx_type not in self.transactions_total[sender]:
                self.transactions_total[sender][tx_type] = 0
            self.transactions_total[sender][tx_type] += 1

            self.sender_tx_counts[sender] = self.sender_tx_counts.get(sender, 0) + 1

    def record_batch(self, duration: float, size: int):
        """Record batch completion."""
        with self.lock:
            self.total_batches += 1
            self.batch_durations.append(duration)
            self.current_batch_size = size

            # Keep only last 500 batch durations
            if len(self.batch_durations) > 500:
                self.batch_durations = self.batch_durations[-500:]

    def get_metrics_text(self) -> str:
        """Generate Prometheus exposition format metrics."""
        with self.lock:
            lines = []
            now = int(time.time())

            # Total transactions counter
            lines.append("# HELP ebpf_blockgen_transactions_total Total transactions by sender, type and node")
            lines.append("# TYPE ebpf_blockgen_transactions_total counter")
            for sender, types in self.transactions_total.items():
                for tx_type, count in types.items():
                    lines.append(
                        f'ebpf_blockgen_transactions_total{{sender="{sender}",type="{tx_type}",node="{self.node_id}"}} {count}'
                    )
            lines.append(f'ebpf_blockgen_transactions_total{{node="{self.node_id}"}} {self.total_transactions}')

            # Success/Failure counters
            lines.append("# HELP ebpf_blockgen_transactions_successful Total successful transactions")
            lines.append("# TYPE ebpf_blockgen_transactions_successful counter")
            lines.append(f'ebpf_blockgen_transactions_successful{{node="{self.node_id}"}} {self.successful_transactions}')

            lines.append("# HELP ebpf_blockgen_transactions_failed Total failed transactions")
            lines.append("# TYPE ebpf_blockgen_transactions_failed counter")
            lines.append(f'ebpf_blockgen_transactions_failed{{node="{self.node_id}"}} {self.failed_transactions}')

            # Latency histogram
            lines.append("# HELP ebpf_blockgen_transaction_seconds Transaction latency in seconds")
            lines.append("# TYPE ebpf_blockgen_transaction_seconds histogram")
            if self.latency_samples:
                buckets = {}
                for bucket_limit in [0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]:
                    count = sum(1 for s in self.latency_samples if s <= bucket_limit)
                    buckets[bucket_limit] = count
                else_bucket = len(self.latency_samples)
                for bucket_limit, count in buckets.items():
                    lines.append(
                        f'ebpf_blockgen_transaction_seconds_bucket{{le="{bucket_limit}",node="{self.node_id}"}} {count}'
                    )
                lines.append(
                    f'ebpf_blockgen_transaction_seconds_bucket{{le="+Inf",node="{self.node_id}"}} {else_bucket}'
                )
                lines.append(
                    f'ebpf_blockgen_transaction_seconds_sum{{node="{self.node_id}"}} {sum(self.latency_samples):.6f}'
                )
                lines.append(
                    f'ebpf_blockgen_transaction_seconds_count{{node="{self.node_id}"}} {len(self.latency_samples)}'
                )
            else:
                lines.append(f'ebpf_blockgen_transaction_seconds_bucket{{le="+Inf",node="{self.node_id}"}} 0')
                lines.append(f'ebpf_blockgen_transaction_seconds_sum{{node="{self.node_id}"}} 0')
                lines.append(f'ebpf_blockgen_transaction_seconds_count{{node="{self.node_id}"}} 0')

            # Batch duration histogram
            lines.append("# HELP ebpf_blockgen_batch_duration_seconds Batch generation duration")
            lines.append("# TYPE ebpf_blockgen_batch_duration_seconds histogram")
            if self.batch_durations:
                buckets = {}
                for bucket_limit in [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]:
                    count = sum(1 for d in self.batch_durations if d <= bucket_limit)
                    buckets[bucket_limit] = count
                else_bucket = len(self.batch_durations)
                for bucket_limit, count in buckets.items():
                    lines.append(
                        f'ebpf_blockgen_batch_duration_seconds_bucket{{le="{bucket_limit}",node="{self.node_id}"}} {count}'
                    )
                lines.append(
                    f'ebpf_blockgen_batch_duration_seconds_bucket{{le="+Inf",node="{self.node_id}"}} {else_bucket}'
                )
                lines.append(
                    f'ebpf_blockgen_batch_duration_seconds_sum{{node="{self.node_id}"}} {sum(self.batch_durations):.6f}'
                )
                lines.append(
                    f'ebpf_blockgen_batch_duration_seconds_count{{node="{self.node_id}"}} {len(self.batch_durations)}'
                )
            else:
                lines.append(f'ebpf_blockgen_batch_duration_seconds_bucket{{le="+Inf",node="{self.node_id}"}} 0')
                lines.append(f'ebpf_blockgen_batch_duration_seconds_sum{{node="{self.node_id}"}} 0')
                lines.append(f'ebpf_blockgen_batch_duration_seconds_count{{node="{self.node_id}"}} 0')

            # Active senders gauge
            lines.append("# HELP ebpf_blockgen_active_senders Number of active senders")
            lines.append("# TYPE ebpf_blockgen_active_senders gauge")
            lines.append(f'ebpf_blockgen_active_senders{{node="{self.node_id}"}} {len(self.active_senders)}')

            # Current batch size gauge
            lines.append("# HELP ebpf_blockgen_current_batch_size Current batch size")
            lines.append("# TYPE ebpf_blockgen_current_batch_size gauge")
            lines.append(f'ebpf_blockgen_current_batch_size{{node="{self.node_id}"}} {self.current_batch_size}')

            # Total batches gauge
            lines.append("# HELP ebpf_blockgen_batches_total Total batches generated")
            lines.append("# TYPE ebpf_blockgen_batches_total counter")
            lines.append(f'ebpf_blockgen_batches_total{{node="{self.node_id}"}} {self.total_batches}')

            # Uptime gauge
            uptime = int(time.time()) - int(self.start_time)
            lines.append("# HELP ebpf_blockgen_uptime_seconds Service uptime in seconds")
            lines.append("# TYPE ebpf_blockgen_uptime_seconds gauge")
            lines.append(f'ebpf_blockgen_uptime_seconds{{node="{self.node_id}"}} {uptime}')

            # Success rate gauge
            if self.total_transactions > 0:
                success_rate = self.successful_transactions / self.total_transactions * 100
            else:
                success_rate = 100.0
            lines.append("# HELP ebpf_blockgen_success_rate_percent Transaction success rate percentage")
            lines.append("# TYPE ebpf_blockgen_success_rate_percent gauge")
            lines.append(f'ebpf_blockgen_success_rate_percent{{node="{self.node_id}"}} {success_rate:.2f}')

            # Transactions per second gauge
            if uptime > 0:
                tps = self.total_transactions / uptime
            else:
                tps = 0.0
            lines.append("# HELP ebpf_blockgen_transactions_per_second Current transaction rate")
            lines.append("# TYPE ebpf_blockgen_transactions_per_second gauge")
            lines.append(f'ebpf_blockgen_transactions_per_second{{node="{self.node_id}"}} {tps:.4f}')

            return "\n".join(lines) + "\n"


# =============================================================================
# Transaction Generator with Realistic Patterns
# =============================================================================

class RealisticTransactionGenerator:
    """Generates transactions with realistic patterns simulating production traffic."""

    # Transaction type distribution: 70% transfers, 15% contracts, 10% votes, 5% swaps
    TRANSACTION_TYPES = {
        "transfer": 0.70,
        "contract": 0.15,
        "vote": 0.10,
        "swap": 0.05,
    }

    # Sender pool (simulates different users)
    SENDER_POOL = [
        "user-42",
        "user-17",
        "user-89",
        "user-3",
        "user-56",
        "trader-bot-1",
        "trader-bot-2",
        "defi-user-7",
        "nft-collector",
        "whale-account",
    ]

    # Common destination address patterns (some addresses repeated)
    COMMON_ADDRESSES = [
        "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
        "0x286C34d174B3A60a5045ECE96C0eB2bC1C5a67eF",
        "0x9f8c163cBA788eFC03410D6F52e9fDb14a3a1b3a",
        "0xaB31bDA8C8F6eC0e5eC6e7e1a3E5c8D2F1A9b7cE",
        "0x1234567890abcdef1234567890abcdef12345678",
        "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
    ]

    TOKENS = ["EBPF", "ETH", "USDC", "DAI", "LINK", "UNI", "AAVE"]

    def __init__(self, failure_rate: float = 0.03):
        self.failure_rate = failure_rate
        self.nonce = 0
        self.nonce_lock = threading.Lock()
        self.current_sender_index = 0

    def get_next_nonce(self) -> int:
        """Get next nonce thread-safely."""
        with self.nonce_lock:
            self.nonce += 1
            return self.nonce

    def select_sender(self) -> str:
        """Select sender, rotating through pool with some randomness."""
        # 80% chance to rotate to next sender, 20% chance to pick random
        if random.random() < 0.8:
            sender = self.SENDER_POOL[self.current_sender_index % len(self.SENDER_POOL)]
            self.current_sender_index += 1
        else:
            sender = random.choice(self.SENDER_POOL)
        return sender

    def select_transaction_type(self) -> str:
        """Select transaction type based on distribution."""
        r = random.random()
        cumulative = 0.0
        for tx_type, probability in self.TRANSACTION_TYPES.items():
            cumulative += probability
            if r <= cumulative:
                return tx_type
        return "transfer"

    def generate_amount(self) -> float:
        """Generate amount using logarithmic distribution (many small, few large)."""
        # Log-normal distribution: most amounts are small, some are large
        log_mean = 2.0
        log_std = 1.5
        amount = math.exp(random.gauss(log_mean, log_std))
        return round(min(amount, 100000.0), 4)  # Cap at 100k

    def generate_destination(self) -> str:
        """Generate destination address - reuse common addresses sometimes."""
        # 60% chance to use common address, 40% new random address
        if random.random() < 0.6:
            return random.choice(self.COMMON_ADDRESSES)
        else:
            return "0x" + "".join(random.choices("0123456789abcdef", k=40))

    def generate_transaction_data(self, tx_type: str, amount: float, destination: str) -> str:
        """Generate realistic transaction data string."""
        if tx_type == "transfer":
            return f"Transfer {amount} {random.choice(self.TOKENS)} to {destination}"
        elif tx_type == "contract":
            actions = ["update", "deploy", "call", "initialize", "setParameter"]
            params = ["stake", "reward", "fee", "limit", "threshold"]
            return f"Update smart contract: action={random.choice(actions)} param={random.choice(params)} value={int(amount)}"
        elif tx_type == "vote":
            proposals = random.randint(1000, 9999)
            decisions = ["yes", "no", "abstain"]
            weights = [0.5, 0.3, 0.2]
            return f"Vote: proposal={proposals} decision={random.choices(decisions, weights=weights)[0]}"
        elif tx_type == "swap":
            token_a = random.choice(self.TOKENS)
            token_b = random.choice([t for t in self.TOKENS if t != token_a])
            return f"Token swap: {amount} {token_a} -> {amount * 0.99:,.2f} {token_b} (0.1% fee)"
        return "Unknown transaction"

    def create_transaction(self) -> dict[str, Any]:
        """Create a single realistic transaction."""
        tx_type = self.select_transaction_type()
        sender = self.select_sender()
        amount = self.generate_amount()
        destination = self.generate_destination()
        nonce = self.get_next_nonce()

        tx = {
            "id": self._generate_tx_id(),
            "data": self.generate_transaction_data(tx_type, amount, destination),
            "type": tx_type,
            "sender": sender,
            "nonce": nonce,
            "timestamp": int(time.time()),
            "metadata": {
                "amount": amount,
                "destination": destination,
                "token": random.choice(self.TOKENS) if tx_type == "transfer" else None,
                "fee_estimated": round(random.uniform(0.001, 0.05), 4),
            },
        }
        return tx

    def _generate_tx_id(self) -> str:
        """Generate unique transaction ID."""
        return hashlib.sha256(
            f"{self.nonce}-{time.time_ns()}-{random.randint(0, 1000000)}".encode()
        ).hexdigest()[:16]


# =============================================================================
# Traffic Pattern Simulator
# =============================================================================

class TrafficPatternSimulator:
    """Simulates realistic traffic patterns with bursts and calm periods."""

    def __init__(self):
        self.state = "calm"  # calm, building, burst, cooling
        self.state_start = time.time()
        self.burst_multiplier = 1.0

    def get_batch_config(self) -> dict:
        """Get current batch configuration based on traffic pattern."""
        elapsed = time.time() - self.state_start

        if self.state == "calm":
            # Calm period: 1-2 transactions, 10-30s duration
            if elapsed > random.uniform(10, 30):
                self._transition("building")
                return {"batch_size": random.randint(1, 2), "latency_offset": 0.0}

        elif self.state == "building":
            # Building up to burst: 2-4 transactions, 3-8s duration
            if elapsed > random.uniform(3, 8):
                self._transition("burst")
                return {"batch_size": random.randint(2, 4), "latency_offset": 0.0}

        elif self.state == "burst":
            # Burst period: 5-15 transactions, 5-15s duration
            if elapsed > random.uniform(5, 15):
                self._transition("cooling")
                return {"batch_size": random.randint(5, 15), "latency_offset": 0.0}

        elif self.state == "cooling":
            # Cooling down: 1-3 transactions, 5-15s duration
            if elapsed > random.uniform(5, 15):
                self._transition("calm")
                return {"batch_size": random.randint(1, 3), "latency_offset": 0.0}

        return {"batch_size": random.randint(1, 3), "latency_offset": 0.0}

    def _transition(self, new_state: str):
        """Transition to new traffic state."""
        self.state = new_state
        self.state_start = time.time()
        logger.debug(f"Traffic pattern transition: {self.state}")


# =============================================================================
# Block Generator (Main Orchestrator)
# =============================================================================

class BlockGenerator:
    """Main block generator with realistic traffic patterns and Prometheus metrics."""

    def __init__(
        self,
        nodes: list[str],
        interval: int = 5,
        batch_size: int = 3,
        sender: str = "block-generator",
        config_path: str = DEFAULT_CONFIG_PATH,
        failure_rate: float = 0.03,
        metrics_port: int = 9101,
        node_id: str | None = None,
    ):
        self.nodes = nodes
        self.interval = interval
        self.base_batch_size = batch_size
        self.sender = sender
        self.config_path = config_path
        self.failure_rate = failure_rate

        # Generate node_id from first node if not provided
        self.node_id = node_id or nodes[0].replace(".", "_") if nodes else "unknown"

        # Initialize components
        self.metrics = PrometheusMetrics(node_id=self.node_id)
        self.tx_generator = RealisticTransactionGenerator(failure_rate=failure_rate)
        self.traffic_pattern = TrafficPatternSimulator()

        # State
        self.nonce = 0
        self.running = False
        self.stats = {
            "total_sent": 0,
            "total_failed": 0,
            "total_confirmed": 0,
            "start_time": None,
        }

        # Metrics HTTP server
        self.metrics_port = metrics_port
        self.metrics_server: HTTPServer | None = None
        self.metrics_thread: threading.Thread | None = None

        # Load saved state if exists
        self._load_state()

    def _load_state(self):
        """Load saved nonce and stats from config file."""
        config_file = Path(self.config_path)
        if config_file.exists():
            try:
                with open(config_file, "r") as f:
                    state = json.load(f)
                self.nonce = state.get("nonce", 0)
                self.tx_generator.nonce = self.nonce
                self.stats["total_sent"] = state.get("total_sent", 0)
                self.stats["total_failed"] = state.get("total_failed", 0)
                self.stats["total_confirmed"] = state.get("total_confirmed", 0)
                logger.info(
                    f"Loaded state: nonce={self.nonce}, sent={self.stats['total_sent']}"
                )
            except Exception as e:
                logger.warning(f"Could not load state: {e}")

    def _save_state(self):
        """Save current state to config file."""
        try:
            config_file = Path(self.config_path)
            config_file.parent.mkdir(parents=True, exist_ok=True)
            state = {
                "nonce": self.nonce,
                "total_sent": self.stats["total_sent"],
                "total_failed": self.stats["total_failed"],
                "total_confirmed": self.stats["total_confirmed"],
                "last_updated": datetime.now(timezone.utc).isoformat(),
            }
            with open(config_file, "w") as f:
                json.dump(state, f, indent=2)
        except Exception as e:
            logger.warning(f"Could not save state: {e}")

    def send_transaction(self, tx: dict) -> bool:
        """Send a transaction to a node with round-robin selection."""
        node = self.nodes[self.stats["total_sent"] % len(self.nodes)]
        url = f"http://{node}:9091/api/v1/transactions"

        start_time = time.time()

        try:
            response = requests.post(
                url,
                json=tx,
                headers={"Content-Type": "application/json"},
                timeout=10,
            )

            latency = time.time() - start_time

            if response.status_code in (200, 201, 202):
                self.stats["total_sent"] += 1
                result = response.json()
                logger.debug(
                    f"Sent tx {tx['id']} to {node}: status={result.get('status', 'unknown')} "
                    f"type={tx['type']} sender={tx['sender']}"
                )
                self.metrics.record_transaction(
                    sender=tx["sender"],
                    tx_type=tx["type"],
                    success=True,
                    latency=latency,
                )
                return True
            else:
                self.stats["total_failed"] += 1
                logger.warning(
                    f"Failed to send tx {tx['id']} to {node}: HTTP {response.status_code} - {response.text[:200]}"
                )
                self.metrics.record_transaction(
                    sender=tx["sender"],
                    tx_type=tx["type"],
                    success=False,
                    latency=latency,
                )
                return False

        except requests.exceptions.RequestException as e:
            latency = time.time() - start_time
            self.stats["total_failed"] += 1
            logger.warning(f"Error sending tx {tx['id']} to {node}: {e}")
            self.metrics.record_transaction(
                sender=tx["sender"],
                tx_type=tx["type"],
                success=False,
                latency=latency,
            )
            return False

    def generate_batch(self) -> int:
        """Generate and send a batch of transactions with realistic patterns."""
        batch_start = time.time()

        # Get batch config from traffic pattern simulator
        config = self.traffic_pattern.get_batch_config()
        batch_size = config["batch_size"]

        # Update active senders tracking
        self.metrics.active_senders = set()

        logger.info(
            f"Generating {batch_size} transactions (traffic: {self.traffic_pattern.state}, "
            f"nonce start: {self.tx_generator.nonce + 1})"
        )

        self.metrics.current_batch_size = batch_size
        success_count = 0

        for i in range(batch_size):
            tx = self.tx_generator.create_transaction()
            self.metrics.active_senders.add(tx["sender"])

            # Add simulated latency variation
            latency_offset = random.uniform(0.01, 0.15)
            time.sleep(latency_offset)

            # Simulate occasional failures
            if random.random() < self.failure_rate:
                logger.debug(f"Simulated failure for tx {tx['id']}")
                self.stats["total_failed"] += 1
                self.metrics.record_transaction(
                    sender=tx["sender"],
                    tx_type=tx["type"],
                    success=False,
                    latency=latency_offset,
                )
                continue

            if self.send_transaction(tx):
                success_count += 1

        batch_duration = time.time() - batch_start
        self.metrics.record_batch(batch_duration, batch_size)

        # Save state after each batch
        self._save_state()

        return success_count

    def _start_metrics_server(self):
        """Start HTTP server for Prometheus metrics."""
        metrics_data = [None]  # Use list for mutability in closure

        class MetricsHandler(BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path == "/metrics":
                    content = metrics_data[0].get_metrics_text()
                    self.send_response(200)
                    self.send_header("Content-Type", "text/plain; version=0.0.0; charset=utf-8")
                    self.end_headers()
                    self.wfile.write(content.encode())
                elif self.path == "/health":
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    health = {
                        "status": "healthy",
                        "node_id": self.server.node_id if hasattr(self.server, 'node_id') else "unknown",
                        "uptime": int(time.time() - (self.server.start_time if hasattr(self.server, 'start_time') else time.time())),
                    }
                    self.wfile.write(json.dumps(health).encode())
                else:
                    self.send_response(404)
                    self.end_headers()

            def log_message(self, format, *args):
                pass  # Suppress default logging

        try:
            server = HTTPServer(("0.0.0.0", self.metrics_port), MetricsHandler)
            server.node_id = self.node_id
            server.start_time = self.stats["start_time"] or time.time()
            self.metrics_server = server
            metrics_data[0] = self.metrics

            self.metrics_thread = threading.Thread(target=server.serve_forever, daemon=True)
            self.metrics_thread.start()
            logger.info(f"Prometheus metrics server started on port {self.metrics_port}")
        except OSError as e:
            logger.error(f"Failed to start metrics server on port {self.metrics_port}: {e}")

    def print_stats(self):
        """Print current statistics."""
        uptime = (
            int(time.time()) - self.stats["start_time"]
            if self.stats["start_time"]
            else 0
        )
        total_attempts = self.stats["total_sent"] + self.stats["total_failed"]
        success_rate = self.stats["total_sent"] / max(1, total_attempts) * 100

        logger.info("=" * 70)
        logger.info("BLOCK GENERATOR STATISTICS")
        logger.info("=" * 70)
        logger.info(f"Uptime:              {uptime}s ({uptime // 3600}h {(uptime % 3600) // 60}m {uptime % 60}s)")
        logger.info(f"Total Sent:          {self.stats['total_sent']}")
        logger.info(f"Total Failed:        {self.stats['total_failed']}")
        logger.info(f"Success Rate:        {success_rate:.1f}%")
        logger.info(f"Current Nonce:       {self.nonce}")
        logger.info(f"Nodes:               {', '.join(self.nodes)}")
        logger.info(f"Interval:            {self.interval}s")
        logger.info(f"Base Batch Size:     {self.base_batch_size}")
        logger.info(f"Traffic Pattern:     {self.traffic_pattern.state}")
        logger.info(f"Active Senders:      {len(self.metrics.active_senders)}")
        logger.info(f"Metrics Server:      port {self.metrics_port}")
        logger.info("-" * 70)
        logger.info("Transaction Types:")
        for sender, types in self.metrics.transactions_total.items():
            total = sum(types.values())
            logger.info(f"  {sender}: {total} tx ({', '.join(f'{k}={v}' for k, v in types.items())})")
        logger.info("=" * 70)

    def run(self):
        """Main loop: generate transactions at configured interval."""
        self.running = True
        self.stats["start_time"] = int(time.time())

        # Start metrics server
        self._start_metrics_server()

        logger.info("=" * 70)
        logger.info("eBPF Blockchain Block Generator Starting")
        logger.info("=" * 70)
        logger.info(f"Nodes:              {', '.join(self.nodes)}")
        logger.info(f"Interval:           {self.interval}s")
        logger.info(f"Base Batch Size:    {self.base_batch_size}")
        logger.info(f"Sender:             {self.sender}")
        logger.info(f"Initial Nonce:      {self.nonce + 1}")
        logger.info(f"Failure Rate:       {self.failure_rate * 100:.1f}%")
        logger.info(f"Metrics Port:       {self.metrics_port}")
        logger.info(f"Node ID:            {self.node_id}")
        logger.info("=" * 70)

        # Print initial stats
        self.print_stats()

        while self.running:
            try:
                self.generate_batch()
                time.sleep(self.interval)
            except KeyboardInterrupt:
                logger.info("Interrupted by user")
                self.running = False
            except Exception as e:
                logger.error(f"Unexpected error: {e}", exc_info=True)
                time.sleep(self.interval)

        # Final stats
        self.print_stats()
        logger.info("Block Generator Service stopped.")

        # Stop metrics server
        if self.metrics_server:
            self.metrics_server.shutdown()


def main():
    parser = argparse.ArgumentParser(
        description="eBPF Blockchain Block Generator Service - Realistic Traffic Simulator",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Generate transactions every 5 seconds to multiple nodes
  python3 block_generator.py --nodes 192.168.2.210,192.168.2.211,192.168.2.212 --interval 5

  # Custom configuration with failure simulation
  python3 block_generator.py --nodes 192.168.2.210 --interval 3 --batch-size 5 --failure-rate 0.03

  # Custom metrics port
  python3 block_generator.py --nodes 192.168.2.210 --metrics-port 9101

  # Use configuration file
  python3 block_generator.py --config /etc/ebpf-blockchain/block-generator.conf
        """,
    )

    parser.add_argument(
        "--nodes",
        type=str,
        default="192.168.2.210",
        help="Comma-separated list of eBPF node IPs (default: 192.168.2.210)",
    )

    parser.add_argument(
        "--interval",
        type=int,
        default=5,
        help="Seconds between transaction batches (default: 5)",
    )

    parser.add_argument(
        "--batch-size",
        type=int,
        default=3,
        help="Base number of transactions per batch (default: 3, actual varies with traffic patterns)",
    )

    parser.add_argument(
        "--sender",
        type=str,
        default="block-generator",
        help="Unique sender ID for nonce tracking (default: block-generator)",
    )

    parser.add_argument(
        "--config",
        type=str,
        default=DEFAULT_CONFIG_PATH,
        help=f"Configuration file path (default: {DEFAULT_CONFIG_PATH})",
    )

    parser.add_argument(
        "--failure-rate",
        type=float,
        default=0.03,
        help="Simulated failure rate (0.0-1.0, default: 0.03 = 3%)",
    )

    parser.add_argument(
        "--metrics-port",
        type=int,
        default=9101,
        help="Port for Prometheus metrics endpoint (default: 9101)",
    )

    parser.add_argument(
        "--node-id",
        type=str,
        default=None,
        help="Custom node ID for metrics (default: derived from first node IP)",
    )

    parser.add_argument(
        "--daemon",
        action="store_true",
        help="Run as daemon (background process)",
    )

    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable debug logging",
    )

    args = parser.parse_args()

    if args.verbose:
        logger.setLevel(logging.DEBUG)
        logging.getLogger("requests").setLevel(logging.WARNING)

    # Parse nodes
    nodes = [n.strip() for n in args.nodes.split(",") if n.strip()]
    if not nodes:
        logger.error("No nodes specified")
        sys.exit(1)

    logger.info(f"Configuration: nodes={nodes}, interval={args.interval}, batch={args.batch_size}")

    # Create and run generator
    generator = BlockGenerator(
        nodes=nodes,
        interval=args.interval,
        batch_size=args.batch_size,
        sender=args.sender,
        config_path=args.config,
        failure_rate=args.failure_rate,
        metrics_port=args.metrics_port,
        node_id=args.node_id,
    )

    if args.daemon:
        # Simple daemonization
        try:
            pid = os.fork()
            if pid > 0:
                logger.info(f"Block Generator daemon started (PID: {pid})")
                sys.exit(0)
        except OSError as e:
            logger.error(f"Daemon fork failed: {e}")
            sys.exit(1)

    generator.run()


if __name__ == "__main__":
    main()
