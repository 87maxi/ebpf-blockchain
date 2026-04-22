# Arquitectura del Framework Aya en ebpf-node

## Tabla de Contenidos

1. [Introducción](#1-introducción)
2. [Fundamentos: eBPF y Linux Kernel](#2-fundamentos-ebpf-y-linux-kernel)
3. [Modelo de Programación Aya](#3-modelo-de-programación-aya)
4. [Arquitectura del Proyecto ebpf-node](#4-arquitectura-del-proyecto-ebpf-node)
5. [Compilación eBPF con Aya-Build](#5-compilación-ebpf-con-aya-build)
6. [Programas eBPF Implementados](#6-programas-ebpf-implementados)
7. [Gestión de Maps BPF](#7-gestión-de-maps-bpf)
8. [Ciclo de Vida de un Programa eBPF](#8-ciclo-de-vida-de-un-programa-ebpf)
9. [Interacción con el Sistema Operativo](#9-interacción-con-el-sistema-operativo)
10. [Observabilidad eBPF](#10-observabilidad-ebpf)
11. [Análisis de Limitaciones Actuales](#11-análisis-de-limitaciones-actuales)
12. [Propuesta de Refactorización Arquitectónica](#12-propuesta-de-refactorización-arquitectónica)
13. [Roadmap de Evolución Aya](#13-roadmap-de-evolución-aya)

---

## 1. Introducción

### 1.1 ¿Qué es Aya?

[Aya](https://github.com/aya-rs/aya) es un framework eBPF escrito íntegramente en Rust que permite desarrollar programas que se ejecutan dentro del kernel de Linux sin necesidad de compilar código C ni depender de bibliotecas externas. Su nombre proviene de "Aya" (明), que significa "claro" o "brillante" en japonés, reflejando su objetivo de proporcionar visibilidad profunda en el sistema.

**Características principales:**
- **Pure Rust**: No requiere bindings FFI para operaciones core
- **Safe by default**: Utiliza el sistema de tipos de Rust para prevenir errores
- **Multi-programa**: Soporta XDP, KProbe, UProbe, CGroup, LSM, TracePoint
- **Hot-reload**: Recarga de programas sin detener la aplicación
- **Observabilidad**: Integración nativa con logging desde el kernel

### 1.2 Versión utilizada en ebpf-node

```toml
# ebpf-node/Cargo.toml
aya = { git = "https://github.com/aya-rs/aya", default-features = false }
aya-build = { git = "https://github.com/aya-rs/aya", default-features = false }
aya-ebpf = { git = "https://github.com/aya-rs/aya", default-features = false }
aya-log = { git = "https://github.com/aya-rs/aya", default-features = false }
aya-log-ebpf = { git = "https://github.com/aya-rs/aya", default-features = false }
```

**Commit actual**: `aa122f319f2c1169d7b97ff4332205eccc09641d`

Esta es una versión desde git main, que corresponde aproximadamente a la serie **0.13.x**.

### 1.3 Contexto del Proyecto

`ebpf-node` es un nodo de blockchain que utiliza eBPF para:
- **Filtrado de red en nivel XDP**: Whitelist/blacklist de IPs con latencia sub-microsegundo
- **Medición de latencia de red**: KProbes en `netif_receive_skb` y `napi_consume_skb`
- **Consenso P2P**: Implementación libp2p con gossipsub
- **Protección replay**: Nonce incremental con RocksDB

---

## 2. Fundamentos: eBPF y Linux Kernel

### 2.1 ¿Qué es eBPF?

eBPF (extended Berkeley Packet Filter) es un subsistema del kernel Linux que permite ejecutar programas sandboxed en tiempo de ejecución sin modificar el código fuente del kernel o recargar módulos.

#### Evolución histórica

```
2014  eBPF introducido (kernel 3.18)    - Filtrado de paquetes
2015  KProbes eBPF                      - Tracing de funciones kernel
2016  SOCKMAP / SOCKCGROUP              - Gestión de sockets
2017  XDP (eXpress Data Path)           - Procesamiento en driver NIC
2018  LSM eBPF                          - Security modules
2019  CO-RE (Compile Once Run Everywhere) - Portabilidad
2020  Sleepable KProbes                 - Kprobes que pueden dormir
2021  Extensions BPF                    - Reusabilidad de helpers
2022  BPF Collateral                  - Mejoras de seguridad
2023  BPF for cgroup sockopt            - Control de sockets
2024  BPF Luna / Veristand            - Verificación avanzada
```

### 2.2 El Virtual Machine eBPF (eBPF VM)

El eBPF VM es una máquina virtual de pila (stack-based) con 11 registros:

```
┌──────────┬──────────────────────────────────────────┐
│ Registro │ Descripción                            │
├──────────┼──────────────────────────────────────────┤
│ r0       │ Accumulator (return value)               │
│ r1-r5    │ Argument registers (call convention)      │
│ r6-r9    │ Callee-saved registers (preserve value)  │
│ r10      │ Read-only frame pointer                  │
└──────────┴──────────────────────────────────────────┘
```

**Características del eBPF VM:**
- **Programas de 512 instructions máximo**
- **Verificador (verifier)**: Analiza el programa antes de cargarlo
- **JIT Compilation**: Compila a código nativo del CPU
- **Helpers**: ~60 funciones provistas por el kernel

### 2.3 BPF Syscall Interface

La interfaz entre user-space y kernel se realiza via syscalls:

```c
// bpf(2) syscall
long bpf(enum BPF_CMD cmd, union bpf_attr *attr);

// Comandos principales:
// BPF_PROG_LOAD    - Cargar un programa eBPF
// BPF_MAP_CREATE   - Crear un map BPF
// BPF_MAP_UPDATE   - Actualizar un map
// BPF_MAP_LOOKUP   - Leer un map
// BPF_PROG_ATTACH  - Adjuntar programa a hook
// BPF_PROG_DETACH  - Desadjuntar programa
```

### 2.4 XDP (eXpress Data Path)

XDP es el hook más temprano disponible en el stack de red de Linux:

```
                    ┌─────────────────────────────────────┐
                    │         Linux Network Stack         │
                    │-------------------------------------│
Packet Flow:       │
                    │
  NIC Driver ─────>│ [XDP Hook] <─── eBPF XDP Program   │
                    │      │                              │
                    │      v                              │
                    │  Skbuff Allocation                  │
                    │      │                              │
                    │      v                              │
                    │  PTP / TC / Bridge / Routing        │
                    │      │                              │
                    │      v                              │
                    │  Netfilter (pre-routing)            │
                    │      │                              │
                    │      v                              │
                    │  Socket Receive                     │
                    │                                     │
                    └─────────────────────────────────────┘

XDP Actions:
  XDP_PASS    → Continuar stack normal
  XDP_DROP    → Descartar packet
  XDP_TX      → Re-enviar packet (LBM)
  XDP_REDIRECT → Enviar a otra NIC/queue
  XDP_ABORTED → Error (cuenta en stats)
```

**Niveles de procesamiento:**
1. **XDP drv** (default): Procesado en driver, sin SKB allocation
2. **XDP hw**: Offload a NIC (programable switches)
3. **XDP native**: Usando AF_XDP socket

---

## 3. Modelo de Programación Aya

### 3.1 Estructura de Crates

```
aya-rs/aya
├── aya           ← User-space library (main)
├── aya-build     ← Build-time helpers
├── aya-ebpf      ← eBPF-side library (no_std)
│   ├── aya-ebpf-bindings
│   ├── aya-ebpf-cty
│   └── aya-ebpf-macros
├── aya-log       ← User-space log consumer
│   ├── aya-log-common
│   ├── aya-log-ebpf      ← eBPF-side logging macros
│   ├── aya-log-ebpf-macros
│   └── aya-log-parser
└── aya-obj       ← BPF object file parser
```

### 3.2 Programa eBPF (Kernel Space)

```rust
// ebpf-node-ebpf/src/main.rs
#![no_std]        // Sin runtime de Rust standard
#![no_main]       // Sin main() - entry point son los macros

use aya_ebpf::{
    bindings::{BPF_F_NO_PREALLOC, xdp_action},
    helpers::bpf_ktime_get_ns,
    macros::{kprobe, map, xdp},           ← Macros que generan código BPF
    maps::{HashMap, LruHashMap, lpm_trie::LpmTrie},
    programs::{ProbeContext, XdpContext},
};
```

**Programas soportados por aya-ebpf:**

| Macro       | Programa       | Descripción                    |
|-------------|----------------|--------------------------------|
| `#[xdp]`    | Xdp            | Packet filtering at NIC       |
| `#[kprobe]` | KProbe         | Kernel function entry trace   |
| `#[kretprobe]`| KRetProbe     | Kernel function return trace  |
| `#[uprobe]` | UProbe         | User function entry trace     |
| `#[uretprobe]`| URetProbe     | User function return trace    |
| `#[tracepoint]`| TracePoint   | Kernel tracepoint handler     |
| `#[cgroup_skb]`| CgroupSkb   | Cgroup packet filtering       |
| `#[sock_ops]` | SockOps      | Socket operation tracing      |
| `#[sk_skb]`   | SkSkb          | Socket/kernel skb filtering   |
| `#[lsm]`      | Lsm            | Linux Security Module         |

### 3.3 Programa User-Space

```rust
// ebpf-node/src/main.rs
use aya::{
    maps::{HashMap, LpmTrie},
    programs::{KProbe, Xdp, XdpFlags},
    Ebpf,
};
```

**Programas soportados por aya (user-space):**

| Type        | Rust Type              | Description         |
|-------------|------------------------|---------------------|
| Xdp         | `aya::programs::Xdp`   | eXpress Data Path   |
| KProbe      | `aya::programs::KProbe`| Kernel probe        |
| KRetProbe   | `aya::programs::KProbe`| Kernel return probe |
| UProbe      | `aya::programs::Uprobe`| User probe          |
| URetProbe   | `aya::programs::Uprobe`| User return probe   |
| TracePoint  | `aya::programs::TracePoint`| Kernel tracepoint |
| CgroupSkb   | `aya::programs::CgroupSkb`| Cgroup packet    |
| SockOps     | `aya::programs::SockOps`| Socket operations |
| SkSkb       | `aya::programs::SkSkb` | Socket/SKB filter   |
| Lsm         | `aya::programs::Lsm`   | Security module     |
| SchedClassifier | `aya::programs::SchedClassifier`| TC filter |

---

## 4. Arquitectura del Proyecto ebpf-node

### 4.1 Estructura del Workspace

```
ebpf-node/
├── Cargo.toml                 ← Workspace manifest
├── ebpf-node/                 ← User-space application
│   ├── build.rs               ← Build script (compila eBPF)
│   ├── Cargo.toml
│   └── src/
│       └── main.rs            ← Aplicación principal
├── ebpf-node-ebpf/            ← eBPF programs
│   ├── build.rs               ← Detecta bpf-linker
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs            ← XDP + KProbes programs
│       └── lib.rs             ← Library target
└── ebpf-node-common/          ← Shared types
    ├── Cargo.toml
    └── src/
        └── lib.rs             ← Empty (features: user)
```

### 4.2 Diagrama de Arquitectura

```
┌─────────────────────────────────────────────────────────────────────┐
│                    EBPF-NODE USER-SPACE                             │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                     Axum HTTP Server                          │ │
│  │  /health  /metrics  /api/v1/*  /rpc  /ws                      │ │
│  └───────────────────────────┬───────────────────────────────────┘ │
│                              │                                     │
│  ┌───────────────────────────┴───────────────────────────────────┐ │
│  │                     libp2p Swarm                              │ │
│  │  gossipsub  identify  mdns  request_response                  │ │
│  └───────────────────────────┬───────────────────────────────────┘ │
│                              │                                     │
│  ┌───────────────────────────┴───────────────────────────────────┐ │
│  │                    Node State Manager                         │ │
│  │  PeerStore  ReplayProtection  SybilProtection                 │ │
│  └───────────────────────────┬───────────────────────────────────┘ │
│                              │                                     │
│  ┌───────────────────────────┴───────────────────────────────────┐ │
│  │                    Aya Ebpf Instance                          │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │ │
│  │  │ XDP Program │  │ KProbe In   │  │ KProbe Out  │          │ │
│  │  │  (ebpf_node)│  │(netif_*)    │  │(napi_*)     │          │ │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │ │
│  │         │                │                │                  │ │
│  │  ┌──────▼────────────────▼────────────────▼──────┐          │ │
│  │  │              BPF Maps                          │          │ │
│  │  │  NODES_WHITELIST  NODES_BLACKLIST             │          │ │
│  │  │  LATENCY_STATS  START_TIMES                   │          │ │
│  │  └───────────────────────────────────────────────┘          │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                  Prometheus Metrics                           │ │
│  │  XDP_PACKETS  LATENCY_BUCKETS  PEERS_CONNECTED  etc.          │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                    RocksDB                                    │ │
│  │  PeerStore  ReplayProtection  SybilProtection                 │ │
│  └───────────────────────────────────────────────────────────────┘ │
└───────────────────────────────────┬─────────────────────────────────┘
                                    │
                                    │ bpf(2) syscalls
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      LINUX KERNEL                                   │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                    eBPF Verifier                              │ │
│  │  - Validates safety                                           │ │
│  │  - Checks bounds                                              │ │
│  │  - Ensures termination                                        │ │
│  └───────────────────────────┬───────────────────────────────────┘ │
│                              │ loaded                             │
│  ┌───────────────────────────▼───────────────────────────────────┐ │
│  │                  eBPF JIT Compiler                            │ │
│  │  Compiles eBPF bytecode to native CPU instructions            │ │
│  └───────────────────────────┬───────────────────────────────────┘ │
│                              │ attached                           │
│  ┌───────────────────────────▼───────────────────────────────────┐ │
│  │              Network Stack Hooks                              │ │
│  │                                                               │ │
│  │  [XDP Hook] ─── ebpf_node() ──→ XDP_PASS / XDP_DROP          │ │
│  │         │                                                     │ │
│  │  [netif_receive_skb] ─── netif_receive_skb() ──→ record start │ │
│  │         │                                                     │ │
│  │  [napi_consume_skb] ─── napi_consume_skb() ──→ calc latency   │ │
│  └───────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.3 Dependencias entre Crates

```
ebpf-node (user-space)
  ├── aya              ← Load programs, manage maps
  ├── aya-log          ← Consume eBPF logs
  ├── ebpf-node-ebpf   ← [build-dependency] compiled eBPF binary
  └── aya-build        ← Build helper

ebpf-node-ebpf (kernel-space)
  ├── aya-ebpf         ← Macros y abstractions for eBPF
  ├── aya-log-ebpf     ← Logging macros for eBPF
  └── network-types    ← Packet header parsing

ebpf-node-common
  └── aya (optional)   ← Shared types when "user" feature enabled
```

---

## 5. Compilación eBPF con Aya-Build

### 5.1 Pipeline de Compilación

```
┌─────────────────────────────────────────────────────────────────┐
│ BUILD SCRIPT PIPELINE                                           │
│                                                                 │
│  cargo build (ebpf-node)                                        │
│       │                                                       │
│       ▼                                                       │
│  ebpf-node/build.rs                                           │
│       │                                                       │
│       ├── cargo_metadata       ← Encuentra ebpf-node-ebpf     │
│       └── aya_build::build_ebpf  ← Compila eBPF               │
│              │                                                │
│              ▼                                               │
│  ebpf-node-ebpf/build.rs                                      │
│       │                                                       │
│       └── which("bpf-linker")   ← Cache key para rebuild      │
│              │                                                │
│              ▼                                               │
│  Cross-compilation Toolchain                                  │
│       │                                                       │
│       ├── rustc (nightly with rust-src)                       │
│       ├── LLVM/Clang (for BPF target)                         │
│       ├── bpf-linker (BPF ELF linker)                         │
│       └── bpftool (verification)                              │
│              │                                                │
│              ▼                                               │
│  Output: target/ebpf-node (ELF BPF object)                    │
│       │                                                       │
│       └── include_bytes_aligned!() en build.rs                │
│              │                                                │
│              ▼                                               │
│  Final binary con eBPF embebido en .rodata                    │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 Configuración de Toolchain

```bash
# Requisitos
rustup toolchain install stable
rustup toolchain install nightly --component rust-src
cargo install bpf-linker
```

### 5.3 Build Script Detalle

```rust
// ebpf-node/build.rs
use aya_build::Toolchain;

fn main() -> anyhow::Result<()> {
    // 1. Encontrar el package ebpf-node-ebpf
    let ebpf_package = /* ... */;
    
    // 2. Configurar package para aya-build
    let ebpf_package = aya_build::Package {
        name: "ebpf-node-ebpf",
        root_dir: "/path/to/ebpf-node-ebpf",
        ..Default::default()
    };
    
    // 3. Compilar eBPF
    aya_build::build_ebpf([ebpf_package], Toolchain::default())
}
```

**Proceso de `aya_build::build_ebpf`:**
1. Establece target `bpf` (`bpfel-unknown-none` / `bpfeb-unknown-none`)
2. Activa `build-std` para compilar `aya-ebpf` sin standard library
3. Invoca `rustc` con flags BPF
4. Link con `bpf-linker`
5. Genera ELF BPF listo para cargar

### 5.4 Profile de Compilación eBPF

```toml
# Cargo.toml workspace
[profile.release.package.ebpf-node-ebpf]
debug = 2                  ← Debug info para symbol naming
codegen-units = 1          ← Single CG para optimizations
```

---

## 6. Programas eBPF Implementados

### 6.1 Programa XDP: `ebpf_node`

```rust
// ebpf-node-ebpf/src/main.rs:40-88

#[xdp]
pub fn ebpf_node(ctx: XdpContext) -> u32 {
    match try_ebpf_node(ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

fn try_ebpf_node(ctx: XdpContext) -> Result<u32, ()> {
    // 1. Parsear Ethernet header
    let ethhdr: *const EthHdr = unsafe { ptr_at(&ctx, 0)? };
    match unsafe { (*ethhdr).ether_type } {
        EtherType::Ipv4 => {}
        _ => return Ok(xdp_action::XDP_PASS),  // Solo IPv4
    }

    // 2. Parsear IPv4 header
    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(&ctx, 34)? };
    let source_addr = unsafe { (*ipv4hdr).src_addr };  // Big-endian u32

    // 3. Check blacklist (reactive)
    let blacklist_key = Key::new(32, source_addr);
    if NODES_BLACKLIST.get(&blacklist_key).is_some() {
        return Ok(xdp_action::XDP_DROP);
    }

    // 4. Check whitelist (preventive)
    let whitelist_key = Key::new(32, source_addr);
    if NODES_WHITELIST.get(&whitelist_key).is_none() {
        return Ok(xdp_action::XDP_DROP);  // Default deny
    }

    // 5. IP approved
    Ok(xdp_action::XDP_PASS)
}
```

**Análisis de Seguridad:**

```
Whitelist-First Strategy:
┌─────────────────────────────────────────────────────┐
│ 1. Recibe packet                                     │
│    ↓                                                 │
│ 2. Es IPv4? ─No──→ XDP_PASS                         │
│    ↓ Sí                                             │
│ 3. Está en BLACKLIST? ─Sí──→ XDP_DROP  (reactive)  │
│    ↓ No                                             │
│ 4. Está en WHITELIST? ─No──→ XDP_DROP  (preventive)│
│    ↓ Sí                                             │
│ 5. XDP_PASS (allowed)                               │
└─────────────────────────────────────────────────────┘

Modelo: Default Deny + Whitelist + Reactive Blacklist
```

**Estructuras de Packet:**

```
Ethernet Header (14 bytes):
┌──────────────────┬──────────────────┬────────────┐
│ Dest MAC (6B)    │ Source MAC (6B)  │ Type (2B)  │
└──────────────────┴──────────────────┴────────────┘
                                              │
                                              ▼
                                              EtherType = 0x0800 (IPv4)

IPv4 Header (20+ bytes):
┌──────────────────────────────────────────────────┐
│ Version (4) │ IHL (4) │ DSCP (8) │ Total Len (16)│
│ Identification (16) │ Flags (3) │ Frag (13)      │
│ TTL (8) │ Protocol (8) │ Header Checksum (16)    │
│ Source IP (32) │ Dest IP (32)                    │
│ Options (variable)                                │
└──────────────────────────────────────────────────┘
```

### 6.2 KProbe: `netif_receive_skb`

```rust
// ebpf-node-ebpf/src/main.rs:90-106

#[kprobe]
pub fn netif_receive_skb(ctx: ProbeContext) -> u32 {
    let _ = try_netif_receive_skb(ctx);
    0
}

fn try_netif_receive_skb(ctx: ProbeContext) -> Result<(), ()> {
    // Primer argumento: pointer a struct sk_buff
    let skb_ptr: u64 = ctx.arg(0).ok_or(())?;
    let start_time = unsafe { bpf_ktime_get_ns() };

    // Guardar timestamp en START_TIMES map
    START_TIMES
        .insert(&skb_ptr, &start_time, 0)
        .map_err(|_| ())?;
    Ok(())
}
```

**Propósito:** Medir tiempo de entrada de packet al stack

```
Timeline de procesamiento:

netif_receive_skb()         napi_consume_skb()
      │                           │
      ▼                           ▼
  [Kernel]                  [Kernel/NAPI]
  Packet entra al           Packet consumido
  stack de red              por application
      │                           │
      ▼                           ▼
  t = bpf_ktime_get_ns()    latency = end - start
  START_TIMES[skb] = t      LATENCY_STATS[bucket]++
```

### 6.3 KProbe: `napi_consume_skb`

```rust
// ebpf-node-ebpf/src/main.rs:108-135

#[kprobe]
pub fn napi_consume_skb(ctx: ProbeContext) -> u32 {
    let _ = try_napi_consume_skb(ctx);
    0
}

fn try_napi_consume_skb(ctx: ProbeContext) -> Result<(), ()> {
    let skb_ptr: u64 = ctx.arg(0).ok_or(())?;

    if let Some(start_time) = unsafe { START_TIMES.get(&skb_ptr) } {
        let end_time = unsafe { bpf_ktime_get_ns() };
        let latency = end_time.saturating_sub(*start_time);

        // Calcular bucket de latencia (power-of-2)
        let bucket = 64 - latency.leading_zeros() as u64;

        // Incrementar contador
        let count = unsafe { LATENCY_STATS.get(&bucket).copied().unwrap_or(0) };
        LATENCY_STATS
            .insert(&bucket, &(count + 1), 0)
            .map_err(|_| ())?;

        // Cleanup
        let _ = START_TIMES.remove(&skb_ptr);
    }

    Ok(())
}
```

**Histograma de Latencia:**

```
LATENCY_STATS buckets (power-of-2):

Bucket (64-log)  Latency Range    Count
─────────────────────────────────────────
0                1 ns             ██████
1                2 ns             ███
2                4 ns             ██████████
...
10               1024 ns (1us)    ███████
...
20               1ms              ██
...
30               1s               ░
...
63               >8 billion ns   ░

Format: Key = bucket index, Value = count
```

---

## 7. Gestión de Maps BPF

### 7.1 Maps Definidos en ebpf-node

```rust
// ebpf-node-ebpf/src/main.rs

/// Whitelist: Longest Prefix Match Trie
#[map]
static NODES_WHITELIST: LpmTrie<u32, u32> = 
    LpmTrie::with_max_entries(1024, BPF_F_NO_PREALLOC);

/// Blacklist: Longest Prefix Match Trie  
#[map]
static NODES_BLACKLIST: LpmTrie<u32, u32> = 
    LpmTrie::with_max_entries(10240, BPF_F_NO_PREALLOC);

/// Histograma de latencia
#[map]
static LATENCY_STATS: HashMap<u64, u64> = 
    HashMap::with_max_entries(64, 0);

/// Tiempos temporales por skb pointer
#[map]
static START_TIMES: LruHashMap<u64, u64> = 
    LruHashMap::with_max_entries(10240, 0);
```

### 7.2 Tipos de Maps Utilizados

```
┌──────────────────────────────────────────────────────────────┐
│ MAP TIPE  │  eBPF Side              │  User Side (Aya)      │
├───────────┼─────────────────────────┼───────────────────────┤
│ LpmTrie   │ aya_ebpf::maps::LpmTrie │ aya::maps::LpmTrie   │
│           │ Key = (prefix_len, ip)  │ LpmTrie<u32, u32, u32│
│           │ Value = u32 (1=blocked) │                     │
│           │ Max = 1024/10240        │                     │
│           │ Flag = BPF_F_NO_PREALLOC│                     │
├───────────┼─────────────────────────┼───────────────────────┤
│ HashMap   │ aya_ebpf::maps::HashMap │ aya::maps::HashMap   │
│           │ Key = u64 (bucket)      │ HashMap<u64, u64>   │
│           │ Value = u64 (count)     │                     │
│           │ Max = 64                │                     │
├───────────┼─────────────────────────┼───────────────────────┤
│ LruHashMap│ aya_ebpf::maps::LruHashMap│ aya::maps::LruHashMap│
│           │ Key = u64 (skb_ptr)     │ LruHashMap<u64, u64>│
│           │ Value = u64 (timestamp) │                   │
│           │ Max = 10240             │                     │
└───────────┴─────────────────────────┴───────────────────────┘
```

### 7.3 Acceso a Maps desde User-Space

```rust
// ebpf-node/src/main.rs:2035-2059 (stats update loop)

// Lectura de LATENCY_STATS
if let Ok(latency_stats) = HashMap::<_, u64, u64>::try_from(
    ebpf.map("LATENCY_STATS").unwrap()
) {
    let mut total_packets: u64 = 0;
    for entry in latency_stats.iter() {
        if let Ok((_, count)) = entry {
            total_packets = total_packets.saturating_add(count);
        }
    }
    XDP_PACKETS_PROCESSED.set(total_packets as i64);
    
    for i in 0..64 {
        if let Ok(count) = latency_stats.get(&i, 0u64) {
            LATENCY_BUCKETS.with_label_values(&[&i.to_string()])
                .set(count as i64);
        }
    }
}

// Lectura de blacklist whitelist sizes
if let Ok(blacklist) = LpmTrie::<_, u32, u32, u32>::try_from(
    ebpf.map("NODES_BLACKLIST").unwrap()
) {
    let blacklist_size = blacklist.iter().count();
    XDP_BLACKLIST_SIZE.set(blacklist_size as i64);
}

// Escritura reactiva (desde gossip handler)
if let Ok(mut blacklist) = LpmTrie::<_, u32, u32, u32>::try_from(
    ebpf.map_mut("NODES_BLACKLIST").unwrap()
) {
    blacklist.insert(&key, 1, 0)?;
}
```

### 7.4 LpmTrie (Longest Prefix Match)

LpmTrie es esencial para IP filtering:

```
LpmTrie<Key = (prefix_length, ip_address), Value = u32>

Ejemplo:
Key: (32, 0xC0A80001) = (32, 192.168.0.1) → Value: 1
Key: (24, 0xC0A80000) = (24, 192.168.0.0) → Value: 1
Key: (16, 0xC0A80000) = (16, 192.168.0.0) → Value: 1

Búsqueda de 192.168.0.1:
  - Coincidencias: /32, /24, /16
  - Longest match = /32
  - Value = 1 → IP whitelisted
```

---

## 8. Ciclo de Vida de un Programa eBPF

### 8.1 Lifecycle en ebpf-node

```rust
// ebpf-node/src/main.rs:1740-1765

// 1. CARGA: Leer eBPF object embebido
let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
    env!("OUT_DIR"),
    "/ebpf-node"
)))?;

// 2. LOGGER: Inicializar aya-log para logs desde kernel
if let Err(e) = aya_log::EbpfLogger::init(&mut ebpf) {
    warn!("failed to initialize eBPF logger: {e}");
}

// 3. XDP LOAD + ATTACH
let xdp_program: &mut Xdp = ebpf
    .program_mut("ebpf_node")
    .unwrap()
    .try_into()?;
xdp_program.load()?;
xdp_program.attach(&opt.iface, XdpFlags::default())?;

// 4. KPROBE IN LOAD + ATTACH
let kprobe_in: &mut KProbe = ebpf
    .program_mut("netif_receive_skb")
    .unwrap()
    .try_into()?;
kprobe_in.load()?;
kprobe_in.attach("netif_receive_skb", 0)?;

// 5. KPROBE OUT LOAD + ATTACH
let kprobe_out: &mut KProbe = ebpf
    .program_mut("napi_consume_skb")
    .unwrap()
    .try_into()?;
kprobe_out.load()?;
kprobe_out.attach("napi_consume_skb", 0)?;

// 6. RUN LOOP... (ebpf instanc persista)

// 7. DROP: Cuando `ebpf` sale de scope, todos los programas
//    se desadjuntan y maps se liberan automáticamente
```

### 8.2 Etapa Detallada

```
┌──────────────────────────────────────────────────────────────────┐
│ ETAPA 1: Ebpf::load()                                            │
│                                                                  │
│  - Lee ELF BPF object                                           │
│  - Parsea sections:                                            │
│    .elf_header, .section_header_table                           │
│    .prog (programas eBPF)                                       │
│    .map (maps BPF)                                              │
│    .license, .version, .btf, .rel                               │
│  - Crea maps en kernel via BPF_MAP_CREATE                       │
│  - Retorna Ebpf instance con maps + programs references         │
├──────────────────────────────────────────────────────────────────┤
│ ETAPA 2: program_mut().try_into()                                │
│                                                                  │
│  - Busca program por nombre en ELF                              │
│  - Convierte BpfProgram → Xdp / KProbe / etc.                  │
│  - Type-safe conversion                                         │
├──────────────────────────────────────────────────────────────────┤
│ ETAPA 3: program.load()                                          │
│                                                                  │
│  - Envía BPF_PROG_LOAD syscall                                  │
│  - Kernel:                                                      │
│    a. Parsea eBPF instructions                                  │
│    b. Ejecuta VERIFIER                                           │
│    c. Compila JIT (if supported)                                │
│    d. Asigna bpf_prog struct en kernel                          │
│  - Retorna FD de programa                                       │
├──────────────────────────────────────────────────────────────────┤
│ ETAPA 4: program.attach()                                        │
│                                                                  │
│  - XDP: BPF_PROG_ATTACH + ifindex                              │
│  - KProbe: BPF_PROG_LOAD + bpf_syscall (kprobe_init)          │
│  - Register handler en kernel hook point                       │
└──────────────────────────────────────────────────────────────────┘
```

### 8.3 Verifier BPF

El verifier es el componente más crítico de seguridad:

```
Verifier Checks:
├── Control Flow
│   ├── Must terminate (no infinite loops)
│   ├── No back-edges that could cause loops
│   └── All paths must return
├── Memory Safety
│   ├── Packet access bounds checking
│   ├── Map access validation
│   └── Pointer arithmetic validation
├── Register Types
│   ├── SCALAR_VALUE
│   ├── PTR_TO_PACKET
│   ├── PTR_TO_PACKET_END
│   ├── PTR_TO_MAP_VALUE
│   ├── PTR_TO_BTF_ID
│   └── ...
├── Helper Calls
│   ├── Valid helper function
│   ├── Correct argument types
│   └── Allowed argument ranges
└── Return Values
    ├── Programs return specific types
    ├── XDP returns xdp_action
    └── KProbe returns int
```

---

## 9. Interacción con el Sistema Operativo

### 9.1 Syscalls BPF Utilizadas

```
┌─────────────────────────────────────────────────────────────────┐
│ SYSCALL        │ USAGE IN EBPF-NODE                            │
├────────────────┼───────────────────────────────────────────────┤
│ BPF_MAP_CREATE │ Loading eBPF object - creates all maps        │
│ BPF_PROG_LOAD  │ Loading XDP and KProbe programs               │
│ BPF_PROG_ATTACH│ Attaching XDP to network interface            │
│ BPF_MAP_UPDATE │ Inserting into NODES_BLACKLIST (reactive)     │
│ BPF_MAP_LOOKUP │ Reading LATENCY_STATS, whitelist, blacklist   │
│ BPF_OBJ_GET    │ (potentially) Getting existing maps           │
└────────────────┴───────────────────────────────────────────────┘
```

### 9.2 Permisiones Requeridas

```bash
# Capabilities necesarias
CAP_BPF          ← bpf(2) syscall
CAP_NET_ADMIN    ← attach XDP, manage interfaces
CAP_SYS_ADMIN    ← (sometimes required for certain map types)

# En ebpf-node/main.rs:1651-1658
let rlim = libc::rlimit {
    rlim_cur: libc::RLIM_INFINITY,
    rlim_max: libc::RLIM_INFINITY,
};
unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
```

### 9.3 Kernel Requirements

```
Minimum Kernel: 5.4 (for CO-RE)
Recommended:    5.15+ (full feature set)

Required Features:
├── eBPF verifier
├── XDP support
├── KProbe support
├── BPF helper: bpf_ktime_get_ns
├── BPF map types: HashMap, LpmTrie, LruHashMap
└── BPF linker: bpf-linker
```

### 9.4 Interacción con Network Stack

```
┌──────────────────────────────────────────────────────────────────┐
│                    NETWORK PACKET FLOW                            │
│                                                                  │
│  NIC Driver                                                      │
│     │                                                           │
│     ▼                                                           │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ XDP HOOK (ebpf_node)                                      │  │
│  │   - Check BLACKLIST                                       │  │
│  │   - Check WHITELIST                                       │  │
│  │   - Action: PASS / DROP                                   │  │
│  └───────────────────────────────────────────────────────────┘  │
│     │ XDP_PASS                                                   │
│     ▼                                                           │
│  napi_schedule()  ← Schedule NAPI polling                       │
│     │                                                           │
│     ▼                                                           │
│  netif_receive_skb() ──→ KProbe: netif_receive_skb()           │
│     │  Record start_time in START_TIMES                         │
│     ▼                                                           │
│  Process packet (L2/L3/L4 processing)                          │
│     │                                                           │
│     ▼                                                           │
│  napi_consume_skb() ──→ KProbe: napi_consume_skb()            │
│     │  Calculate latency = end_time - start_time               │
│     │  Update LATENCY_STATS histogram                           │
│     ▼                                                           │
│  Deliver to socket / discard                                    │
└──────────────────────────────────────────────────────────────────┘
```

---

## 10. Observabilidad eBPF

### 10.1 aya-log Pipeline

```
┌──────────────────────────────────────────────────────────────────┐
│                    AYA-LOG ARCHITECTURE                           │
│                                                                  │
│  eBPF Side                       User Side                       │
│  ──────────                       ────────                       │
│                                                                  │
│  info!(&ctx, "message: {}", val)  EbpfLogger::init(&mut ebpf)   │
│       │                                │                         │
│       ▼                                ▼                         │
│  bpf_trace_printk()              Ring Buffer Map                │
│  (5 x 16 byte buffers)           (automatically created)        │
│       │                                │                         │
│       └─────────── BPF_MAP_LOOKUP ─────┘                         │
│                                │                                 │
│                                ▼                                 │
│                          log::info!()                            │
│                          (via env_logger / tracing)              │
└──────────────────────────────────────────────────────────────────┘
```

**Limitación actual**: `bpf_trace_printk()` está limitado a 5 buffers de 16 bytes.
Para producción, usar anillos de eventos BPF (BPF ringbuf) que son más flexibles.

### 10.2 Prometheus Metrics Integration

```rust
// ebpf-node/src/main.rs:197-238

// Metrics eBPF-specific:
static ref XDP_PACKETS_PROCESSED: IntGauge = ...;
static ref XDP_PACKETS_DROPPED: IntGauge = ...;
static ref XDP_BLACKLIST_SIZE: IntGauge = ...;
static ref XDP_WHITELIST_SIZE: IntGauge = ...;
static ref EBPF_ERRORS: IntCounter = ...;
static ref LATENCY_BUCKETS: IntGaugeVec = ...;

// Update loop (cada 10 segundos):
tokio::select! {
    _ = stats_interval.tick() => {
        // Read from BPF maps
        let latency_stats = HashMap::try_from(ebpf.map("LATENCY_STATS")?);
        // Update Prometheus gauges
        for (bucket, count) in latency_stats.iter() {
            LATENCY_BUCKETS.with_label_values(&[bucket]).set(count);
        }
    }
}
```

---

## 11. Análisis de Limitaciones Actuales

### 11.1 Problemas Identificados

#### 11.1.1 Arquitectura Monolítica

**Problema**: Todo el código en un solo archivo [`main.rs`](ebpf-node/ebpf-node/src/main.rs:1) de 2406 líneas.

```
ebpf-node/src/main.rs:
├── imports (1-38)
├── lazy_static metrics (40-239)
├── data structures (240-498)
├── helper functions (500-575)
├── API handlers (577-1041)
├── metrics functions (1043-1238)
├── CLI options (1240-1293)
├── PeerStore (1295-1350)
├── ReplayProtection (1352-1461)
├── SybilProtection (1463-1597)
├── libp2p Behaviour (1599-1605)
├── logging setup (1607-1634)
├── main() function (1636-2396)
└── utility functions (2398-2405)
```

**Impacto**:
- Dificulta navegación y mantenimiento
- Compilación incremental más lenta
- Diff y code review complejos

#### 11.2.2 Gestión de Maps sin Abstraction

**Problema**: Acceso directo a maps sin capa de abstracción.

```rust
// Actual: disperso y sin统一管理
// En stats loop:
ebpf.map("LATENCY_STATS").unwrap()
ebpf.map("NODES_BLACKLIST").unwrap()
ebpf.map("NODES_WHITELIST").unwrap()

// En gossip handler:
ebpf.map_mut("NODES_BLACKLIST").unwrap()
```

**Impacto**:
- Typos pueden causar panics en runtime
- Sin type safety para map access
- Dificulta hot-reload de maps

#### 11.3.3 eBPF Object Embebido

**Problema**: Uso de `include_bytes_aligned!` sin verificación de integridad.

```rust
let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
    env!("OUT_DIR"),
    "/ebpf-node"
)))?;
```

**Impacto**:
- No se puede verificar signature del eBPF binary
- No hay separación clara entre user/kernel code
- Debugging de eBPF object embebido es difícil

#### 11.4.4 Sin CO-RE (Compile Once Run Everywhere)

**Problema**: Dependencia de toolchain específica sin portabilidad CO-RE.

```toml
# No se usan BTF (BPF Type Format)
# Se compila directamente sin relocate
```

**Impacto**:
- Requiere nightly toolchain con rust-src
- No portable entre versiones de kernel
- Necesita cross-compilation manual

#### 11.5.5 Ebpf Instance Persistida Todo el Runtime

**Problema**: `ebpf` variable vive hasta el final de `main()`.

```rust
// main() → let mut ebpf = ...; → loop → Ok(())
// ebpf no puede ser movido o clonado
```

**Impacto**:
- No hay hot-reload de programas
- No se puede actualizar eBPF sin reiniciar
- Memory leak potencial si hay references cíclicas

#### 11.6.6 KProbes sin Fentry/Fexit

**Problema**: Uso de KProbes en lugar de fentry/fexit (más eficiente).

```rust
// Actual: KProbes (general purpose)
#[kprobe]
pub fn netif_receive_skb(ctx: ProbeContext) -> u32

// Mejor: fentry (optimized, no context parsing needed)
#[bpf_program]
fn netif_receive_skb_fentry(ctx: bpf_context::netif_receive_skb) -> u64
```

**Impacto**:
- KProbes tienen más overhead que fentry
- Context parsing manual (`ctx.arg(0)`)
- Menos seguro (no type-checked arguments)

### 11.7 Resumen de Limitaciones

| # | Limitación | Severidad | Complejidad Fix |
|---|-----------|-----------|-----------------|
| 1 | Monolito main.rs | Media | Baja |
| 2 | Sin abstraction para maps | Media | Baja |
| 3 | eBPF embebido sin verification | Baja | Media |
| 4 | Sin CO-RE | Alta | Alta |
| 5 | Sin hot-reload | Media | Media |
| 6 | KProbes vs fentry | Media | Media |

---

## 12. Propuesta de Refactorización Arquitectónica

### 12.1 Objetivos de Refactorización

```
┌──────────────────────────────────────────────────────────────────┐
│                    REFACTORING GOALS                              │
│                                                                  │
│  1. Modularidad                                                 │
│     - Separar código en módulos lógicos                        │
│     - Reducir acoplamiento entre componentes                   │
│                                                                  │
│  2. Abstraction para eBPF                                       │
│     - Capa de abstraction para programs y maps                 │
│     - Type-safe map access                                     │
│                                                                  │
│  3. CO-RE Portabilidad                                          │
│     - Soporte BTF para portabilidad entre kernels              │
│     - Eliminar dependencia de toolchain específica              │
│                                                                  │
│  4. Hot-Reload                                                  │
│     - Recargar programas eBPF sin reiniciar                     │
│     - Graceful update de maps                                   │
│                                                                  │
│  5. Observabilidad Mejorada                                     │
│     - Migrar de bpf_trace_printk a ringbuf                     │
│     - Mejor structured logging                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 12.2 Nueva Estructura de Modules

```
ebpf-node/
├── ebpf-node/                          ← User-space
│   ├── src/
│   │   ├── main.rs                     ← Entry point, CLI
│   │   ├── config/                     ← Configuration
│   │   │   ├── mod.rs
│   │   │   ├── node.rs                 ← NodeConfig
│   │   │   └── cli.rs                  ← Opt / CLI parsing
│   │   ├── ebpf/                       ← eBPF management
│   │   │   ├── mod.rs
│   │   │   ├── loader.rs               ← Ebpf loading
│   │   │   ├── programs.rs             ← Program attach/detach
│   │   │   ├── maps.rs                 ← Type-safe map access
│   │   │   └── metrics.rs              ← eBPF → Prometheus sync
│   │   ├── api/                        ← HTTP API
│   │   │   ├── mod.rs
│   │   │   ├── health.rs               ← Health handler
│   │   │   ├── node.rs                 ← Node info handler
│   │   │   ├── network.rs              ← Network handlers
│   │   │   ├── transactions.rs         ← Transaction handlers
│   │   │   ├── blocks.rs               ← Block handlers
│   │   │   ├── security.rs             ← Security handlers
│   │   │   └── metrics.rs              ← Metrics handler
│   │   ├── p2p/                        ← libp2p
│   │   │   ├── mod.rs
│   │   │   ├── swarm.rs                ← Swarm setup
│   │   │   ├── gossip.rs               ← Gossipsub handling
│   │   │   ├── sync.rs                 ← Historical sync
│   │   │   └── behaviour.rs            ← MyBehaviour
│   │   ├── security/                   ← Security managers
│   │   │   ├── mod.rs
│   │   │   ├── peer_store.rs           ← PeerStore
│   │   │   ├── replay.rs               ← ReplayProtection
│   │   │   └── sybil.rs                ← SybilProtection
│   │   └── db/                         ← Database
│   │       ├── mod.rs
│   │       └── rocksdb.rs              ← RocksDB setup
│   └── build.rs                        ← Build script (updated)
│
├── ebpf-node-ebpf/                     ← eBPF programs
│   ├── src/
│   │   ├── main.rs                     ← XDP program
│   │   ├── tracing.rs                  ← KProbes / fentry
│   │   └── lib.rs
│   └── build.rs
│
└── ebpf-node-common/                   ← Shared types
    ├── src/
    │   ├── mod.rs
    │   ├── transaction.rs
    │   ├── network_message.rs
    │   ├── block.rs
    │   └── response_types.rs
    └── Cargo.toml
```

### 12.3 Abstraction para eBPF Maps

```rust
// ebpf-node/src/ebpf/maps.rs

use aya::{Ebpf, maps::{HashMap, LpmTrie}};
use anyhow::Result;

/// Type-safe eBPF map manager
pub struct EbpfMaps<'a> {
    ebpf: &'a mut Ebpf,
}

impl<'a> EbpfMaps<'a> {
    pub fn new(ebpf: &'a mut Ebpf) -> Self {
        Self { ebpf }
    }

    /// Type-safe access to LATENCY_STATS
    pub fn latency_stats(&mut self) -> Result<HashMap<'_, u64, u64>> {
        Ok(HashMap::try_from(
            self.ebpf.map("LATENCY_STATS")
                .map_err(|e| anyhow!("Failed to get LATENCY_STATS: {}", e))?
        )?)
    }

    /// Type-safe access to NODES_WHITELIST
    pub fn whitelist(&mut self) -> Result<LpmTrie<'_, u32, u32, u32>> {
        Ok(LpmTrie::try_from(
            self.ebpf.map("NODES_WHITELIST")
                .map_err(|e| anyhow!("Failed to get NODES_WHITELIST: {}", e))?
        )?)
    }

    /// Type-safe access to NODES_BLACKLIST (mutable)
    pub fn blacklist_mut(&mut self) -> Result<LpmTrie<'_, u32, u32, u32>> {
        Ok(LpmTrie::try_from(
            self.ebpf.map_mut("NODES_BLACKLIST")
                .map_err(|e| anyhow!("Failed to get NODES_BLACKLIST: {}", e))?
        )?)
    }

    /// Get whitelist size
    pub fn whitelist_size(&mut self) -> Result<usize> {
        Ok(self.whitelist()?.iter().count())
    }

    /// Get blacklist size
    pub fn blacklist_size(&mut self) -> Result<usize> {
        Ok(self.blacklist_mut()?.iter().count())
    }

    /// Block IP in blacklist
    pub fn block_ip(&mut self, ip: u32) -> Result<()> {
        use aya::maps::lpm_trie::Key;
        let key = Key::new(32, ip);
        self.blacklist_mut()?.insert(&key, 1, 0)
    }
}
```

### 12.4 Separación de eBPF Programs

```rust
// ebpf-node-ebpf/src/main.rs ← XDP program only

#![no_std]
#![no_main]

use aya_ebpf::{
    bindings::xdp_action,
    macros::xdp,
    programs::XdpContext,
};

mod xdp;          // XDP program (whitelist/blacklist)
mod common;       // Shared utilities

#[xdp]
pub fn xdp_main(ctx: XdpContext) -> u32 {
    xdp::try_xdp_filter(ctx)
        .unwrap_or(xdp_action::XDP_ABORTED)
}

// ebpf-node-ebpf/src/tracing.rs ← KProbes only

#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::bpf_ktime_get_ns,
    macros::kprobe,
    programs::ProbeContext,
};

mod tracing;      // KProbes for latency measurement
mod common;

#[kprobe]
pub fn netif_receive_skb_entry(ctx: ProbeContext) -> u32 {
    tracing::record_entry(ctx)
}

#[kprobe]
pub fn napi_consume_skb_exit(ctx: ProbeContext) -> u32 {
    tracing::record_exit(ctx)
}
```

### 12.5 Hot-Reload Architecture

```rust
// ebpf-node/src/ebpf/loader.rs

use aya::{Ebpf, programs::{Xdp, KProbe, XdpFlags}};
use std::sync::{Arc, Mutex};
use std::path::Path;

/// Hot-reloadable eBPF manager
pub struct EbpfManager {
    inner: Arc<Mutex<Ebpf>>,
    iface: String,
}

impl EbpfManager {
    pub fn new(ebpf: Ebpf, iface: String) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ebpf)),
            iface,
        }
    }

    /// Reload eBPF program (detach old, attach new)
    pub fn reload(&mut self, new_bytes: &[u8]) -> Result<(), EbpfError> {
        let mut ebpf = self.inner.lock().unwrap();
        
        // 1. Detach all programs
        self.detach_all(&mut ebpf)?;
        
        // 2. Load new eBPF object
        let mut new_ebpf = Ebpf::load(new_bytes)?;
        
        // 3. Attach new programs
        self.attach_all(&mut new_ebpf, &self.iface)?;
        
        // 4. Swap
        *ebpf = new_ebpf;
        
        Ok(())
    }

    fn detach_all(&self, ebpf: &mut Ebpf) -> Result<(), EbpfError> {
        // Detach XDP
        if let Some(xdp) = ebpf.program_mut("ebpf_node") {
            let xdp: &mut Xdp = xdp.try_into()?;
            // xdp.detach()?; // Would need stored ifindex
        }
        // ... other programs
        Ok(())
    }

    fn attach_all(&self, ebpf: &mut Ebpf, iface: &str) -> Result<(), EbpfError> {
        // Attach XDP
        if let Some(xdp) = ebpf.program_mut("ebpf_node") {
            let xdp: &mut Xdp = xdp.try_into()?;
            xdp.load()?;
            xdp.attach(iface, XdpFlags::default())?;
        }
        // ... other programs
        Ok(())
    }
}
```

### 12.6 Migración a CO-RE

```bash
# Requisitos CO-RE
# 1. Kernel con BTF (/sys/kernel/btf/vmlinux)
# 2. clang/LLVM 14+
# 3. bpftool 5.15+
# 4. aya 0.12+ (ya soporta CO-RE)

# En Cargo.toml:
[workspace.dependencies]
aya = { git = "https://github.com/aya-rs/aya", features = ["loader"] }
aya-ebpf = { git = "https://github.com/aya-rs/aya" }

# aya con feature "loader" habilitada automáticamente CO-RE
```

**Beneficios de CO-RE:**
- Un binary eBPF funciona en cualquier kernel con BTF
- No se necesita recompilar para cada versión de kernel
- Menor overhead de build

### 12.7 Migración a Ringbuf

```rust
// eBPF side: usar bpf_ringbuf_output en lugar de bpf_trace_printk

use aya_ebpf::{
    macros::map,
    maps::RingBuf,
};

#[map]
static LOG_BUFFER: RingBuf = RingBuf::with_byte_size(1024 * 1024, 0);

fn log_message(ctx: &XdpContext, msg: &str) {
    let entry = LOG_BUFFER.alloc::<LogEntry>().unwrap();
    entry.timestamp = bpf_ktime_get_ns();
    entry.action = xdp_action;
    entry.message = msg[..128].try_into().unwrap();
    
    LOG_BUFFER.output(entry, 0);
}

// User side: consumir ringbuf async
use aya::maps::RingBuf;

let mut ringbuf: RingBuf = RingBuf::try_from(
    ebpf.map_mut("LOG_BUFFER")?
)?;

tokio::task::spawn(async move {
    for item in ringbuf {
        let entry = item.data().unwrap();
        info!(
            timestamp = entry.timestamp,
            message = entry.message,
            "eBPF log"
        );
    }
});
```

---

## 13. Roadmap de Evolución Aya

### 13.1 Estado Actual de Aya (2024)

```
Versiones disponibles:
├── 0.12.x (stable)
├── 0.13.x (current, git main)
└── 0.14.x (in development)
```

### 13.2 Features en Pipeline

| Feature | Estado | Versión | Descripción |
|---------|--------|---------|-------------|
| CO-RE | ✓ | 0.12+ | Compile Once Run Everywhere |
| Ringbuf | ✓ | 0.12+ | BPF ring buffer para logging |
| BPF Luna | WIP | 0.14+ | Verifier mejorado |
| Hot-Reload | WIP | 0.14+ | Recarga de programas en runtime |
| eXpress Data Path V2 | Planned | 0.15+ | XDP con AF_XDP mejorado |
| BPF Iterator | Research | TBD | Iterar estructuras kernel |
| BPF Collateral | Active | 0.13+ | Library de funciones reusables |

### 13.3 Orientación del Proyecto

```
Aya Framework Evolution:

2023: CO-RE stabilization
        │
        ▼
2024: Observability improvements (ringbuf, perf event)
      Hot-reload support
      Better error messages
        │
        ▼
2025: BPF Luna (advanced verification)
      eBPF application framework (higher-level API)
      Multi-program orchestration
        │
        ▼
2026+: BPF as a first-class systems programming target
      eBPF ecosystem maturity
      Production-grade tooling
```

### 13.4 Recomendaciones para ebpf-node

```
PRIORITY 1 (Inmediato):
├── Refactorizar main.rs en módulos
├── Crear abstraction layer para maps
└── Mejorar error handling

PRIORITY 2 (Corto plazo):
├── Migrar a ringbuf para logging eBPF
├── Añadir verificación de integridad del eBPF binary
└── Mejorar documentation

PRIORITY 3 (Mediano plazo):
├── Implementar hot-reload de programas
├── Migrar KProbes a fentry/fexit (si kernel soporta)
└── Habilitar CO-RE completo

PRIORITY 4 (Largo plazo):
├── Evaluación de bpfman para gestión de eBPF
├── Soporte para múltiples interfaces de red
└── Upgrade a aya 0.14+ cuando stable
```

---

## Appendix A: Glossary

| Término | Descripción |
|---------|-------------|
| **eBPF** | extended Berkeley Packet Filter - VM del kernel |
| **BPF** | Berkeley Packet Filter - original packet filter |
| **XDP** | eXpress Data Path - hook más temprano en red |
| **KProbe** | Kernel probe - trace function entry/return |
| **UProbe** | User probe - trace user function entry/return |
| **CO-RE** | Compile Once Run Everywhere - portabilidad eBPF |
| **BTF** | BPF Type Format - type info para eBPF |
| **Verifier** | Componente kernel que valida seguridad de eBPF |
| **JIT** | Just-In-Time compiler para eBPF |
| **Map** | Data structure shared between eBPF and user-space |
| **Helper** | Functions provided by kernel to eBPF programs |
| **ELF** | Executable and Linkable Format - eBPF object |
| **SKB** | Socket Buffer - kernel packet representation |
| **NAPI** | New API - kernel packet polling mechanism |

## Appendix B: Referencias

- [Aya Documentation](https://aya-rs.dev/book/)
- [Aya GitHub](https://github.com/aya-rs/aya)
- [BPF Documentation](https://docs.kernel.org/bpf/)
- [LWN: eBPF](https://lwn.net/Kernel/Index/#BPF)
- [BPF Performance Tools](https://www.bpfperformance tools.com/)
- [Linux Network Stack](https://docs.kernel.net/networking/)

## Appendix C: Comandos Útiles

```bash
# Verificar BTF
cat /sys/kernel/btf/vmlinux

# Listar programas eBPF cargados
sudo bpftool prog list

# Listar maps eBPF
sudo bpftool map list

# Listar programas XDP
sudo bpftool prog show | grep xdp

# Ver attach points
sudo bpftool prog show

# Monitor eBPF logs
sudo tail -f /sys/kernel/debug/tracing/trace_pipe

# Verificar kernel version
uname -r

# Instalar bpf-linker
cargo install bpf-linker

# Verificar eBPF programs
sudo bpftool prog dump xlated prog id <ID>
```

---

*Documento generado como parte del análisis arquitectónico de ebpf-node.*
*Fecha: 2026-04-21*
