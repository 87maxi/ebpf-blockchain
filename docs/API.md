# eBPF Blockchain - API Documentation

## Overview

The eBPF Blockchain node exposes a REST API for interaction, monitoring, and management. All API endpoints are served by the Axum HTTP server on port `9091` by default.

### Base URL

```
http://<node-host>:9091/api/v1
```

### Content Types

- **Request**: `application/json`
- **Response**: `application/json`

### Authentication

Currently, the API does not require authentication in development mode. For production deployments, API key authentication should be configured via environment variables:

```bash
export API_AUTH_KEY="your-secret-key"
```

When enabled, include the API key in requests:

```bash
curl -H "X-API-Key: $API_AUTH_KEY" http://localhost:9091/api/v1/node/info
```

---

## Endpoints

### Node

#### GET /api/v1/node/info

Get comprehensive node information.

**Response:**

```json
{
  "node_id": "12D3KooW..." ,
  "version": "1.0.0",
  "uptime_seconds": 3600,
  "peers_connected": 5,
  "blocks_proposed": 42,
  "blocks_validated": 128,
  "transactions_processed": 256,
  "current_height": 42,
  "is_validator": true,
  "stake": 10000,
  "reputation_score": 0.95
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `node_id` | string | libp2p PeerId |
| `version` | string | Software version |
| `uptime_seconds` | integer | Seconds since start |
| `peers_connected` | integer | Active peer count |
| `blocks_proposed` | integer | Total blocks proposed |
| `blocks_validated` | integer | Total blocks validated |
| `transactions_processed` | integer | Total transactions processed |
| `current_height` | integer | Current blockchain height |
| `is_validator` | boolean | Whether node is a validator |
| `stake` | integer | Node stake amount |
| `reputation_score` | float | Reputation score (0.0 - 1.0) |

---

### Network

#### GET /api/v1/network/peers

Get list of connected peers.

**Query Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `transport` | string | Filter by transport (QUIC, TCP) |
| `validator` | boolean | Filter by validator status |

**Response:**

```json
{
  "peers": [
    {
      "peer_id": "12D3KooW...",
      "address": "/ip4/192.168.1.100/tcp/9000",
      "transport": "QUIC",
      "latency_ms": 5.2,
      "reputation": 0.95,
      "is_validator": true,
      "connected_since": "2026-04-21T08:00:00Z",
      "messages_sent": 128,
      "messages_received": 256
    }
  ],
  "total": 5
}
```

---

#### GET /api/v1/network/config

Get or update network configuration.

**GET Response:**

```json
{
  "p2p_port": 9000,
  "quic_port": 9001,
  "max_connections": 100,
  "bootstrap_peers": [
    "/ip4/192.168.1.100/tcp/9000"
  ],
  "mdns_enabled": true,
  "gossipsub_params": {
    "mesh_size": 12,
    "random_mesh_size": 4
  }
}
```

**PUT Request:**

```json
{
  "max_connections": 200,
  "bootstrap_peers": [
    "/ip4/192.168.1.100/tcp/9000",
    "/ip4/192.168.1.101/tcp/9000"
  ]
}
```

**Response:**

```json
{
  "success": true,
  "config": { ... }
}
```

---

### Transactions

#### POST /api/v1/transactions

Create and submit a new transaction.

**Request:**

```json
{
  "id": "tx-001",
  "data": "hello world",
  "nonce": 1
}
```

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique transaction ID |
| `data` | string | Yes | Transaction data/payload |
| `nonce` | integer | Yes | Monotonically increasing nonce |

**Response (201 Created):**

```json
{
  "hash": "0xabc123...",
  "status": "pending",
  "block_number": null,
  "timestamp": "2026-04-21T10:00:00Z",
  "nonce": 1
}
```

**Response (200 OK - Already processed):**

```json
{
  "error": "Transaction already processed",
  "tx_id": "tx-001"
}
```

**Error Responses:**

| Status | Description |
|--------|-------------|
| 400 | Invalid transaction (bad nonce, missing fields) |
| 409 | Replay detected (duplicate nonce) |
| 429 | Rate limited |

---

#### GET /api/v1/transactions/{id}

Get transaction by ID.

**Response:**

```json
{
  "id": "tx-001",
  "hash": "0xabc123...",
  "data": "hello world",
  "nonce": 1,
  "status": "confirmed",
  "block_number": 42,
  "confirmations": 3,
  "timestamp": "2026-04-21T10:00:00Z"
}
```

---

### Blocks

#### GET /api/v1/blocks/latest

Get the latest block.

**Response:**

```json
{
  "height": 42,
  "hash": "0xdef456...",
  "parent_hash": "0xabc123...",
  "proposer": "12D3KooW...",
  "timestamp": "2026-04-21T10:00:00Z",
  "transactions": [
    {
      "id": "tx-001",
      "data": "hello world",
      "nonce": 1
    }
  ],
  "quorum_votes": 5,
  "total_validators": 7
}
```

---

#### GET /api/v1/blocks/{height}

Get block by height.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `height` | integer | Block height |

**Response:**

```json
{
  "height": 42,
  "hash": "0xdef456...",
  "parent_hash": "0xabc123...",
  "proposer": "12D3KooW...",
  "timestamp": "2026-04-21T10:00:00Z",
  "transactions": [...],
  "quorum_votes": 5,
  "total_validators": 7
}
```

**Error Response (404):**

```json
{
  "error": "Block not found",
  "height": 999
}
```

---

### Security

#### GET /api/v1/security/blacklist

Get current IP blacklist.

**Response:**

```json
{
  "blacklist": [
    {
      "ip": "192.168.1.200",
      "reason": "malicious_activity",
      "added_at": "2026-04-21T08:00:00Z",
      "duration_hours": 24
    }
  ],
  "total": 1
}
```

---

#### PUT /api/v1/security/blacklist

Add or remove IPs from blacklist.

**Request:**

```json
{
  "action": "add",
  "ip": "192.168.1.200",
  "reason": "suspicious_activity",
  "duration_hours": 12
}
```

**Response:**

```json
{
  "success": true,
  "ip": "192.168.1.200",
  "action": "added"
}
```

---

#### GET /api/v1/security/whitelist

Get current IP whitelist.

**Response:**

```json
{
  "whitelist": [
    {
      "peer_id": "12D3KooW...",
      "ip": "192.168.1.100",
      "added_at": "2026-04-20T10:00:00Z"
    }
  ],
  "total": 1
}
```

---

### Health & Metrics

#### GET /health

Health check endpoint.

**Response (200 OK):**

```json
{
  "status": "healthy",
  "uptime_seconds": 3600,
  "version": "1.0.0",
  "checks": {
    "service": "ok",
    "database": "ok",
    "network": "ok",
    "consensus": "ok"
  }
}
```

**Response (503 Unhealthy):**

```json
{
  "status": "unhealthy",
  "uptime_seconds": 3600,
  "checks": {
    "service": "ok",
    "database": "degraded",
    "network": "ok",
    "consensus": "ok"
  }
}
```

---

#### GET /metrics

Prometheus metrics endpoint.

**Response:** Text/plain format

```
# HELP peers_connected Number of connected peers
# TYPE peers_connected gauge
peers_connected 5

