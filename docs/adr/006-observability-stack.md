# ADR-006: Observability Stack Selection

**Status:** Accepted  
**Date:** 2026-01-20  
**Authors:** eBPF Blockchain Team

## Context

The blockchain node requires comprehensive observability:

1. **Metrics** - Real-time performance and health monitoring
2. **Logging** - Structured logs for debugging and auditing
3. **Tracing** - Distributed traces for request flow analysis
4. **Alerting** - Automated alerts for critical events
5. **Visualization** - Dashboards for operational visibility

### Options Considered

| Component | Metrics | Logs | Traces | Dashboards |
|-----------|---------|------|--------|------------|
| **Prometheus** | ⭐⭐⭐⭐⭐ | ❌ | ❌ | ⭐⭐ |
| **Grafana** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Loki** | ❌ | ⭐⭐⭐⭐⭐ | ❌ | ⭐⭐⭐⭐ |
| **Tempo** | ❌ | ❌ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| **ELK Stack** | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ |
| **Datadog** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

## Decision

We chose the **Grafana Stack** (Prometheus + Loki + Tempo + Grafana):

1. **Integration** - All components work seamlessly together
2. **Open source** - No vendor lock-in, free to use
3. **Scalability** - Can scale from single-node to distributed
4. **Visualization** - Grafana provides excellent dashboards
5. **Ecosystem** - Large community, many plugins and integrations

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Grafana (:3000)                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                     │
│  │  Metrics │  │   Logs   │  │  Traces  │                     │
│  │Dashboard │  │ Dashboard│  │ Dashboard│                     │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                     │
└───────┼─────────────┼─────────────┼─────────────────────────────┘
        │             │             │
        ▼             ▼             ▼
┌──────────┐  ┌──────────┐  ┌──────────┐
│Prometheus│  │   Loki   │  │  Tempo   │
│ (:9090)  │  │  (:3100) │  │  (:3200) │
└──────────┘  └──────────┘  └──────────┘
```

## Consequences

### Positive

- **Integration**: Components designed to work together
- **Cost**: All open source, no licensing fees
- **Flexibility**: Each component can be scaled independently
- **Visualization**: Grafana is the industry standard for dashboards
- **Alerting**: Prometheus Alertmanager provides robust alerting

### Negative

- **Resource usage**: Multiple services consume significant resources
- **Complexity**: Multiple components to configure and maintain
- **Storage**: Logs and traces can consume significant disk space
- **Learning curve**: Each component has its own query language

### Mitigations

- Use **Docker Compose** for simplified deployment
- Configure **retention policies** to manage storage
- Use **Promtail** for efficient log collection
- Provide **pre-built dashboards** for common use cases

## Configuration

```yaml
# Prometheus
scrape_interval: 15s
evaluation_interval: 15s
scrape_configs:
  - job_name: 'ebpf-node'
    static_configs:
      - targets: ['localhost:9090']

# Loki
schema_config:
  configs:
    - from: 2020-10-24
      store: boltdb-shipper
      object_store: filesystem
      schema: v11
      index:
        prefix: index_
        period: 24h

# Tempo
storage:
  backend: filesystem
  wal:
    path: /tmp/tempo/wal
  filesystem:
    path: /tmp/tempo/blocks
```

## References

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Loki Documentation](https://grafana.com/docs/loki/latest/)
- [Tempo Documentation](https://grafana.com/docs/tempo/latest/)
- [Grafana Documentation](https://grafana.com/docs/grafana/latest/)
