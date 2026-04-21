# Fase 1: Seguridad - Implementación

**Fecha:** 2026-04-21
**Estado:** ✅ Completada

## Resumen

Esta fase implementa tres mecanismos de seguridad fundamentales para el eBPF Blockchain:

1. **Replay Protection**: Prevención de reutilización de transacciones mediante nonces
2. **Sybil Protection**: Prevención de ataques de identidad múltiple
3. **Whitelist XDP**: Filtrado preventivo de IPs en el kernel

## 1. Replay Protection

### Implementación

- [`Transaction`](ebpf-node/ebpf-node/src/main.rs:106) struct modificado con campos `nonce` y `timestamp`
- [`ReplayProtection`](ebpf-node/ebpf-node/src/main.rs:460) struct con las siguientes funciones:
  - `validate_nonce()`: Verifica que el nonce sea incremental por sender
  - `update_nonce()`: Actualiza el último nonce visto por sender
  - `mark_processed()`: Marca un tx_id como procesado
  - `is_processed()`: Verifica si un tx_id ya fue procesado
  - `cleanup_old_processed()`: Limpia transacciones procesadas > 24h

### Constantes de Seguridad

```rust
const NONCE_MAX_AGE_SECS: u64 = 300;  // 5 minutos
const NONCE_KEY_PREFIX: &str = "nonce:";
const PROCESSED_TX_PREFIX: &str = "processed_tx:";
```

### Validación en el Event Loop

Cuando se recibe un `TxProposal`:
1. Se verifica que el timestamp no sea antiguo (> 5 min)
2. Se verifica que el tx_id no haya sido procesado
3. Se verifica que el nonce sea incremental
4. Si pasa todas las validaciones, se registra el nonce y se propaga
5. Si falla, se incrementa `TRANSACTIONS_REPLAY_REJECTED` y se descarta

### Métricas

- `ebpf_node_transactions_replay_rejected_total`: Transacciones rechazadas por replay protection

## 2. Sybil Protection

### Implementación

- [`SybilProtection`](ebpf-node/ebpf-node/src/main.rs:560) struct con las siguientes funciones:
  - `count_connections_per_ip()`: Cuenta conexiones activas por IP
  - `register_connection()`: Registra una conexión peer_id -> IP
  - `unregister_connection()`: Elimina registro al desconectar
  - `check_ip_limit()`: Verifica que no se exceda el límite (3 conexiones/IP)
  - `get_whitelisted_peers()`: Obtiene peers de la whitelist
  - `add_to_whitelist()`: Añade peer a whitelist
  - `remove_from_whitelist()`: Elimina peer de whitelist

### Límites

- Máximo 3 conexiones por IP address
- Whitelist de peers confiables en RocksDB

### Integración en Event Loop

- `ConnectionEstablished`: Registra conexión y verifica límite
- `ConnectionClosed`: Elimina registro de conexión
- `IncomingConnection`: Pre-verifica límite antes de aceptar

### Métricas

- `ebpf_node_sybil_attempts_total`: Intentos de ataque Sybil detectados

## 3. Whitelist XDP

### Implementación en eBPF

- [`NODES_WHITELIST`](ebpf-node/ebpf-node-ebpf/src/main.rs:21): LpmTrie para IPs confiables
- [`NODES_BLACKLIST`](ebpf-node/ebpf-node-ebpf/src/main.rs:28): LpmTrie para IPs maliciosas

### Lógica de Filtrado

```
1. Check BLACKLIST -> DROP si está
2. Check WHITELIST -> DROP si NO está
3. PASS si está en whitelist y no en blacklist
```

### Ventajas

- **Preventiva**: Solo traffic de peers confiables es aceptado
- **Reactiva**: Blacklist complementa para threat detection dinámico
- **Hot-update**: Ambos maps pueden actualizarse desde el usuario space

## Archivos Modificados

| Archivo | Cambios |
|---------|---------|
| [`ebpf-node/ebpf-node/src/main.rs`](ebpf-node/ebpf-node/src/main.rs) | +300 líneas (ReplayProtection, SybilProtection, validaciones) |
| [`ebpf-node/ebpf-node-ebpf/src/main.rs`](ebpf-node/ebpf-node-ebpf/src/main.rs) | Whitelist + Blacklist maps, lógica de filtrado |

## Criterios de Aceptación

- [x] Transacciones duplicadas rechazadas
- [x] Nonces incrementales requeridos
- [x] Timestamp window de 5 minutos
- [x] Cache de transacciones de 24h con cleanup automático
- [x] Máximo 3 conexiones por IP
- [x] Whitelist XDP activa
- [x] Blacklist reactiva mantenida
- [x] Métricas Prometheus para seguridad

## Próximos Pasos

1. **Fase 2: Observabilidad** - Grafana dashboards, Loki logging, Tempo tracing
2. Testing de seguridad con ataques simulados
3. Documentación de operación para whitelist management

## Comandos Útiles

```bash
# Verificar métricas de seguridad
curl http://localhost:9090/metrics | grep -E "replay|sybil"

# Añadir IP a whitelist (desde user space)
# Se hace via API o configuración inicial

# Ver logs de seguridad
journalctl -u ebpf-blockchain -f | grep -E "sybil|replay"
```
