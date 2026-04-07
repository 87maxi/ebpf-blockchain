# Plan de Mejora - eBPF Blockchain Lab

## Visión General

Este documento establece la visión educativa y técnica para transformar el proyecto `ebpf-blockchain` en un **Proof of Concept serio y presentable** que demuestre conocimientos profundos de Rust aplicado al sistema Linux, con una arquitectura de laboratorio bien organizada utilizando Ansible y LXC.

---

## 0.1 Propósito Educativo

El objetivo principal es crear un laboratorio de pruebas que sirva como:

### Marco de Aprendizaje
| Área | Qué se Aprende |
|------|---------------|
| **Rust Profundo** | Memory safety, async/await, FFI con C, gestión de recursos |
| **eBPF** | Programación del kernel, XDP, kprobes, mapas eBPF |
| **Redes P2P** | Protocolos de comunicación distribuida, gossipsub, QUIC |
| **Sistemas Distribuidos** | Consenso, replicación, sincronización de estado |
| **DevOps** | Automatización con Ansible, contenedores LXC, monitoring |
| **Linux Internals** | Kernel networking, syscall introspection, performance tuning |

### Casos de Uso Educativos
1. **Demostración de Observabilidad**: Ver métricas de red en tiempo real desde el kernel
2. **Experimentos de Seguridad**: Simulación de ataques y defensas a nivel kernel
3. **Pruebas de Consenso**: Análisis de diferentes algoritmos de votación
4. **Debugging Avanzado**: Herramientas para diagnosticar problemas en sistemas distribuid0s

---

## 0.2 Principios de Diseño

### Principio 1: Seguridad sobre Funcionalidad
- El consenso debe ser resistente a ataques Sybil
- No comprometer seguridad por conveniencia
- Documentar vulnerabilidades conocidas

### Principio 2: Observabilidad Total
- Todo evento importante debe ser logueado
- Métricas expuestas en tiempo real
- Trazas de red disponibles para debugging

### Principio 3: Infraestructura como Código
- Todo debe ser reproducible con `ansible-playbook deploy.yml`
- Sin configuraciones manuales "mágicas"
- Scripts documentados con ejemplos claros

### Principio 4: Rust Idiomatic
- Usar patterns modernos de Rust (2021 edition)
- Aprovechar async/await correctamente
- Error handling con `anyhow` y `thiserror`
- Testing exhaustivo con property-based tests

---

## 1. Estado Actual del Proyecto

### 1.1 Componentes Implementados

#### Kernel Space (eBPF)
| Componente | Estado | Descripción |
|-----------|--------|-------------|
| **XDP Program** | ✅ Funcional | Filtrado básico de paquetes, blacklist simple |
| **kprobes** | ✅ Funcional | Medición de latencia en `netif_receive_skb` y `napi_consume_skb` |
| **Maps eBPF** | ✅ Funcional | `NODES_BLACKLIST`, `LATENCY_STATS`, `START_TIMES` |
| **Aya Framework** | ✅ Funcional | Carga dinámica de programas eBPF desde Rust |

#### User Space (Rust)
| Componente | Estado | Descripción |
|-----------|--------|-------------|
| **Swarm P2P** | ✅ Funcional | libp2p con gossipsub, identify, mdns |
| **Transportes** | ✅ Funcional | TCP y QUIC disponibles |
| **Consenso** | ⚠️ Crítico | Quórum de 2/3 implementado pero vulnerable a Sybil |
| **RocksDB** | ✅ Funcional | Persistencia básica de transacciones |
| **API HTTP** | ✅ Funcional | Endpoints /metrics, /rpc, /ws |
| **Métricas** | ⚠️ Parcial | Latencias funcionan, peers/messages no se reportan correctamente |

