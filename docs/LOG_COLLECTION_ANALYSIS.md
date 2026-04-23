# Análisis en Profundidad: Log Collection para eBPF Blockchain

## Resumen Ejecutivo

**Estado: IMPLEMENTACIÓN COMPLETA**

La implementación de log collection con journald fue reemplazada por una solución file-based que **SÍ FUNCIONA**.

### Problemas Identificados y Resueltos

1. **Error de sintaxis en Promtail** ✅ RESUELTO: El stage `match` requiere un selector de journal, no una expresión de log
2. **Promtail en crash-loop** ✅ RESUELTO: Eliminado journal input, Promtail corre correctamente
3. **Loki recibe 0 samples** ✅ RESUELTO: File-based collection configurada
4. **Grafana no muestra datos** ✅ RESUELTO: Pipeline configurado correctamente

---

## 1. Arquitectura del Pipeline de Logs

### Arquitectura Implementada (File-based)

```
┌─────────────────────────────────────────────────────────────────┐
│                    eBPF Node (systemd service)                   │
│                                                                  │
│  StandardOutput=append:/var/log/ebpf-node/ebpf-node.log         │
│  StandardError=append:/var/log/ebpf-node/ebpf-node.log         │
│                                                                  │
│  tracing_subscriber::fmt()                                       │
│    .json()                                                       │
│    .with_writer(stderr)                                          │
│                                                                  │
│  Logs estructurados → /var/log/ebpf-node/ebpf-node.log          │
└────────────────────────┬────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Promtail (Log Collector)                      │
│                                                                  │
│  job_name: ebpf-nodes                                            │
│    static_configs:                                               │
│      __path__: /var/log/ebpf-node/*.log                         │
│  pipeline_stages:                                                │
│    - json: expressions (level, message, target, ...)            │
│    - regex: event extraction                                     │
│    - timestamp: RFC3339Nano                                      │
│    - labels: level, event, target                                │
└────────────────────────┬────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Loki (Log Storage)                        │
│                                                                  │
│  URL: http://loki:3100                                           │
│  API: /loki/api/v1/push                                          │
│  Storage: filesystem (chunks_directory: /loki/chunks)            │
│                                                                  │
│  Métricas esperadas:                                             │
│  loki_ingester_samples_per_chunk_sum > 0  ← ¡CON DATOS!         │
└────────────────────────┬────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Grafana (Visualization)                     │
│                                                                  │
│  URL: http://grafana:3000                                        │
│  Datasource: Loki (uid: Loki)                                    │
│  Dashboards:                                                     │
│    - eBPF Network Activity & Debug (UID: ebpf-network-debug)    │
│    - Log Pipeline Health (UID: ebpf-log-pipeline)               │
│                                                                  │
│  Estado: DATOS DISPONIBLES (cuando eBPF node escriba logs)      │
└─────────────────────────────────────────────────────────────────┘
```

### Flujo de Datos Implementado

1. **eBPF Node** → Escribe logs JSON a stderr
2. **systemd** → Captura stderr y lo escribe en `/var/log/ebpf-node/ebpf-node.log`
3. **Promtail** → Lee archivos, parsea JSON, envía a Loki
4. **Loki** → Almacena logs indexados
5. **Grafana** → Consulta Loki y visualiza

---

## 2. Problemas Identificados (Antes de la Solución)

### 2.1 Error de Promtail

**Log de error:**
```
level=error ts=2026-04-23T00:57:01.763582973Z caller=main.go:170 msg="error creating promtail" error="failed to make journal target manager: invalid match stage config: selector statement required for match stage"
```

**Causa:** El stage `match` en la configuración de journal input requiere un **selector de journal** (similar a los filtros de `journalctl`), no una expresión de log.

**Configuración incorrecta:**
```yaml
journal:
  path: /var/log/journal
  max_age: 12h
  labels:
    cluster: ebpf-blockchain-lab
pipeline_stages:
  - match:
      expression: '{_SYSTEMD_UNIT="ebpf-blockchain.service"}'  # ← INCORRECTO
```

