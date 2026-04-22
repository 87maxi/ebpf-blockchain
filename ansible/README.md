# Ansible Playbooks para eBPF Blockchain

Playbooks automatizados para gestión de nodos eBPF blockchain con LXC, Prometheus y Grafana.

## Requisitos

- Ansible >= 2.9
- LXD/LXC instalado y configurado
- Docker y Docker Compose (para monitoring y dev environment)
- SSH acceso a localhost (para connection: local)

## Estructura

```
ansible/
├── ansible.cfg              # Configuración de Ansible
├── inventory/
│   ├── hosts.yml            # Inventario de nodos
│   └── group_vars/
│       └── all.yml          # Variables globales
├── roles/
│   ├── lxc_node/            # Rol para gestionar nodos LXC
│   ├── dependencies/        # Rol para instalar dependencias
│   ├── monitoring/          # Rol para Prometheus/Grafana
│   └── dev_environment/     # Rol para ambiente de desarrollo local
└── playbooks/
    ├── deploy_cluster.yml   # Setup completo del cluster LXC
    ├── deploy.yml           # Deploy de eBPF node en nodos remotos
    ├── health_check.yml     # Verificar estado del nodo
    ├── backup.yml           # Backup automatizado
    ├── disaster_recovery.yml# Recuperación de desastres
    ├── factory_reset.yml    # Factory reset de nodos
    ├── fix_network.yml      # Reparar conectividad de red
    ├── rebuild_and_restart.yml # Reconstruir y reiniciar nodo
    ├── repair_and_restart.yml  # Reparar y reiniciar nodo
    ├── rollback.yml         # Rollback de despliegue
    ├── setup_dev_environment.yml  # Configurar ambiente de desarrollo
    └── setup_cluster.yml    # Setup completo (legacy)
```

## Uso

### 1. Setup completo del cluster LXC

```bash
cd ansible
ansible-playbook playbooks/deploy_cluster.yml
```

Esto crea la red LXC, los nodos y configura iptables.

### 2. Deploy de eBPF node en nodos

```bash
ansible-playbook playbooks/deploy.yml -i inventory/hosts.yml
```

### 3. Verificar estado del nodo

```bash
ansible-playbook playbooks/health_check.yml -i inventory/hosts.yml
```

### 4. Configurar ambiente de desarrollo local

```bash
# Configurar todos los servicios de observabilidad
ansible-playbook playbooks/setup_dev_environment.yml

# Verificar servicios
docker compose -f ~/Documentos/source/ebpf-blockchain/dev-environment/docker-compose.yml ps
```

### 5. Configurar monitoring (Prometheus + Grafana)

```bash
ansible-playbook playbooks/deploy.yml --tags monitoring
```

### 6. Backup

```bash
ansible-playbook playbooks/backup.yml -i inventory/hosts.yml
```

### 7. Eliminar cluster

```bash
# Factory reset de todos los nodos
ansible-playbook playbooks/factory_reset.yml
```

## Inventario

El archivo `inventory/hosts.yml` define los grupos de nodos:

- `lxc_nodes`: Nodos eBPF principales (LXC containers)
- `attacker_nodes`: Nodos para testing de ataques
- `victim_nodes`: Nodos víctimas
- `monitoring`: Servidor de monitoring (localhost)
- `dev_environment`: Ambiente de desarrollo local (localhost)

## Ambiente de Desarrollo

Para configurar un ambiente de desarrollo local con todos los servicios de observabilidad:

```bash
# Configurar ambiente de desarrollo
ansible-playbook playbooks/setup_dev_environment.yml

# Verificar servicios
docker compose -f dev-environment/docker-compose.yml ps
```

### Servicios disponibles en modo desarrollo

| Servicio    | Puerto  | URL                        |
|-------------|---------|----------------------------|
| Prometheus  | 9090    | http://localhost:9090      |
| Grafana     | 3000    | http://localhost:3000      |
| Loki        | 3100    | http://localhost:3100      |
| Tempo       | 3200    | http://localhost:3200      |
| Alertmanager| 9093    | http://localhost:9093      |

### Configuración de desarrollo

El ambiente de desarrollo incluye:
- Retención de métricas reducida (15 días para Prometheus, 72h para Loki/Tempo)
- Logs en nivel debug
- Todos los servicios expuestos en puertos locales
- Volúmenes para persistencia de datos

### Variables de desarrollo

```yaml
dev_environment:
  prometheus_port: 9090
  grafana_port: 3000
  loki_port: 3100
  tempo_port: 3200
  prometheus_retention_time: "15d"
  loki_retention_period: "720h"
  tempo_retention_period: "720h"
  dev_mode: true
  debug_enabled: true
  log_level: "debug"
```

## Configuración de variables

### Variables globales (en inventory/hosts.yml)

```yaml
all:
  vars:
    ansible_user: root
    ansible_connection: lxc
    project_dir: /home/maxi/Documentos/source/ebpf-blockchain
    lxc_network: 192.168.2.0/24
    lxc_gateway: 192.168.2.200
    lxc_profile: ebpf-blockchain
    lxc_bridge: lxdbr1
```

### Variables por nodo

```yaml
node_name: ebpf-node-1
node_ip: 192.168.2.210
ansible_host: ebpf-blockchain
```

## Servicios disponibles después del setup

- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (admin/admin)
- **Loki**: http://localhost:3100
- **Tempo**: http://localhost:3200

## Troubleshooting

### Error: LXC no está instalado

```bash
# Instalar LXD
snap install lxd
lxd init
```

### Error: Docker no está instalado

```bash
# Instalar Docker
curl -fsSL https://get.docker.com | sh
systemctl enable docker
```

### Verificar conectividad de nodos

```bash
# Desde el host
lxc exec ebpf-node-1 -- ping -c 2 8.8.8.8

# Ver IP del nodo
lxc list
```

### Logs de Ansible

```bash
# Con más detalle
ansible-playbook playbooks/deploy.yml -vvv

# Simular sin ejecutar
ansible-playbook playbooks/deploy.yml --check
```

## Notas

- Los playbooks usan `connection: local` y `ansible_connection: lxc` para ejecutar comandos en contenedores LXC
- El perfil de LXC `ebpf-blockchain` se crea automáticamente con privilegios para eBPF
- Las dependencias se marcan como instaladas con el archivo `~/.deps-installed` en cada nodo
- El dashboard de Grafana se aprovisiona automáticamente con paneles para métricas de eBPF
- El ambiente de desarrollo usa Docker Compose para orquestar los servicios de observabilidad
- Los archivos de configuración de desarrollo se generan desde templates Jinja2 en `roles/dev_environment/templates/`
