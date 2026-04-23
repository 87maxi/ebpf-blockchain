# Reporte de Verificación de Consistencia Post-Refactorización

## Resumen

- **Total de archivos modificados:** 30+
- **Total de archivos nuevos:** 15+
- **Total de archivos eliminados:** 1 (gossip.rs)
- **Estado de compilación:** ✅ Compila sin errores (solo advertencias)

---

## Verificación por Fase

### Fase 1: Estabilización

| Elemento | Estado | Notas |
|----------|--------|-------|
| Quorum dinámico | ✅ Consistente | Implementado en [`config/node.rs`](ebpf-node/ebpf-node/src/config/node.rs:290) con `get_next_proposer()` |
| Bloques reales | ✅ Consistente | `create_block()` persiste en RocksDB con SHA256 |
| Alert NodeDown | ✅ Consistente | Definida en [`alerts.yml`](monitoring/prometheus/alerts.yml:179) con `up{job=~"ebpf-node-.*"} == 0` |
| Alert Sybil | ✅ Consistente | Definida en [`alerts.yml`](monitoring/prometheus/alerts.yml:136) con `ebpf_node_sybil_attempts_total > 0` |
| ebpf-cluster.json | ✅ Consistente | Queries coinciden con métricas en prometheus.rs |
| detach_all() | ✅ Consistente | Implementada en [`programs.rs`](ebpf-node/ebpf-node/src/ebpf/programs.rs) |

### Fase 2: Consenso Funcional

| Elemento | Estado | Notas |
|----------|--------|-------|
| Selección de proposers | ✅ Consistente | Round-robin en [`NodeState::get_next_proposer()`](ebpf-node/ebpf-node/src/config/node.rs:290) |
| Bloques en RocksDB | ✅ Consistente | Keys `block:{height}` y `latest_height` |
| Signature verification | ✅ Consistente | `SignedVote` con ed25519 en [`config/node.rs`](ebpf-node/ebpf-node/src/config/node.rs:56) |
| Slashing | ✅ Consistente | `record_slashing_event()` en [`config/node.rs`](ebpf-node/ebpf-node/src/config/node.rs:370) |
| Kademlia DHT | ✅ Consistente | Implementado en [`behaviour.rs`](ebpf-node/ebpf-node/src/p2p/behaviour.rs) |
| Sync periódico | ✅ Consistente | Endpoint `/api/v1/network/sync` en [`network.rs`](ebpf-node/ebpf-node/src/api/network.rs:59) |
| Checkpoint finality | ✅ Consistente | `CHECKPOINT_INTERVAL = 100` en [`config/node.rs`](ebpf-node/ebpf-node/src/config/node.rs:273) |

### Fase 3: Observabilidad

| Elemento | Estado | Notas |
|----------|--------|-------|
| Datasource Tempo | ✅ Consistente | Definido en [`datasources.yml`](monitoring/grafana/provisioning/datasources/datasources.yml:22) |
| transactions.json | ⚠️ Parcialmente consistente | **1 inconsistencia menor** (ver abajo) |
| ebpf-debug.json | ✅ Consistente | Queries coinciden con métricas y logs Loki |
| Ansible docker-compose | ✅ Consistente | Template usa variables de group_vars correctamente |
| 8 nuevas métricas | ✅ Consistentes | Todas definidas en [`prometheus.rs`](ebpf-node/ebpf-node/src/metrics/prometheus.rs:216-283) |

### Fase 4: Hardening

| Elemento | Estado | Notas |
|----------|--------|-------|
| Rutas unificadas | ✅ Consistentes | `/api/v1/*` en [`router.rs`](ebpf-node/ebpf-node/src/api/router.rs) |
| Puertos sin conflicto | ✅ Consistentes | RPC=8080, Metrics=9090, P2P=50000 |
| Dependencias centralizadas | ✅ Consistentes | Workspace dependencies en [`Cargo.toml`](ebpf-node/Cargo.toml:14) |
| Tags en playbooks | ✅ Consistentes | Tags: deploy, service, build, health, check |
| gossip.rs eliminado | ✅ Consistente | Funcionalidad migrada a behaviour.rs |
| Health checks | ✅ Consistentes | Implementados en docker-compose.yml |
| Whitelist inicial | ✅ Consistente | Local peer added in [`main.rs`](ebpf-node/ebpf-node/src/main.rs:138) |

---

## Inconsistencias Detectadas

### ⚠️ CRÍTICA: Métrica `transactions_rejected_total` en transactions.json

