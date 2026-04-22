# Plan de Implementación Detallada para Reestructuración de Ansible

## 1. Introducción

Este plan detalla la implementación de la reestructuración propuesta para el sistema Ansible, con enfoque en crear un ambiente de desarrollo local optimizado que integre el sistema de observabilidad con Docker.

## 2. Objetivos

- Crear un ambiente de desarrollo local completo
- Integrar el sistema de observabilidad con Ansible
- Mantener compatibilidad con entornos de producción
- Simplificar la configuración para nuevos desarrolladores

## 3. Estructura de Implementación

### 3.1 Directorios y Archivos a Crear

#### 3.1.1 Directorio `ansible/roles/dev_environment/`
- `tasks/main.yml`: Tareas principales del rol
- `handlers/main.yml`: Handlers para el rol
- `templates/`: Plantillas específicas
- `vars/main.yml`: Variables del rol

#### 3.1.2 Archivos en `ansible/`
- `playbooks/setup_dev_environment.yml`: Playbook para configurar ambiente de desarrollo
- `templates/docker-compose.dev.yml.j2`: Plantilla para Docker Compose en desarrollo

### 3.2 Archivos a Modificar

#### 3.2.1 `ansible/roles/monitoring/tasks/main.yml`
- Actualizar para soportar diferentes ambientes
- Añadir lógica para configurar el entorno de desarrollo

#### 3.2.2 `ansible/inventory/hosts.yml`
- Añadir grupo específico para desarrollo local

## 4. Implementación Paso a Paso

### 4.1 Crear el Rol `dev_environment`

#### 4.1.1 Crear directorio y archivos del rol
```bash
mkdir -p ansible/roles/dev_environment/{tasks,handlers,templates,vars}
```

#### 4.1.2 Crear `ansible/roles/dev_environment/tasks/main.yml`
```yaml
---
- name: Verificar instalación de Docker
  command: which docker
  register: docker_check
  failed_when: false
  changed_when: false

- name: Instalar Docker si no está presente
  apt:
    name: docker.io
    state: present
  when: docker_check.rc != 0

- name: Verificar instalación de Docker Compose
  command: which docker-compose
  register: compose_check
  failed_when: false
  changed_when: false

- name: Instalar Docker Compose si no está presente
  apt:
    name: docker-compose
    state: present
  when: compose_check.rc != 0

- name: Crear directorios para desarrollo
  file:
    path: "{{ project_dir }}/dev-environment"
    state: directory
  changed_when: false

- name: Crear directorio para logs de desarrollo
  file:
    path: "{{ project_dir }}/dev-environment/logs"
    state: directory
  changed_when: false

- name: Crear directorio para datos de desarrollo
  file:
    path: "{{ project_dir }}/dev-environment/data"
    state: directory
  changed_when: false

- name: Copiar configuración de observabilidad para desarrollo
  copy:
    src: "{{ project_dir }}/monitoring/"
    dest: "{{ project_dir }}/dev-environment/monitoring/"
    recursive: yes
  when: "'dev_environment' in group_names"

- name: Configurar Docker Compose para desarrollo
  template:
    src: docker-compose.dev.yml.j2
    dest: "{{ project_dir }}/dev-environment/docker-compose.yml"
    mode: '0644'
  register: compose_template

- name: Iniciar servicios de desarrollo
  shell: |
    cd {{ project_dir }}/dev-environment
    docker-compose up -d
  when: compose_template.changed
  changed_when: true
```

#### 4.1.3 Crear `ansible/roles/dev_environment/vars/main.yml`
```yaml
---
# Variables específicas para el entorno de desarrollo
dev_environment:
  docker_compose_version: "2.21.0"
  prometheus_port: 9090
  grafana_port: 3000
  loki_port: 3100
  tempo_port: 3200
  prometheus_config_path: "{{ project_dir }}/dev-environment/monitoring/prometheus"
  grafana_config_path: "{{ project_dir }}/dev-environment/monitoring/grafana"
  loki_config_path: "{{ project_dir }}/dev-environment/monitoring/loki"
  tempo_config_path: "{{ project_dir }}/dev-environment/monitoring/tempo"
```

