#!/usr/bin/env python3
"""
Block Generator Service for eBPF Blockchain Network

Generates transactions periodically to trigger block creation in the eBPF cluster.
This service sends transactions to one or more eBPF nodes via their REST API.

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
import os
import random
import string
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import requests

# Default configuration file path
DEFAULT_CONFIG_PATH = os.path.expanduser("~/.ebpf-blockchain/block-generator.conf")

# Logging setup
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
logger = logging.getLogger(__name__)


class BlockGenerator:
    """Generates transactions to trigger block creation in eBPF blockchain network."""

    def __init__(
        self,
        nodes: list[str],
        interval: int = 5,
        batch_size: int = 3,
        sender: str = "block-generator",
        config_path: str = DEFAULT_CONFIG_PATH,
    ):
        self.nodes = nodes
        self.interval = interval
        self.batch_size = batch_size
        self.sender = sender
        self.config_path = config_path
        self.nonce = 0
        self.running = False
        self.stats = {
            "total_sent": 0,
            "total_failed": 0,
            "total_confirmed": 0,
            "start_time": None,
        }

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

    def generate_transaction_id(self) -> str:
        """Generate a unique transaction ID."""
        return hashlib.sha256(
            f"{self.nonce}-{time.time_ns()}-{random.randint(0, 1000000)}".encode()
        ).hexdigest()[:16]

    def generate_transaction_data(self) -> str:
        """Generate random transaction data."""
        templates = [
            f"Transfer {random.randint(1, 1000)} tokens to {self._random_addr()}",
            f"Update smart contract parameter: key={self._random_key()}, value={random.randint(0, 100)}",
            f"Registry event: action={random.choice(['create', 'update', 'delete'])} entity={self._random_entity()}",
            f"Vote: proposal={random.randint(1000, 9999)} decision={random.choice(['yes', 'no', 'abstain'])}",
            f"Token swap: from={self._random_token()} to={self._random_token()} amount={random.randint(10, 5000)}",
        ]
        return random.choice(templates)

    def _random_addr(self) -> str:
        return "0x" + "".join(random.choices("0123456789abcdef", k=40))

    def _random_key(self) -> str:
        return "key_" + "".join(random.choices("0123456789abcdef", k=8))

    def _random_entity(self) -> str:
        return random.choice(["user", "device", "sensor", "contract", "account"])

    def _random_token(self) -> str:
        return random.choice(["EBPF", "ETH", "USDC", "DAI", "LINK"])

    def create_transaction(self) -> dict:
        """Create a new transaction."""
        tx_id = self.generate_transaction_id()
        self.nonce += 1

        tx = {
            "id": tx_id,
            "data": self.generate_transaction_data(),
            "nonce": self.nonce,
            "timestamp": int(time.time()),
        }
        return tx

    def send_transaction(self, tx: dict) -> bool:
        """Send a transaction to a random node."""
        # Round-robin across nodes
        node = self.nodes[self.stats["total_sent"] % len(self.nodes)]
        url = f"http://{node}:9091/api/v1/transactions"

        try:
            response = requests.post(
                url, json=tx, headers={"Content-Type": "application/json"}, timeout=10
            )

            if response.status_code in (200, 201, 202):
                self.stats["total_sent"] += 1
                result = response.json()
                logger.info(
                    f"Sent tx {tx['id']} to {node}: status={result.get('status', 'unknown')}"
                )
                return True
            else:
                self.stats["total_failed"] += 1
                logger.warning(
                    f"Failed to send tx {tx['id']} to {node}: HTTP {response.status_code} - {response.text[:200]}"
                )
                return False

        except requests.exceptions.RequestException as e:
            self.stats["total_failed"] += 1
            logger.warning(f"Error sending tx {tx['id']} to {node}: {e}")
            return False

    def generate_batch(self) -> int:
        """Generate and send a batch of transactions. Returns number of successful sends."""
        logger.info(
            f"Generating batch of {self.batch_size} transactions (nonce start: {self.nonce + 1})"
        )

        success_count = 0
        for i in range(self.batch_size):
            tx = self.create_transaction()
            if self.send_transaction(tx):
                success_count += 1

        # Save state after each batch
        self._save_state()

        return success_count

    def print_stats(self):
        """Print current statistics."""
        uptime = (
            int(time.time()) - self.stats["start_time"]
            if self.stats["start_time"]
            else 0
        )
        logger.info("=" * 60)
        logger.info("BLOCK GENERATOR STATISTICS")
        logger.info("=" * 60)
        logger.info(f"Uptime:           {uptime}s ({uptime // 60}m {uptime % 60}s)")
        logger.info(f"Total Sent:       {self.stats['total_sent']}")
        logger.info(f"Total Failed:     {self.stats['total_failed']}")
        logger.info(f"Success Rate:     {self.stats['total_sent'] / max(1, self.stats['total_sent'] + self.stats['total_failed']) * 100:.1f}%")
        logger.info(f"Current Nonce:    {self.nonce}")
        logger.info(f"Nodes:            {', '.join(self.nodes)}")
        logger.info(f"Interval:         {self.interval}s")
        logger.info(f"Batch Size:       {self.batch_size}")
        logger.info("=" * 60)

    def run(self):
        """Main loop: generate transactions at configured interval."""
        self.running = True
        self.stats["start_time"] = int(time.time())

        logger.info("Block Generator Service starting...")
        logger.info(f"Nodes: {', '.join(self.nodes)}")
        logger.info(f"Interval: {self.interval}s | Batch Size: {self.batch_size}")
        logger.info(f"Sender: {self.sender} | Initial Nonce: {self.nonce + 1}")

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


def main():
    parser = argparse.ArgumentParser(
        description="eBPF Blockchain Block Generator Service",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Generate transactions every 5 seconds to a single node
  python3 block_generator.py --nodes 192.168.2.210 --interval 5

  # Generate transactions to multiple nodes
  python3 block_generator.py --nodes 192.168.2.210,192.168.2.211,192.168.2.212

  # Custom batch size and interval
  python3 block_generator.py --nodes 192.168.2.210 --interval 10 --batch-size 5

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
        help="Number of transactions per batch (default: 3)",
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
