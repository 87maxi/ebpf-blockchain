# ADR-004: Storage Choice - RocksDB

**Status:** Accepted  
**Date:** 2026-01-18  
**Authors:** eBPF Blockchain Team

## Context

The blockchain node needs persistent storage for:

1. **Blocks** - Complete history of the blockchain
2. **Transactions** - Transaction data and state
3. **State** - Current world state, stake balances
4. **Security** - Nonce tracking, blacklist/whitelist
5. **Performance** - Fast reads and writes for consensus

### Options Considered

| Database | Pros | Cons | Suitability |
|----------|------|------|-------------|
| **RocksDB** | Embedded, fast, LSM-tree | Write amplification at high WAL | ✅ High |
| **SQLite** | Simple, ACID | Not ideal for high write throughput | ⚠️ Medium |
| **LevelDB** | Simple RocksDB variant | No built-in compression | ⚠️ Medium |
| **BadgerDB** | Simple API | Less mature, smaller ecosystem | ⚠️ Medium |
| **PostgreSQL** | Rich features, ACID | Not embedded, overhead | ❌ Low |

## Decision

We chose **RocksDB** as the embedded database for:

1. **Performance** - LSM-tree architecture optimized for write-heavy workloads
2. **Embedded** - No external service dependency, low latency
3. **Compression** - Built-in compression reduces disk usage
4. **Ecosystem** - Used by Prometheus, Kafka, Hive
5. **Rust bindings** - rocksdb-rs provides safe Rust interface

### Data Model

```
blocks/{hash}       → Block data
blocks/head         → Current head block hash
transactions/{id}   → Transaction data
state/{key}         → World state
stake/{peer_id}     → Validator stake
nonce/{sender}      → Nonce tracking
blacklist/{ip}      → Blacklisted IPs
```

## Consequences

### Positive

- **Write performance**: LSM-tree handles high write throughput efficiently
- **Embedded**: No external service, minimal latency
- **Compression**: Built-in compression (Snappy, Zstd)
- **Reliability**: Write-ahead log ensures durability
- **Ecosystem**: Battle-tested in production at scale

### Negative

- **Write amplification**: Heavy write workloads cause compaction overhead
- **Memory usage**: Memtable and block cache consume RAM
- **Tuning required**: Parameters need tuning for specific workloads
- **Recovery time**: Large databases take time to recover

### Mitigations

- Configure **appropriate cache sizes** based on available RAM
- Use **batch writes** for consensus operations
- Enable **compression** to reduce disk I/O
- Implement **periodic compaction** during low-activity periods

## Configuration

```toml
[storage]
path = "/var/lib/ebpf-blockchain/data"
cache_size_mb = 1024
max_open_files = 100
compression = "snappy"
write_buffer_size_mb = 64
max_write_buffer_number = 4
```

## References

- [RocksDB Documentation](https://rocksdb.org/)
- [RocksDB Tuning Guide](https://github.com/facebook/rocksdb/wiki/RocksDB-Tuning-Guide)
- [rocksdb-rs Documentation](https://docs.rs/rocksdb/)
