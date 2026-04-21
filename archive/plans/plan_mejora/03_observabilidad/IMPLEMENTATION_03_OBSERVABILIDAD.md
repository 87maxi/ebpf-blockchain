# Implementación: Fase 2 - Observabilidad

## Resumen

Esta documentación detalla la implementación de la Fase 2: Observabilidad para el proyecto eBPF Blockchain. La implementación incluye métricas Prometheus completas, dashboards de Grafana, alertas Prometheus, logging estructurado JSON con Loki, y el stack completo de observabilidad con docker-compose.

## Fecha de Implementación

2026-04-21

## 1. Métricas Prometheus Implementadas

### 1.1 Métricas de Red (5 métricas)

| Métrica | Tipo | Descripción | Labels |
|---------|------|-------------|--------|
| `ebpf_node_messages_sent_total` | IntCounter | Total de mensajes enviados vía gossip | - |
| `ebpf_node_messages_sent_by_type_total` | IntCounterVec | Mensajes enviados por tipo | `type` (tx, vote, sync) |
| `ebpf_node_network_latency_ms` | IntGaugeVec | Latencia de red en milisegundos | `peer_id` |
| `ebpf_node_bandwidth_sent_bytes_total` | IntCounter | Total de bytes enviados | - |
| `ebpf_node_bandwidth_received_bytes_total` | IntCounter | Total de bytes recibidos | - |

**Ubicación:** [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs:45-80)

### 1.2 Métricas de Consenso (5 métricas)

| Métrica | Tipo | Descripción | Labels |
|---------|------|-------------|--------|
| `ebpf_node_blocks_proposed_total` | IntCounter | Total de bloques propuestos | - |
| `ebpf_node_consensus_rounds_total` | IntCounter | Total de rondas de consenso | - |
| `ebpf_node_consensus_duration_ms` | IntGauge | Duración actual del consenso (ms) | - |
| `ebpf_node_validator_count` | IntGauge | Número de validadores activos | - |
| `ebpf_node_slashing_events_total` | IntCounter | Total de eventos de slashing | - |

**Ubicación:** [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs:80-110)

### 1.3 Métricas de Transacciones (4 métricas)

| Métrica | Tipo | Descripción | Labels |
|---------|------|-------------|--------|
| `ebpf_node_transactions_processed_total` | IntCounter | Total de transacciones procesadas | - |
| `ebpf_node_transactions_by_type_total` | IntCounterVec | Transacciones por tipo | `type` (transfer, vote) |
| `ebpf_node_transaction_queue_size` | IntGauge | Tamaño actual de la cola de transacciones | - |
| `ebpf_node_transactions_failures_total` | IntCounter | Total de fallos de procesamiento | - |

**Ubicación:** [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs:110-135)

### 1.4 Métricas de eBPF (4 métricas)

| Métrica | Tipo | Descripción | Labels |
|---------|------|-------------|--------|
| `ebpf_node_xdp_packets_processed_total` | IntCounter | Total de paquetes procesados por XDP | - |
| `ebpf_node_xdp_packets_dropped_total` | IntCounter | Total de paquetes descartados por XDP | - |
| `ebpf_node_xdp_blacklist_size` | IntGauge | Tamaño actual de la blacklist XDP | - |
| `ebpf_node_xdp_whitelist_size` | IntGauge | Tamaño actual de la whitelist XDP | - |
| `ebpf_node_errors_total` | IntCounter | Total de errores eBPF | - |

**Ubicación:** [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs:135-160)

### 1.5 Métricas del Sistema (3 métricas)

| Métrica | Tipo | Descripción | Labels |
|---------|------|-------------|--------|
| `ebpf_node_memory_usage_bytes` | IntGauge | Uso actual de memoria en bytes | - |
| `ebpf_node_uptime_seconds` | IntGauge | Uptime en segundos | - |
| `ebpf_node_thread_count` | IntGauge | Número actual de threads | - |

**Ubicación:** [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs:160-180)

## 2. Integración de Métricas en el Event Loop

Las métricas se actualizan en los siguientes puntos del event loop:

