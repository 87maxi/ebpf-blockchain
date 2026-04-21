# ADR-003: Use eBPF for Network Security

**Status:** Accepted  
**Date:** 2026-01-17  
**Authors:** eBPF Blockchain Team

## Context

Network security is a core requirement for the eBPF Blockchain project. We need to:

1. **Filter malicious traffic** at the highest performance level
2. **Monitor network activity** for security events
3. **Block attacks** before they reach application layer
4. **Maintain low latency** even under attack
5. **Provide observability** into network security events

### Options Considered

| Approach | Pros | Cons | Performance |
|----------|------|------|-------------|
| **eBPF XDP** | Kernel-level, microsecond latency | Limited program complexity | ⭐⭐⭐⭐⭐ |
| **eBPF TC** | Flexible, good latency | Slightly slower than XDP | ⭐⭐⭐⭐ |
| **Netfilter/iptables** | Mature, wide support | Slower, user-space copies | ⭐⭐ |
| **Application-level** | Most flexible | Highest latency | ⭐ |
| **DPDK** | Maximum performance | Complex, no kernel integration | ⭐⭐⭐⭐⭐ |

## Decision

We chose **eBPF with XDP (eXpress Data Path)** for packet filtering and **KProbes/Tracepoints** for monitoring:

1. **Maximum performance** - XDP runs at driver level, microsecond latency
2. **Kernel integration** - No need for custom packet processing pipeline
3. **Dynamic updates** - Rules can be updated without restarting
4. **Safety** - eBPF verifier guarantees no kernel damage
5. **Observability** - KProbes provide fine-grained monitoring

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  XDP (Driver Level)                                             │
│  - Blacklist/whitelist filtering                                │
│  - DDoS mitigation                                              │
│  - Action: XDP_PASS / XDP_DROP                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  KProbes / Tracepoints                                          │
│  - Latency monitoring                                           │
│  - Security event tracking                                      │
│  - Performance metrics                                          │
└─────────────────────────────────────────────────────────────────┘
```

## Consequences

### Positive

- **Performance**: Microsecond-level packet filtering
- **Safety**: eBPF verifier prevents kernel damage
- **Flexibility**: Programs can be updated dynamically
- **Observability**: Deep visibility into kernel operations
- **Security**: Multiple layers (XDP filter + application validation)

### Negative

- **Complexity**: eBPF programming requires specialized knowledge
- **Kernel version**: Requires kernel ≥ 5.10 with BTF
- **Limited state**: eBPF programs have restricted map access
- **Debugging**: Harder to debug than user-space code

### Mitigations

- Use **Aya framework** for safe eBPF program development in Rust
- Maintain **minimum kernel version** documentation
- Implement **graceful fallback** when eBPF unavailable
- Use **structured logging** for observability

## References

- [XDP Documentation](https://kernel.org/doc/html/latest/networking/xdp.html)
- [Aya Framework](https://aya-rs.dev/)
- [eBPF Flowchart](https://ebpf.io/flowchart/)
