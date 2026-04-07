# ETAPA 1: ESTABILIZACIÓN

**Estado:** Pendiente  
**Duración estimada:** 2 semanas  
**Prioridad:** 🔴 CRÍTICA  
**Meta:** Alcanzar 100% de funcionalidad estable

---

## 1. Resumen Ejecutivo

Esta etapa se enfoca en resolver los problemas críticos de estabilidad y funcionalidad que impiden que el proyecto sea un Proof of Concept (PoC) funcional. Se estima un 65-80% de completitud actual en estas áreas.

### Métricas Actuales vs. Objetivo

| Área | Estado Actual | Meta PoC | Crítica |
|------|---------------|----------|---------|
| Persistencia de datos | 0% | 100% | 🔴 |
| Estabilidad de red P2P | 70% | 100% | 🟠 |
| Métricas completas | 80% | 100% | 🟡 |
| Automatización | 65% | 100% | 🟠 |

---

## 2. Problemas Críticos Identificados

### 2.1 Persistencia de Datos - Vólátil (0% completitud)

**Problema:** El sistema utiliza `/tmp` para almacenamiento, perdiendo todos los datos al reiniciar.

**Impacto:**
- Pérdida completa de estado del blockchain
- Necesidad de regenerar todos los datos al reiniciar
- Imposibilidad de persistencia entre sesiones

**Ubicación del código:**
```
ebpf-node/src/storage/
```

**Requisitos:**
- Cambiar almacenamiento de `/tmp` a `/var/lib/ebpf-blockchain/`
- Implementar migración de datos para versiones anteriores
- Configurar permisos correctos para el usuario del servicio
- Implementar backup automático

### 2.2 Red P2P - Estabilidad Parcial (70% completitud)

**Problema:** La red utiliza libp2p con gossipsub v1.1 y QUIC, pero presenta inestabilidades.

**Impacto:**
- Desconexiones frecuentes entre nodos
- Latencia inconsistente en la propagación de mensajes
- Problemas de sincronización entre nodos

**Ubicación del código:**
```
ebpf-node/src/network/
```

**Requisitos:**
- Mejorar configuración de reconnect automática
- Optimizar parámetros de gossipsub
- Implementar health-checks de conexión
- Aumentar timeout de QUIC

### 2.3 Métricas - Reportes Incompletos (80% completitud)

**Problema:** Los peers y mensajes no se reportan correctamente en Prometheus.

**Impacto:**
- Métricas incompletas de red
- Dificultad para monitorear el estado de la red
- Falta de visibilidad en el tráfico P2P

**Ubicación del código:**
```
ebpf-node/src/metrics/
```

**Requisitos:**
- Corregir contadores de peers
- Implementar métricas de mensajes por tipo
- Agregar métricas de latencia por peer
- Mejorar documentación de métricas

### 2.4 eBPF XDP - Blacklist Reactiva (85% completitud)

**Problema:** El sistema solo detecta ataques y aplica blacklist después de ocurridos.

**Impacto:**
- Vulnerabilidad durante el periodo de detección
- Posibilidad de ataques DDoS iniciales

**Ubicación del código:**
```
ebpf-node/src/ebpf/xdp/
```

**Requisitos:**
- Implementar detección proactiva
- Agregar threshold configurable
- Implementar whitelist inicial

### 2.5 kprobes - Latency Tracking (100% completitud ✅)

**Estado:** Implementado correctamente.

**Nota:** No requiere cambios en esta etapa.

### 2.6 LXC Network - Tráfico Bloqueado (0% completitud)

**Problema:** El tráfico entre contenedores LXC está bloqueado en openSUSE.

**Impacto:**
- Imposibilidad de comunicación entre nodos en LXD
- Requiere configuración específica de red

**Requisitos:**
- Configurar reglas de firewall LXD
- Implementar bridge networking correcto
- Configurar NAT si es necesario

### 2.7 Ansible - Manejo de Errores Deficiente (65% completitud)

**Problema:** El playbook de ansible tiene manejo de errores deficiente.

