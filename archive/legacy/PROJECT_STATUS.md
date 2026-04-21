# eBPF Blockchain Project - Estado y Problemas

## 1. Estructura del Proyecto

```
ebpf-blockchain/
├── ebpf-node/                          # Código principal del nodo
│   ├── Cargo.toml                      # Workspace de Rust
│   ├── ebpf-node/                      # Aplicación principal (user space)
│   │   ├── src/main.rs                 # Main: libp2p + eBPF + Prometheus
│   │   └── Cargo.toml
│   ├── ebpf-node-ebpf/                # Programa eBPF (kernel space)
│   │   ├── src/main.rs                # XDP + kprobes
│   │   └── Cargo.toml
│   └── ebpf-node-common/               # Código compartido
│
├── ansible/                            # Automatización
│   └── playbooks/
│       ├── cluster.yml                 # Playbook principal (actual)
│       ├── create_cluster.yml          # Playbooks anteriores (no usados)
│       ├── start_cluster.yml
│       ├── full_cluster.yml
│       └── ...
│
├── monitoring/                         # Monitoreo
│   ├── prometheus/
│   │   └── prometheus.yml              # Config de scrapeo
│   ├── grafana/
│   │   └── provisioning/
│   │       ├── dashboards/
│   │       │   ├── ebpf-debug.json    # Dashboard principal
│   │       │   └── ebpf-cluster.json
│   │       └── datasources/
│   │           └── prometheus.yml
│   └── docker-compose.yml              # Prometheus + Grafana
│
└── scripts/                            # Scripts auxiliares (no usados)
    ├── create_cluster.sh
    ├── start_cluster.sh
    └── ...
```

---

## 2. Estado Actual

### ✅ Lo que funciona:
1. **eBPF**: Los programas XDP y kprobes cargan correctamente en los contenedores LXC
2. **Métricas de latencia**: Se generan y almacenan en mapas eBPF
3. **Prometheus**: Scrappea las métricas de los nodos correctamente
4. **Grafana**: Dashboard configurado y mostrando métricas de latencia

### ❌ Problemas identificados:

---

## 3. Problemas Detallados

### Problema 1: Métricas de Peers y Messages no se generan

**Archivo**: `ebpf-node/ebpf-node/src/main.rs`

**Causa**: El código define los contadores `PEERS_CONNECTED` y `MESSAGES_RECEIVED` pero nunca son expuestos en el handler de métricas. Solo `LATENCY_BUCKETS` está siendo exportado.

**Código problemático** (líneas 32-43):
```rust
static ref MESSAGES_RECEIVED: IntCounterVec = register_int_counter_vec!(...);
static ref PEERS_CONNECTED: IntGaugeVec = register_int_gauge_vec!(...);
```

Estos contadores se actualizan en los eventos de swarm (líneas 232-236) pero no se están exportando correctamente.

**Solución**: Necesita verificarse que las métricas se registren correctamente en el handler `/metrics`.

---

### Problema 2: Conexiones P2P entre nodos fallan

**Síntoma**: Los nodos no pueden conectarse entre sí. Errores:
- TCP: `Connection attempt to peer failed with Transport([..., Timeout])`
- UDP/QUIC: Timeout en el handshake

**Causa raíz**: El tráfico entre contenedores LXD está siendo bloqueado en la capa de red del bridge `lxdbr1`.

**Análisis**:
- El bridge `lxdbr1` está configurado con `ipv4.nat: true` (NAT saliente funciona)
- NO hay reglas de FORWARD explícitas para permitir tráfico entre contenedores
- Docker está usando interfaces que podrían interferir (`br-f8d50be610ff`)

**Verificación realizada**:
```bash
# TCP connectivity test - FALLA
lxc exec ebpf-node-2 -- timeout 3 bash -c "echo > /dev/tcp/192.168.2.201/50000"
# Resultado: TCP port closed or timeout
```

**Solución requerida**: Configurar reglas de red en el host para permitir tráfico entre contenedores:

1. Habilitar IP forwarding
2. Agregar reglas de FORWARD para el bridge lxdbr1
3. O usar un perfil de red LXD sin NAT para comunicación directa entre contenedores

---

### Problema 3: Ansible Playbook incompleto

**Archivo**: `ansible/playbooks/cluster.yml`

**Problemas**:
1. No maneja correctamente el DNS (los contenedores pierden DNS después de reiniciar)
2. No instala `bpf-linker` en todos los nodos (solo nodo 1)
3. No copia el binary compilado a los otros nodos
4. No configura `--bootstrap-peers` automáticamente
5. No reinstala dependencias si fallan (usa `|| true` que oculta errores)

**Código problemático** (líneas 54-66):
```yaml
- name: Install Rust
  shell: |
    for i in $(seq 1 {{ num_nodes }}); do
      lxc exec ebpf-node-$i -- bash -c "curl ... | sh" 2>/dev/null || true
    done
  failed_when: false  # Oculta errores!
```

---

### Problema 4: Grafana Dashboard no recibe todas las métricas

**Archivos**: 
- `monitoring/grafana/provisioning/dashboards/ebpf-debug.json`
- `monitoring/grafana/provisioning/datasources/prometheus.yml`

