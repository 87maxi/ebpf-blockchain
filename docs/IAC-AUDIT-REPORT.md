# Informe de Auditoría IaC y Observabilidad - eBPF Blockchain

**Fecha:** 2026-05-03
**Versión:** 1.0
**Tipo:** Auditoría de Infraestructura como Código y Sistema de Observabilidad

---

## Tabla de Contenidos

1. [Resumen Ejecutivo](#1-resumen-ejecutivo)
2. [Actores del Sistema - Inventario Ansible](#2-actores-del-sistema---inventario-ansible)
3. [Análisis de Playbooks](#3-análisis-de-playbooks)
4. [Análisis de Roles](#4-análisis-de-roles)
5. [Consistencia del Sistema de Observabilidad](#5-consistencia-del-sistema-de-observabilidad)
6. [Brechas e Inconsistencias](#6-brechas-e-inconsistencias)
7. [Recomendaciones](#7-recomendaciones)

---

## 1. Resumen Ejecutivo

### Estado General

| Área | Estado | Puntuación |
|------|--------|------------|
| Inventario Ansible | ✅ Completo | 8.5/10 |
| Playbooks | ✅ Funcional | 8/10 |
| Roles | ⚠️ Parcial | 7/10 |
| Observabilidad (monitoring/) | ⚠️ Inconsistente | 6.5/10 |
| Consistencia Ansible vs monitoring/ | ❌ Crítica | 4/10 |

### Hallazgo Principal

**Existe una inconsistencia CRÍTICA entre la definición de observabilidad en Ansible y el directorio `monitoring/`.** El sistema tiene DOS definiciones de la infraestructura de monitoreo que no están sincronizadas:

1. **Ansible** genera archivos mediante templates Jinja2
2. **monitoring/** tiene archivos estáticos ya creados

Ambos definen servicios diferentes con configuraciones incompatibles.

---

## 2. Actores del Sistema - Inventario Ansible

### 2.1 Actores Definidos en [`inventory/hosts.yml`](ansible/inventory/hosts.yml)

| Grupo | Nodo | IP | Tipo | Estado |
|-------|------|----|------|--------|
| `lxc_nodes` | ebpf-node-1 | 192.168.2.210 | Validator | ✅ Presente |
| `lxc_nodes` | ebpf-node-2 | 192.168.2.211 | Validator | ✅ Presente |
| `lxc_nodes` | ebpf-node-3 | 192.168.2.212 | Validator | ✅ Presente |
| `attacker_nodes` | ebpf-attacker-1 | IPv6 | Attacker | ✅ Presente |
| `victim_nodes` | ebpf-victim-1 | IPv6 | Victim | ✅ Presente |
| `monitoring` | localhost | localhost | Monitoring | ✅ Presente |
| `dev_environment` | localhost | localhost | Development | ✅ Presente |

### 2.2 Evaluación de Cobertura

| Actor | Definido en Inventario | Tiene Playbook | Tiene Rol | Deploy Automático |
|-------|------------------------|----------------|-----------|-------------------|
| ebpf-node-1 | ✅ | ✅ | ✅ | ✅ |
| ebpf-node-2 | ✅ | ✅ | ✅ | ✅ |
| ebpf-node-3 | ✅ | ✅ | ✅ | ✅ |
| ebpf-attacker-1 | ✅ | ❌ | ❌ | ❌ |
| ebpf-victim-1 | ✅ | ❌ | ❌ | ❌ |
| Monitoring Stack | ✅ | ✅ | ✅ | ✅ |

### 2.3 Brechas Identificadas

| Brecha | Severidad | Descripción |
|--------|-----------|-------------|
| **B1** | ALTA | ebpf-attacker-1 no tiene playbook de deploy ni rol específico |
| **B2** | ALTA | ebpf-victim-1 no tiene playbook de deploy ni rol específico |
| **B3** | MEDIA | IPs IPv6 en attacker/victim nodes pueden causar problemas de conectividad |
| **B4** | BAJA | Credenciales Grafana hardcoded en inventario |

---

## 3. Análisis de Playbooks

### 3.1 Playbooks Disponibles

| Playbook | Propósito | Estado | Tags |
|----------|-----------|--------|------|
| [`deploy_cluster.yml`](ansible/playbooks/deploy_cluster.yml) | Setup completo del cluster LXC | ✅ Funcional | cluster, network, deploy |
| [`deploy.yml`](ansible/playbooks/deploy.yml) | Deploy de eBPF node en nodos remotos | ✅ Funcional | deploy, build, service |
| [`health_check.yml`](ansible/playbooks/health_check.yml) | Verificar estado del nodo | ✅ Funcional | health |
| [`backup.yml`](ansible/playbooks/backup.yml) | Backup automatizado | ✅ Funcional | backup |
| [`disaster_recovery.yml`](ansible/playbooks/disaster_recovery.yml) | Recuperación de desastres | ✅ Funcional | recovery |
| [`factory_reset.yml`](ansible/playbooks/factory_reset.yml) | Factory reset de nodos | ✅ Funcional | reset |
| [`fix_network.yml`](ansible/playbooks/fix_network.yml) | Reparar conectividad | ✅ Funcional | network |
| [`rebuild_and_restart.yml`](ansible/playbooks/rebuild_and_restart.yml) | Reconstruir y reiniciar | ✅ Funcional | rebuild |
| [`repair_and_restart.yml`](ansible/playbooks/repair_and_restart.yml) | Reparar y reiniciar | ✅ Funcional | repair |
| [`rollback.yml`](ansible/playbooks/rollback.yml) | Rollback de despliegue | ✅ Funcional | rollback |
| [`setup_dev_environment.yml`](ansible/playbooks/setup_dev_environment.yml) | Ambiente de desarrollo | ✅ Funcional | dev |
| [`setup_ebpf_nodes.yml`](ansible/playbooks/setup_ebpf_nodes.yml) | Setup de nodos eBPF | ✅ Funcional | setup |

### 3.2 Análisis de deploy_cluster.yml

**Flujo de Ejecución:**
```
1. Crear red LXC (lxdbr1)
2. Habilitar IP forwarding
3. Configurar iptables
4. Lanzar 3 nodos LXC (ebpf-node-1, 2, 3)
5. Configurar límites de memoria
6. Configurar DNS
7. Configurar red (IPv4)
8. Instalar dependencias del sistema
9. Instalar Rust (nightly)
10. Instalar bpf-linker
11. Configurar red por nodo
12. Deploy de binario
13. Iniciar servicio systemd
```

**Observaciones:**
- ✅ Crea los 3 nodos validator correctamente
- ✅ Configura red IPv4 consistente con Prometheus targets
- ❌ NO crea nodos attacker ni victim
- ❌ NO configura monitoring stack (depende de playbook separado)
- ⚠️ IPs hardcodeadas (192.168.2.210-212)

---

## 4. Análisis de Roles

### 4.1 Roles Disponibles

| Rol | Propósito | Estado |
|-----|-----------|--------|
| `common` | Variables comunes | ✅ |
| `dependencies` | Instalar dependencias del sistema | ✅ |
| `lxc_node` | Gestionar nodos LXC | ✅ |
| `monitoring` | Prometheus/Grafana stack | ⚠️ |
| `dev_environment` | Ambiente de desarrollo | ✅ |

### 4.2 Rol monitoring - Análisis Detallado

**Template Prometheus** ([`prometheus.yml.j2`](ansible/roles/monitoring/templates/prometheus.yml.j2)):
```yaml
scrape_configs:
  - job_name: 'ebpf_nodes'
    static_configs:
      - targets: ['192.168.2.11:9090']  # i=1 → 10+1=11
      - targets: ['192.168.2.12:9090']  # i=2 → 10+2=12
      - targets: ['192.168.2.13:9090']  # i=3 → 10+3=13
```

**INCONSISTENCIA CRÍTICA:** El template usa `range(1, 4)` con `10 + i`, lo que genera IPs `.11`, `.12`, `.13`. Pero el inventario define los nodos con IPs `.210`, `.211`, `.212`.

| Fuente | IP Node-1 | IP Node-2 | IP Node-3 |
|--------|-----------|-----------|-----------|
| Inventario | 192.168.2.210 | 192.168.2.211 | 192.168.2.212 |
| Template Prometheus | 192.168.2.11 | 192.168.2.12 | 192.168.2.13 |
| monitoring/prometheus.yml | 192.168.2.210 | 192.168.2.211 | 192.168.2.212 |

**Template Docker Compose** ([`docker-compose.monitoring.yml.j2`](ansible/roles/monitoring/templates/docker-compose.monitoring.yml.j2)):
- ❌ NO incluye health checks
- ❌ NO incluye `ebpf-log-forwarder` con la misma configuración que `monitoring/docker-compose.yml`
- ❌ Grafana NO tiene `depends_on: tempo`
- ⚠️ Retención Prometheus 30d vs 15d en monitoring/

---

## 5. Consistencia del Sistema de Observabilidad

### 5.1 Comparación: Ansible vs monitoring/

| Característica | Ansible Template | monitoring/ | Consistente |
|---------------|------------------|-------------|-------------|
| **Prometheus Image** | v2.48.0 | v2.48.0 | ✅ |
| **Grafana Image** | 10.2.0 | 10.2.0 | ✅ |
| **Loki Image** | 2.9.0 | 2.9.0 | ✅ |
| **Tempo Image** | 2.3.0 | 2.3.0 | ✅ |
| **Alertmanager** | ✅ | ✅ | ✅ |
| **Promtail** | ✅ | ✅ | ✅ |
| **Node Exporter** | ✅ | ✅ | ✅ |
| **Log Forwarder** | Parcial | ✅ Completo | ❌ |
| **Health Checks** | ❌ | ✅ Todos | ❌ |
| **Retención Prometheus** | 30d | 15d | ❌ |
| **IPs Nodos** | .11, .12, .13 | .210, .211, .212 | ❌ CRÍTICA |
| **Network Name** | ebpf-observability | ebpf-observability | ✅ |
| **Grafana Datasources** | API call | File provisioning | ❌ |
| **Dashboards** | 1 template | 12 JSON files | ❌ |

### 5.2 Servicios en monitoring/docker-compose.yml

| Servicio | Puerto | Health Check | Volumen | Estado |
|----------|--------|--------------|---------|--------|
| prometheus | 9090 | ✅ | ✅ prometheus-data | ✅ |
| alertmanager | 9093 | ✅ | ✅ alertmanager-data | ✅ |
| grafana | 3000 | ✅ | ✅ grafana-data, grafana-config | ✅ |
| loki | 3100 | ✅ | ✅ loki-data | ✅ |
| promtail | 9080 | ✅ | ✅ promtail-data | ✅ |
| tempo | 3200, 9097, 4317, 4318 | ✅ | ✅ tempo-data | ✅ |
| node-exporter | 9100 | ✅ | ✅ host mounts | ✅ |
| ebpf-log-forwarder | N/A | ❌ | ✅ positions.json | ⚠️ |

### 5.3 Dashboards de Grafana

| Dashboard | Ubicación | Provisionado | Estado |
|-----------|-----------|--------------|--------|
| ebpf-cluster.json | provisioning/dashboards/ | ✅ | ✅ |
| ebpf-debug.json | provisioning/dashboards/ | ✅ | ✅ |
| consensus.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| health-overview.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| log-pipeline-health.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| network-activity-debug.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| network-p2p.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| transactions.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| block-generator-debug.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| security-threat.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| consensus-integrity.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| network-attack-surface.json | grafana/dashboards/ | ⚠️ Manual | ❓ |
| ebpf-security-monitor.json | grafana/dashboards/ | ⚠️ Manual | ❓ |

**Problema:** Solo 2 dashboards están en `provisioning/dashboards/` (auto-provisionados). Los 11 restantes están en `grafana/dashboards/` pero NO están configurados para auto-provisioning.

### 5.4 Flujo de Datos de Observabilidad

```
┌─────────────────────────────────────────────────────────────────┐
│                    NODOS eBPF (LXC)                              │
│                                                                 │
│  ebpf-node-1 (:9090) ──┐                                       │
│  ebpf-node-2 (:9090) ──┼──▶ Prometheus (scrape cada 15s)       │
│  ebpf-node-3 (:9090) ──┘                                       │
│                                                                 │
│  journalctl logs ──▶ ebpf-log-forwarder ──▶ Loki (:3100)       │
│                                                                 │
│  OTLP traces ────────────────────────────▶ Tempo (:4317)       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    MONITORING STACK                              │
│                                                                 │
│  Prometheus ──▶ Grafana (metrics)                               │
│  Loki ────────▶ Grafana (logs)                                  │
│  Tempo ───────▶ Grafana (traces)                                │
│                                                                 │
│  Alertmanager ◀── Prometheus (alerts)                           │
└─────────────────────────────────────────────────────────────────┘
```

---

## 6. Brechas e Inconsistencias

### 6.1 Brechas Críticas

| ID | Descripción | Impacto | Ubicación |
|----|-------------|---------|-----------|
| **C1** | IPs Prometheus inconsistentes entre Ansible y monitoring/ | Prometheus NO puede scrapeear nodos | [`prometheus.yml.j2`](ansible/roles/monitoring/templates/prometheus.yml.j2:21) |
| **C2** | Sin playbook para nodos attacker/victim | No se pueden desplegar automáticamente | ansible/playbooks/ |
| **C3** | Dashboards no provisionados automáticamente | Requieren configuración manual en Grafana | provisioning/dashboards/ |
| **C4** | Health checks ausentes en template Ansible | Sin detección automática de fallos | [`docker-compose.monitoring.yml.j2`](ansible/roles/monitoring/templates/docker-compose.monitoring.yml.j2) |

### 6.2 Brechas Altas

| ID | Descripción | Impacto | Ubicación |
|----|-------------|---------|-----------|
| **H1** | Retención Prometheus diferente (30d vs 15d) | Confusión en política de datos | Templates vs monitoring/ |
| **H2** | ebpf-log-forwarder sin health check | Pérdida de logs no detectada | docker-compose.yml |
| **H3** | Grafana datasources por API vs file | Inconsistencia en provisioning | monitoring role |
| **H4** | Sin variables de entorno para credenciales | Seguridad comprometida | inventory/hosts.yml |

### 6.3 Brechas Medias

| ID | Descripción | Impacto |
|----|-------------|---------|
| **M1** | Network name hardcodeado en deploy_cluster.yml | Flexibilidad reducida |
| **M2** | Sin validación de conectividad post-deploy | Fallos silenciosos |
| **M3** | bootnodes vacío en group_vars | Los nodos no saben conectarse |
| **M4** | Sin playbook de integración end-to-end | Testing manual requerido |

---

## 7. Recomendaciones

### 7.1 Prioridad P0 (Corregir Inmediatamente)

#### R0-1: Sincronizar IPs de Prometheus

**Opción A:** Corregir template Ansible para usar IPs del inventario:
```jinja2
# ansible/roles/monitoring/templates/prometheus.yml.j2
- job_name: 'ebpf_nodes'
  static_configs:
{% for host in groups['lxc_nodes'] %}
    - targets: ['{{ hostvars[host].node_ip }}:9090']
      labels:
        node_name: '{{ hostvars[host].node_name }}'
{% endfor %}
```

**Opción B:** Usar el archivo monitoring/prometheus.yml como fuente de verdad y eliminar el template.

#### R0-2: Crear Playbooks para Attacker/Victim Nodes

Crear `ansible/playbooks/deploy_attacker.yml` y `ansible/playbooks/deploy_victim.yml` con:
- Build del binario con flags específicos
- Configuración de red
- Servicio systemd con parámetros de ataque

#### R0-3: Provisionar todos los Dashboards

Mover todos los dashboards a `provisioning/dashboards/` y actualizar `dashboards.yaml`:
```yaml
apiVersion: 1
providers:
  - name: 'default'
    orgId: 1
    folder: ''
    type: file
    disableDeletion: false
    editable: true
    options:
      path: /var/lib/grafana/dashboards
      foldersFromFilesStructure: false
```

### 7.2 Prioridad P1 (Corto Plazo)

#### R1-1: Unificar Definiciones de Monitoring

Decidir si usar:
- **Opción A:** Solo Ansible (generar todo desde templates)
- **Opción B:** Solo monitoring/ (archivos estáticos, Ansible solo los copia)

Recomendación: **Opción B** - Los archivos en monitoring/ son más completos y tienen health checks.

#### R1-2: Agregar Health Checks al Template

Si se mantiene el template Ansible, agregar health checks a todos los servicios.

#### R1-3: Externalizar Credenciales

Usar Ansible Vault para credenciales sensibles:
```bash
ansible-vault encrypt inventory/group_vars/secrets.yml
```

### 7.3 Prioridad P2 (Mediano Plazo)

#### R2-1: Playbook de Integración End-to-End

Crear `ansible/playbooks/full_deploy.yml` que ejecute:
1. deploy_cluster.yml (crear nodos LXC)
2. deploy.yml (deploy binario en nodos)
3. setup monitoring stack
4. Verificar conectividad
5. Verificar métricas en Prometheus
6. Verificar dashboards en Grafana

#### R2-2: Validación Post-Deploy

Agregar tasks de validación:
```yaml
- name: Verify Prometheus can scrape nodes
  uri:
    url: "http://localhost:9090/api/v1/targets"
    return_content: true
  register: prometheus_targets
  retries: 5
  delay: 10

- name: Check all targets are UP
  assert:
    that:
      - prometheus_targets.json.data.activeTargets | selectattr('health', 'equalto', 'up') | list | length >= 3
```

---

## 8. Diagrama de Arquitectura Actual vs Deseada

### Arquitectura Actual (Fragmentada)

```
┌─────────────────────────┐     ┌──────────────────────────┐
│   Ansible (Templates)   │     │   monitoring/ (Static)   │
│                         │     │                          │
│  prometheus.yml.j2      │     │  prometheus.yml          │
│  docker-compose.j2      │     │  docker-compose.yml      │
│  dashboard.json.j2      │     │  12 dashboard JSONs      │
│                         │     │                          │
│  IPs: .11, .12, .13    │     │  IPs: .210, .211, .212   │
│  Sin health checks      │     │  Con health checks       │
│  Retención: 30d         │     │  Retención: 15d          │
└─────────────────────────┘     └──────────────────────────┘
         │                                │
         ▼                                ▼
    INCONSISTENTE ←────────────────→ INCONSISTENTE
```

### Arquitectura Deseada (Unificada)

```
┌─────────────────────────────────────────────────────────────┐
│                    monitoring/ (Fuente de Verdad)            │
│                                                             │
│  docker-compose.yml  ←── Definición completa de servicios   │
│  prometheus.yml      ←── IPs correctas, scrape configs      │
│  grafana/provisioning/ ←── Todos los dashboards auto-loaded │
│  loki/               ←── Configuración Loki                 │
│  tempo/              ←── Configuración Tempo                │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                    Ansible (Orquestación)                    │
│                                                             │
│  deploy_cluster.yml  ←── Crear nodos LXC                   │
│  deploy.yml          ←── Build y deploy binario             │
│  monitoring role     ←── Copiar monitoring/ y docker-compose up │
└─────────────────────────────────────────────────────────────┘
```

---

*Documento generado como parte de la auditoría de IaC y Observabilidad del sistema eBPF Blockchain.*
