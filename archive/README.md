# Archive - Documentación Histórica del Proyecto eBPF Blockchain

## Propósito

Este directorio contiene toda la documentación que **ya no forma parte de la documentación activa** del proyecto. Se archiva para mantener el repositorio limpio y organizado, preservando la información histórica para referencia futura.

## Estructura

```
archive/
├── README.md              # Este archivo (explicación de la organización)
├── plans/                 # Planes de mejora y documentación de implementación
│   └── plans-original/    # Planes originales del proyecto
├── specs/                 # Especificaciones técnicas por etapa
├── legacy/                # Documentación antigua, obsoleta o reemplazada
└── references/            # Material de referencia que se preserva pero no es parte activa
```

## Justificación de la Organización

### Lo que se mantiene en `docs/` (Documentación Activa)

La documentación en el directorio principal (`docs/`) y [`README.md`](../README.md) representa **lo que se implementó realmente** en el proyecto. Cada documento fue creado como parte de la **Fase 4: Documentación Completa** del proyecto.

#### Architecture Decision Records (ADRs)

Los ADRs en [`docs/adr/`](../docs/adr/) documentan las decisiones técnicas críticas tomadas durante el desarrollo, con su contexto, decisión y consecuencias:

| ADR | Decisión | Justificación |
|-----|----------|---------------|
| [`001-rust-implementation.md`](../docs/adr/001-rust-implementation.md) | Implementación en Rust | Memory safety, zero-cost abstractions, sistema de ownership |
| [`002-consensus-algorithm.md`](../docs/adr/002-consensus-algorithm.md) | PoS con 2/3 quorum | Tolerancia a fallos bizantinos, eficiencia energética |
| [`003-ebpf-for-security.md`](../docs/adr/003-ebpf-for-security.md) | eBPF/XDP para seguridad | Rendimiento en kernel space, drop packets antes de TCP/IP stack |
| [`004-rocksdb-storage.md`](../docs/adr/004-rocksdb-storage.md) | RocksDB como storage | Alto rendimiento, compaction, snapshot nativo |
| [`005-libp2p-networking.md`](../docs/adr/005-libp2p-networking.md) | libp2p con QUIC/mplex | Abstracción de red, NAT traversal, modularidad |
| [`006-observability-stack.md`](../docs/adr/006-observability-stack.md) | Prometheus+Loki+Tempo | Estándar industry, correlación metrics-logs-traces |

### Lo que se archivó

#### `archive/plans/` - Planes de Mejora

Contiene los documentos de planificación de fases del proyecto (`04_AUTOMATIZACION.md`, `05_DOCUMENTACION.md`, etc.) y sus resúmenes de implementación (`IMPLEMENTATION_*.md`). Estos documents son **históricos** porque las fases ya fueron implementadas y la planificación fue reemplazada por la documentación final en `docs/`.

#### `archive/specs/` - Especificaciones por Etapa

Las especificaciones técnicas detalladas de cada etapa (`etapa1-specs.md` through `etapa5-specs.md`) fueron **implementadas y validadas**. La información técnica está ahora integrada en:
- [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) - Arquitectura del sistema
- [`docs/API.md`](../docs/API.md) - Especificaciones de la API
- [`docs/CONTRIBUTING.md`](../docs/CONTRIBUTING.md) - Estándares de desarrollo

#### `archive/legacy/` - Documentación Obsoleta

Incluye:
- Guías de instalación/reemplazadas por [`docs/DEPLOYMENT.md`](../docs/DEPLOYMENT.md)
- Guías de seguridad/reemplazadas por [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) (Security Architecture section)
- Documentación de laboratorio/reemplazada por la estructura actual
- Documentos antiguos de proyecto/actualizados en `README.md`

#### `archive/references/` - Material de Referencia

Preservado para consulta pero no parte de la documentación activa:
- [`rfc.md`](./references/rfc.md) - RFC 001: Arquitectura original (diseño conceptual)
- [`RPC_DOCUMENTATION.md`](./references/RPC_DOCUMENTATION.md) - Documentación RPC (integrada en [`docs/API.md`](../docs/API.md))
- [`ebpf-blockchain.yaml`](./references/ebpf-blockchain.yaml) - Perfil LXC (referencia de configuración)
- [`TUTORIAL-AYA.md`](./references/TUTORIAL-AYA.md) - Tutorial eBPF con Aya (referencia para desarrolladores)

## Principios de Organización

1. **Solo lo implementado stays active**: Si una característica fue implementada y validada, su documentación va en `docs/`
2. **ADRs documentan el porqué**: Cada decisión técnica importante tiene un ADR que explica el contexto y la justificación
3. **Planificación → archive**: Documentos de planificación van a `archive/plans/`
4. **Especificaciones → archive**: Specs detallados van a `archive/specs/` (la información está integrada en docs activos)
5. **Referencias preservadas**: Material de referencia útil va a `archive/references/`
6. **Obsoleto → legacy**: Documentación reemplazada o desactualizada va a `archive/legacy/`

## Verificación de Implementación

Para verificar que la documentación refleja lo implementado:

| Documento | Refleja | Verificar en código |
|-----------|---------|---------------------|
| [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) | Componentes del sistema | [`ebpf-node/ebpf-node/src/main.rs`](../ebpf-node/ebpf-node/src/main.rs) |
| [`docs/API.md`](../docs/API.md) | Endpoints de la API | [`ebpf-node/ebpf-node/src/main.rs`](../ebpf-node/ebpf-node/src/main.rs:308) (handlers) |
| [`docs/CONTRIBUTING.md`](../docs/CONTRIBUTING.md) | Estándares de código | [`.github/workflows/ci-cd.yml`](../.github/workflows/ci-cd.yml) |
| [`docs/DEPLOYMENT.md`](../docs/DEPLOYMENT.md) | Procedimientos de deploy | [`ansible/playbooks/deploy.yml`](../ansible/playbooks/deploy.yml) |
| [`docs/OPERATIONS.md`](../docs/OPERATIONS.md) | Procedimientos operativos | [`monitoring/`](../monitoring/) stack |
| [`docs/openapi.yml`](../docs/openapi.yml) | Especificación API | [`ebpf-node/ebpf-node/src/main.rs`](../ebpf-node/ebpf-node/src/main.rs:10) (axum routes) |