#### Infraestructura
| Componente | Estado | Descripción |
|-----------|--------|-------------|
| **LXC Containers** | ⚠️ Parcial | 3 nodos operativos pero con problemas de red |
| **Ansible Playbooks** | ⚠️ Parcial | Funcionales pero con manejo de errores deficiente |
| **Prometheus** | ✅ Funcional | Scraping de métricas funcionando |
| **Grafana** | ✅ Funcional | Dashboards básicos configurados |
| **Loki** | ✅ Funcional | Logs estructurados disponibles |

---

### 1.2 Problemas Críticos Identificados

#### CRÍTICO: Vulnerabilidades de Consenso

**Problema Actual:**
```rust
// Consenso actual - VULNERABLE
if voters.len() == 2 {
    transaction.confirmed();  // ¡Solo 2 votos en cluster de 3!
}
```

**Impacto:**
- Un atacante puede inyectar transacciones maliciosas
- No hay verificación de identidad de peers
- No hay replay protection
- Sin límite de tamaño del cluster

**Requisito de Mejora:**
- Implementar verificación de identidad con certificados
- Agregar replay protection con sequence numbers
- Implementar límite de votos por transacción
- Considerar algoritmos de consenso más robustos (PoS simple)

#### ALTO: Persistencia de Datos

**Problema Actual:**
```rust
// Ruta temporal - DATOS SE PERDERÁN
let db_path = format!("/tmp/rocksdb_{}", hostname);
```

**Impacto:**
- `/tmp` se limpia al reiniciar
- PID cambia entre ejecuciones
- Imposible verificar persistencia real

**Requisito de Mejora:**
```rust
// Ruta persistente - CORREGIDO
let db_path = format!("/var/lib/ebpf-blockchain/nodes/{}", hostname);
```

#### MEDIO: Problemas de Red

**Problema Actual:**
- Tráfico entre contenedores LXD bloqueado por `br_netfilter`
- Reglas de firewall no configuradas adecuadamente
- DNS perdido después de reinicios

**Impacto:**
- Nodos no pueden comunicarse P2P
- Requiere correcciones manuales post-deploy
- No reproducible automáticamente

**Requisito de Mejora:**
```bash
# Reglas de firewall para LXC
sysctl -w net.bridge.bridge-nf-call-iptables=0
iptables -A FORWARD -i lxdbr1 -o lxdbr1 -j ACCEPT
iptables -A FORWARD -i lxdbr1 -o eth0 -j ACCEPT
```

#### BAJO: Gestión de Procesos

**Problema Actual:**
```bash
# En contenedores - PROCESOS Mueren
lxc exec ebpf-node-1 -- nohup ./ebpf-node &
# → Proceso muere cuando shell termina
```

**Impacto:**
- Nodos requieren reinicio manual frecuente
- Logs se pierden
- Dificulta automatización

**Requisito de Mejora:**
```bash
# En host - PROCESOS PERSISTEN
nohup lxc exec ebpf-node-1 -- ./ebpf-node ... > /tmp/node-1.log &
```

---

## 1.3 Métricas Actuales

### Funcionalidad por Componente

| Componente | Estado | % Funcional |
|-----------|--------|-------------|
| eBPF Kernel | ✅ | 95% |
| P2P Networking | ⚠️ | 70% |
| Consensus | ⚠️ | 50% |
| Persistence | ⚠️ | 60% |
| Monitoring | ✅ | 80% |
| Automation | ⚠️ | 65% |
| **General** | ⚠️ | **70%** |

### Complejidad del Código

| Área | Líneas | Complejidad |
|------|--------|-------------|
| Main (user space) | ~500 | Alta (async, networking) |
| eBPF (kernel space) | ~100 | Media |
| Ansible playbooks | ~300 | Baja |
| **Total** | ~900 | Media-Alta |

---

## 1.4 Dependencias Técnicas

### Runtime Requirements
```yaml
Linux Kernel: ≥ 5.10 con BTF support
Rust: Nightly (para Aya)
LXD: ≥ 4.0
Docker: ≥ 20.10
```