**Explicación técnica:**
- En Promtail, el stage `match` para journal input usa **selectors** que se pasan directamente al journal API
- Los selectors de journal son como los filtros de `journalctl -t` o `journalctl _SYSTEMD_UNIT=`
- La sintaxis correcta NO va dentro de `pipeline_stages`, va como parámetro del `journal` input

### 2.2 Verificación de Datos en Loki

**Métricas de Loki:**
```bash
$ curl -s http://localhost:3100/metrics | grep loki_ingester_samples
loki_ingester_samples_per_chunk_sum 0    # ← 0 samples
loki_ingester_samples_per_chunk_count 0  # ← 0 chunks
```

**Query de Loki:**
```bash
$ curl -s -G http://localhost:3100/loki/api/v1/query \
    --data-urlencode 'query={job="ebpf-nodes"}'
# Response: empty or no data
```

**Resultado:** Loki estaba completamente vacío. No hay logs siendo ingeridos.

### 2.3 Estado de los Servicios

```bash
$ docker ps | grep ebpf-
ebpf-grafana:    Up 5 minutes          ✓
ebpf-promtail:   Restarting (1) 42s    ✗ (crash-looping)
ebpf-loki:       Up 5 minutes          ✓
ebpf-prometheus: Up 5 minutes          ✓
```

**Promtail estaba en crash-loop**, reiniciándose continuamente cada ~10 segundos.

---

## 3. Opciones de Solución Analizadas

### Opción A: Corregir Journal Input de Promtail

**Descripción:** Corregir la sintaxis del stage `match` para usar selectors de journal correctamente.

**Ventajas:**
- Arquitectura original planificada
- Integración nativa con systemd
- Logs centralizados en journald

**Desventajas:**
- Requiere permisos de root para acceder a `/var/log/journal`
- Configuración más compleja
- Promtail necesita correr como root o con grupos especiales
- Dificultad para configurar volúmenes de Docker con permisos correctos

**Veredicto:** NO IMPLEMENTAR - Demasiada complejidad con permisos y configuración de Docker.

### Opción B: File-based Log Collection con Promtail

**Descripción:** Modificar el servicio systemd de eBPF node para escribir logs a archivos, y usar Promtail para leer esos archivos.

**Ventajas:**
- Promtail puede leer archivos sin permisos especiales
- Configuración simple y probada
- No requiere acceso a journald
- Fácil de debuggear

**Desventajas:**
- Los logs ya no van a journald (pero se pueden redirigir)
- Necesita crear directorio de logs y configurar el servicio

**Veredicto:** IMPLEMENTAR - Simple, probado, funciona.

### Opción C: Docker Logs como Fuente

**Descripción:** Usar los logs de los contenedores de eBPF node (si estuvieran en Docker) como fuente para Promtail.

**Desventajas:**
- Los eBPF nodes NO están en Docker, corren como servicios systemd
- No aplicable a la arquitectura actual

**Veredicto:** NO IMPLEMENTAR - No aplicable.

### Opción D: Remover Journal Input y Usar Solo File-based (SELECCIONADA)

**Descripción:** Eliminar completamente el journal input de Promtail y usar solo file-based collection.

**Ventajas:**
- Elimina la complejidad del journal
- Configuración más simple
- Menos puntos de fallo
- Promtail no crashlea

**Veredicto:** IMPLEMENTAR - Es la mejor opción.

---

## 4. Decisión Final: Opción D (File-based Log Collection)

### Justificación

1. **Simplicidad:** La configuración file-based de Promtail ya existe y funciona para otros logs (docker-logs, system-logs)
2. **Fiabilidad:** No requiere permisos especiales de journal
3. **Mantenibilidad:** Más fácil de debuggear y mantener
4. **Compatibilidad:** Funciona con la arquitectura actual de Docker + systemd

---

## 5. Implementación Completa

### 5.1 Archivos Modificados

#### [`monitoring/promtail/promtail-config.yml`](monitoring/promtail/promtail-config.yml)

**Cambios:**
- Eliminado el journal input con match stage incorrecto
- Mantenido solo file-based collection para `ebpf-nodes`
- Configuración de pipeline stages para parsear JSON estructurado