**Impacto:**
- Fallos silenciosos en el despliegue
- Dificultad para diagnosticar problemas
- Rollback complicado

**Ubicación del código:**
```
ansible/
```

**Requisitos:**
- Implementar error handling robusto
- Agregar rollback automático
- Implementar health-checks post-despliegue
- Mejorar logging

---

## 3. Plan de Implementación

### 3.1 Semana 1: Persistencia y Red

#### Día 1-2: Implementación de RocksDB

**Tareas:**
1. Investigar configuración actual de almacenamiento
2. Implementar cambio de `/tmp` a `/var/lib/ebpf-blockchain/`
3. Configurar permisos de directorio
4. Implementar migración de datos antiguos

**Código objetivo:**
```rust
// ebpf-node/src/storage/rocksdb.rs
// Cambiar de /tmp a /var/lib
```

**Criterios de aceptación:**
- [ ] Los datos persisten después de reinicio
- [ ] Permisos correctos para usuario no-root
- [ ] Migración automática de datos antiguos
- [ ] Backup automático cada 24h

#### Día 3-4: Mejora de Estabilidad P2P

**Tareas:**
1. Configurar reconnect automático con backoff exponencial
2. Optimizar parámetros de gossipsub
3. Implementar health-checks periódicos
4. Aumentar timeout de QUIC de 30s a 60s

**Código objetivo:**
```rust
// ebpf-node/src/network/p2p.rs
// Mejorar configuración de conexión
```

**Criterios de aceptación:**
- [ ] Reconexión automática tras fallos
- [ ] 99% de uptime de conexión
- [ ] Menos de 1s de latencia promedio
- [ ] Health-checks cada 30s

#### Día 5-6: Corrección de Métricas

**Tareas:**
1. Corregir contadores de peers
2. Implementar métricas de mensajes por tipo
3. Agregar métricas de latencia por peer
4. Documentar todas las métricas expuestas

**Código objetivo:**
```rust
// ebpf-node/src/metrics/
// Corregir y expandir métricas
```

**Criterios de aceptación:**
- [ ] Todos los peers contados correctamente
- [ ] Mensajes categorizados por tipo
- [ ] Latencia medida por peer
- [ ] Documentación completa en OpenAPI

#### Día 7: Pruebas Integración

**Tareas:**
1. Pruebas de persistencia de datos
2. Pruebas de estabilidad P2P (24h)
3. Pruebas de métricas completas
4. Documentación de cambios

**Criterios de aceptación:**
- [ ] Todas las pruebas pasan
- [ ] Métricas 100% completas
- [ ] Documentación actualizada

### 3.2 Semana 2: eBPF, LXC y Ansible

#### Día 8-9: eBPF XDP Proactivo

**Tareas:**
1. Implementar detección proactiva de anomalías
2. Agregar threshold configurable
3. Implementar whitelist inicial
4. Pruebas de performance

**Código objetivo:**
```rust
// ebpf-node/src/ebpf/xdp/blacklist.rs
// Implementar detección proactiva
```

**Criterios de aceptación:**
- [ ] Detección antes de ataques masivos
- [ ] Threshold ajustable via config
- [ ] Whitlist de nodos confiables
- [ ] 0% de impacto en performance

#### Día 10-11: Configuración LXC

**Tareas:**
1. Investigar configuración de red LXD en openSUSE
2. Configurar bridge networking
3. Implementar reglas de firewall
4. Documentar configuración requerida

**Código objetivo:**
```
ansible/playbooks/network_lxc.yml
```

**Criterios de aceptación:**
- [ ] Comunicación entre contenedores LXC
- [ ] Reglas de firewall seguras
- [ ] NAT configurado si es necesario
- [ ] Documentación clara de requirements

#### Día 12-13: Mejora Ansible

**Tareas:**
1. Implementar error handling robusto
2. Agregar rollback automático
3. Implementar health-checks post-despliegue
4. Mejorar logging