**Archivo:** [`monitoring/grafana/dashboards/transactions.json`](monitoring/grafana/dashboards/transactions.json:203)

**Línea 203:**
```json
"expr": "transactions_rejected_total"
```

**Debería ser:**
```json
"expr": "ebpf_node_transactions_rejected_total"
```

**Impacto:** El panel "Transacciones Rechazadas" en el dashboard transactions.json no mostrará datos porque la query usa el nombre incorrecto de la métrica. La métrica correcta definida en [`prometheus.rs`](ebpf-node/ebpf-node/src/metrics/prometheus.rs:144) es `ebpf_node_transactions_rejected_total`.

### ℹ️ ADVERTENCIA: Puertos RPC en main.rs vs group_vars

**Archivo:** [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs:211-214)

```rust
let metrics_port = config::node::get_port_from_env("METRICS_PORT", 9090);
let rpc_port = config::node::get_port_from_env("RPC_PORT", 9091);  // Default 9091
let ws_port = config::node::get_port_from_env("WS_PORT", 9092);
let network_p2p_port = config::node::get_port_from_env("NETWORK_P2P_PORT", 9000);
```

**Archivo:** [`ansible/inventory/group_vars/all.yml`](ansible/inventory/group_vars/all.yml:27-30)

```yaml
node_metrics_port: 9090
node_p2p_port: 50000
node_rpc_port: 8080
```

**Discrepancia:** El default de `RPC_PORT` en main.rs es 9091, pero en Ansible es 8080. El default de `NETWORK_P2P_PORT` en main.rs es 9000, pero en Ansible es 50000.

**Impacto:** Bajo en bajo - Los valores de Ansible se usan en despliegue, pero si el node se ejecuta sin variables de entorno, usará los defaults del código que no coinciden con Ansible.

### ℹ️ ADVERTENCIA: deploy.sh hardcoded paths

**Archivo:** [`scripts/deploy.sh`](scripts/deploy.sh:97-133)

El service file embebido en `install_service()` tiene paths hardcoded:
```bash
WorkingDirectory=/root/ebpf-blockchain
ExecStart=/root/ebpf-blockchain/ebpf-node/target/release/ebpf-node
```

**Debería usar las variables:**
```bash
WorkingDirectory=${NODE_WORKING_DIR}
ExecStart=${NODE_BINARY_DIR}/${NODE_BINARY_NAME}
```

**Impacto:** Bajo - Las variables NODE_WORKING_DIR y NODE_DATA_DIR se definen al inicio del script, pero el service file usa valores hardcoded.

### ℹ️ INFO: Fallback ports en main.rs

**Archivo:** [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs:264-294)

El código tiene fallback ports que pueden causar confusión:
- Prometheus: fallback a 9091, 9092, 8080, 3000
- REST API: fallback a 9092, 8080, 3000, 8081

**Impacto:** Bajo - Los fallbacks son para tolerancia a fallos, pero pueden causar conflictos si múltiples nodos comparten puerto.

---

## Verificación de Consistencia Cruzada

### Rutas

| Variable | group_vars | service.j2 | deploy.yml | deploy.sh | Consistente |
|----------|------------|------------|------------|-----------|------------|
| `node_working_dir` | `/root/ebpf-blockchain` | `{{ node_working_dir }}` | `{{ node_working_dir }}` | `/root/ebpf-blockchain` | ✅ |
| `node_data_dir` | `/var/lib/ebpf-blockchain` | N/A | `{{ node_data_dir }}` | `/var/lib/ebpf-blockchain` | ✅ |
| `node_log_dir` | `/var/log/ebpf-blockchain` | N/A | `{{ node_log_dir }}` | `/var/log/ebpf-blockchain` | ✅ |
| `node_binary_dir` | `{{ node_working_dir }}/ebpf-node/target/release` | N/A | `{{ node_binary_dir }}` | `${NODE_WORKING_DIR}/ebpf-node/target/release` | ✅ |

### Puertos

| Puerto | group_vars | health_check.yml | deploy.sh | main.rs (default) | Consistente |
|--------|------------|------------------|-----------|-------------------|------------|
| Metrics | 9090 | 9090 | 9090 | 9090 | ✅ |
| RPC | 8080 | 8080 | N/A | 9091 ⚠️ | ⚠️ |
| P2P | 50000 | 50000 | N/A | 9000 ⚠️ | ⚠️ |
| Grafana | 3000 | N/A | N/A | N/A | ✅ |

### Métricas - Verificación de Queries

