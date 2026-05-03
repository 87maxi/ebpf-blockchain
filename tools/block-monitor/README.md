# Block Monitor - eBPF Blockchain Real-Time Monitoring Interface

A real-time web monitoring interface for the eBPF Blockchain network. Connect to node WebSocket endpoints to monitor blocks, transactions, and security events as they happen.

## Features

- **Real-time Block Monitoring**: View blocks as they are created, confirmed, or rejected
- **Security Alert Detection**: Automatic flagging of suspicious blocks (Replay, DoubleSpend, Sybil, DDoS)
- **Advanced Filtering**: Search by hash, proposer, or content; filter by status, type, and time range
- **Live Statistics Dashboard**: Track total blocks, suspicious blocks, rejected blocks, validated blocks, and transaction rate
- **Event Log**: Real-time event stream with color-coded severity levels
- **Block Detail Modal**: Click any block to view complete details including transactions and flags

## Quick Start

### Opening the Interface

The interface is a static HTML application - no build step required. Simply open `index.html` in a browser:

```bash
# Using any HTTP server
cd tools/block-monitor
python3 -m http.server 8080
# Then visit http://localhost:8080
```

Or open the file directly:
```bash
xdg-open tools/block-monitor/index.html  # Linux
open tools/block-monitor/index.html       # macOS
```

### Connecting to a Node

1. Enter the WebSocket URL (default: `ws://192.168.2.13:9090/ws`)
2. Enter the API URL (default: `http://192.168.2.13:9090`)
3. Click **Connect**

The interface will automatically reconnect if the connection is lost.

## Interface Components

### Dashboard Stats
| Metric | Description |
|--------|-------------|
| Total Blocks | Number of blocks received |
| Suspicious | Blocks flagged with security alerts |
| Rejected | Blocks rejected by the network |
| Validated | Blocks confirmed by quorum |
| Tx/Second | Transaction rate (last 60s) |
| Latest Height | Highest block height seen |

### Block Status Indicators
| Status | Icon | Meaning |
|--------|------|---------|
| Validated | ✅ | Block confirmed by quorum |
| Pending | ⏳ | Block created, awaiting confirmation |
| Rejected | ❌ | Block rejected (reason shown in details) |
| Suspicious | 🚨 | Block flagged by security system |

### Security Flags
| Flag | Description |
|------|-------------|
| 🔄 Replay | Replay attack detected |
| 💰 DoubleSpend | Double spend attempt |
| 👥 Sybil | Sybil node transaction |
| 💣 DDoS | DDoS flood transaction |

## WebSocket Events

The interface listens for these event types from the node:

```json
{"event": "BlockCreated", "height": 42, "hash": "0x...", "proposer": "...", "tx_count": 5}
{"event": "BlockConfirmed", "height": 42, "hash": "0x...", "voters": 3}
{"event": "BlockRejected", "height": 43, "hash": "0x...", "reason": "replay_detected"}
{"event": "SecurityAlert", "type": "sybil", "source": "...", "action": "blocked"}
{"event": "TxProcessed", "tx_id": "...", "status": "confirmed"}
```

## Architecture

```
js/
├── app.js          # Main application - initializes components and wires events
├── websocket.js    # WebSocket client with auto-reconnect
├── table.js        # Block table rendering and detail modal
└── filters.js      # Search and filter management
```

## Browser Compatibility

- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+

Requires WebSocket support and modern JavaScript (ES2020+).