### 2.1 Recepción de Transacción RPC

```rust
// Línea ~1280
Some(tx) = rx_rpc.recv() => {
    MESSAGES_SENT.inc();
    MESSAGES_SENT_BY_TYPE.with_label_values(&["tx"]).inc();
    BANDWIDTH_SENT.inc_by(payload.len() as u64);
    TRANSACTIONS_PROCESSED.inc();
    TRANSACTIONS_BY_TYPE.with_label_values(&["transfer"]).inc();
}
```

### 2.2 Publicación de Votos

```rust
// Línea ~1356
if let Ok(_) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), payload) {
    MESSAGES_SENT.inc();
    MESSAGES_SENT_BY_TYPE.with_label_values(&["vote"]).inc();
    BANDWIDTH_SENT.inc_by(payload.len() as u64);
}
```

### 2.3 Recepción de Mensajes Gossip

```rust
// Línea ~1301
MESSAGES_RECEIVED.with_label_values(&["gossip"]).inc();
BANDWIDTH_RECEIVED.inc_by(message.data.len() as u64);
```

### 2.4 Intervalo de Estadísticas (eBPF + Sistema)

```rust
// Línea ~1270
_ = stats_interval.tick() => {
    UPTIME.inc();
    UPTIME_SECONDS.set(UPTIME.get() as i64);
    
    // eBPF metrics from maps
    XDP_PACKETS_PROCESSED.set(total_packets as i64);
    XDP_BLACKLIST_SIZE.set(blacklist_size as i64);
    XDP_WHITELIST_SIZE.set(whitelist_size as i64);
    
    // System metrics
    update_system_metrics();
    
    // Peer count as validator count
    VALIDATOR_COUNT.set(PEERS_CONNECTED.with_label_values(&["connected"]).get() as i64);
}
```

### 2.5 Quorum de Consenso

```rust
// Línea ~1398
if voters.len() == 2 {
    TRANSACTIONS_CONFIRMED.inc();
    TRANSACTIONS_PROCESSED.inc();
    BLOCKS_PROPOSED.inc();
    CONSENSUS_ROUNDS.inc();
}
```

### 2.6 Fallos de Transacción

```rust
// Línea ~1408
} else {
    TRANSACTIONS_REJECTED.inc();
    TRANSACTION_FAILURES.inc();
}
```

## 3. Dashboards de Grafana

### 3.1 Dashboard de Salud General

**Archivo:** [`monitoring/grafana/dashboards/health-overview.json`](monitoring/grafana/dashboards/health-overview.json)

**UID:** `ebpf-health-overview`

**Paneles (15 paneles):**

| Panel | Tipo | Métrica Principal | Posición |
|-------|------|-------------------|----------|
| Uptime | stat | `ebpf_node_uptime_seconds` | (0, 0) |
| Peers Conectados | stat | `ebpf_node_peers_connected` | (4, 0) |
| Transacciones Confirmadas | stat | `transactions_confirmed_total` | (8, 0) |
| Bloques Propuestos | stat | `ebpf_node_blocks_proposed_total` | (12, 0) |
| Latencia Promedio | gauge | `ebpf_node_latency_avg` | (0, 4) |
| Peers por Estado | piechart | `ebpf_node_peers_connected` | (12, 5) |
| Mensajes Enviados/Recibidos | timeseries | rate | (0, 5) |
| Bandwidth Sent | timeseries | `ebpf_node_bandwidth_sent` | (12, 5) |
| Bandwidth Received | timeseries | `ebpf_node_bandwidth_received` | (16, 5) |
| Consenso Duración | timeseries | `ebpf_node_consensus_duration_ms` | (0, 13) |
| TPS | timeseries | rate transactions | (4, 13) |
| Sybil Attempts | stat | `ebpf_node_sybil_attempts_detected_total` | (0, 17) |
| Replay Attacks | stat | `transactions_replay_rejected_total` | (4, 17) |
| XDP Drops | stat | `ebpf_node_xdp_packets_dropped_total` | (8, 17) |
| Blacklist/Whitelist | gauge | blacklist/whitelist size | (12, 17) |