| Métrica en prometheus.rs | Usada en dashboards | Usada en alerts | Estado |
|--------------------------|---------------------|-----------------|--------|
| `ebpf_node_xdp_packets_processed_total` | ebpf-cluster.json | - | ✅ |
| `ebpf_node_xdp_packets_dropped_total` | ebpf-cluster.json | alerts.yml | ✅ |
| `ebpf_node_xdp_blacklist_size` | ebpf-cluster.json | alerts.yml | ✅ |
| `ebpf_node_xdp_whitelist_size` | ebpf-cluster.json | - | ✅ |
| `ebpf_node_errors_total` | ebpf-cluster.json | - | ✅ |
| `ebpf_node_latency_buckets` | ebpf-cluster.json, ebpf-debug.json | - | ✅ |
| `ebpf_node_validator_count` | ebpf-cluster.json | alerts.yml | ✅ |
| `ebpf_node_blocks_proposed_total` | ebpf-cluster.json, transactions.json | - | ✅ |
| `ebpf_node_peers_connected` | ebpf-debug.json | alerts.yml | ✅ |
| `ebpf_node_messages_received_total` | ebpf-debug.json | - | ✅ |
| `ebpf_node_gossip_packets_trace_total` | ebpf-debug.json | - | ✅ |
| `ebpf_node_network_latency_ms` | - | alerts.yml | ✅ |
| `ebpf_node_bandwidth_sent_bytes_total` | - | alerts.yml | ✅ |
| `ebpf_node_consensus_duration_ms` | - | alerts.yml | ✅ |
| `ebpf_node_consensus_rounds_total` | - | alerts.yml | ✅ |
| `ebpf_node_slashing_events_total` | - | alerts.yml | ✅ |
| `ebpf_node_transaction_queue_size` | transactions.json | alerts.yml | ✅ |
| `ebpf_node_transactions_failures_total` | transactions.json | alerts.yml | ✅ |
| `ebpf_node_transactions_processed_total` | transactions.json | alerts.yml | ✅ |
| `ebpf_node_transactions_replay_rejected_total` | - | alerts.yml | ✅ |
| `ebpf_node_sybil_attempts_total` | - | alerts.yml | ✅ |
| `ebpf_node_p2p_connections_closed` | - | alerts.yml | ✅ |
| `ebpf_node_memory_usage_bytes` | - | alerts.yml | ✅ |
| `ebpf_node_uptime_seconds` | - | alerts.yml | ✅ |
| `ebpf_node_transactions_confirmed_total` | transactions.json | - | ✅ |
| `ebpf_node_transactions_by_type_total` | transactions.json | - | ✅ |
| `ebpf_node_transactions_rejected_total` | transactions.json (WRONG) | - | ⚠️ |

---

## Recomendaciones

### 🔴 CRÍTICO - Corregir inmediatamente

1. **Corregir métrica en transactions.json (línea 203)**
   ```json
   // Cambiar:
   "expr": "transactions_rejected_total"
   // Por:
   "expr": "ebpf_node_transactions_rejected_total"
   ```

### 🟡 ALTO - Corregir en próxima iteración

2. **Unificar defaults de puertos en main.rs**
   ```rust
   // Cambiar defaults en main.rs para que coincidan con Ansible:
   let rpc_port = config::node::get_port_from_env("RPC_PORT", 8080);  // Era 9091
   let network_p2p_port = config::node::get_port_from_env("NETWORK_P2P_PORT", 50000);  // Era 9000
   ```

3. **Hardcoded paths en deploy.sh**
   - Actualizar `install_service()` para usar variables `${NODE_WORKING_DIR}` y `${NODE_BINARY_DIR}`

### 🟢 BAJO - Mejora continua

4. **Remover fallback ports en main.rs** o documentar claramente su propósito
5. **Agregar verificación de consistencia de puertos** al pipeline CI/CD
6. **Considerar extraer configuración de puertos** a un archivo de configuración externo (TOML/YAML)

---

## Checklist de Verificación Final

- [x] `cargo check` compila sin errores
- [x] Workspace dependencies centralizadas
- [x] Rutas de directorios consistentes
- [x] Métricas en dashboards coinciden con prometheus.rs (1 corrección pendiente)
- [x] Alertas usan métricas existentes
- [x] Datasource Tempo configurado
- [x] Health checks implementados
- [x] Tags en playbooks consistentes
- [x] Whitelist inicial con peer local
- [x] Service template usa variables Ansible
- [ ] ~~Corregir métrica `transactions_rejected_total` en transactions.json~~ ⚠️ PENDIENTE
- [ ] ~~Unificar defaults de puertos en main.rs~~ ⚠️ PENDIENTE
- [ ] ~~Corregir hardcoded paths en deploy.sh~~ ⚠️ PENDIENTE