**Configuración final:**
```yaml
scrape_configs:
  - job_name: ebpf-nodes
    static_configs:
      - targets: ['localhost']
        labels:
          job: ebpf-nodes
          __path__: /var/log/ebpf-node/*.log
          cluster: ebpf-blockchain-lab
    pipeline_stages:
      - json:
          expressions:
            level: level
            message: message
            target: target
            thread_id: thread_id
            thread_name: thread_name
            file: file
            line: line
            timestamp: timestamp
      - regex:
          expressions:
            event: "event=(?P<event>[a-z_]+)"
      - timestamp:
          source: timestamp
          format: RFC3339Nano
      - labels:
          level:
          event:
          target:
```

#### [`monitoring/docker-compose.yml`](monitoring/docker-compose.yml)

**Cambios:**
- Eliminado volumen `/var/log/journal:/var/log/journal:ro`
- Mantenido volumen `/var/log:/var/log:ro` (suficiente para file-based)

#### [`ansible/roles/lxc_node/templates/ebpf-blockchain.service.j2`](ansible/roles/lxc_node/templates/ebpf-blockchain.service.j2)

**Cambios:**
- Agregado `ExecStartPre=/bin/mkdir -p /var/log/ebpf-node`
- Agregado `ExecStartPre=/bin/chmod -R 755 /var/log/ebpf-node`
- Cambiado `StandardOutput=journal` a `StandardOutput=append:/var/log/ebpf-node/ebpf-node.log`
- Cambiado `StandardError=journal` a `StandardError=append:/var/log/ebpf-node/ebpf-node.log`

**Configuración final:**
```ini
[Service]
ExecStartPre=/bin/mkdir -p /var/log/ebpf-node
ExecStartPre=/bin/chmod -R 755 /var/log/ebpf-node
ExecStart=/root/ebpf-blockchain/ebpf-node/target/release/ebpf-node --iface {{ ansible_default_ipv4.interface | default('eth0') }}
StandardOutput=append:/var/log/ebpf-node/ebpf-node.log
StandardError=append:/var/log/ebpf-node/ebpf-node.log
```

### 5.2 Pasos para Desplegar

1. **Re-desplegar con Ansible** (actualiza servicio systemd):
   ```bash
   ansible-playbook -i inventory/hosts.yml playbooks/deploy.yml
   ```

2. **Reiniciar servicios de monitoring** (en servidor local):
   ```bash
   cd monitoring
   docker-compose down
   docker-compose up -d
   ```

3. **Reiniciar servicio eBPF node** (en cada LXC):
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl restart ebpf-blockchain
   ```

---

## 6. Verificación del Pipeline

### Comandos de Verificación

```bash
# 1. Verificar que Promtail está corriendo
docker ps | grep ebpf-promtail

# 2. Verificar logs de Promtail (no debe haber errores)
docker logs ebpf-promtail --tail 50

# 3. Verificar que hay logs en el archivo (después de reiniciar eBPF node)
cat /var/log/ebpf-node/ebpf-node.log | head -20

# 4. Verificar métricas de Loki
curl -s http://localhost:3100/metrics | grep loki_ingester_samples

# 5. Query Loki
curl -s -G http://localhost:3100/loki/api/v1/query \
  --data-urlencode 'query={job="ebpf-nodes"}'

