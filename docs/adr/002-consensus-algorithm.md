# ADR-002: Consensus Algorithm Choice

**Status:** Accepted  
**Date:** 2026-01-16  
**Authors:** eBPF Blockchain Team

## Context

The consensus mechanism is the core of any blockchain system. For eBPF Blockchain, we need an algorithm that:

1. Works in a **trusted environment** (LXD containers, known validators)
2. Provides **finality** without excessive latency
3. Supports **security against Sybil attacks**
4. Integrates with **eBPF-based identity verification**
5. Scales efficiently with validator count

### Options Considered

| Algorithm | Pros | Cons | Suitability |
|-----------|------|------|-------------|
| **Proof of Stake (PoS)** | Energy efficient, fast finality | Requires stake management | ✅ High |
| **Proof of Work (PoW)** | No trust required, battle-tested | Energy intensive, slow | ❌ Low |
| **PBFT** | Fast finality, deterministic | Poor scalability (>100 nodes) | ⚠️ Medium |
| **HotStuff** | Linear communication, fast | Complex implementation | ⚠️ Medium |
| **Raft** | Simple, fast | Not Byzantine fault tolerant | ❌ Low |

## Decision

We chose **Proof of Stake (PoS) with 2/3 quorum** for the following reasons:

1. **Energy efficiency** - No wasteful mining competition
2. **Fast finality** - 2/3 quorum achievable with few validators
3. **Sybil resistance** - Stake requirement prevents Sybil attacks
4. **eBPF integration** - Stake and reputation managed via eBPF-verified identity
5. **Simplicity** - Easier to implement and audit than PBFT or HotStuff

### Algorithm Overview

```
1. Validators are selected based on stake weight
2. Proposer creates a block proposal
3. Validators vote via Gossipsub broadcast
4. Block finalized when 2/3 quorum reached
5. Finality is probabilistic (N confirmations = high confidence)
```

## Consequences

### Positive

- **Performance**: Fast block finality (~1-3 seconds)
- **Security**: 2/3 quorum tolerates up to 1/3 malicious validators
- **Scalability**: Works well with 10-100 validators
- **Energy**: Minimal energy consumption
- **Identity**: Stake managed via eBPF-verified peer identity

### Negative

- **Stake distribution**: Requires initial stake distribution mechanism
- **Nothing at stake**: Validators may want to vote on conflicting blocks (mitigated by slashing)
- **Long-range attacks**: Old state can be rewritten (mitigated by checkpointing)

### Mitigations

- Implement **slashing** for double-voting
- Use **checkpoint finality** every N blocks
- Combine with **eBPF Sybil protection** for identity verification
- Track **reputation scores** via eBPF monitoring

## References

- [Casper FFG (Ethereum PoS)](https://ethereum.org/en/developers/docs/consensus-algorithms/pos/)
- [PBFT Paper (Castro & Liskov)](https://pmc.ncbi.nlm.nih.gov/articles/PMC3980137/)
- [HotStuff Paper](https://arxiv.org/abs/1803.05069)