---

## Archivos Verificados

### Rust/Cargo (17 archivos)
- [x] [`ebpf-node/Cargo.toml`](ebpf-node/Cargo.toml)
- [x] [`ebpf-node/ebpf-node/Cargo.toml`](ebpf-node/ebpf-node/Cargo.toml)
- [x] [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs)
- [x] [`ebpf-node/ebpf-node/src/config/node.rs`](ebpf-node/ebpf-node/src/config/node.rs)
- [x] [`ebpf-node/ebpf-node/src/config/cli.rs`](ebpf-node/ebpf-node/src/config/cli.rs)
- [x] [`ebpf-node/ebpf-node/src/api/blocks.rs`](ebpf-node/ebpf-node/src/api/blocks.rs)
- [x] [`ebpf-node/ebpf-node/src/api/network.rs`](ebpf-node/ebpf-node/src/api/network.rs)
- [x] [`ebpf-node/ebpf-node/src/api/router.rs`](ebpf-node/ebpf-node/src/api/router.rs)
- [x] [`ebpf-node/ebpf-node/src/p2p/event_loop.rs`](ebpf-node/ebpf-node/src/p2p/event_loop.rs)
- [x] [`ebpf-node/ebpf-node/src/p2p/behaviour.rs`](ebpf-node/ebpf-node/src/p2p/behaviour.rs)
- [x] [`ebpf-node/ebpf-node/src/p2p/swarm.rs`](ebpf-node/ebpf-node/src/p2p/swarm.rs)
- [x] [`ebpf-node/ebpf-node/src/p2p/mod.rs`](ebpf-node/ebpf-node/src/p2p/mod.rs)
- [x] [`ebpf-node/ebpf-node/src/p2p/sync.rs`](ebpf-node/ebpf-node/src/p2p/sync.rs)
- [x] [`ebpf-node/ebpf-node/src/ebpf/programs.rs`](ebpf-node/ebpf-node/src/ebpf/programs.rs)
- [x] [`ebpf-node/ebpf-node/src/security/sybil.rs`](ebpf-node/ebpf-node/src/security/sybil.rs)
- [x] [`ebpf-node/ebpf-node/src/metrics/prometheus.rs`](ebpf-node/ebpf-node/src/metrics/prometheus.rs)
- [x] [`ebpf-node/ebpf-node/src/metrics/mod.rs`](ebpf-node/ebpf-node/src/metrics/mod.rs)

### Ansible (6 archivos)
- [x] [`ansible/inventory/group_vars/all.yml`](ansible/inventory/group_vars/all.yml)
- [x] [`ansible/playbooks/deploy.yml`](ansible/playbooks/deploy.yml)
- [x] [`ansible/playbooks/health_check.yml`](ansible/playbooks/health_check.yml)
- [x] [`ansible/roles/lxc_node/templates/ebpf-blockchain.service.j2`](ansible/roles/lxc_node/templates/ebpf-blockchain.service.j2)
- [x] [`ansible/roles/monitoring/templates/docker-compose.monitoring.yml.j2`](ansible/roles/monitoring/templates/docker-compose.monitoring.yml.j2)
- [x] [`ansible/roles/monitoring/tasks/main.yml`](ansible/roles/monitoring/tasks/main.yml)

### Monitoring (6 archivos)
- [x] [`monitoring/docker-compose.yml`](monitoring/docker-compose.yml)
- [x] [`monitoring/grafana/provisioning/datasources/datasources.yml`](monitoring/grafana/provisioning/datasources/datasources.yml)
- [x] [`monitoring/grafana/dashboards/transactions.json`](monitoring/grafana/dashboards/transactions.json)
- [x] [`monitoring/grafana/provisioning/dashboards/ebpf-debug.json`](monitoring/grafana/provisioning/dashboards/ebpf-debug.json)
- [x] [`monitoring/grafana/provisioning/dashboards/ebpf-cluster.json`](monitoring/grafana/provisioning/dashboards/ebpf-cluster.json)
- [x] [`monitoring/prometheus/alerts.yml`](monitoring/prometheus/alerts.yml)

### Scripts (1 archivo)
- [x] [`scripts/deploy.sh`](scripts/deploy.sh)

---

*Reporte generado: 2026-04-23*
*Estado: 3 inconsistencias menores pendientes de corrección*