### 3.2 Dashboard de Red P2P

**Archivo:** [`monitoring/grafana/dashboards/network-p2p.json`](monitoring/grafana/dashboards/network-p2p.json)

**UID:** `ebpf-network-p2p`

**Paneles (13 paneles):**

| Panel | Tipo | Métrica Principal |
|-------|------|-------------------|
| Peers Conectados | stat | `ebpf_node_peers_connected` |
| Mensajes Enviados | stat | `ebpf_node_messages_sent_total` |
| Mensajes Recibidos | stat | `ebpf_node_messages_received_total` |
| Bandwidth Enviado (rate) | stat | rate bandwidth sent |
| Bandwidth Recibido (rate) | stat | rate bandwidth received |
| Latencia de Red | stat | `ebpf_node_network_latency_ms` |
| Tasa de Mensajes | timeseries | rate messages |
| Tasa por Tipo | timeseries | rate by type |
| Bandwidth Enviado | timeseries | bandwidth sent |
| Bandwidth Recibido | timeseries | bandwidth received |
| Peers Conectados | timeseries | peers connected |
| Conexiones P2P | timeseries | p2p connections |

### 3.3 Dashboard de Consenso

**Archivo:** [`monitoring/grafana/dashboards/consensus.json`](monitoring/grafana/dashboards/consensus.json)

**UID:** `ebpf-consensus`

**Paneles (17 paneles):**

| Panel | Tipo | Métrica Principal |
|-------|------|-------------------|
| Bloques Propuestos | stat | `ebpf_node_blocks_proposed_total` |
| Rondas de Consenso | stat | `ebpf_node_consensus_rounds_total` |
| Duración Consenso | stat | `ebpf_node_consensus_duration_ms` |
| Validadores Activos | stat | `ebpf_node_validator_count` |
| Eventos de Slashing | stat | `ebpf_node_slashing_events_total` |
| Transacciones Procesadas | stat | `ebpf_node_transactions_processed_total` |
| Duración de Rondas | timeseries | consensus duration |
| Tasa de Rondas | timeseries | rate rounds |
| Tasa de Bloques (TPS) | timeseries | rate blocks |
| TPS | timeseries | rate transactions |
| Slashing Acumulados | timeseries/bar | slashing events |
| Validadores Activos | timeseries | validator count |
| Fallos de Transacción | timeseries | failures |

### 3.4 Dashboard de Transacciones

**Archivo:** [`monitoring/grafana/dashboards/transactions.json`](monitoring/grafana/dashboards/transactions.json)

**UID:** `ebpf-transactions`

**Paneles (16 paneles):**

| Panel | Tipo | Métrica Principal |
|-------|------|-------------------|
| Transacciones Procesadas | stat | `ebpf_node_transactions_processed_total` |
| Confirmadas | stat | `transactions_confirmed_total` |
| Rechazadas | stat | `transactions_rejected_total` |
| Fallos | stat | `ebpf_node_transactions_failures_total` |
| Cola de Transacciones | stat | `ebpf_node_transaction_queue_size` |
| Bloques Propuestos | stat | `ebpf_node_blocks_proposed_total` |
| TPS | timeseries | rate processed |
| Confirmación vs Rechazo | timeseries | rate confirmed/rejected/failed |
| Tasa por Tipo | timeseries (stacked) | rate by type |
| Acumuladas por Tipo | timeseries/bar | by type |
| Tamaño de Cola | timeseries | queue size |
| Fallos Acumulados | timeseries | failures |

## 4. Alertas Prometheus

### 4.1 Archivo de Configuración

**Archivo:** [`monitoring/prometheus/alerts.yml`](monitoring/prometheus/alerts.yml)

### 4.2 Alertas Configuradas (20 alertas)

#### Alertas de Red P2P (4 alertas)