**Código objetivo:**
```
ansible/playbooks/deploy.yml
ansible/playbooks/rollback.yml
ansible/playbooks/health_check.yml
```

**Criterios de aceptación:**
- [ ] Errores reportados claramente
- [ ] Rollback automático en fallos
- [ ] Health-checks post-despliegue
- [ ] Logging detallado en todos los pasos

#### Día 14: Pruebas Finales y Documentación

**Tareas:**
1. Pruebas integrales de todos los componentes
2. Documentación final de cambios
3. Actualización de README
4. Release notes

**Criterios de aceptación:**
- [ ] Todas las pruebas pasan
- [ ] Documentación completa
- [ ] Release notes actualizados
- [ ] Métricas 100% completas

---

## 4. Configuración de Entorno

### 4.1 Requisitos de Sistema

```bash
# openSUSE Leap 15.4 o superior
# Kernel >= 5.10 con BTF habilitado
# Rust Nightly (para Aya/eBPF)
# LXD >= 4.0
# Docker >= 20.10
```

### 4.2 Directorio de Persistencia

```bash
# Crear directorio para datos persistentes
sudo mkdir -p /var/lib/ebpf-blockchain/data
sudo chown -R $USER:$USER /var/lib/ebpf-blockchain
sudo chmod 750 /var/lib/ebpf-blockchain

# Configurar backup automático
cat > /etc/cron.d/ebpf-blockchain-backup << EOF
0 2 * * * $USER /var/lib/ebpf-blockchain/bin/backup.sh
EOF
```

### 4.3 Configuración de Red LXC

```yaml
# ansible/playbooks/config_lxc.yml
- name: Configurar red LXC para ebpf-blockchain
  hosts: all
  become: yes
  tasks:
    - name: Crear bridge para contenedores
      command: lxc network create ebpf-bridge type bridge
      register: bridge_result
      
    - name: Configurar firewall
      lineinfile:
        path: /etc/lxc/network/ebpf-bridge/lxc.network
        line: 'lxc.net.0.type: bridge'
      notify: restart lxd
```

### 4.4 Variables de Entorno

```bash
# .env para desarrollo
ROCKSDB_PATH=/var/lib/ebpf-blockchain/data
NETWORK_P2P_PORT=9000
NETWORK_QUIC_PORT=9001
METRICS_PORT=9090
LXC_BRIDGE_NAME=ebpf-bridge

# Variables para producción
ROCKSDB_PATH=/var/lib/ebpf-blockchain/data
BACKUP_ENABLED=true
BACKUP_INTERVAL=24h
METRICS_EXPORT=prometheus
```

---

## 5. Tests y Validación

### 5.1 Tests Unitarios

**Cobertura requerida:** ≥80%

```bash
cd ebpf-node
cargo test --lib -- --test-threads=1
```

### 5.2 Tests de Integración

```bash
cd ebpf-node
cargo test --test integration -- --test-threads=1
```

### 5.3 Pruebas de Estabilidad

```bash
# Prueba de persistencia
./tests/persistence_test.sh

# Prueba de red (24h)
./tests/network_stability_test.sh

# Prueba de métricas
./tests/metrics_test.sh
```

### 5.4 Criterios de Aceptación

- [ ] Todos los tests unitarios pasan (≥80% cobertura)
- [ ] Pruebas de integración exitosas
- [ ] Persistencia verificada tras 3 reinicios
- [ ] Red estable por 24h sin desconexiones
- [ ] Métricas completas y correctas
- [ ] Documentación actualizada

---

## 6. Rollback Plan

### 6.1 Punto de Restauración

```bash
# Guardar estado actual antes de cambios
cp -r /var/lib/ebpf-blockchain /var/lib/ebpf-blockchain.backup.$(date +%Y%m%d)
```

### 6.2 Comandos de Rollback

```bash
# Restaurar datos antiguos
sudo mv /var/lib/ebpf-blockchain.backup.$DATE /var/lib/ebpf-blockchain

# Restaurar configuración anterior
git checkout HEAD~1 -- ansible/
git checkout HEAD~1 -- ebpf-node/src/storage/
```