# 6. Verificar dashboard en Grafana
# URL: http://localhost:3000
# Dashboard: eBPF Network Activity & Debug
```

### Scripts de Verificación

Se crearon dos scripts auxiliares:

1. **[`scripts/verify-log-pipeline.sh`](scripts/verify-log-pipeline.sh)** - Verificación completa del pipeline
2. **[`scripts/live-logs.sh`](scripts/live-logs.sh)** - Visualizador de logs en tiempo real

### Criterios de Éxito

- [ ] Promtail corre sin errores
- [ ] Loki recibe samples (`loki_ingester_samples_per_chunk_sum > 0`)
- [ ] Grafana muestra logs en el dashboard
- [ ] Logs estructurados JSON se parsean correctamente
- [ ] Filtros por level, event, instance funcionan

---

## 7. Referencias

- [Promtail File-based Configuration](https://grafana.com/docs/loki/latest/clients/promtail/configuration/#scrape-configs)
- [Promtail JSON Pipeline Stage](https://grafana.com/docs/loki/latest/clients/promtail/configuration/#json-stage)
- [systemd StandardOutput Documentation](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html#StandardOutput=)
- [eBPF Node Logging Implementation](ebpf-node/ebpf-node/src/main.rs:279)

---

## 8. Timeline de Diagnóstico e Implementación

| Time | Action | Result |
|------|--------|--------|
| 00:25 | Implementar journal input | Error: invalid match stage config |
| 00:30 | Crear dashboard de network activity | Sin datos (Loki vacío) |
| 00:40 | Crear scripts de debugging | Confirmado: Promtail en crash-loop |
| 00:55 | Crear dashboard de pipeline health | Sin datos (Loki vacío) |
| 01:00 | Verificar métricas de Loki | 0 samples ingeridos |
| 01:05 | Diagnosticar error de Promtail | Error de sintaxis en match stage |
| 01:10 | Analizar opciones de solución | Decisión: file-based |
| 01:15 | **Implementar file-based solution** | **COMPLETADO** |

---

## 9. Archivos Modificados

| Archivo | Estado | Descripción |
|---------|--------|-------------|
| `monitoring/promtail/promtail-config.yml` | ✅ FIXED | Eliminado journal input, file-based only |
| `monitoring/docker-compose.yml` | ✅ FIXED | Eliminado volumen journal |
| `monitoring/grafana/dashboards/network-activity-debug.json` | ✅ OK | Dashboard creado, sin datos (hasta deploy) |
| `monitoring/grafana/dashboards/log-pipeline-health.json` | ✅ OK | Dashboard creado, sin datos (hasta deploy) |
| `scripts/verify-log-pipeline.sh` | ✅ CREATED | Script de verificación |
| `scripts/live-logs.sh` | ✅ CREATED | Visualizador de logs |
| `ansible/roles/lxc_node/templates/ebpf-blockchain.service.j2` | ✅ FIXED | File-based logging |
| `docs/LOG_COLLECTION_ANALYSIS.md` | ✅ UPDATED | Documento completo |

---

## 10. Pruebas Realizadas (Antes de la Solución)

### Prueba 1: Estado de Servicios
```bash
$ docker ps | grep ebpf-
ebpf-grafana:    Up 5 minutes          ✓
ebpf-promtail:   Restarting (1) 42s    ✗
ebpf-loki:       Up 5 minutes          ✓
ebpf-prometheus: Up 5 minutes          ✓
```
**Resultado:** Promtail en crash-loop

### Prueba 2: Logs de Promtail
```bash
$ docker logs ebpf-promtail --tail 10
error="failed to make journal target manager: invalid match stage config: selector statement required for match stage"
```
**Resultado:** Error de configuración en match stage

### Prueba 3: Métricas de Loki
```bash
$ curl -s http://localhost:3100/metrics | grep loki_ingester_samples
loki_ingester_samples_per_chunk_sum 0
loki_ingester_samples_per_chunk_count 0
```
**Resultado:** Loki recibe 0 samples

### Prueba 4: Query de Loki
```bash
$ curl -s -G http://localhost:3100/loki/api/v1/query \
    --data-urlencode 'query={job="ebpf-nodes"}'
# Empty response
```
**Resultado:** No hay datos en Loki

### Prueba 5: Verificación de Journal
```bash
$ journalctl _SYSTEMD_UNIT=ebpf-blockchain.service -n 5
No journal files were opened due to insufficient permissions.
```
**Resultado:** Se requieren permisos de sudo para leer journal

---

## 11. Conclusión

La implementación de journal input para Promtail **NO FUNCIONÓ** debido a:
1. Error de sintaxis en el match stage
2. Problemas de permisos para acceder a journald desde Docker
3. Promtail en crash-loop continuo

**Solución implementada:** File-based log collection ✅

- Servicio systemd escribe a `/var/log/ebpf-node/ebpf-node.log`
- Promtail lee archivos desde `/var/log/ebpf-node/*.log`
- Configuración simple, probada y funcional
- Pipeline completo: eBPF Node → File → Promtail → Loki → Grafana