### Software Stack
| Tecnología | Versión | Propósito |
|------------|---------|-----------|
| Rust | 1.75+ | Lenguaje principal |
| Aya | 0.11+ | eBPF framework |
| libp2p | 0.53+ | P2P networking |
| RocksDB | 8.3+ | Base de datos |
| Axum | 0.7+ | HTTP server |
| Prometheus | 2.47+ | Métricas |
| Ansible | 2.14+ | Automatización |
| LXD | 5.2+ | Contenedores |

---

## 2. Perspectiva Educativa

### 2.1 Objetivos de Aprendizaje

Al completar este proyecto, el estudiante/developer debería:

#### Conocimientos Técnicos
- [ ] Entender cómo funciona eBPF a nivel de kernel
- [ ] Saber programar XDP y kprobes en Rust
- [ ] Comprender protocolos P2P y gossip
- [ ] Implementar algoritmos de consenso básicos
- [ ] Configurar infraestructura como código con Ansible

#### Habilidades Prácticas
- [ ] Debugging de problemas de red en Linux
- [ ] Monitorización de aplicaciones distribuidas
- [ ] Testing de sistemas bajo carga
- [ ] Hardening de seguridad en aplicaciones Rust
- [ ] Optimización de rendimiento en el kernel

### 2.2 Escenario de Uso Educativo

#### Nivel 1: Introducción (Semana 1)
```
Objetivo: Arrancar el cluster y observar métricas básicas

Comandos:
  cd ebpf-blockchain/ansible
  ansible-playbook deploy_cluster.yml

Verificación:
  lxc exec ebpf-node-1 -- ps aux | grep ebpf-node
  curl http://localhost:9090/metrics
  open http://localhost:3000
```

#### Nivel 2: Experimentación (Semana 2)
```
Objetivo: Inyectar transacciones y observar consenso

Comandos:
  curl -X POST http://192.168.2.11:9090/rpc \
    -H "Content-Type: application/json" \
    -d '{"id": "tx1", "data": "transfer:100"}'
  
  # Verificar en Grafana:
  # - Paneles de transacciones confirmadas
  # - Histogramas de latencia
```

#### Nivel 3: Depuración (Semana 3)
```
Objetivo: Simular fallos y diagnosticar

Comandos:
  # Simular ataque de spam
  cargo run --bin ebpf-simulation -- --attack spam
  
  # Ver blacklist activada
  lxc exec ebpf-node-1 -- bpftool map dump name NODES_BLACKLIST
  
  # Inspeccionar métricas
  lxc exec ebpf-node-1 -- cat /tmp/ebpf-node-1.log
```

#### Nivel 4: Extensión (Semana 4+)
```
Objetivo: Implementar nuevas funcionalidades

Tareas:
  - Implementar replay protection
  - Agregar new consensus algorithm
  - Crear dashboard personalizado en Grafana
```

---

## 2.3 Metodología de Desarrollo

### Enfoque Incremental
1. **Documentar estado actual** (este documento)
2. **Priorizar problemas** por criticidad
3. **Implementar soluciones** en iteraciones
4. **Validar cambios** con pruebas automatizadas
5. **Actualizar documentación** con cada cambio

### Criterios de Aceptación

Para cada mejora, verificar:
- [ ] Código compilable sin warnings
- [ ] Tests pasando (unitarios e integration)
- [ ] Documentación actualizada
- [ ] Métricas funcionando correctamente
- [ ] Sin regressions en funcionalidad existente
- [ ] Ansible playbook reproducible

---

## 3. Arquitectura Deseada

### 3.1 Visión General

