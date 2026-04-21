# eBPF Blockchain POC - Etapa 1: Corrección de Problemas Críticos

## Descripción General
La primera etapa del proyecto se enfoca en resolver los problemas críticos que impiden la demostración funcional del POC. Este esfuerzo se centra en restaurar la conectividad entre nodos LXD, hacer funcionar completamente el sistema de métricas y mejorar la automatización de despliegue con Ansible.

## Objetivos Específicos

### 1. Restaurar Conectividad entre Nodos LXD
- Configurar reglas de firewall para permitir comunicación entre contenedores
- Resolver bloqueos de red que impiden el P2P
- Validar comunicación en el puerto 50000

### 2. Hacer Funcionar el Sistema de Métricas
- Corregir métricas PEERS_CONNECTED y MESSAGES_RECEIVED
- Implementar todas las métricas de rendimiento necesarias
- Validar visualización en Grafana

### 3. Mejorar Automatización con Ansible
- Actualizar playbooks para manejo de errores
- Configurar nodos automáticamente sin intervención manual
- Implementar validaciones de conectividad

## Requisitos Técnicos

### Entorno de Desarrollo
- Sistema operativo: Linux (Ubuntu 20.04+)
- Herramientas: Rust 1.70+, Ansible 2.12+, LXD
- Red: Contenedores LXD configurados con redes bridge

### Componentes a Implementar

#### 1. Solución de Conectividad de Red

**Detalles de Implementación:**
```bash
# Reglas iptables necesarias para LXD
iptables -A INPUT -p tcp --dport 50000 -j ACCEPT
iptables -A INPUT -p udp --dport 50000 -j ACCEPT
iptables -A FORWARD -d 10.0.0.0/8 -j ACCEPT
```

**Validaciones Esperadas:**
- Nodos pueden conectarse entre sí
- Tráfico P2P en puerto 50000 funciona
- No hay bloqueos de LXD

#### 2. Mejora del Sistema de Métricas

**Detalles de Implementación:**
```rust
use prometheus::{IntGauge, register_int_gauge};

lazy_static! {
    static ref PEERS_CONNECTED: IntGauge = register_int_gauge!(
        "ebpf_blockchain_peers_connected",
        "Number of connected peers"
    ).unwrap();
    
    static ref MESSAGES_RECEIVED: IntGauge = register_int_gauge!(
        "ebpf_blockchain_messages_received",
        "Number of messages received"
    ).unwrap();
}

// Funciones para actualizar métricas
pub fn update_peers_connected(count: usize) {
    PEERS_CONNECTED.set(count as i64);
}

pub fn increment_messages_received() {
    MESSAGES_RECEIVED.inc();
}
```

**Validaciones Esperadas:**
- Métricas completamente funcionales
- Visualización en Grafana
- Datos actualizados en tiempo real

#### 3. Mejoras en Ansible

**Detalles de Implementación:**
```yaml
---
- name: Configurar nodo blockchain
  hosts: all
  become: yes
  vars:
    node_port: 50000
  tasks:
    - name: Verificar conectividad
      ping:
      retries: 3
      delay: 2
      register: ping_result
      ignore_errors: yes
      failed_when: ping_result is failed
      
    - name: Configurar firewall
      iptables:
        chain: INPUT
        protocol: tcp
        destination_port: "{{ node_port }}"
        jump: ACCEPT
        comment: "Allow P2P communication"
        
    - name: Iniciar servicio blockchain
      systemd:
        name: ebpf-blockchain
        state: started
        enabled: yes
      retries: 3
      delay: 5
```

**Validaciones Esperadas:**
- Despliegue automatizado sin errores
- Validación de configuración antes de inicio
- Manejo de errores y retry automático

## Criterios de Éxito

### Métricas de Éxito
- ✅ Todos los nodos se comunican correctamente
- ✅ Métricas se muestran en Grafana
- ✅ Ansible funciona sin errores
- ✅ Sistema listo para fase 2

### Pruebas de Validación
1. **Conectividad:** Verificar que nodos puedan descubrirse y comunicarse
2. **Métricas:** Confirmar que PEERS_CONNECTED y MESSAGES_RECEIVED se actualicen
3. **Ansible:** Validar que el playbook de despliegue funcione completamente

## Riesgos y Consideraciones

### Posibles Problemas
- Configuración incorrecta de LXD puede persistir
- Reglas de firewall pueden no aplicarse correctamente
- Problemas de permisos en el sistema

### Mitigación de Riesgos
- Validación exhaustiva de cada regla de firewall
- Pruebas de conectividad después de cada cambio
- Documentación detallada de configuraciones

## Dependencias

### Herramientas Necesarias
- Ansible 2.12+
- Docker/LXD para contenedores
- Prometheus y Grafana para métricas
- Rust toolchain para compilación

### Recursos Requeridos
- Acceso a nodos LXD
- Permisos de administrador para iptables
- Acceso a directorios de configuración

## Entregables

### Archivos Generados
1. Archivo de reglas de firewall actualizadas
2. Implementación de métricas en Rust
3. Playbooks de Ansible mejorados
4. Documentación de configuración

### Resultados Esperados
- Sistema completamente funcional de conectividad
- Métricas funcionales y visibles
- Despliegue automatizado sin errores