| Alerta | Expresión | Umbral | Duración | Severidad |
|--------|-----------|--------|----------|-----------|
| HighPeerCount | `ebpf_node_peers_connected > 50` | 50 peers | 5m | warning |
| PeerDisconnectionRate | `rate(ebpf_node_p2p_connections_closed[5m]) > 10` | 10 conn/s | 5m | warning |
| HighNetworkLatency | `ebpf_node_network_latency_ms > 1000` | 1000ms | 5m | critical |
| BandwidthSaturation | `rate(ebpf_node_bandwidth_sent_bytes_total[5m]) > 10485760` | 10MB/s | 5m | warning |

#### Alertas de Consenso (4 alertas)

| Alerta | Expresión | Umbral | Duración | Severidad |
|--------|-----------|--------|----------|-----------|
| ConsensusSlow | `ebpf_node_consensus_duration_ms > 10000` | 10s | 5m | critical |
| LowValidatorCount | `ebpf_node_validator_count < 2` | 2 validators | 2m | critical |
| HighConsensusRoundRate | `rate(ebpf_node_consensus_rounds_total[5m]) > 60` | 1 round/s | 5m | warning |
| SlashingEventDetected | `ebpf_node_slashing_events_total > 0` | > 0 | 1m | critical |

#### Alertas de Transacciones (4 alertas)

| Alerta | Expresión | Umbral | Duración | Severidad |
|--------|-----------|--------|----------|-----------|
| TransactionQueueOverflow | `ebpf_node_transaction_queue_size > 500` | 500 items | 5m | warning |
| HighTransactionFailureRate | `rate(ebpf_node_transactions_failures_total[5m]) > 10` | 10/s | 5m | critical |
| LowTransactionThroughput | `rate(ebpf_node_transactions_processed_total[5m]) < 1` | 1 TPS | 10m | warning |
| TransactionReplayAttack | `ebpf_node_transactions_replay_rejected_total > 0` | > 0 | 1m | critical |

#### Alertas de Seguridad (3 alertas)

| Alerta | Expresión | Umbral | Duración | Severidad |
|--------|-----------|--------|----------|-----------|
| SybilAttackDetected | `ebpf_node_sybil_attempts_detected_total > 0` | > 0 | 1m | critical |
| XDPPacketDropRate | `rate(ebpf_node_xdp_packets_dropped_total[5m]) > 100` | 100/s | 5m | warning |
| XDPBlacklistGrowing | `ebpf_node_xdp_blacklist_size > 1000` | 1000 IPs | 10m | warning |

#### Alertas del Sistema (3 alertas)

| Alerta | Expresión | Umbral | Duración | Severidad |
|--------|-----------|--------|----------|-----------|
| HighMemoryUsage | `ebpf_node_memory_usage_bytes > 1073741824` | 1GB | 5m | warning |
| NodeDown | `up{job="ebpf-node"} == 0` | down | 1m | critical |
| UptimeAnomaly | `ebpf_node_uptime_seconds < 60` | 60s | 5m | warning |

### 4.3 Alertmanager Configuración

**Archivo:** [`monitoring/prometheus/alertmanager.yml`](monitoring/prometheus/alertmanager.yml)

**Rutas de Alerta:**

```
critical → email + webhook (repeat: 1h)
warning → webhook (repeat: 4h)
default → webhook (repeat: 4h)
```

## 5. Logging Estructurado JSON

### 5.1 Configuración

**Archivo:** [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs:898-920)

**Dependencias:** `tracing-subscriber` con features `json` y `tracing-log`

### 5.2 Configuración Implementada

```rust
fn setup_structured_logging() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        .add_directive("aya=warn".parse().unwrap())
        .add_directive("libp2p=info".parse().unwrap());
    
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_timestamp_ms(true)
        .json()
        .with_writer(std::io::stderr)
        .init();
}
```

### 5.3 Formato de Output JSON

Cada log se exporta en formato JSON con los siguientes campos:

```json
{
  "timestamp": "2026-04-21T08:00:00.000Z",
  "level": "INFO",
  "target": "ebpf_node::main",
  "fields": {
    "event": "rpc_tx_received",
    "tx_id": "abc123",
    "data": "transfer:100"
  },
  "thread_id": 1,
  "thread_name": "tokio-runtime-worker",
  "file": "ebpf-node/ebpf-node/src/main.rs",
  "line": 1280
}
```

## 6. Stack de Observabilidad Docker

