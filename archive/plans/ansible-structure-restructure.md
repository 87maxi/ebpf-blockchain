# Plan de Reestructuración de Ansible para Ambiente de Desarrollo

## 1. Análisis Actual

### 1.1 Estructura Actual de Ansible
El proyecto actualmente tiene una estructura de Ansible con los siguientes componentes:

- **ansible.cfg**: Configuración general de Ansible
- **inventory/hosts.yml**: Inventario de nodos con grupos definidos:
  - `lxc_nodes`: Nodos eBPF principales
  - `attacker_nodes`: Nodos para testing de ataques
  - `victim_nodes`: Nodos víctimas
  - `monitoring`: Servidor de monitoring (localhost)
- **roles/**: Directorio con roles específicos:
  - `lxc_node/`: Gestión de nodos LXC
  - `dependencies/`: Instalación de dependencias
  - `monitoring/`: Configuración de Prometheus/Grafana
- **playbooks/**: Playbooks para automatizar tareas:
  - `setup_cluster.yml`: Setup completo del cluster
  - `create_node.yml`: Crear un nodo individual
  - `create_cluster.yml`: Crear múltiples nodos
  - `destroy_node.yml`: Eliminar un nodo
  - `install_deps.yml`: Instalar dependencias
  - `configure_monitoring.yml`: Configurar monitoring
  - `cluster_status.yml`: Ver estado del cluster

### 1.2 Sistema de Observabilidad
El sistema de observabilidad está integrado con Docker Compose y utiliza:
- **Prometheus**: Para recolección de métricas
- **Grafana**: Para visualización de métricas
- **Loki**: Para agregación de logs
- **Tempo**: Para tracing distribuido
- **Promtail**: Para recolección de logs desde contenedores

## 2. Problemas Identificados

### 2.1 Complejidad en la Configuración
- La configuración de los servicios de observabilidad está separada del directorio de Ansible
- El archivo `docker-compose.yml` está en el directorio `monitoring/` en la raíz del proyecto
- La integración entre Ansible y Docker Compose no es óptima

### 2.2 Falta de Desarrollo Local
- No hay una configuración específica para desarrollo local
- No hay soporte para ejecutar el sistema completo en un solo nodo para desarrollo

### 2.3 Configuración de Métricas
- Las métricas de los nodos eBPF no están configuradas para ser recolectadas automáticamente
- No hay una configuración de alertas específica para el entorno de desarrollo

## 3. Propuesta de Reestructuración

### 3.1 Estructura de Directorios Actualizada

```
ansible/
├── ansible.cfg
├── inventory/
│   ├── hosts.yml
│   └── group_vars/
│       └── all.yml
├── roles/
│   ├── common/
│   ├── dependencies/
│   ├── lxc_node/
│   ├── monitoring/
│   └── dev_environment/
├── playbooks/
│   ├── setup_cluster.yml
│   ├── create_node.yml
│   ├── create_cluster.yml
│   ├── destroy_node.yml
│   ├── install_deps.yml
│   ├── configure_monitoring.yml
│   ├── cluster_status.yml
│   └── setup_dev_environment.yml
└── templates/
    └── docker-compose.dev.yml.j2
```

### 3.2 Nuevos Componentes

#### 3.2.1 Rol `dev_environment`
Este rol se encargará de configurar el ambiente de desarrollo local con todas las herramientas necesarias.

#### 3.2.2 Playbook `setup_dev_environment.yml`
Este playbook configurará un ambiente de desarrollo completo en un solo nodo local.

#### 3.2.3 Plantilla `docker-compose.dev.yml.j2`
Plantilla para el entorno de desarrollo local que incluirá:
- Todos los servicios de observabilidad (Prometheus, Grafana, Loki, Tempo)
- Configuración específica para desarrollo
- Volumen para persistencia de datos en desarrollo

## 4. Mejoras Propuestas

### 4.1 Integración con Docker Compose
- Mover el archivo `docker-compose.yml` dentro del directorio `ansible/` para mejor integración
- Crear una estructura de plantillas que permita configurar diferentes ambientes (desarrollo, producción, testing)

### 4.2 Configuración de Métricas para Desarrollo
- Configurar Prometheus para recolectar métricas de los nodos eBPF en desarrollo
- Crear dashboards específicos para el entorno de desarrollo
- Configurar alertas básicas para el entorno de desarrollo

### 4.3 Soporte para Desarrollo Local
- Implementar un playbook que permita ejecutar todo el sistema en un solo nodo local
- Crear configuraciones específicas para desarrollo que no requieran LXC
- Permitir ejecutar el sistema sin necesidad de múltiples nodos

## 5. Implementación Detallada

### 5.1 Crear el Rol `dev_environment`
Este rol incluirá:
- Instalación de herramientas de desarrollo necesarias
- Configuración de Docker y Docker Compose
- Configuración de servicios de observabilidad en modo desarrollo
- Configuración de variables de entorno específicas para desarrollo

### 5.2 Actualizar Playbooks
- Crear `setup_dev_environment.yml` para configurar el ambiente de desarrollo
- Actualizar `configure_monitoring.yml` para soportar diferentes ambientes
- Asegurar que los playbooks puedan ejecutarse en modo local o en nodos remotos

### 5.3 Crear Plantillas Específicas
- Crear plantilla `docker-compose.dev.yml.j2` para el entorno de desarrollo
- Crear plantilla `prometheus.dev.yml.j2` para configuración específica de desarrollo

## 6. Beneficios de la Reestructuración

### 6.1 Mejora en la Experiencia de Desarrollo
- Ambiente de desarrollo más fácil de configurar
- Menos dependencias externas para ejecutar el sistema
- Configuración más intuitiva para nuevos desarrolladores

### 6.2 Flexibilidad en Ambientes
- Soporte para múltiples ambientes (desarrollo, testing, producción)
- Configuración modular que permite cambiar fácilmente entre ambientes
- Menos duplicación de configuraciones

### 6.3 Mejora en la Observabilidad
- Métricas más completas en entornos de desarrollo
- Dashboards específicos para el desarrollo
- Alertas configurables para diferentes niveles de entorno

## 7. Consideraciones de Implementación

### 7.1 Compatibilidad
- Mantener compatibilidad con la estructura actual para producción
- No romper la funcionalidad existente de los playbooks actuales
- Asegurar que las nuevas configuraciones sean opcionales

### 7.2 Documentación
- Actualizar la documentación en `ansible/README.md`
- Crear documentación específica para el entorno de desarrollo
- Incluir ejemplos de uso para ambos ambientes

### 7.3 Pruebas
- Probar el nuevo entorno de desarrollo
- Verificar que los playbooks existentes sigan funcionando
- Asegurar que la integración con el sistema de observabilidad funcione correctamente

## 8. Próximos Pasos

1. Crear el rol `dev_environment`
2. Implementar el playbook `setup_dev_environment.yml`
3. Crear las plantillas específicas para desarrollo
4. Actualizar la documentación
5. Probar la nueva estructura de desarrollo
6. Verificar compatibilidad con entornos existentes
