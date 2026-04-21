# ADR-005: P2P Networking with libp2p

**Status:** Accepted  
**Date:** 2026-01-19  
**Authors:** eBPF Blockchain Team

## Context

The blockchain node requires a peer-to-peer networking layer that:

1. Supports **decentralized communication** without central servers
2. Provides **message propagation** for blocks and transactions
3. Handles **peer discovery** automatically
4. Ensures **secure communication** between peers
5. Scales to **dozens of validators**

### Options Considered

| Protocol | Pros | Cons | Suitability |
|----------|------|------|-------------|
| **libp2p** | Modular, mature, Rust support | Complexity | ✅ High |
| **Custom TCP** | Full control | Reinventing the wheel | ❌ Low |
| **QUIC only** | Low latency, encrypted | No existing ecosystem | ⚠️ Medium |
| **IPFS** | Content-addressed, P2P | Focused on file storage | ⚠️ Medium |

## Decision

We chose **libp2p** with the following stack:

1. **Transport**: QUIC (encrypted, low-latency) with TCP fallback
2. **Multiplexing**: mplex for stream multiplexing
3. **Discovery**: mDNS for local, Kademlia DHT for global
4. **Routing**: Kademlia for peer routing
5. **Gossipsub**: 1.1 for message propagation

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      libp2p Swarm                               │
├──────────────┬──────────────┬──────────────┬───────────────────┤
│  Transport   │  Multiplex   │   Routing    │    Protocol       │
│              │              │              │                   │
│  QUIC/TCP    │  mplex       │  Kademlia    │  Gossipsub 1.1    │
│  + TLS       │              │  (DHT)       │                   │
└──────────────┴──────────────┴──────────────┴───────────────────┘
```

## Consequences

### Positive

- **Maturity**: libp2p used by Ethereum, Filecoin, IPFS
- **Modularity**: Easy to swap components
- **Security**: Built-in encryption via TLS/QUIC
- **Ecosystem**: Large community, many implementations
- **Rust support**: `libp2p` crate provides full Rust bindings

### Negative

- **Complexity**: libp2p has many components and concepts
- **Resource usage**: Swarm maintains many connections
- **Startup time**: DHT convergence takes time
- **Debugging**: Distributed debugging is challenging

### Mitigations

- Use **Gossipsub 1.1** for efficient message propagation
- Configure **mDNS** for fast local peer discovery
- Implement **peer scoring** to filter bad peers
- Use **structured logging** for debugging

## Configuration

```toml
[network]
p2p_port = 9000
quic_port = 9001
max_connections = 100
mdns_enabled = true

[gossipsub]
mesh_size = 12
random_mesh_size = 4
fanout_ttl = 60s
```

## References

- [libp2p Documentation](https://docs.libp2p.io/)
- [Gossipsub 1.1 Specification](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md)
- [libp2p Rust Crate](https://crates.io/crates/libp2p)