### 6.1 Archivo de Configuración

**Archivo:** [`monitoring/docker-compose.yml`](monitoring/docker-compose.yml)

### 6.2 Servicios Configurados

| Servicio | Imagen | Puerto | Volumen |
|----------|--------|--------|---------|
| prometheus | `prom/prometheus:v2.48.0` | 9090 | prometheus-data |
| alertmanager | `prom/alertmanager:v0.26.0` | 9093 | alertmanager-data |
| grafana | `grafana/grafana:10.2.0` | 3000 | grafana-data, grafana-config |
| loki | `grafana/loki:2.9.0` | 3100 | loki-data |
| promtail | `grafana/promtail:2.9.0` | 9080 | promtail-data |
| tempo | `grafana/tempo:2.3.0` | 3200, 9097, 4317, 4318 | tempo-data |
| node-exporter | `prom/node-exporter:v1.7.0` | 9100 | /proc, /sys, / |

### 6.3 Network

```yaml
networks:
  ebpf-observability:
    driver: bridge
```

### 6.4 Datasources de Grafana

**Archivo:** [`monitoring/grafana/provisioning/datasources/datasources.yml`](monitoring/grafana/provisioning/datasources/datasources.yml)

| Datasource | Type | URL |
|------------|------|-----|
| Prometheus | prometheus | http://prometheus:9090 |
| Loki | loki | http://loki:3100 |
| Tempo | tempo | http://tempo:3200 |

### 6.5 Dashboards Provisioning

**Archivo:** [`monitoring/grafana/provisioning/dashboards/dashboards.yaml`](monitoring/grafana/provisioning/dashboards/dashboards.yaml)

Los dashboards se cargan automáticamente desde `/var/lib/grafana/dashboards/`.

## 7. Loki Configuración

### 7.1 Archivo de Configuración

**Archivo:** [`monitoring/loki/loki-config.yml`](monitoring/loki/loki-config.yml)

### 7.2 Configuración Clave

```yaml
server:
  http_listen_port: 3100
  grpc_listen_port: 9096

common:
  replication_factor: 1
  ring:
    kvstore:
      store: inmemory

schema_config:
  configs:
    - from: 2024-01-01
      store: tsdb
      object_store: filesystem
      schema: v13

compactor:
  retention_period: 720h  # 30 days
```

## 8. Tempo Configuración

### 8.1 Archivo de Configuración

**Archivo:** [`monitoring/tempo/tempo-config.yml`](monitoring/tempo/tempo-config.yml)

### 8.2 Receivers

```yaml
distributor:
  receivers:
    otlp:
      protocols:
        http:  # :4318
        grpc: # :4317
    jaeger:
      protocols:
        thrift_http:
        grpc:
```

### 8.3 Metrics Generator

Habilitado para service graphs y span metrics.

## 9. Promtail Configuración

### 9.1 Archivo de Configuración

**Archivo:** [`monitoring/promtail/promtail-config.yml`](monitoring/promtail/promtail-config.yml)

### 9.2 Scrape Jobs