### 6.3 Verificación Post-Rollback

```bash
# Verificar integridad de datos
./bin/ebpf-blockchain-cli db check

# Verificar servicios
systemctl status ebpf-blockchain
```

---

## 7. Monitoreo Durante Implementación

### 7.1 Métricas a Monitorear

- Tiempo de despliegue (meta: <15 min)
- Uptime del servicio (meta: 99.9%)
- Latencia promedio (meta: <1s)
- Tasa de errores (meta: 0%)
- Memoria utilizada (meta: <1GB)

### 7.2 Logging

```bash
# Logs de aplicación
journalctl -u ebpf-blockchain -f

# Logs de eBPF
cat /sys/kernel/debug/tracing/trace_pipe

# Logs de red
./bin/ebpf-blockchain-cli network stats
```

### 7.3 Alertas

Configurar alertas para:
- Servicio caído
- Alto uso de memoria (>80%)
- Alta latencia (>2s)
- Errores de conexión (>10/hora)

---

## 8. Entregables

### 8.1 Código

- [ ] Implementación de RocksDB con persistencia
- [ ] Mejora de estabilidad P2P
- [ ] Corrección de métricas
- [ ] eBPF XDP proactivo
- [ ] Configuración LXC
- [ ] Ansible mejorado

### 8.2 Documentación

- [ ] README actualizado
- [ ] CHANGELOG actualizado
- [ ] Documentación de API
- [ ] Guía de despliegue
- [ ] Runbook de operaciones

### 8.3 Tests

- [ ] Tests unitarios (≥80% cobertura)
- [ ] Tests de integración
- [ ] Tests de estabilidad (24h)
- [ ] Tests de carga

### 8.4 Infraestructura

- [ ] Playbooks Ansible actualizados
- [ ] Scripts de backup automatizados
- [ ] Configuración LXC documentada
- [ ] Dockerfiles actualizados

---

## 9. Riesgos y Mitigación

### 9.1 Riesgos Técnicos

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| Incompatibilidad RocksDB | Baja | Alto | Testing exhaustivo antes |
| Problemas de red LXC | Media | Medio | Documentación clara, fallback |
| Performance eBPF | Baja | Medio | Profiling antes de merge |
| Rollback complejo | Baja | Medio | Tests de rollback |

### 9.2 Riesgos de Tiempo

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| Retraso en implementación | Media | Medio | Priorizar features críticas |
| Bugs no esperados | Alta | Medio | Buffer de tiempo en plan |
| Dependencias externas | Baja | Alto | Testing con versiones específicas |

---

## 10. Criterios de Finalización

La Etapa 1 se considera completada cuando:

1. ✅ **Persistencia:** Datos sobreviven reinicios sin pérdida
2. ✅ **Red P2P:** 99% de uptime en pruebas de 24h
3. ✅ **Métricas:** 100% de métricas implementadas y funcionando
4. ✅ **eBPF XDP:** Detección proactiva sin performance impact
5. ✅ **LXC:** Comunicación entre contenedores estable
6. ✅ **Ansible:** Rollback automático funcionando
7. ✅ **Tests:** ≥80% cobertura y todas las pruebas pasan
8. ✅ **Documentación:** Completa y actualizada

---

## 11. Referencias

- [Documento 01_ESTRUCTURA_PROYECTO.md](../01_ESTRUCTURA_PROYECTO.md)
- [Diagnóstico inicial del proyecto](./DIAGNOSTICO_INICIAL.md)
- [Esquema de arquitectura](../docs/ARCHITECTURE.md)
- [Especificación de requisitos](../docs/REQUIREMENTS.md)

---

## 12. Historial de Cambios

| Versión | Fecha | Cambios | Autor |
|---------|-------|---------|-------|
| 1.0 | 2026-01-26 | Creación inicial del documento | @ebpf-dev |

---

*Documento bajo revisión para Etapa 1 de la mejora del proyecto ebpf-blockchain*