### 4.2 Crear Plantilla de Docker Compose para Desarrollo

#### 4.2.1 Crear `ansible/templates/docker-compose.dev.yml.j2`
```yaml
version: '3.8'

services:
  # =====================
  # PROMETHEUS - Metrics Storage & Alerting
  # =====================
  prometheus:
    image: prom/prometheus:v2.48.0
    container_name: ebpf-prometheus-dev
    restart: unless-stopped
    ports:
      - "{{ dev_environment.prometheus_port }}:9090"
    volumes:
      - {{ dev_environment.prometheus_config_path }}/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - {{ dev_environment.prometheus_config_path }}/alerts.yml:/etc/prometheus/alerts.yml:ro
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=15d'
      - '--web.enable-lifecycle'
      - '--web.enable-admin-api'
      - '--alertmanager.url=http://alertmanager:9093'
    networks:
      - ebpf-dev-observability

  # =====================
  # ALERTMANAGER - Alert Routing
  # =====================
  alertmanager:
    image: prom/alertmanager:v0.26.0
    container_name: ebpf-alertmanager-dev
    restart: unless-stopped
    ports:
      - "9093:9093"
    volumes:
      - {{ dev_environment.prometheus_config_path }}/alertmanager.yml:/etc/alertmanager/alertmanager.yml:ro
      - alertmanager-data:/alertmanager
    command:
      - '--config.file=/etc/alertmanager/alertmanager.yml'
      - '--storage.path=/alertmanager'
      - '--web.listen-address=0.0.0.0:9093'
    networks:
      - ebpf-dev-observability

  # =====================
  # GRAFANA - Visualization
  # =====================
  grafana:
    image: grafana/grafana:10.2.0
    container_name: ebpf-grafana-dev
    restart: unless-stopped
    ports:
      - "{{ dev_environment.grafana_port }}:3000"
    environment:
      - GF_SECURITY_ADMIN_USER=admin
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
      - GF_SERVER_ROOT_URL=http://localhost:3000
      - GF_FEATURE_TOGGLES_ENABLE=traceqlEditor
    volumes:
      - {{ dev_environment.grafana_config_path }}/provisioning:/etc/grafana/provisioning:ro
      - {{ dev_environment.grafana_config_path }}/dashboards:/var/lib/grafana/dashboards:ro
      - grafana-data:/var/lib/grafana
      - grafana-config:/etc/grafana
    depends_on:
      - prometheus
      - loki
      - tempo
    networks:
      - ebpf-dev-observability

  # =====================
  # LOKI - Log Aggregation
  # =====================
  loki:
    image: grafana/loki:2.9.0
    container_name: ebpf-loki-dev
    restart: unless-stopped
    ports:
      - "{{ dev_environment.loki_port }}:3100"
    volumes:
      - {{ dev_environment.loki_config_path }}/loki-config.yml:/etc/loki/loki-config.yml:ro
      - loki-data:/loki
    command:
      - '-config.file=/etc/loki/loki-config.yml'
    networks:
      - ebpf-dev-observability

  # =====================
  # PROMTAIL - Log Collection
  # =====================
  promtail:
    image: grafana/promtail:2.9.0
    container_name: ebpf-promtail-dev
    restart: unless-stopped
    ports:
      - "9080:9080"
    volumes:
      - /var/log:/var/log:ro
      - /var/lib/docker/containers:/var/lib/docker/containers:ro
      - {{ dev_environment.prometheus_config_path }}/promtail-config.yml:/etc/promtail/promtail-config.yml:ro
    command:
      - '-config.file=/etc/promtail/promtail-config.yml'
    networks:
      - ebpf-dev-observability

  # =====================
  # TEMPO - Tracing
  # =====================
  tempo:
    image: grafana/tempo:2.3.0
    container_name: ebpf-tempo-dev
    restart: unless-stopped
    ports:
      - "{{ dev_environment.tempo_port }}:3200"
      - "4317:4317"
      - "4318:4318"
    volumes:
      - {{ dev_environment.tempo_config_path }}/tempo-config.yml:/etc/tempo/tempo-config.yml:ro
      - tempo-data:/tmp/tempo
    command:
      - '-config.file=/etc/tempo/tempo-config.yml'
    networks:
      - ebpf-dev-observability

networks:
  ebpf-dev-observability:
    driver: bridge

volumes:
  prometheus-data:
  alertmanager-data:
  grafana-data:
  grafana-config:
  loki-data:
  tempo-data:
```