**Problema**: El dashboard espera:
- `ebpf_node_peers_connected` - NO se genera
- `ebpf_node_messages_received_total` - NO se genera  
- `ebpf_node_latency_buckets` - SÍ se genera
- `ebpf_node_uptime` - NO está implementado

**Estado actual de paneles**:
- ✅ Latency Buckets - FUNCIONA
- ❌ Messages Received - NO HAY DATOS
- ❌ Peers Connected - NO HAY DATOS
- ❌ Uptime - NO IMPLEMENTADO

---

### Problema 5: Código fuente no tiene soporte para bootstrap de peers

**Archivo**: `ebpf-node/ebpf-node/src/main.rs`

**Estado actual**:
- ✅ tiene soporte para `--bootstrap-peers` (agregado recientemente)
- ✅ tiene soporte para múltiples `--listen-addresses`
- ❌ NO tiene mDNS para descubrimiento automático
- ❌ NO tiene Kademlia DHT (aunque está en las dependencias de Cargo.toml)

**Para que los nodos se conecten automáticamente** se necesita:
1. Usar `--bootstrap-peers` con la dirección del primer nodo
2. O agregar mDNS al comportamiento de libp2p

---

### Problema 6: Archivos duplicados/confusos

**Problemas de estructura**:
- Existen múltiples directorios con contenido similar:
  - `ebpf-blockchain/` y `ebpf-node/`
  - `monitoring/` y `ebpf-blockchain/monitoring/`
- Hay múltiples scripts en `/scripts/` que no se usan
- Hay múltiples playbooks de Ansible que no se usan

---

## 4. Cómo debería funcionar

### Flujo esperado:

```
┌─────────────────────────────────────────────────────────────┐
│                    CLUSTER P2P eBPF                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │  LXC Node 1  │    │  LXC Node 2  │    │  LXC Node 3  │ │
│  │ 192.168.2.201│◄──►│ 192.168.2.202│◄──►│ 192.168.2.203│ │
│  │              │    │              │    │              │ │
│  │ libp2p/gossip│    │ libp2p/gossip│    │ libp2p/gossip│ │
│  │ QUIC/TCP    │    │ QUIC/TCP    │    │ QUIC/TCP    │ │
│  │              │    │              │    │              │ │
│  │ XDP Program  │    │ XDP Program  │    │ XDP Program  │ │
│  │ (eBPF)       │    │ (eBPF)       │    │ (eBPF)       │ │
│  └──────────────┘    └──────────────┘    └──────────────┘ │
│          │                  │                  │         │
│          └──────────────────┼──────────────────┘         │
│                             │                            │
│                             ▼                            │
│                  ┌──────────────────────┐               │
│                  │     Prometheus       │               │
│                  │   (scrape metrics)  │               │
│                  └──────────────────────┘               │
│                             │                            │
│                             ▼                            │
│                  ┌──────────────────────┐               │
│                  │      Grafana        │               │
│                  │  (dashboards)       │               │
│                  └──────────────────────┘               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Métricas esperadas:

| Métrica | Descripción | Estado |
|---------|-------------|--------|
| `ebpf_node_latency_buckets` | Histograma de latencia de red | ✅ Funcionando |
| `ebpf_node_peers_connected` | Número de peers conectados | ❌ No se genera |
| `ebpf_node_messages_received_total` | Mensajes gossip recibidos | ❌ No se genera |
| `ebpf_node_uptime` | Tiempo de actividad del nodo | ❌ No implementado |

---

## 5. Plan de solución

### Prioridad 1 - Conectividad P2P (Bloqueante):
1. [ ] Configurar reglas de red para permitir tráfico entre contenedores LXD
2. [ ] Probar conectividad básica (ping/tcp) entre nodos
3. [ ] Verificar que libp2p puede establecer conexiones

### Prioridad 2 - Métricas:
1. [ ] Arreglar generación de métricas de peers
2. [ ] Arreglar generación de métricas de mensajes
3. [ ] Agregar métrica de uptime

### Prioridad 3 - Automatización:
1. [ ] Mejorar Ansible playbook para manejar errores
2. [ ] Agregar soporte automático para bootstrap peers
3. [ ] Asegurar que todos los nodos tengan el binary

### Prioridad 4 - Descubrimiento:
1. [ ] Agregar mDNS para descubrimiento automático en red local
2. [ ] O usar bootstrap nodes estáticos

---

## 6. Comandos actuales para iniciar nodos

```bash
# Iniciar nodo 1 (bootstrap)
lxc exec ebpf-node-1 -- /root/ebpf-blockchain/ebpf-node/target/release/ebpf-node \
  --iface eth0 \
  --listen-addresses '/ip4/0.0.0.0/tcp/50000,/ip4/0.0.0.0/udp/50000/quic-v1'

# Iniciar nodo 2 con bootstrap
lxc exec ebpf-node-2 -- /root/ebpf-blockchain/ebpf-node/target/release/ebpf-node \
  --iface eth0 \
  --bootstrap-peers '/ip4/192.168.2.201/tcp/50000/p2p/PEER_ID'

# Obtener PEER_ID del nodo 1
lxc exec ebpf-node-1 -- grep "Local Peer ID" /tmp/ebpf.log
```

---

*Última actualización: 2026-03-28*