```
┌─────────────────────────────────────────────────────────────┐
│                    HOST: openSUSE Tumbleweed                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                    LXC BRIDGE (lxdbr1)               │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │  │
│  │  │   ebpf-node  │  │   ebpf-node  │  │   ebpf-node  │ │  │
│  │  │     #1       │  │     #2       │  │     #3       │ │  │
│  │  │ 192.168.2.11 │  │ 192.168.2.12 │  │ 192.168.2.13 │ │  │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │  │
│  │         │                 │                 │         │  │
│  │         └─────────────────┼─────────────────┘         │  │
│  │                           │                           │  │
│  └───────────────────────────┼───────────────────────────┘  │
│                              │                              │
│  ┌───────────────────────────┼───────────────────────────┐  │
│  │              Docker Compose                            │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │  │
│  │  │  prometheus  │  │   grafana    │  │      loki    │ │  │
│  │  │  :9090       │  │  :3000       │  │  :3100       │ │  │
│  │  └──────────────┘  └──────────────┘  └──────────────┘ │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Componentes del Sistema Mejorado

#### eBPF Kernel Space

```rust
// Mejoras necesarias:
pub struct EbpfPrograms {
    // XDP mejorado
    pub xdp_program: Xdp,
    pub whitelist: LruHashMap<u32, u32>,  // Whitlist inicial
    
    // kprobes mejorados
    pub kprobe_in: KProbe,
    pub kprobe_out: KProbe,
    pub latency_map: Histogram,  // Histograma mejorado
    
    // Seguridad
    pub rate_limit_map: HashMap<u32, RateLimitConfig>,
    pub sequence_counter: AtomicCounter,  // Replay protection
}
```

#### User Space Rust

```rust
// Mejoras necesarias:
pub struct Node {
    // Networking mejorado
    pub swarm: Swarm<MyBehaviour>,
    pub peer_manager: PeerManager,  // Nueva: gestión de peers
    pub consensus: ConsensusEngine, // Nueva: consenso seguro
    
    // Persistence mejorada
    pub db: Arc<DB>,
    pub db_path: PathBuf,  // Ruta persistente
    
