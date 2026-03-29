# Ansible Playbooks para eBPF Blockchain

Playbooks automatizados para gestión de nodos eBPF blockchain con LXC, Prometheus y Grafana.

## Requisitos

- Ansible >= 2.9
- LXD/LXC instalado y configurado
- Docker y Docker Compose (para monitoring)
- SSH acceso a localhost (para connection: local)

## Estructura

```
ansible/
├── ansible.cfg              # Configuración de Ansible
├── inventory/
│   └── hosts.yml           # Inventario de nodos
├── roles/
│   ├── lxc_node/           # Rol para gestionar nodos LXC
│   ├── dependencies/       # Rol para instalar dependencias
│   └── monitoring/         # Rol para Prometheus/Grafana
└── playbooks/
    ├── setup_cluster.yml   # Setup completo del cluster
    ├── create_node.yml     # Crear un nodo individual
    ├── create_cluster.yml  # Crear múltiples nodos
    ├── destroy_node.yml    # Eliminar un nodo
    ├── install_deps.yml    # Instalar dependencias
    ├── configure_monitoring.yml  # Configurar monitoring
    └── cluster_status.yml  # Ver estado del cluster
```

## Uso

### 1. Setup completo del cluster

```bash
cd ansible
ansible-playbook playbooks/setup_cluster.yml
```

Esto crea los nodos, instala dependencias y configura monitoring.

### 2. Crear un nodo específico

```bash
# Con variables inline
ansible-playbook playbooks/create_node.yml --extra-vars "node_name=ebpf-node-3 node_ip=192.168.2.212"

# O usar el inventario
ansible-playbook -i inventory/hosts.yml playbooks/create_node.yml --extra-vars "node_name=mi-nodo node_ip=192.168.2.220"
```

### 3. Crear cluster de múltiples nodos

```bash
ansible-playbook playbooks/create_cluster.yml --extra-vars "node_count=5"
```

### 4. Instalar dependencias en nodos existentes

```bash
# Todos los nodos
ansible-playbook playbooks/install_deps.yml

# Nodo específico
ansible-playbook -i inventory/hosts.yml playbooks/install_deps.yml --limit ebpf-node-1
```

### 5. Configurar monitoring (Prometheus + Grafana)

```bash
ansible-playbook playbooks/configure_monitoring.yml
```

### 6. Ver estado del cluster

```bash
ansible-playbook playbooks/cluster_status.yml
```

### 7. Eliminar un nodo

```bash
ansible-playbook playbooks/destroy_node.yml --extra-vars "node_name=ebpf-node-3 force=true"
```

## Inventario

El archivo `inventory/hosts.yml` define los grupos de nodos:

- `lxc_nodes`: Nodos eBPF principales
- `attacker_nodes`: Nodos para testing de ataques
- `victim_nodes`: Nodos víctimas
- `monitoring`: Servidor de monitoring (localhost)

### Agregar nuevos nodos al inventario

Editar `inventory/hosts.yml`:

```yaml
lxc_nodes:
  hosts:
    ebpf-node-1:
      ansible_host: ebpf-blockchain
      node_ip: 192.168.2.210
    ebpf-node-NEW:
      ansible_host: ebpf-node-NEW
      node_ip: 192.168.2.2XX
```

## Configuración de variables

### Variables globales (en inventory/hosts.yml)

```yaml
all:
  vars:
    project_dir: /home/maxi/Documentos/source/ebpf-blockchain
    lxc_network: 192.168.2.0/24
    lxc_gateway: 192.168.2.200
    lxc_profile: ebpf-blockchain
```

### Variables por nodo

```yaml
node_name: ebpf-node-1
node_ip: 192.168.2.210
lxc_gateway: 192.168.2.200
clone_source: ebpf-blockchain  # Opcional: clonar de nodo existente
```

## Servicios disponibles después del setup

- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (admin/admin)

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
ansible-playbook playbooks/create_node.yml -vvv

# Simular sin ejecutar
ansible-playbook playbooks/create_node.yml --check
```

## Notas

- Los playbooks usan `connection: local` y `ansible_connection: lxc` para ejecutar comandos en contenedores LXC
- El perfil de LXC `ebpf-blockchain` se crea automáticamente con privilegios para eBPF
- Las dependencias se marcan como instaladas con el archivo `~/.deps-installed` en cada nodo
- El dashboard de Grafana se aprovisiona automáticamente con paneles para métricas de eBPF