# HELP blocks_proposed Total blocks proposed
# TYPE blocks_proposed counter
blocks_proposed 42

# HELP ebpf_xdp_packets_processed Total packets processed by XDP
# TYPE ebpf_xdp_packets_processed counter
ebpf_xdp_packets_processed 1000000

# HELP ebpf_xdp_packets_dropped Total packets dropped by XDP
# TYPE ebpf_xdp_packets_dropped counter
ebpf_xdp_packets_dropped 500
```

---

### WebSocket

#### GET /ws

WebSocket endpoint for real-time events.

**Connection:**

```javascript
const ws = new WebSocket('ws://localhost:9092/ws');

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  console.log('Event:', message.type, message.data);
};
```

**Event Types:**

| Type | Description | Payload |
|------|-------------|---------|
| `block_proposed` | New block proposed | Block info |
| `block_validated` | Block validated | Block info |
| `transaction` | New transaction | Transaction info |
| `peer_connected` | Peer connected | Peer info |
| `peer_disconnected` | Peer disconnected | Peer info |
| `security_alert` | Security event | Alert info |

**Example Payload:**

```json
{
  "type": "block_proposed",
  "timestamp": "2026-04-21T10:00:00Z",
  "data": {
    "height": 43,
    "proposer": "12D3KooW...",
    "transactions": 5
  }
}
```

---

## Error Codes

| HTTP Code | Description | Example Response |
|-----------|-------------|------------------|
| 200 | OK | Success |
| 201 | Created | Transaction submitted |
| 400 | Bad Request | Invalid input |
| 401 | Unauthorized | Invalid API key |
| 403 | Forbidden | Insufficient permissions |
| 404 | Not Found | Resource not found |
| 409 | Conflict | Duplicate transaction |
| 429 | Too Many Requests | Rate limited |
| 500 | Internal Server Error | Unexpected error |

**Error Response Format:**

```json
{
  "error": "Bad Request",
  "message": "Invalid nonce: must be greater than 5",
  "code": "INVALID_NONCE",
  "timestamp": "2026-04-21T10:00:00Z"
}
```

---

## Rate Limiting

| Endpoint | Rate Limit | Window |
|----------|-----------|--------|
| `/api/v1/transactions` | 100 req/min | Per IP |
| `/api/v1/network/peers` | 60 req/min | Per IP |
| `/api/v1/security/*` | 30 req/min | Per IP |
| Other endpoints | 120 req/min | Per IP |

Rate limit headers are included in responses:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1713698460
```

---

## OpenAPI Specification

For machine-readable API documentation, see [docs/openapi.yml](docs/openapi.yml).

---

## Examples

### cURL Examples

```bash
# Get node info
curl http://localhost:9091/api/v1/node/info

# Submit transaction
curl -X POST http://localhost:9091/api/v1/transactions \
  -H "Content-Type: application/json" \
  -d '{"id": "tx-002", "data": "transfer 100 tokens", "nonce": 2}'

# Get latest block
curl http://localhost:9091/api/v1/blocks/latest

# View metrics
curl http://localhost:9090/metrics

# Health check
curl http://localhost:9091/health
```

### Python Example

```python
import requests

BASE_URL = "http://localhost:9091/api/v1"

# Get node info
response = requests.get(f"{BASE_URL}/node/info")
node_info = response.json()
print(f"Node ID: {node_info['node_id']}")

# Submit transaction
tx = {
    "id": "tx-003",
    "data": "hello blockchain",
    "nonce": 3
}
response = requests.post(f"{BASE_URL}/transactions", json=tx)
print(f"Transaction status: {response.json()['status']}")
```

### JavaScript Example

```javascript
// Get node info
fetch('http://localhost:9091/api/v1/node/info')
  .then(r => r.json())
  .then(data => console.log('Node:', data.node_id));

// Submit transaction
fetch('http://localhost:9091/api/v1/transactions', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    id: 'tx-004',
    data: 'hello world',
    nonce: 4
  })
})
  .then(r => r.json())
  .then(data => console.log('Status:', data.status));
```
