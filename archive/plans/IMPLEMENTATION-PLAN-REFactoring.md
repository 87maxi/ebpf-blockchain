# Plan de Implementación: Refactorización de Arquitectura Aya

## Tabla de Contenidos

1. [Visión General](#1-visión-general)
2. [Análisis de Dependencias entre Refactorizaciones](#2-análisis-de-dependencias-entre-refactorizaciones)
3. [Fase 1: Separación de Módulos User-Space](#3-fase-1-separación-de-módulos-user-space)
4. [Fase 2: Abstraction Type-Safe para eBPF Maps](#4-fase-2-abstraction-type-safe-para-ebpf-maps)
5. [Fase 3: Separación de eBPF Programs](#5-fase-3-separación-de-ebpf-programs)
6. [Fase 4: Hot-Reload Architecture](#6-fase-4-hot-reload-architecture)
7. [Fase 5: Migración a Ringbuf](#7-fase-5-migración-a-ringbuf)
8. [Fase 6: Migración a CO-RE](#8-fase-6-migración-a-core)
9. [Resumen de Fases](#9-resumen-de-fases)

---

## 1. Visión General

### 1.1 Estado Actual

El proyecto [`ebpf-node`](ebpf-node/ebpf-node/src/main.rs:1) tiene un **archivo monolítico de 2406 líneas** que combina:
- Parseo de CLI (clap)
- Carga y attach de programas eBPF (Aya)
- Swarming P2P (libp2p)
- Handlers HTTP (axum)
- Gestión de seguridad (Sybil, Replay Protection)
- Base de datos (RocksDB)
- Métricas (Prometheus)
- Broadcast WebSocket

### 1.2 Objetivos

```
┌──────────────────────────────────────────────────────────────────┐
│                    OBJETIVOS DE REFACTORIZACIÓN                    │
│                                                                  │
│  1. Modularidad: Separar en 7+ módulos lógicos                 │
│  2. Type-Safety: Abstraction para eBPF maps                    │
│  3. Observabilidad: Migrar de bpf_trace_printk a ringbuf       │
│  4. Portabilidad: Implementar CO-RE                             │
│  5. Resiliencia: Hot-reload de programas eBPF                  │
│  6. Mantenibilidad: Reducir main.rs a <200 líneas              │
└──────────────────────────────────────────────────────────────────┘
```

### 1.3 Nueva Estructura de Archivos

```
ebpf-node/ebpf-node/src/
├── main.rs                  ← Entry point (<200 líneas)
├── config/
│   ├── mod.rs               ← Module declarations
│   ├── cli.rs               ← Opt / CLI parsing (clap)
│   ├── node.rs              ← NodeConfig
│   └── paths.rs             ← Path configuration
├── ebpf/
│   ├── mod.rs               ← Module declarations
│   ├── loader.rs            ← Ebpf loading (aya::Ebpf)
│   ├── programs.rs          ← Program attach/detach
│   ├── maps.rs              ← Type-safe map abstraction
│   ├── metrics.rs           ← eBPF → Prometheus sync
│   └── hot_reload.rs        ← Hot-reload manager
├── api/
│   ├── mod.rs               ← Module declarations
│   ├── health.rs            ← Health handler
│   ├── node.rs              ← Node info handler
│   ├── network.rs           ← Network handlers (peers, config)
│   ├── transactions.rs      ← Transaction handlers
│   ├── blocks.rs            ← Block handlers
│   ├── security.rs          ← Security handlers (blacklist, whitelist)
│   └── metrics.rs           ← Prometheus metrics handler
├── p2p/
│   ├── mod.rs               ← Module declarations
│   ├── swarm.rs             ← Swarm setup (libp2p)
│   ├── gossip.rs            ← Gossipsub handling
│   ├── sync.rs              ← Historical sync
│   ├── rpc.rs               ← RPC channel setup
│   └── behaviour.rs         ← MyBehaviour struct
├── security/
│   ├── mod.rs               ← Module declarations
│   ├── peer_store.rs        ← PeerStore (RocksDB)
│   ├── replay.rs            ← ReplayProtection
│   ├── sybil.rs             ← SybilProtection
│   └── keypair.rs           ← Identity keypair management
├── db/
│   ├── mod.rs               ← Module declarations
│   ├── rocksdb.rs           ← RocksDB setup/config
│   └── backup.rs            ← Backup/cleanup functions
└── metrics/
    ├── mod.rs               ← Module declarations
    ├── prometheus.rs        ← Metric definitions (lazy_static)
    └── system.rs            ← System metrics collection
```

---

## 2. Análisis de Dependencias entre Refactorizaciones

```
┌──────────────────────────────────────────────────────────────────┐
│                   DEPENDENCIAS ENTRE FASES                        │
│                                                                  │
│  Fase 1 (Módulos)                                                │
│    ├──→ Fase 2 (Maps)                                           │
│    ├──→ Fase 3 (eBPF Programs)                                  │
│    └──→ Fase 4 (Hot-Reload) ←─ Requires: Fase 2 + Fase 3       │
│                                                                  │
│  Fase 5 (Ringbuf)                                                │
│    └──→ Independiente (puede hacer en paralelo)                 │
│                                                                  │
│  Fase 6 (CO-RE)                                                  │
│    └──→ Independiente (requiere kernel con BTF)                 │
│                                                                  │
│  CRITICAL PATH:                                                  │
│  Fase 1 → Fase 2 → Fase 4                                      │
│        Fase 1 → Fase 3 ───────────────────────────────────      │
└──────────────────────────────────────────────────────────────────┘
```

### 2.1 Prioridad de Ejecución

| Prioridad | Fase | Dependency | Effort |
|-----------|------|------------|--------|
| P0 (Critical) | Fase 1: Separación de módulos | Ninguna | Alto |
| P1 (High) | Fase 2: Maps abstraction | Fase 1 | Medio |
| P1 (High) | Fase 3: Separación eBPF | Fase 1 | Medio |
| P2 (Medium) | Fase 4: Hot-Reload | Fase 1, 2, 3 | Alto |
| P3 (Low) | Fase 5: Ringbuf | Ninguna | Bajo |
| P3 (Low) | Fase 6: CO-RE | Ninguna (kernel BTF) | Medio |

---

## 3. Fase 1: Separación de Módulos User-Space

### 3.1 Descripción

Separar el archivo monolítico [`main.rs`](ebpf-node/ebpf-node/src/main.rs:1) (2406 líneas) en módulos lógicos.

### 3.2 Tareas

#### Tarea 1.1: Crear estructura de directorios

```bash
cd ebpf-node/ebpf-node/src
mkdir -p config ebpf api p2p security db metrics
```

#### Tarea 1.2: Extraer CLI y Config → `config/`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:1256) (líneas ~1256-1293)

**Contenido a extraer**:
- `struct Opt` (clap CLI parser)
- `fn load_saved_peers()`
- `fn save_peers()`
- `fn get_bootstrap_peers_from_env()`
- `struct NodeConfig`
- `fn get_port_from_env()`
- `fn get_current_timestamp()`
- `fn format_iso_timestamp()`
- Error response helpers

**Archivo destino**: [`config/cli.rs`](ebpf-node/ebpf-node/src/config/cli.rs)

```rust
// config/cli.rs
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ebpf-node", author = "ebpf-team", version, about = "eBPF P2P Node")]
pub struct Opt {
    #[arg(short, long, default_value = "eth0")]
    pub iface: String,

    #[arg(short, long, default_value_t = 9090)]
    pub tcp_port: u16,

    #[arg(short, long, default_value_t = 9091)]
    pub metrics_port: u16,

    #[arg(short, long, default_value_t = 9092)]
    pub p2p_port: u16,

    #[arg(short, long)]
    pub peers: Option<Vec<String>>,

    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
}

// fn get_port_from_env()
// fn get_bootstrap_peers_from_env()
// fn load_saved_peers()
// fn save_peers()
```

**Archivo destino**: [`config/node.rs`](ebpf-node/ebpf-node/src/config/node.rs)

```rust
// config/node.rs
use serde::{Serialize, Deserialize};

// struct NodeConfig
// struct NodeState
// struct Block
// impl Block::compute_hash()
```

#### Tarea 1.3: Extraer Response Types → `api/`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:291) (líneas ~291-498)

**Contenido a extraer**:
- `struct Transaction`
- `enum NetworkMessage`
- `struct Block`, `struct BlockSummary`
- Response types: `NodeInfoResponse`, `PeerListResponse`, `PeerDetail`, etc.
- `struct ErrorResponse`, `HealthResponse`, `HealthChecks`

**Archivo destino**: [`api/responses.rs`](ebpf-node/ebpf-node/src/api/responses.rs)

```rust
// api/responses.rs
use serde::{Serialize, Deserialize};

// All response types (Transaction, NetworkMessage, Block, etc.)
```

#### Tarea 1.4: Extraer Handlers HTTP → `api/`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:580) (líneas ~580-1041)

**Contenido a extraer**:

| Handler | Líneas | Archivo Destino |
|---------|--------|-----------------|
| `health_handler` | ~580-645 | `api/health.rs` |
| `node_info_handler` | ~622-645 | `api/node.rs` |
| `network_peers_handler` | ~648-679 | `api/network.rs` |
| `network_config_get_handler` | ~682-695 | `api/network.rs` |
| `network_config_put_handler` | ~698-715 | `api/network.rs` |
| `transactions_create_handler` | ~718-780 | `api/transactions.rs` |
| `transactions_get_handler` | ~783-846 | `api/transactions.rs` |
| `blocks_latest_handler` | ~849-885 | `api/blocks.rs` |
| `blocks_by_height_handler` | ~888-936 | `api/blocks.rs` |
| `security_blacklist_get_handler` | ~939-950 | `api/security.rs` |
| `security_blacklist_put_handler` | ~953-984 | `api/security.rs` |
| `security_whitelist_get_handler` | ~987-1009 | `api/security.rs` |
| `metrics_handler` | ~1011-1017 | `api/metrics.rs` |
| `rpc_handler` | ~1019-1025 | `api/rpc.rs` |
| `ws_handler` + `handle_socket` | ~1027-1041 | `api/ws.rs` |

**Ejemplo de [`api/health.rs`](ebpf-node/ebpf-node/src/api/health.rs)**:

```rust
// api/health.rs
use axum::{extract::State, Json, http::StatusCode};
use std::sync::Arc;
use crate::config::node::NodeState;

pub async fn health_handler(
    State(state): State<Arc<NodeState>>,
) -> impl axum::response::IntoResponse {
    // health_handler code
}
```

#### Tarea 1.5: Extraer Métricas → `metrics/`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:40) (líneas ~40-239)

**Contenido a extraer**:
- Todos los `lazy_static!` metric definitions
- `fn initialize_metrics()`
- `fn update_system_metrics()`

**Archivo destino**: [`metrics/prometheus.rs`](ebpf-node/ebpf-node/src/metrics/prometheus.rs)

```rust
// metrics/prometheus.rs
use prometheus::{
    Encoder, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, TextEncoder,
    register_int_counter, register_int_counter_vec, register_int_gauge,
    register_int_gauge_vec,
};
use lazy_static::lazy_static;

lazy_static! {
    static ref LATENCY_BUCKETS: IntGaugeVec = /* ... */;
    static ref MESSAGES_RECEIVED: IntCounterVec = /* ... */;
    // ... all other metrics
}

pub fn initialize_metrics();
pub fn gather_metrics() -> Vec<prometheus::proto::MetricFamily>;
```

**Archivo destino**: [`metrics/system.rs`](ebpf-node/ebpf-node/src/metrics/system.rs)

```rust
// metrics/system.rs
pub fn update_system_metrics();
```

#### Tarea 1.6: Extraer Security → `security/`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:1256) (líneas ~1256-1605)

**Contenido a extraer**:

| Struct | Líneas | Archivo Destino |
|--------|--------|-----------------|
| `PeerStore` | ~1301-1350 | `security/peer_store.rs` |
| `ReplayProtection` | ~1359-1461 | `security/replay.rs` |
| `SybilProtection` | ~1466-1605 | `security/sybil.rs` |

**Ejemplo de [`security/replay.rs`](ebpf-node/ebpf-node/src/security/replay.rs)**:

```rust
// security/replay.rs
use rocksdb::DB;
use crate::db::rocksdb::DbHandle;

pub struct ReplayProtection {
    db: DbHandle,
}

impl ReplayProtection {
    pub fn new(db: DbHandle) -> Self { /* ... */ }
    pub fn validate_nonce(&self, sender: &str, nonce: u64) -> Result<u64, String> { /* ... */ }
    pub fn update_nonce(&self, sender: &str, nonce: u64) -> anyhow::Result<()> { /* ... */ }
    pub fn mark_processed(&self, tx_id: &str, timestamp: u64) -> anyhow::Result<()> { /* ... */ }
    pub fn is_processed(&self, tx_id: &str) -> bool { /* ... */ }
    pub fn cleanup_old_processed(&self, max_age_secs: u64) { /* ... */ }
}
```

#### Tarea 1.7: Extraer libp2p P2P → `p2p/`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:19) (líneas ~19-32) + código de setup

**Contenido a extraer**:

| Componente | Líneas (aprox) | Archivo Destino |
|------------|----------------|-----------------|
| `struct MyBehaviour` | ~1600-1605 | `p2p/behaviour.rs` |
| `fn setup_structured_logging()` | ~1637-1634 | `p2p/swarm.rs` |
| Swarm setup | ~1798-1850 | `p2p/swarm.rs` |
| Gossipsub setup | ~1850-1900 | `p2p/gossip.rs` |
| Dial peers | ~1900-1913 | `p2p/swarm.rs` |

**Ejemplo de [`p2p/swarm.rs`](ebpf-node/ebpf-node/src/p2p/swarm.rs)**:

```rust
// p2p/swarm.rs
use libp2p::{Swarm, SwarmBuilder};
use crate::p2p::behaviour::MyBehaviour;

pub fn create_swarm(keypair: libp2p::identity::Keypair) -> Swarm<MyBehaviour> {
    // swarm setup code
}
```

#### Tarea 1.8: Extraer Database → `db/`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:1691) (líneas ~1691-1734)

**Contenido a extraer**:
- `fn get_data_dir()`
- `fn setup_data_dir()`
- `fn create_backup()`
- `fn cleanup_backups()`
- DB open/recovery logic

**Archivo destino**: [`db/rocksdb.rs`](ebpf-node/ebpf-node/src/db/rocksdb.rs)
**Archivo destino**: [`db/backup.rs`](ebpf-node/ebpf-node/src/db/backup.rs)

#### Tarea 1.9: Extraer eBPF Setup → `ebpf/`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:1740) (líneas ~1740-1765)

**Contenido a extraer**:
- eBPF loading
- Program attach/detach
- KProbe attach

**Archivo destino**: [`ebpf/loader.rs`](ebpf-node/ebpf-node/src/ebpf/loader.rs)
**Archivo destino**: [`ebpf/programs.rs`](ebpf-node/ebpf-node/src/ebpf/programs.rs)

#### Tarea 1.10: Extraer Main Event Loop → `p2p/` o `main.rs`

**Origen**: [`main.rs`](ebpf-node/ebpf-node/src/main.rs:2028) (líneas ~2028-2406)

**Contenido a extraer**:
- `tokio::select!` loop
- Stats interval handler
- Gossipsub event handling
- Transaction proposal handling
- Vote handling
- Connection handling

**Archivo destino**: [`p2p/event_loop.rs`](ebpf-node/ebpf-node/src/p2p/event_loop.rs)

#### Tarea 1.11: Reescribir `main.rs`

**Nuevo [`main.rs`](ebpf-node/ebpf-node/src/main.rs)** (~150-200 líneas):

```rust
// main.rs
mod config;
mod ebpf;
mod api;
mod p2p;
mod security;
mod db;
mod metrics;

use config::{cli::Opt, node::NodeState};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    
    // Setup logging
    // Setup data dirs
    
    // Initialize metrics
    metrics::prometheus::initialize_metrics();
    
    // Setup database
    let db = db::rocksdb::init_db()?;
    
    // Setup security
    let replay_protection = security::replay::ReplayProtection::new(db.clone());
    let sybil_protection = security::sybil::SybilProtection::new(db.clone());
    let peer_store = security::peer_store::PeerStore::new(db.clone());
    
    // Setup eBPF
    let ebpf_manager = ebpf::loader::load(&opt.iface)?;
    
    // Setup P2P
    let (tx_rpc, rx_rpc) = tokio::sync::mpsc::channel::<Transaction>(100);
    let (tx_ws, _rx_ws) = tokio::sync::broadcast::channel::<String>(100);
    let mut swarm = p2p::swarm::create_swarm(/* keypair */);
    
    // Setup NodeState
    let node_state = Arc::new(NodeState {
        db: db.clone(),
        tx_rpc: tx_rpc.clone(),
        tx_ws: tx_ws.clone(),
        replay_protection,
        sybil_protection,
        peer_store,
        // ...
    });
    
    // Setup HTTP API
    let app = api::router::create(node_state.clone(), tx_rpc, tx_ws);
    
    // Spawn HTTP server
    let http_handle = tokio::spawn(async move {
        axum::serve(listener, app).await
    });
    
    // Spawn metrics server
    let metrics_handle = tokio::spawn(async move {
        metrics::serve().await
    });
    
    // Run P2P event loop
    p2p::event_loop::run(swarm, node_state, rx_rpc).await?;
    
    Ok(())
}
```

### 3.3 Verificación

```bash
cd ebpf-node/
cargo build 2>&1 | tee /tmp/refactor-phase1.log
cargo test 2>&1 | tee /tmp/refactor-phase1-test.log
```

### 3.4 Riesgos y Mitigación

| Riesgo | Mitigación |
|--------|------------|
| Circular dependencies | Invertir dependencias: main → modules, nunca module → module (excepto los permitidos) |
| Pérdida de funcionalidad | Mantener git branch por fase, hacer diff antes de merge |
| Compilación fallida | Compilar después de cada módulo extraído, no al final |

---

## 4. Fase 2: Abstraction Type-Safe para eBPF Maps

### 4.1 Descripción

Crear una capa de abstracción type-safe para acceder a los maps eBPF, reemplazando el patrón actual de `try_from` repetido.

### 4.2 Estado Actual

Patrón actual en [`main.rs`](ebpf-node/ebpf-node/src/main.rs:2035):

```rust
// Patrón actual: repetitivo y propenso a errores
if let Ok(latency_stats) = HashMap::<_, u64, u64>::try_from(ebpf.map("LATENCY_STATS").unwrap()) {
    for entry in latency_stats.iter() {
        // ...
    }
}

if let Ok(blacklist) = LpmTrie::<_, u32, u32>::try_from(ebpf.map("NODES_BLACKLIST").unwrap()) {
    let blacklist_size = blacklist.iter().count();
    // ...
}

if let Ok(whitelist) = LpmTrie::<_, u32, u32>::try_from(ebpf.map("NODES_WHITELIST").unwrap()) {
    let whitelist_size = whitelist.iter().count();
    // ...
}
```

### 4.3 Diseño Propuesto

**Archivo**: [`ebpf/maps.rs`](ebpf-node/ebpf-node/src/ebpf/maps.rs)

```rust
// ebpf/maps.rs
use aya::{Ebpf, maps::{HashMap, LpmTrie, LruHashMap}};
use aya::maps::lpm_trie::Key;
use anyhow::{Result, Context};

/// Type-safe eBPF map manager
pub struct EbpfMaps<'a> {
    ebpf: &'a Ebpf,
}

impl<'a> EbpfMaps<'a> {
    pub fn new(ebpf: &'a Ebpf) -> Self {
        Self { ebpf }
    }

    /// Type-safe access to LATENCY_STATS (HashMap<u64, u64>)
    pub fn latency_stats(&self) -> Result<HashMap<'a, u64, u64>> {
        HashMap::try_from(
            self.ebpf.map("LATENCY_STATS")
                .context("Failed to get LATENCY_STATS map")?
        )
    }

    /// Type-safe access to NODES_WHITELIST (LpmTrie<u32, u32>)
    pub fn whitelist(&self) -> Result<LpmTrie<'a, u32, u32>> {
        LpmTrie::try_from(
            self.ebpf.map("NODES_WHITELIST")
                .context("Failed to get NODES_WHITELIST map")?
        )
    }

    /// Type-safe access to NODES_BLACKLIST (mutable LpmTrie)
    pub fn blacklist(&self) -> Result<LpmTrie<'a, u32, u32>> {
        LpmTrie::try_from(
            self.ebpf.map("NODES_BLACKLIST")
                .context("Failed to get NODES_BLACKLIST map")?
        )
    }

    /// Type-safe access to LATENCY_STATS LruHashMap (if used)
    pub fn start_times(&self) -> Result<LruHashMap<'a, u64, u64>> {
        LruHashMap::try_from(
            self.ebpf.map("START_TIMES")
                .context("Failed to get START_TIMES map")?
        )
    }

    /// Get whitelist size
    pub fn whitelist_size(&self) -> Result<usize> {
        Ok(self.whitelist()?.iter().filter_map(Result::ok).count())
    }

    /// Get blacklist size
    pub fn blacklist_size(&self) -> Result<usize> {
        Ok(self.blacklist()?.iter().filter_map(Result::ok).count())
    }

    /// Block IP in blacklist
    pub fn block_ip(&self, ip: u32, prefix_len: u32) -> Result<()> {
        let key = Key::new(prefix_len, ip);
        self.blacklist()?.insert(&key, &1, 0)
            .context("Failed to insert IP into blacklist")
    }

    /// Unblock IP from blacklist
    pub fn unblock_ip(&self, ip: u32, prefix_len: u32) -> Result<()> {
        let key = Key::new(prefix_len, ip);
        self.blacklist()?.remove(&key)
            .context("Failed to remove IP from blacklist")
    }

    /// Check if IP is in whitelist
    pub fn is_whitelisted(&self, ip: u32, prefix_len: u32) -> Result<bool> {
        let key = Key::new(prefix_len, ip);
        Ok(self.whitelist()?.get(&key, 0).is_some())
    }

    /// Get latency stats as a HashMap<u64, u64> -> Vec<(bucket, count)>
    pub fn get_latency_stats(&self) -> Result<Vec<(u64, u64)>> {
        let stats = self.latency_stats()?;
        let mut result = Vec::new();
        for entry in stats.iter() {
            if let Ok((k, v)) = entry {
                result.push((k, v));
            }
        }
        Ok(result)
    }
}
```

### 4.4 Migración de Uso

**Antes** (en [`ebpf/metrics.rs`](ebpf-node/ebpf-node/src/ebpf/metrics.rs)):

```rust
if let Ok(latency_stats) = HashMap::<_, u64, u64>::try_from(ebpf.map("LATENCY_STATS").unwrap()) {
    // ...
}
```

**Después**:

```rust
let maps = EbpfMaps::new(&ebpf);
let stats = maps.get_latency_stats()?;
for (bucket, count) in stats {
    LATENCY_BUCKETS.with_label_values(&[&bucket.to_string()]).set(count as i64);
}
```

### 4.5 Tareas

1. Crear [`ebpf/maps.rs`](ebpf-node/ebpf-node/src/ebpf/maps.rs) con `EbpfMaps` struct
2. Actualizar [`ebpf/mod.rs`](ebpf-node/ebpf-node/src/ebpf/mod.rs) para exportar `maps`
3. Reemplazar todos los `HashMap::try_from(ebpf.map(...))` por `EbpfMaps::new(&ebpf).latency_stats()`
4. Reemplazar todos los `LpmTrie::try_from(ebpf.map(...))` por `EbpfMaps::new(&ebpf).whitelist()` / `.blacklist()`
5. Agregar tests unitarios para `EbpfMaps`

### 4.6 Verificación

```bash
cd ebpf-node/
cargo build
cargo test ebpf::maps
```

---

## 5. Fase 3: Separación de eBPF Programs

### 5.1 Descripción

Separar el archivo eBPF actual (`ebpf-node-ebpf/src/main.rs`) en módulos para XDP y KProbes.

### 5.2 Estado Actual

**Archivo**: [`ebpf-node-ebpf/src/main.rs`](ebpf-node/ebpf-node-ebpf/src/main.rs:1) (145 líneas)

Contiene 3 programas en un solo archivo:
- `ebpf_node` (XDP) - líneas 41-88
- `netif_receive_skb` (KProbe) - líneas 92-106
- `napi_consume_skb` (KProbe) - líneas 110-135

### 5.3 Diseño Propuesto

**Nuevo archivo**: [`ebpf-node-ebpf/src/xdp.rs`](ebpf-node/ebpf-node-ebpf/src/xdp.rs)

```rust
// ebpf-node-ebpf/src/xdp.rs
use aya_ebpf::{
    bindings::xdp_action,
    macros::map,
    programs::XdpContext,
};
use aya_ebpf_helper::*;

use crate::common;

#[map]
static NODES_WHITELIST: LpmTrie<u32, u32> = LpmTrie::with_max_entries(1024, BPF_F_NO_PREALLOC);

#[map]
static NODES_BLACKLIST: LpmTrie<u32, u32> = LpmTrie::with_max_entries(10240, BPF_F_NO_PREALLOC);

pub fn try_xdp_filter(ctx: XdpContext) -> Result<u32, ()> {
    // try_ebpf_node code (from main.rs lines 61-88)
}

#[xdp]
pub fn xdp_main(ctx: XdpContext) -> u32 {
    try_xdp_filter(ctx).unwrap_or(xdp_action::XDP_ABORTED)
}
```

**Nuevo archivo**: [`ebpf-node-ebpf/src/tracing.rs`](ebpf-node/ebpf-node-ebpf/src/tracing.rs)

```rust
// ebpf-node-ebpf/src/tracing.rs
use aya_ebpf::{
    helpers::bpf_ktime_get_ns,
    macros::map,
    programs::ProbeContext,
};

#[map]
static START_TIMES: LruHashMap<u64, u64> = LruHashMap::with_max_entries(10240, 0);

#[map]
static LATENCY_STATS: HashMap<u64, u64> = HashMap::with_max_entries(64, 0);

pub fn try_netif_receive_skb(ctx: ProbeContext) -> Result<(), ()> {
    // try_netif_receive_skb code (from main.rs lines 97-106)
}

#[kprobe]
pub fn netif_receive_skb_entry(ctx: ProbeContext) -> u32 {
    try_netif_receive_skb(ctx).unwrap_or_else(|_| 0)
}

pub fn try_napi_consume_skb(ctx: ProbeContext) -> Result<(), ()> {
    // try_napi_consume_skb code (from main.rs lines 115-135)
}

#[kprobe]
pub fn napi_consume_skb_entry(ctx: ProbeContext) -> u32 {
    try_napi_consume_skb(ctx).unwrap_or_else(|_| 0)
}
```

**Nuevo archivo**: [`ebpf-node-ebpf/src/main.rs`](ebpf-node/ebpf-node-ebpf/src/main.rs) (~30 líneas)

```rust
// ebpf-node-ebpf/src/main.rs
#![no_std]
#![no_main]

mod xdp;
mod tracing;
mod common;

#[xdp]
pub fn ebpf_node(ctx: XdpContext) -> u32 {
    xdp::try_xdp_filter(ctx).unwrap_or(xdp_action::XDP_ABORTED)
}

#[kprobe]
pub fn netif_receive_skb(ctx: ProbeContext) -> u32 {
    tracing::try_netif_receive_skb(ctx).unwrap_or_else(|_| 0)
}

#[kprobe]
pub fn napi_consume_skb(ctx: ProbeContext) -> u32 {
    tracing::try_napi_consume_skb(ctx).unwrap_or_else(|_| 0)
}
```

### 5.4 Tareas

1. Crear [`ebpf-node-ebpf/src/xdp.rs`](ebpf-node/ebpf-node-ebpf/src/xdp.rs)
2. Crear [`ebpf-node-ebpf/src/tracing.rs`](ebpf-node/ebpf-node-ebpf/src/tracing.rs)
3. Crear [`ebpf-node-ebpf/src/common.rs`](ebpf-node/ebpf-node-ebpf/src/common.rs) para utilities compartidas
4. Actualizar [`ebpf-node-ebpf/src/main.rs`](ebpf-node/ebpf-node-ebpf/src/main.rs) para re-exportar
5. Actualizar [`ebpf-node-ebpf/src/lib.rs`](ebpf-node/ebpf-node-ebpf/src/lib.rs) si es necesario
6. Compilar y verificar: `cargo build --target bpfel-unknown-none`

### 5.5 Verificación

```bash
cd ebpf-node/ebpf-node-ebpf/
cargo build --target bpfel-unknown-none
cd ../ebpf-node/
cargo build
```

---

## 6. Fase 4: Hot-Reload Architecture

### 6.1 Descripción

Implementar hot-reload para programas eBPF sin reiniciar la aplicación completa.

### 6.2 Estado Actual

No hay hot-reload. Cualquier cambio en los programas eBPF requiere reiniciar la aplicación completa.

### 6.3 Diseño Propuesto

**Archivo**: [`ebpf/hot_reload.rs`](ebpf-node/ebpf-node/src/ebpf/hot_reload.rs)

```rust
// ebpf/hot_reload.rs
use aya::{Ebpf, programs::{Xdp, KProbe, XdpFlags}};
use aya::util::nr_cpus;
use std::sync::{Arc, Mutex};
use std::path::Path;
use anyhow::{Result, Context};

/// Hot-reloadable eBPF manager
pub struct EbpfHotReloadManager {
    inner: Arc<Mutex<Ebpf>>,
    iface: String,
    ifindex: u32,
}

impl EbpfHotReloadManager {
    pub fn new(ebpf: Ebpf, iface: String, ifindex: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ebpf)),
            iface,
            ifindex,
        }
    }

    /// Reload eBPF program (detach old, attach new)
    pub fn reload(&self, new_bytes: &[u8]) -> Result<()> {
        let mut old_ebpf = self.inner.lock().unwrap();
        
        // 1. Save current map FDs (to preserve data)
        let map_fds = self.save_map_fds(&old_ebpf)?;
        
        // 2. Detach all programs
        self.detach_all(&mut old_ebpf)?;
        
        // 3. Load new eBPF object
        let mut new_ebpf = Ebpf::load(new_bytes)
            .context("Failed to load new eBPF object")?;
        
        // 4. Attach new programs
        self.attach_all(&mut new_ebpf)?;
        
        // 5. Swap
        drop(old_ebpf);
        *self.inner.lock().unwrap() = new_ebpf;
        
        Ok(())
    }

    fn save_map_fds(&self, ebpf: &Ebpf) -> Result<Vec<(String, i32)>> {
        let mut fds = Vec::new();
        for map_name in &["NODES_WHITELIST", "NODES_BLACKLIST", "LATENCY_STATS", "START_TIMES"] {
            if let Ok(map) = ebpf.map(map_name) {
                fds.push((map_name.clone(), map.fd().unwrap()));
            }
        }
        Ok(fds)
    }

    fn detach_all(&self, ebpf: &mut Ebpf) -> Result<()> {
        // Detach XDP
        if let Some(xdp) = ebpf.program_mut("ebpf_node") {
            let xdp: &mut Xdp = xdp.try_into()?;
            xdp.detach()?;
        }
        // Detach KProbes
        if let Some(kp) = ebpf.program_mut("netif_receive_skb") {
            let kp: &mut KProbe = kp.try_into()?;
            kp.detach()?;
        }
        if let Some(kp) = ebpf.program_mut("napi_consume_skb") {
            let kp: &mut KProbe = kp.try_into()?;
            kp.detach()?;
        }
        Ok(())
    }

    fn attach_all(&self, ebpf: &mut Ebpf) -> Result<()> {
        // Load all programs first
        if let Some(xdp) = ebpf.program_mut("ebpf_node") {
            let xdp: &mut Xdp = xdp.try_into()?;
            xdp.load()?;
            xdp.attach(&self.iface, XdpFlags::default())?;
        }
        if let Some(kp) = ebpf.program_mut("netif_receive_skb") {
            let kp: &mut KProbe = kp.try_into()?;
            kp.load()?;
            kp.attach("netif_receive_skb", 0)?;
        }
        if let Some(kp) = ebpf.program_mut("napi_consume_skb") {
            let kp: &mut KProbe = kp.try_into()?;
            kp.load()?;
            kp.attach("napi_consume_skb", 0)?;
        }
        Ok(())
    }

    /// Get reference to eBPF for map access
    pub fn ebpf(&self) -> std::sync::MutexGuard<'_, Ebpf> {
        self.inner.lock().unwrap()
    }
}
```

### 6.4 API de Hot-Reload

**Nuevo endpoint HTTP**: `POST /api/v1/ebpf/reload`

```rust
// api/ebpf.rs
use axum::{extract::State, Json, http::StatusCode};
use std::sync::Arc;
use crate::ebpf::hot_reload::EbpfHotReloadManager;

pub async fn reload_handler(
    State(manager): State<Arc<EbpfHotReloadManager>>,
) -> impl axum::response::IntoResponse {
    // Read new eBPF binary from build artifact
    let new_bytes = include_bytes!("/path/to/compiled/ebpf");
    
    match manager.reload(new_bytes) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({
            "status": "reloaded",
        }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": e.to_string(),
        }))),
    }
}
```

### 6.5 Tareas

1. Crear [`ebpf/hot_reload.rs`](ebpf-node/ebpf-node/src/ebpf/hot_reload.rs)
2. Actualizar [`ebpf/mod.rs`](ebpf-node/ebpf-node/src/ebpf/mod.rs)
3. Reemplazar `Ebpf` por `EbpfHotReloadManager` en `NodeState`
4. Crear [`api/ebpf.rs`](ebpf-node/ebpf-node/src/api/ebpf.rs) con endpoint de reload
5. Actualizar `main.rs` para usar `EbpfHotReloadManager`
6. Agregar tests de hot-reload

### 6.6 Verificación

```bash
cd ebpf-node/
cargo build
# Test reload endpoint
curl -X POST http://localhost:9091/api/v1/ebpf/reload
```

---

## 7. Fase 5: Migración a Ringbuf

### 7.1 Descripción

Migrar de `bpf_trace_printk` a `ringbuf` para logging desde el kernel.

### 7.2 Estado Actual

Se usa `aya-log` que depende de `bpf_trace_printk`:

```rust
// En ebpf-node/src/main.rs:1745-1747
if let Err(e) = aya_log::EbpfLogger::init(&mut ebpf) {
    warn!("failed to initialize eBPF logger: {e}");
}
```

**Problema**: `bpf_trace_printk` tiene limitaciones:
- Buffer de solo 1024 bytes
- Formato de string fijo
- Alto overhead de serialización
- No recomendado para producción

### 7.3 Diseño Propuesto

**eBPF side** (`ebpf-node-ebpf/src/xdp.rs`):

```rust
use aya_ebpf::maps::RingBuf;
use aya_ebpf::helpers::bpf_ringbuf_output;

#[map]
static LOG_BUFFER: RingBuf = RingBuf::with_byte_size(1024 * 1024, 0);

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LogRecord {
    pub timestamp: u64,
    pub level: u32,
    pub message_len: u32,
    pub message: [u8; 256],
}

impl LogRecord {
    pub fn new(level: u32, message: &str) -> Self {
        let mut record = Self {
            timestamp: 0, // bpf_ktime_get_ns(),
            level,
            message_len: 0,
            message: [0; 256],
        };
        let bytes = message.as_bytes();
        let len = bytes.len().min(256);
        record.message[..len].copy_from_slice(&bytes[..len]);
        record.message_len = len as u32;
        record
    }
}

pub fn log_xdp_drop(ip: u32) {
    let record = LogRecord::new(1, &format!("Dropping packet from IP {}", ip));
    let _ = unsafe {
        bpf_ringbuf_output(
            &LOG_BUFFER as *const _ as u64,
            &record as *const _ as u64,
            std::mem::size_of::<LogRecord>() as u64,
            0,
        )
    };
}
```

**User-space** (`ebpf/metrics.rs`):

```rust
use aya::maps::RingBuf;

pub fn setup_ringbuf_listener(manager: &EbpfHotReloadManager) -> anyhow::Result<()> {
    let ringbuf = RingBuf::try_from(
        manager.ebpf().map("LOG_BUFFER")
            .context("Failed to get LOG_BUFFER")?
    )?;
    
    let mut builder = ringbuf.builder();
    
    std::thread::spawn(move || {
        for record in builder.pull_records() {
            if let Ok(record) = record {
                let log_record = unsafe { &*(record.data() as *const LogRecord) };
                debug!(
                    timestamp = log_record.timestamp,
                    level = log_record.level,
                    message = std::str::from_utf8_unchecked(&log_record.message[..log_record.message_len as usize]),
                    "eBPF Log"
                );
            }
        }
    });
    
    Ok(())
}
```

### 7.4 Tareas

1. Crear struct `LogRecord` en [`ebpf-node-ebpf/src/common.rs`](ebpf-node/ebpf-node-ebpf/src/common.rs)
2. Reemplazar `aya_log::EbpfLogger::init()` por `RingBuf` en eBPF programs
3. Crear listener de Ringbuf en user-space
4. Actualizar logs en XDP y KProbes para usar `bpf_ringbuf_output`
5. Compilar y verificar

### 7.5 Verificación

```bash
cd ebpf-node/
cargo build
# Verificar logs en syslog/journal
journalctl -u ebpf-node -f | grep "eBPF Log"
```

---

## 8. Fase 6: Migración a CO-RE

### 8.1 Descripción

Implementar CO-RE (Compile Once Run Everywhere) para portabilidad entre kernels.

### 8.2 Requisitos de Sistema

```bash
# Verificar BTF en kernel
ls -la /sys/kernel/btf/vmlinux

# Verificar bpftool
bpftool --version

# Verificar LLVM/clang
clang --version

# Requisitos mínimos
# - Kernel 5.4+ con BTF habilitado
# - LLVM 14+
# - bpftool 5.15+
# - aya 0.12+ con feature "loader"
```

### 8.3 Cambios en Cargo.toml

**Actual** (`ebpf-node/Cargo.toml`):

```toml
[workspace.dependencies]
aya = { git = "https://github.com/aya-rs/aya", default-features = false }
aya-ebpf = { git = "https://github.com/aya-rs/aya", default-features = false }
```

**Después de CO-RE**:

```toml
[workspace.dependencies]
aya = { git = "https://github.com/aya-rs/aya", default-features = false, features = ["loader"] }
aya-ebpf = { git = "https://github.com/aya-rs/aya", default-features = false }
aya-build = { git = "https://github.com/aya-rs/aya", default-features = false }
```

### 8.4 Cambios en Build Script

**Actual** (`ebpf-node/build.rs`):

```rust
aya_build::build_ebpf([ebpf_package], Toolchain::default())
```

**Después de CO-RE**:

```rust
// CO-RE: No se necesita toolchain específica
aya_build::build_ebpf([ebpf_package], None) // None = auto-detect
```

### 8.5 Tareas

1. Verificar que el kernel del target tiene BTF habilitado
2. Actualizar `Cargo.toml` con feature `"loader"`
3. Actualizar `build.rs` para CO-RE
4. Agregar verificación de BTF en `main.rs`:

```rust
fn verify_btf() -> anyhow::Result<()> {
    let btf_path = "/sys/kernel/btf/vmlinux";
    if !std::path::Path::new(btf_path).exists() {
        anyhow::bail!(
            "BTF not found at {}. eBPF CO-RE requires a BTF-enabled kernel (5.4+).",
            btf_path
        );
    }
    info!("BTF verified at {}", btf_path);
    Ok(())
}
```

5. Compilar y verificar en múltiples kernels

### 8.6 Verificación

```bash
cd ebpf-node/
cargo build
# Test en kernel con BTF
./target/debug/ebpf-node --iface eth0
```

---

## 9. Resumen de Fases

### 9.1 Timeline Estimado

| Fase | Tareas | Complejidad | Dependency |
|------|--------|-------------|------------|
| Fase 1: Separación de módulos | 11 tareas | Alta | Ninguna |
| Fase 2: Maps abstraction | 5 tareas | Media | Fase 1 |
| Fase 3: Separación eBPF | 5 tareas | Media | Fase 1 |
| Fase 4: Hot-Reload | 6 tareas | Alta | Fase 1, 2, 3 |
| Fase 5: Ringbuf | 5 tareas | Baja | Ninguna |
| Fase 6: CO-RE | 5 tareas | Media | Ninguna (kernel BTF) |

### 9.2 Critical Path

```
Fase 1 → Fase 2 → Fase 4
    └──→ Fase 3 ────┘
```

### 9.3 Paralelización

```
Fase 1 (Critical Path)
    ├──→ Fase 2 ──→ Fase 4
    ├──→ Fase 3 ──→ Fase 4
    └──→ Fase 5 (Paralelo, no bloquea)

Fase 6 (Paralelo, depende de kernel BTF)
```

### 9.4 Criterios de Aceptación por Fase

| Fase | Criterio de Aceptación |
|------|------------------------|
| Fase 1 | `main.rs` < 200 líneas, `cargo build` pasa |
| Fase 2 | `EbpfMaps` reemplaza todos los `try_from`, tests pasan |
| Fase 3 | XDP y KProbes en archivos separados, compilación eBPF pasa |
| Fase 4 | Hot-reload endpoint funciona sin reiniciar proceso |
| Fase 5 | Logs de eBPF van a ringbuf, `aya_log` eliminado |
| Fase 6 | Binary funciona en kernels diferentes sin recompilar |

---

## Appendix A: Comandos de Verificación

```bash
# Verificar compilación después de cada fase
cd ebpf-node/
cargo build 2>&1 | tee /tmp/refactor-phase-N.log

# Verificar tests
cargo test 2>&1 | tee /tmp/refactor-phase-N-test.log

# Verificar tamaño de main.rs
wc -l ebpf-node/ebpf-node/src/main.rs

# Verificar estructura de archivos
find ebpf-node/ebpf-node/src -name "*.rs" | sort

# Verificar eBPF compilation
cd ebpf-node/ebpf-node-ebpf/
cargo build --target bpfel-unknown-none 2>&1 | tee /tmp/ebpf-build.log
```

## Appendix B: Checklist de Riesgos

- [ ] Cada fase compila independientemente
- [ ] Tests pasan después de cada fase
- [ ] Git branch por fase para revertir si es necesario
- [ ] Diff review antes de merge
- [ ] Documentación actualizada
- [ ] CHANGELOG actualizado