### 4.3 Crear Playbook para Configuración de Desarrollo

#### 4.3.1 Crear `ansible/playbooks/setup_dev_environment.yml`
```yaml
---
- name: Configurar ambiente de desarrollo local
  hosts: localhost
  gather_facts: true
  vars:
    ansible_connection: local
  roles:
    - dev_environment
  become: true
```

### 4.4 Actualizar el Rol de Monitoring

#### 4.4.1 Actualizar `ansible/roles/monitoring/tasks/main.yml`
```yaml
---
- name: Verificar instalación de Docker
  command: which docker
  register: docker_check
  failed_when: false
  changed_when: false

- name: Fallar si Docker no está instalado
  fail:
    msg: "Docker es requerido. Por favor instale Docker primero."
  when: docker_check.rc != 0

- name: Verificar instalación de Docker Compose
  command: docker compose version
  register: compose_check
  failed_when: false
  changed_when: false
  ignore_errors: true

- name: Establecer comando de compose
  set_fact:
    compose_cmd: "docker compose"
  when: compose_check.rc == 0

- name: Establecer comando de compose (legacy)
  set_fact:
    compose_cmd: "docker-compose"
  when: compose_check.rc != 0

- name: Crear directorios para monitoring
  file:
    path: "{{ project_dir }}/monitoring"
    state: directory
  changed_when: false

- name: Crear directorios para configuración de Prometheus
  file:
    path: "{{ project_dir }}/monitoring/prometheus"
    state: directory
  changed_when: false

- name: Crear directorios para provisionamiento de Grafana
  file:
    path: "{{ project_dir }}/monitoring/grafana/provisioning/{dashboards,datasources}"
    state: directory
  changed_when: false

- name: Desplegar configuración de Prometheus
  template:
    src: prometheus.yml.j2
    dest: "{{ project_dir }}/monitoring/prometheus/prometheus.yml"
    mode: '0644'
  register: prometheus_config

- name: Crear archivo docker-compose para monitoring
  template:
    src: docker-compose.monitoring.yml.j2
    dest: "{{ project_dir }}/monitoring/docker-compose.yml"
    mode: '0644'
  register: compose_file

- name: Iniciar stack de monitoring
  shell: |
    cd {{ project_dir }}/monitoring
    {{ compose_cmd }} up -d
  register: compose_up
  changed_when: true
  when: prometheus_config.changed or compose_file.changed

- name: Esperar a que Prometheus esté listo
  wait_for:
    port: 9090
    host: localhost
    timeout: 60
  ignore_errors: true

- name: Esperar a que Grafana esté listo
  wait_for:
    port: 3000
    host: localhost
    timeout: 60
  ignore_errors: true

- name: Configurar datasource de Prometheus en Grafana
  uri:
    url: "http://localhost:3000/api/datasources"
    method: POST
    user: admin
    password: admin
    body_format: json
    body:
      name: Prometheus
      type: prometheus
      url: http://prometheus:9090
      access: proxy
      isDefault: true
    status_code: [200, 409]
  ignore_errors: true

- name: Desplegar dashboards de Grafana
  template:
    src: dashboard.json.j2
    dest: "{{ project_dir }}/monitoring/grafana/dashboards/{{ item }}.json"
    mode: '0644'
  loop:
    - consensus
    - health-overview
    - network-p2p
    - transactions
  when: "'monitoring' in group_names"

- name: Recargar configuración de Grafana
  uri:
    url: "http://localhost:3000/api/admin/provisioning/datasources/reload"
    method: POST
    user: admin
    password: admin
    status_code: 200
  ignore_errors: true
```

