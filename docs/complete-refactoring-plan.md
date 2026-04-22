# Plan Completo de Refactoring de ebpf-node

## Visión General

Este documento resume todas las fases de refactoring implementadas en el proyecto ebpf-node, que tiene como objetivo mejorar la arquitectura del sistema eBPF para hacerlo más modular, mantenible y escalable.

## Fases Implementadas

### Fase 1: Separación de Módulos User-Space

**Objetivo**: Organizar el código de usuario-space en módulos bien definidos

**Cambios Implementados**:
- Creación de estructura modular en `ebpf-node/src/ebpf/`
- Separación de responsabilidades entre:
  - `loader.rs`: Carga de programas eBPF
  - `programs.rs`: Gestión de programas (attach/detach)
  - `maps.rs`: Acceso seguro a mapas eBPF
  - `metrics.rs`: Métricas de rendimiento
  - `hot_reload.rs`: Arquitectura de hot-reload

### Fase 2: Abstraction Type-Safe para eBPF Maps

**Objetivo**: Crear una capa de abstracción segura para el acceso a mapas eBPF

**Cambios Implementados**:
- Implementación de `EbpfMaps` struct con métodos type-safe
- Métodos para acceso a:
  - `latency_stats()`: Estadísticas de latencia
  - `whitelist()`: Lista blanca de IPs
  - `blacklist()`: Lista negra de IPs
  - `whitelist_size()`: Tamaño de lista blanca
  - `blacklist_size()`: Tamaño de lista negra
  - `block_ip()`: Bloquear IP
  - `unblock_ip()`: Desbloquear IP
  - `is_whitelisted()`: Verificar IP en lista blanca
  - `get_latency_stats()`: Obtener estadísticas de latencia

### Fase 3: Separación de eBPF Programs (XDP + KProbes)

**Objetivo**: Modularizar los programas eBPF en módulos separados

**Cambios Implementados**:
- Creación de estructura modular en `ebpf-node-ebpf/src/programs/`
- Implementación de:
  - `programs/xdp.rs`: Programa XDP para filtrado de paquetes
  - `programs/kprobes.rs`: Programas KProbe para medición de latencia
  - `programs/maps.rs`: Mapas compartidos entre programas
- Uso de mapas compartidos para evitar duplicados
- Eliminación de duplicados de definiciones de mapas

### Fase 4: Hot-Reload Architecture

**Objetivo**: Implementar capacidad de recarga dinámica de programas eBPF

**Cambios Implementados**:
- Creación de `EbpfHotReloadManager` para gestión de hot-reload
- Implementación de métodos:
  - `init()`: Inicialización de programas
  - `reload()`: Recarga completa de programas
  - `get_ebpf()`: Acceso al estado actual de eBPF
- Endpoint REST `/api/v1/ebpf/reload` para recarga programática
- Integración con el sistema de estado del nodo

### Fase 5: Migración a Ringbuf

**Objetivo**: Reemplazar `bpf_trace_printk` con `ringbuf` para mejorar el rendimiento

**Cambios Implementados**:
- Creación de módulo `ringbuf.rs` para manejo de Ringbuf
- Implementación de estructuras de eventos:
  - `LatencyEvent`: Para eventos de latencia
  - `PacketEvent`: Para eventos de paquetes
- Actualización de KProbes para usar Ringbuf en lugar de `START_TIMES` y `LATENCY_STATS`
- Actualización de XDP para usar Ringbuf en lugar de logging tradicional
- Documentación de la migración en `ringbuf-migration.md`

### Fase 6: Migración a CO-RE

**Objetivo**: Implementar soporte para CO-RE (Compile Once Run Everywhere)

**Cambios Implementados**:
- Configuración del build system para usar `bpf-linker`
- Actualización de `build.rs` para usar `aya-build`
- Actualización de `Cargo.toml` para incluir dependencias necesarias
- Documentación de la migración en `core-migration.md`

## Estructura de Directorios

```
ebpf-node/
├── ebpf-node/
│   ├── src/
│   │   ├── ebpf/
│   │   │   ├── loader.rs
│   │   │   ├── programs.rs
│   │   │   ├── maps.rs
│   │   │   ├── metrics.rs
│   │   │   ├── hot_reload.rs
│   │   │   └── mod.rs
│   │   ├── api/
│   │   │   ├── ebpf.rs
│   │   │   └── mod.rs
│   │   └── main.rs
│   └── Cargo.toml
├── ebpf-node-ebpf/
│   ├── src/
│   │   ├── programs/
│   │   │   ├── mod.rs
│   │   │   ├── maps.rs
│   │   │   ├── xdp.rs
│   │   │   ├── kprobes.rs
│   │   │   └── ringbuf.rs
│   │   └── main.rs
│   └── Cargo.toml
└── docs/
    ├── hot-reload-architecture.md
    ├── ringbuf-migration.md
    ├── core-migration.md
    └── refactoring-summary.md
```

## Beneficios Obtenidos

### Modularidad
- Código más organizado y fácil de mantener
- Separación clara de responsabilidades
- Reutilización de componentes

### Seguridad
- Acceso type-safe a mapas eBPF
- Evita duplicados de definiciones
- Manejo adecuado de errores

### Flexibilidad
- Hot-reload dinámico de programas
- Actualizaciones sin interrupciones
- API REST para gestión programática

### Rendimiento
- Acceso optimizado a mapas compartidos
- Menor uso de memoria
- Mejor mantenimiento del estado

## Arquitectura General

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   User-Space    │    │   eBPF Programs │    │   eBPF Maps     │
│                 │    │                 │    │                 │
│  ┌───────────┐  │    │  ┌───────────┐  │    │  ┌───────────┐  │
│  │   Main    │  │    │  │   XDP     │  │    │  │  Shared   │  │
│  │  (Entry)  │  │    │  │  Program  │  │    │  │  Maps     │  │
│  └───────────┘  │    │  └───────────┘  │    │  └───────────┘  │
│                 │    │                 │    │                 │
│  ┌───────────┐  │    │  ┌───────────┐  │    │  ┌───────────┐  │
│  │  API      │  │    │  │  KProbes  │  │    │  │  Access   │  │
│  │  Layer    │  │    │  │  Programs │  │    │  │  Layer    │  │
│  └───────────┘  │    │  └───────────┘  │    │  └───────────┘  │
│                 │    │                 │    │                 │
│  ┌───────────┐  │    │  ┌───────────┐  │    │  ┌───────────┐  │
│  │  Hot-     │  │    │  │  Shared   │  │    │  │  Type-    │  │
│  │  Reload   │  │    │  │  Maps     │  │    │  │  Safe     │  │
│  │  Manager  │  │    │  │  Access   │  │    │  │  Access   │  │
│  └───────────┘  │    │  └───────────┘  │    │  └───────────┘  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Conclusión

Las fases de refactoring implementadas han mejorado significativamente la arquitectura del proyecto ebpf-node, haciéndolo más modular, seguro y mantenible. La implementación sigue las mejores prácticas de desarrollo de software y está lista para ser utilizada en producción con capacidades avanzadas de gestión de programas eBPF.

Los cambios realizados permiten:
1. Mejor mantenimiento del código
2. Mayor portabilidad del sistema
3. Mejor rendimiento en el manejo de datos eBPF
4. Capacidad de actualización dinámica sin interrupciones
5. Sistema de logging más eficiente