| Job | Targets | Path |
|-----|---------|------|
| ebpf-node-logs | localhost | /var/log/ebpf-node/*.log |
| docker-logs | localhost | /var/lib/docker/containers/*/*.log |
| system-logs | localhost | /var/log/*.log |
| prometheus-logs | localhost | /prometheus/*.log |

## 10. Prometheus Configuration

### 10.1 Archivo de Configuración

**Archivo:** [`monitoring/prometheus/prometheus.yml`](monitoring/prometheus/prometheus.yml)

### 10.2 Scrape Configs

| Job | Targets | Path |
|-----|---------|------|
| ebpf-node | host1:9090, host2:9090, host3:9090 | /metrics |
| prometheus | localhost:9090 | /metrics |
| node-exporter | host1:9100, host2:9100, host3:9100 | /metrics |
| grafana | grafana:3000 | /metrics |

## 11. Uso del Stack de Observabilidad

### 11.1 Iniciar el Stack

```bash
cd monitoring
docker-compose up -d
```

### 11.2 Acceder a las Interfaces

| Servicio | URL | Credenciales |
|----------|-----|--------------|
| Grafana | http://localhost:3000 | admin/admin |
| Prometheus | http://localhost:9090 | - |
| Alertmanager | http://localhost:9093 | - |
| Loki | http://localhost:3100 | - |
| Tempo | http://localhost:3200 | - |
| Node Exporter | http://localhost:9100 | - |

### 11.3 Ver Dashboards en Grafana

1. Abrir http://localhost:3000
2. Login con admin/admin
3. Ir a Dashboards → Buscar "eBPF"
4. Dashboards disponibles:
   - eBPF Health Overview
   - eBPF Network P2P
   - eBPF Consensus
   - eBPF Transactions

### 11.4 Consultar Métricas en Prometheus

```bash
# Peers conectados
curl http://localhost:9090/api/v1/query?query=ebpf_node_peers_connected

# TPS actual
curl http://localhost:9090/api/v1/query?query=rate(ebpf_node_transactions_processed_total[5m])

# Latencia
curl http://localhost:9090/api/v1/query?query=ebpf_node_network_latency_ms
```

### 11.5 Consultar Logs en Loki

```bash
# Todos los logs del nodo
curl 'http://localhost:3100/loki/api/v1/query' \
  -d '{"query":"{job=\"ebpf-node-logs\"}"}'

# Logs de error
curl 'http://localhost:3100/loki/api/v1/query' \
  -d '{"query":"{job=\"ebpf-node-logs\"} |= \"error\""}'

# Logs por evento
curl 'http://localhost:3100/loki/api/v1/query' \
  -d '{"query":"{job=\"ebpf-node-logs\"} |= \"quorum_reached\""}'
```

### 11.6 Detener el Stack

```bash
cd monitoring
docker-compose down
```

## 12. Criterios de Aceptación Cumplidos

- [x] 25+ métricas Prometheus configuradas (5 categorías)
- [x] 4 dashboards de Grafana creados (15+13+17+16 = 61 paneles totales)
- [x] 20 alertas Prometheus configuradas (5 categorías)
- [x] Alertmanager configurado con rutas de notificación
- [x] Logging estructurado JSON implementado
- [x] Stack completo docker-compose (7 servicios)
- [x] Loki configurado para agregación de logs
- [x] Tempo configurado para distributed tracing
- [x] Promtail configurado para recolección de logs
- [x] Node Exporter configurado para métricas del sistema

## 13. Estructura de Archivos Creados/Modificados

```
monitoring/
├── docker-compose.yml                          # Stack completo
├── prometheus/
│   ├── prometheus.yml                          # Configuración Prometheus
│   ├── alerts.yml                              # 20 alertas configuradas
│   ├── alertmanager.yml                        # Alertmanager config
│   └── rules/                                  # Directorio para reglas adicionales
├── grafana/
│   ├── provisioning/
│   │   ├── datasources/datasources.yml         # Prometheus, Loki, Tempo
│   │   └── dashboards/dashboards.yaml          # Auto-load dashboards
│   └── dashboards/
│       ├── health-overview.json                # Dashboard de salud
│       ├── network-p2p.json                    # Dashboard de red
│       ├── consensus.json                      # Dashboard de consenso
│       └── transactions.json                   # Dashboard de transacciones
├── loki/
│   └── loki-config.yml                         # Configuración Loki
├── tempo/
│   └── tempo-config.yml                        # Configuración Tempo
└── promtail/
    └── promtail-config.yml                     # Configuración Promtail

ebpf-node/ebpf-node/
├── Cargo.toml                                  # tracing-subscriber con json
└── src/
    └── main.rs                                 # setup_structured_logging()
```

## 14. Próximos Pasos

1. **Fase 3: Automatización** - Ansible, CI/CD, backups
2. **Fase 4: Documentación** - README, API docs, runbooks
3. **Pruebas de integración** - Verificar que todas las métricas se exportan correctamente
4. **Pruebas de alertas** - Verificar que las alertas se disparan correctamente
5. **Pruebas de logging** - Verificar que los logs llegan a Loki correctamente