### 4.5 Actualizar Inventario

#### 4.5.1 Actualizar `ansible/inventory/hosts.yml`
```yaml
---
all:
  vars:
    ansible_user: root
    ansible_connection: lxc
    project_dir: /home/maxi/Documentos/source/ebpf-blockchain
    lxc_network: 192.168.2.0/24
    lxc_gateway: 192.168.2.200
    lxc_profile: ebpf-blockchain
    lxc_bridge: lxdbr1

  children:
    lxc_nodes:
      vars:
        node_type: ebpf
      hosts:
        ebpf-node-1:
          ansible_host: ebpf-blockchain
          node_ip: 192.168.2.210
          node_name: ebpf-blockchain
        ebpf-node-2:
          ansible_host: ebpf-blockchain-2
          node_ip: 192.168.2.211
          node_name: ebpf-blockchain-2

    attacker_nodes:
      vars:
        node_type: attacker
      hosts:
        ebpf-attacker-1:
          ansible_host: ebpf-attacker-1
          node_ip: 192.168.2.221
          node_name: ebpf-attacker-1

    victim_nodes:
      vars:
        node_type: victim
      hosts:
        ebpf-victim-1:
          ansible_host: ebpf-victim-1
          node_ip: 192.168.2.231
          node_name: ebpf-victim-1

    monitoring:
      vars:
        node_type: monitoring
        ansible_connection: local
      hosts:
        localhost:
          ansible_host: localhost
          prometheus_port: 9090
          grafana_port: 3000
          grafana_admin: admin
          grafana_password: admin

    dev_environment:
      vars:
        node_type: development
        ansible_connection: local
      hosts:
        localhost:
          ansible_host: localhost
```

## 5. Pruebas y Validación

### 5.1 Pruebas de Funcionalidad
- Verificar que el playbook de desarrollo funcione correctamente
- Comprobar que los servicios de observabilidad se inicien correctamente
- Validar que las métricas se recojan correctamente en el entorno de desarrollo

### 5.2 Pruebas de Compatibilidad
- Asegurar que los playbooks existentes sigan funcionando
- Verificar que la configuración de producción no se vea afectada
- Comprobar que las variables de entorno se manejen correctamente

## 6. Documentación

### 6.1 Actualizar README.md
- Documentar el nuevo entorno de desarrollo
- Explicar cómo ejecutar el playbook de desarrollo
- Incluir instrucciones para configurar el entorno de desarrollo

### 6.2 Crear Documentación Específica
- Crear documentación para el rol `dev_environment`
- Documentar las nuevas plantillas de Docker Compose
- Incluir ejemplos de uso

## 7. Consideraciones Finales

### 7.1 Mantenimiento
- El sistema debe ser fácil de mantener y actualizar
- Las configuraciones deben ser modulares y reutilizables
- Se debe mantener la compatibilidad con entornos existentes

### 7.2 Escalabilidad
- El diseño debe permitir fácil expansión a otros ambientes
- Las configuraciones deben ser configurables mediante variables
- El sistema debe ser adaptable a diferentes necesidades de desarrollo

## 8. Cronograma de Implementación

### Semana 1
- Crear estructura de directorios
- Implementar rol `dev_environment`
- Crear plantillas de Docker Compose

### Semana 2
- Implementar playbooks de desarrollo
- Actualizar roles existentes
- Probar funcionalidad básica

### Semana 3
- Validar compatibilidad con entornos existentes
- Documentar el nuevo sistema
- Realizar pruebas completas

### Semana 4
- Revisión final
- Optimización de configuraciones
- Preparación para producción