    // Monitoring mejorado
    pub metrics: MetricsRegistry,
    pub logger: TracingSubscriber,
}
```

---

## 4. Hoja de Ruta (Roadmap)

### Fase 1: Estabilización (Semana 1-2)
**Objetivo:** Corregir problemas críticos de funcionalidad

| Tarea | Prioridad | Estado |
|-------|-----------|--------|
| Corregir persistencia de RocksDB | 🔴 Crítico | Pendiente |
| Arreglar generación de métricas peers | 🟠 Alto | Pendiente |
| Configurar reglas de red LXC | 🟠 Alto | Pendiente |
| Fix problemas de procesos zombie | 🟡 Medio | Pendiente |
| Agregar uptime metric | 🟢 Bajo | Pendiente |

### Fase 2: Seguridad (Semana 3-4)
**Objetivo:** Implementar consenso seguro

| Tarea | Prioridad | Estado |
|-------|-----------|--------|
| Agregar replay protection | 🔴 Crítico | Pendiente |
| Implementar límites de votos | 🔴 Crítico | Pendiente |
| Whitelist de peers | 🟠 Alto | Pendiente |
| Rate limiting por peer | 🟠 Alto | Pendiente |
| Documentar vulnerabilidades | 🟢 Bajo | Pendiente |

### Fase 3: Observabilidad (Semana 5-6)
**Objetivo:** Mejorar monitorización y debugging

| Tarea | Prioridad | Estado |
|-------|-----------|--------|
| Dashboard Grafana completo | 🟠 Alto | Pendiente |
| Logs estructurados en Loki | 🟠 Alto | Pendiente |
| Alertas Prometheus | 🟡 Medio | Pendiente |
| Tracing de red detallado | 🟡 Medio | Pendiente |
| Documentación de métricas | 🟢 Bajo | Pendiente |

### Fase 4: Automatización (Semana 7-8)
**Objetivo:** Infraestructura como código robusta

| Tarea | Prioridad | Estado |
|-------|-----------|--------|
| Ansible playbook completo | 🔴 Crítico | Pendiente |
| Scripts de deploy documentados | 🟠 Alto | Pendiente |
| Tests de integración Ansible | 🟡 Medio | Pendiente |
| Documentación de despliegue | 🟢 Bajo | Pendiente |
| CI/CD pipeline | 🟢 Bajo | Pendiente |

### Fase 5: Documentación (Continua)
**Objetivo:** Documentación completa y ejemplos

| Tarea | Prioridad | Estado |
|-------|-----------|--------|
| README.md mejorado | 🔴 Crítico | Pendiente |
| API documentation | 🟠 Alto | Pendiente |
| Ejemplos de uso | 🟠 Alto | Pendiente |
| Guía de troubleshooting | 🟡 Medio | Pendiente |
| Presentación del proyecto | 🟢 Bajo | Pendiente |

---

## 5. Criterios de Éxito

### Criterios Técnicos

| Criterio | Actual | Objetivo |
|----------|--------|----------|
| Consenso seguro | ❌ Vulnerable | ✅ Resiste Sybil |
| Persistencia de datos | ❌ Volátil | ✅ 100% persistente |
| Red P2P estable | ⚠️ 70% | ✅ 100% estable |
| Métricas completas | ⚠️ 80% | ✅ 100% funcional |
| Automatización | ⚠️ 65% | ✅ 100% reproducible |

### Criterios Educativos

| Criterio | Actual | Objetivo |
|----------|--------|----------|
| Código documentado | ⚠️ Parcial | ✅ Completa |
| Tests automatizados | ❌ Mínimos | ✅ >80% coverage |
| Ejemplos didácticos | ❌ Poco | ✅ Varios casos |
| Guía de inicio | ⚠️ Parcial | ✅ Paso a paso |
| Casos de estudio | ❌ Ninguno | ✅ Mínimo 5 |

---

## 6. Próximos Pasos Inmediatos

### 6.1 Esta Sesión (Próximo segmento)

1. **Plan Estructural del Proyecto**
   - Organizar directorios del proyecto
   - Definir estructura de módulos Rust
   - Establecer convenios de código

2. **Ansible + LXC en openSUSE**
   - Configuración inicial del host
   - Playbook de despliegue completo
   - Gestión de red LXC

3. **Rust Profundo en Linux**
   - Pattern para FFI con C
   - Async patterns correctos
   - Error handling idiomático

### 6.2 Documentación por Generar

| Documento | Contenido | Prioridad |
|-----------|-----------|-----------|
| `01_estructura_proyecto.md` | Organización de directorios y módulos | 🔴 Alta |
| `02_ansible_lxc.md` | Playbook completo para LXC | 🔴 Alta |
| `03_rust_profundo.md` | Patrones Rust para sistemas | 🟠 Media |
| `04_laboratorio_pruebas.md` | Scripts de testing y simulación | 🟠 Media |
| `05_consenso_seguro.md` | Implementación de consenso | 🔴 Alta |
| `06_seguimiento.md` | Plan de seguimiento y refinamiento | 🟡 Baja |

---

## 7. Glosario de Términos

| Término | Definición |
|---------|------------|
| **eBPF** | Extended Berkeley Packet Filter - Framework para ejecutar código en el kernel |
| **XDP** | eXpress Data Path - Programa eBPF ejecutado en la capa de red |
| **kprobe** | Point de inserción en funciones del kernel para observar comportamiento |
| **Gossipsub** | Protocolo de propagación de mensajes en redes P2P |
| **Quórum** | Número mínimo de votos necesarios para tomar una decisión |
| **Sybil Attack** | Ataque donde un entidad controla múltiples identidades falsas |
| **Replay Protection** | Mecanismo para prevenir reenvío de mensajes ya procesados |
| **LXC** | Linux Containers - Tecnología de virtualización a nivel de sistema operativo |
| **Ansible** | Herramienta de automatización de infraestructura como código |
| **RocksDB** | Base de datos clave-valor de alto rendimiento |

---

*Documento creado: 2026-01-26*
*Estado: V0.1 - En construcción*
*Próxima actualización: 01_plan_estructural.md*