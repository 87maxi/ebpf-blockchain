# Guía Completa: Laboratorio eBPF Blockchain con LXC

## Replicar el Ambiente de Testing Paso a Paso

**Última actualización:** Marzo 2026  
**Autor:** Maximiliano Paredes  
**Versión del sistema:** eBPF Blockchain v1.0 con RFC 001

---

## Tabla de Contenidos

1. [Arquitectura General](#1-arquitectura-general)
2. [Requisitos del Sistema](#2-requisitos-del-sistema)
3. [Configuración de Red LXC](#3-configuración-de-red-lxc)
4. [Creación del Perfil LXC](#4-creación-del-perfil-lxc)
5. [Creación del Primer Nodo](#5-creación-del-primer-nodo)
6. [Instalación de Dependencias](#6-instalación-de-dependencias)
7. [Montaje del Proyecto](#7-montaje-del-proyecto)
8. [Compilación y Ejecución](#8-compilación-y-ejecución)
9. [Stack de Monitoreo (Prometheus + Grafana)](#9-stack-de-monitoreo-prometheus--grafana)
10. [Creación de Nodos Adicionales](#10-creación-de-nodos-adicionales)
11. [Verificación del Sistema](#11-verificación-del-sistema)
12. [Scripts de Automatización](#12-scripts-de-automatización)
13. [Testing P2P y Blacklist](#13-testing-p2p-y-blacklist)
14. [Solución de Problemas](#14-solución-de-problemas)

---

## 1. Arquitectura General

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           HOST (SUSE Linux)                             │
│                                                                         │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    Docker Network (host mode)                    │  │
│  │  ┌─────────────────┐     ┌─────────────────┐                     │  │
│  │  │   Prometheus    │     │     Grafana     │                     │  │
│  │  │   :9090         │     │     :3000       │                     │  │
│  │  └────────┬────────┘     └────────┬────────┘                     │  │
│  │           │                       │                               │  │
│  └───────────┼───────────────────────┼───────────────────────────────┘  │
│              │                       │                                  │
│              │   ┌───────────────────┘                                  │
│              │   │                                                      │
│  ┌───────────▼───▼────────────────────────────────────────────────┐   │
│  │                     lxdbr1 (192.168.2.0/24)                     │   │
│  │                          ▲                                       │   │
│  │                    enp6s0 (uplink)                               │   │
│  └─────────────────────────┼───────────────────────────────────────┘   │
│                            │                                              │
│  ┌─────────────────────────┼───────────────────────────────────────┐   │
│  │              LXC Containers (Privileged)                         │   │
│  │  ┌──────────────────────┴──────────────────────┐                │   │
│  │  │                                              │                │   │
│  │  ▼                                              ▼                │   │
│  │  ┌─────────────────┐    ┌─────────────────┐                    │   │
│  │  │  ebpf-blockchain │    │ ebpf-blockchain-2                │   │
│  │  │  192.168.2.210   │    │   192.168.2.211  │                    │   │
│  │  │                  │    │                  │                    │   │
│  │  │  • eBPF (XDP)    │    │  • eBPF (XDP)   │                    │   │
│  │  │  • KProbes       │    │  • KProbes      │                    │   │
│  │  │  • libp2p        │    │  • libp2p       │                    │   │
│  │  │  • Prometheus    │    │  • Prometheus   │                    │   │
│  │  │    :9090         │    │    :9090        │                    │   │
│  │  └──────────────────┘    └──────────────────┘                    │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

### Componentes del Nodo eBPF

```
┌─────────────────────────────────────────────────────────────┐
│                    eBPF Node Architecture                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              USER SPACE (Rust + Aya)                │   │
│  ├─────────────────────────────────────────────────────┤   │
│  │  • libp2p Swarm (QUIC + Gossipsub v1.1)            │   │
│  │  • Prometheus Metrics Server (:9090/metrics)        │   │
│  │  • XDP Program Loader                               │   │
│  │  • LpmTrie Manager (NODES_BLACKLIST)               │   │
│  │  • KProbes Handler                                  │   │
│  └───────────────────────┬─────────────────────────────┘   │
│                          │                                   │
│                     eBPF Maps                               │
│  ┌───────────────────────┼─────────────────────────────┐   │
│  │                       │                               │   │
│  ▼                       ▼                               ▼   │
│ ┌────────────┐  ┌───────────────┐  ┌──────────────────┐   │
│ │LPM_TRIE    │  │  HASH_MAP    │  │   HASH_MAP       │   │
│ │NODES_BLACK │  │LATENCY_STATS │  │   START_TIMES    │   │
│ │   LIST     │  │ (histogram)  │  │   (timestamps)   │   │
│ └─────┬──────┘  └───────┬───────┘  └────────┬─────────┘   │
│       │                  │                   │             │
│  ┌────┴──────────────────┴──────────────────┴─────────┐  │
│  │                  KERNEL SPACE                        │  │
│  ├──────────────────────────────────────────────────────┤  │
│  │  ┌────────────────┐     ┌────────────────────────┐    │  │
│  │  │  XDP (eBPF)    │     │  KProbes (eBPF)       │    │  │
│  │  │  ebpf_node()   │     │  • netif_receive_skb │    │  │
│  │  │                │     │  • napi_consume_skb  │    │  │
│  │  │  • Drop if     │     │                      │    │  │
│  │  │    blacklisted │     │  • Track packet      │    │  │
│  │  │  • Pass other  │     │    latency           │    │  │
│  │  └────────────────┘     └────────────────────────┘    │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Requisitos del Sistema

### 2.1 Hardware
- CPU: 4+ cores (para contenedores LXC)
- RAM: 16GB+ (8GB por nodo LXC)
- Almacenamiento: 50GB+ SSD
- Interfaces de red: 2 NICs (una para host, una para LXC bridge)

### 2.2 Software del Host
```bash
# SUSE Linux (o Ubuntu/Debian)
- LXD 5.x+
- Docker + Docker Compose
- Kernel Linux 5.10+ (con soporte BTF)
- iptables

# Verificar kernel
uname -r

# Instalar LXD si no está
sudo snap install lxd  # Ubuntu
# o
sudo zypper install lxd  # SUSE

# Instalar Docker si no está
sudo zypper install docker  # SUSE
# o
sudo apt install docker.io  # Ubuntu
```

### 2.3 Verificación de Capacidades eBPF
```bash
# Verificar soporte de eBPF
grep -rBP1 'bpf\|BPF' /boot/config-$(uname -r) 2>/dev/null | head -30

# Verificar BPF syscall
cat /proc/sys/kernel/bpf_stats_enabled 2>/dev/null || echo "No disponible"

# Instalar bpftool
sudo apt install linux-tools-$(uname -r)  # Ubuntu
# o
sudo zypper install bpftool  # SUSE
```

---

## 3. Configuración de Red LXC

### 3.1 Identificar Interfaces de Red
```bash
# Listar interfaces
ip link show
# Ejemplo de salida:
# 1: lo: <LOOPBACK,UP> ...
# 2: enp5s0: <BROADCAST,MULTICAST> ...
# 3: enp6s0: <BROADCAST,MULTICAST> ...
```

### 3.2 Crear Bridge LXD Exclusivo
```bash
# IMPORTANTE: Usar una interfaz física dedicada para LXC
# En este ejemplo, enp6s0 se dedicará a lxdbr1

# Detener LXD si está corriendo
sudo systemctl stop lxd lxd-agent

# Eliminar bridge existente si hay conflicto
sudo ip link delete lxdbr1 2>/dev/null || true

# Crear red LXD con bridge dedicado
sudo lxc network create lxdbr1 --type bridge

# Configurar subnet
sudo lxc network set lxdbr1 ipv4.address=192.168.2.200/24
sudo lxc network set lxdbr1 ipv4.dhcp.ranges=192.168.2.210-192.168.2.250
sudo lxc network set lxdbr1 ipv4.nat=true

# Verificar configuración
sudo lxc network show lxdbr1
```

### 3.3 Configurar Firewall (SUSE)
```bash
# SUSE requiere reglas explícitas de FORWARD para el bridge
sudo iptables -I FORWARD -i lxdbr1 -j ACCEPT
sudo iptables -I FORWARD -o lxdbr1 -j ACCEPT

# Verificar reglas
sudo iptables -L FORWARD -v -n | grep lxdbr1
```

### 3.4 Mover Interfaz Física al Bridge
```bash
# IMPORTANTE: Esto puede desconectar la sesión SSH
# Ejecutar en consola local o tener acceso de rescue

# Eliminar IP de enp6s0
sudo ip addr flush dev enp6s0

# Agregar al bridge
sudo ip link set enp6s0 master lxdbr1

# Verificar
ip addr show enp6s0
# Debe mostrar: master lxdbr1
```

---

## 4. Creación del Perfil LXC

### 4.1 Crear Perfil ebpf-blockchain
```bash
# Crear perfil
lxc profile create ebpf-blockchain

# Editar perfil con configuración completa
cat << 'EOF' | lxc profile edit ebpf-blockchain
name: ebpf-blockchain
description: "Entorno de desarrollo eBPF con Rust + Aya - Perfil LXC"
config:
  limits.cpu: "4"
  limits.memory: 8GiB
  security.privileged: "true"
  security.syscalls.intercept.bpf: "true"
  security.syscalls.intercept.bpf.devices: "true"
devices:
  root:
    path: /
    pool: default
    type: disk
  eth0:
    name: eth0
    network: lxdbr1
    type: nic
EOF

# Verificar perfil
lxc profile show ebpf-blockchain
```

### 4.2 Verificar Perfil Creado
```bash
lxc profile list
# Output esperado:
# +-----------------+
# | NAME            |
# +-----------------+
# | default         |
# +-----------------+
# | ebpf-blockchain |
# +-----------------+
```

---

## 5. Creación del Primer Nodo

### 5.1 Lanzar Contenedor
```bash
# Crear y lanzar contenedor Ubuntu 22.04
lxc launch ubuntu:22.04 ebpf-blockchain -p ebpf-blockchain

# Esperar a que esté disponible
sleep 10

# Ver estado
lxc list
# Output esperado:
# +-----------------+---------+--------------------+---------------------------+
# |      NAME       |  STATE  |        IPV4        |          IPV6             |
# +-----------------+---------+--------------------+---------------------------+
# | ebpf-blockchain | RUNNING | 192.168.2.210/24   | fd42:cb45:...:fec1:4c4c   |
# +-----------------+---------+--------------------+---------------------------+
```

### 5.2 Configurar Red Estática (DHCP puede fallar)
```bash
# Configurar IP estática
lxc exec ebpf-blockchain -- bash -c "ip addr flush dev eth0"
lxc exec ebpf-blockchain -- bash -c "ip addr add 192.168.2.210/24 dev eth0"
lxc exec ebpf-blockchain -- bash -c "ip route add default via 192.168.2.200"

# Configurar DNS
lxc exec ebpf-blockchain -- bash -c "echo 'nameserver 8.8.8.8' > /etc/resolv.conf"

# Verificar conectividad
lxc exec ebpf-blockchain -- ping -c 3 8.8.8.8
```

### 5.3 Verificar Acceso SSH (Opcional)
```bash
# Desde el host
ssh ubuntu@192.168.2.210
# Contraseña: ubuntu (primer inicio)

# O usar lxc exec
lxc exec ebpf-blockchain -- bash
```

---

## 6. Instalación de Dependencias

### 6.1 Actualizar Sistema e Instalar Paquetes Base
```bash
lxc exec ebpf-blockchain -- bash -c "
apt update && \
apt upgrade -y && \
apt install -y build-essential clang llvm libelf-dev libbpf-dev curl git \
    pkg-config zlib1g-dev libssl-dev
"
```

### 6.2 Instalar Rust
```bash
# Descargar e instalar rustup
lxc exec ebpf-blockchain -- bash -c "
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
"

# Verificar instalación
lxc exec ebpf-blockchain -- /root/.cargo/bin/rustc --version
lxc exec ebpf-blockchain -- /root/.cargo/bin/cargo --version
```

### 6.3 Instalar Toolchain Nightly
```bash
# Instalar nightly con componente rust-src (requerido para eBPF)
lxc exec ebpf-blockchain -- bash -c "
/root/.cargo/bin/rustup toolchain install nightly --component rust-src
"

# Establecer nightly como default
lxc exec ebpf-blockchain -- bash -c "
/root/.cargo/bin/rustup default nightly
"

# Verificar
lxc exec ebpf-blockchain -- bash -c "
source /root/.cargo/env && rustc --version
"
```

### 6.4 Instalar Herramientas eBPF
```bash
# Instalar bpf-linker (requerido para compilar eBPF con Rust)
lxc exec ebpf-blockchain -- bash -c "
source /root/.cargo/env && \
cargo install bpf-linker --force
"

# Opcional: cargo-watch para recompilación automática
lxc exec ebpf-blockchain -- bash -c "
source /root/.cargo/env && \
cargo install cargo-watch --force
"

# Verificar instalación
lxc exec ebpf-blockchain -- bash -c "
source /root/.cargo/env && \
which bpf-linker && which cargo-watch
"
```

---

## 7. Montaje del Proyecto

### 7.1 Montar Directorio del Host en LXC
```bash
# Montar proyecto como disco
lxc config device add ebpf-blockchain workspace \
    disk \
    source=/home/maxi/Documentos/source/ebpf-blockchain \
    path=/root/ebpf-blockchain

# Verificar montaje
lxc exec ebpf-blockchain -- ls -la /root/ebpf-blockchain/
# Debes ver: ebpf-node/, scripts/, rfc.md, etc.
```

### 7.2 Estructura del Proyecto
```bash
lxc exec ebpf-blockchain -- bash -c "
tree /root/ebpf-blockchain -L 2
"
# Output esperado:
# ebpf-blockchain/
# ├── ebpf-node/
# │   ├── ebpf-node/           # User space (Rust)
# │   │   ├── src/main.rs
# │   │   ├── Cargo.toml
# │   │   └── target/          # Build output
# │   └── ebpf-node-ebpf/      # Kernel space (eBPF)
# │       ├── src/main.rs
# │       └── Cargo.toml
# ├── scripts/
# │   ├── create_node.sh
# │   ├── create_cluster.sh
# │   ├── setup_cluster.sh
# │   ├── verify_rfc001.sh
# │   ├── test_blacklist.sh
# │   └── test_attack.sh
# ├── ebpf-blockchain/          # Monitoring stack
# │   ├── docker-compose.yml
# │   ├── prometheus.yml
# │   └── provisioning/
# ├── ebpf-blockchain.yaml     # LXC profile
# ├── rfc.md                   # RFC 001
# └── lxc-install.md           # Documentación
```

---

## 8. Compilación y Ejecución

### 8.1 Compilar el Proyecto
```bash
# Compilar dentro del contenedor
lxc exec ebpf-blockchain -- bash -c "
source /root/.cargo/env && \
cd /root/ebpf-blockchain/ebpf-node && \
cargo build
"

# Tiempo estimado: 2-5 minutos (compilación primera vez)
```

### 8.2 Verificar Compilación
```bash
# Verificar binario
lxc exec ebpf-blockchain -- bash -c "
ls -lh /root/ebpf-blockchain/ebpf-node/target/debug/ebpf-node
"
```

### 8.3 Ejecutar el Nodo
```bash
# Ejecutar en foreground (para pruebas)
lxc exec ebpf-blockchain -- bash -c "
source /root/.cargo/env && \
RUST_LOG=info /root/ebpf-blockchain/ebpf-node/target/debug/ebpf-node --iface eth0
"

# Output esperado:
# [INFO  ebpf_node] Local Peer ID: 12D3KooWGVPbU6Aqpj7YgqhZUAoi485GSnzS8vxtVz7ZSu71b1Qn
# [INFO  ebpf_node] Prometheus metrics server listening on 0.0.0.0:9090/metrics
# --- Latency Histogram (nanoseconds, power of 2 buckets) ---
# Bucket 2^24: 1 packets
# Bucket 2^31: 2 packets
```

### 8.4 Ejecutar en Background (Producción)
```bash
lxc exec ebpf-blockchain -- bash -c "
source /root/.cargo/env && \
nohup /root/ebpf-blockchain/ebpf-node/target/debug/ebpf-node --iface eth0 > /tmp/ebpf.log 2>&1 &
"

# Verificar que está corriendo
lxc exec ebpf-blockchain -- ps aux | grep ebpf-node

# Ver logs
lxc exec ebpf-blockchain -- tail -f /tmp/ebpf.log
```

### 8.5 Modo Desarrollo con Hot-Reload
```bash
# Dentro del contenedor, ejecutar:
source /root/.cargo/env
cd /root/ebpf-blockchain/ebpf-node

# Recompilar automáticamente al guardar archivos
RUST_LOG=info cargo watch -c \
    -w ebpf-node/src/ \
    -w ebpf-node-ebpf/src/ \
    -x 'run --bin ebpf-node -- --iface eth0'
```

---

## 9. Stack de Monitoreo (Prometheus + Grafana)

### 9.1 Estructura de Archivos
```
ebpf-blockchain/
├── docker-compose.yml          # Definición de servicios
├── prometheus.yml              # Targets de scrape
└── provisioning/
    ├── datasources/
    │   └── prometheus.yaml     # Datasource de Prometheus
    └── dashboards/
        ├── dashboards.yaml     # Proveedor de dashboards
        └── ebpf-node.json     # Dashboard de ejemplo
```

### 9.2 Configuración de Prometheus (prometheus.yml)
```yaml
global:
  scrape_interval: 5s
  evaluation_interval: 15s

scrape_configs:
  - job_name: "prometheus"
    static_configs:
      - targets: ["localhost:9090"]

  - job_name: "ebpf_node_1"
    static_configs:
      - targets: ["192.168.2.210:9090"]
    metrics_path: /metrics

  - job_name: "ebpf_node_2"
    static_configs:
      - targets: ["192.168.2.211:9090"]
    metrics_path: /metrics
```

### 9.3 Docker Compose (docker-compose.yml)
```yaml
version: '3.8'

services:
  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    network_mode: "host"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
    restart: unless-stopped

  grafana:
    image: grafana/grafana:10.4.0
    container_name: grafana
    network_mode: "host"
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_NAME=Main Org.
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Admin
      - GF_SECURITY_ADMIN_USER=admin
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
    volumes:
      - grafana_data:/var/lib/grafana
      - ./provisioning/datasources:/etc/grafana/provisioning/datasources:ro
      - ./provisioning/dashboards:/etc/grafana/provisioning/dashboards:ro
    restart: unless-stopped

volumes:
  grafana_data:
```

### 9.4 Datasource Prometheus (provisioning/datasources/prometheus.yaml)
```yaml
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://localhost:9090
    isDefault: true
    uid: prometheus
    editable: true
    jsonData:
      httpMethod: POST
```

### 9.5 Dashboard JSON (provisioning/dashboards/ebpf-node.json)
```json
{
  "annotations": { "list": [] },
  "editable": true,
  "panels": [
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "fieldConfig": {
        "defaults": {
          "color": { "mode": "palette-classic" },
          "custom": { "drawStyle": "line", "fillOpacity": 10, "lineWidth": 1 }
        }
      },
      "gridPos": { "h": 8, "w": 24, "x": 0, "y": 0 },
      "id": 1,
      "targets": [
        {
          "expr": "ebpf_node_latency_buckets",
          "legendFormat": "Bucket 2^{{bucket}}",
          "refId": "A"
        }
      ],
      "title": "eBPF Node Latency Buckets",
      "type": "timeseries"
    }
  ],
  "refresh": "5s",
  "schemaVersion": 39,
  "tags": ["ebpf", "node", "blockchain"],
  "time": { "from": "now-15m", "to": "now" },
  "title": "eBPF Node Dashboard",
  "uid": "ebpf-node-1"
}
```

### 9.6 Iniciar el Stack de Monitoreo
```bash
# Ir al directorio del stack
cd /home/maxi/Documentos/source/ebpf-blockchain/ebpf-blockchain

# Iniciar servicios
docker compose up -d

# Ver estado
docker compose ps

# Ver logs
docker compose logs -f prometheus
docker compose logs -f grafana
```

### 9.7 Verificar Prometheus
```bash
# API de targets
curl http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | select(.job=="ebpf_node_1")'

# Query de métricas
curl http://localhost:9090/api/v1/query?query=ebpf_node_latency_buckets | jq
```

### 9.8 Acceder a Grafana
```
URL: http://localhost:3000
Usuario: admin
Contraseña: admin
```

---

## 10. Creación de Nodos Adicionales

### 10.1 Usando Script Automatizado
```bash
# Crear un nodo nuevo
cd /home/maxi/Documentos/source/ebpf-blockchain
./scripts/create_node.sh ebpf-blockchain-2 192.168.2.210

# El script:
# 1. Crea/clona el contenedor
# 2. Configura IP estática
# 3. Configura DNS
# 4. Verifica conectividad
# 5. Muestra comandos útiles
```

### 10.2 Creación Manual
```bash
# Crear contenedor
lxc launch ubuntu:22.04 ebpf-blockchain-2 -p ebpf-blockchain

# Esperar
sleep 10

# Configurar red
lxc exec ebpf-blockchain-2 -- bash -c "
ip addr flush dev eth0
ip addr add 192.168.2.211/24 dev eth0
ip route add default via 192.168.2.200
echo 'nameserver 8.8.8.8' > /etc/resolv.conf
"

# Instalar dependencias (si no están)
lxc exec ebpf-blockchain-2 -- bash -c "
apt update && apt install -y build-essential clang llvm libelf-dev libbpf-dev curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
/root/.cargo/bin/rustup toolchain install nightly --component rust-src
"

# Montar proyecto
lxc config device add ebpf-blockchain-2 workspace \
    disk \
    source=/home/maxi/Documentos/source/ebpf-blockchain \
    path=/root/ebpf-blockchain

# Compilar
lxc exec ebpf-blockchain-2 -- bash -c "
source /root/.cargo/env
cd /root/ebpf-blockchain/ebpf-node
cargo build
"

# Iniciar
lxc exec ebpf-blockchain-2 -- bash -c "
source /root/.cargo/env
nohup /root/ebpf-blockchain/ebpf-node/target/debug/ebpf-node --iface eth0 > /tmp/ebpf.log 2>&1 &
"
```

### 10.3 Crear Múltiples Nodos (Cluster)
```bash
# Crear cluster de 3 nodos atacantes
cd /home/maxi/Documentos/source/ebpf-blockchain
./scripts/create_cluster.sh 3

# Listar todos los nodos
lxc list --format csv -c n,4 | grep -E "ebpf-blockchain|ebpf-attacker"
```

---

## 11. Verificación del Sistema

### 11.1 Verificar que los Nodos Están Corriendo
```bash
lxc list --format csv -c n,4,s | grep ebpf
```

### 11.2 Verificar Métricas de Cada Nodo
```bash
# Nodo 1
curl -s http://192.168.2.210:9090/metrics | head -20

# Nodo 2
curl -s http://192.168.2.211:9090/metrics | head -20
```

### 11.3 Verificar Prometheus Scraping
```bash
# Ver targets activos
curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets'
```

### 11.4 Ejecutar Script de Verificación RFC
```bash
cd /home/maxi/Documentos/source/ebpf-blockchain
./scripts/verify_rfc001.sh
```

### 11.5 Verificación de Componentes

| Componente | Verificación | Comando |
|------------|--------------|---------|
| XDP | Verificar programa cargado | `lxc exec ebpf-blockchain -- bpftool prog list` |
| KProbes | Verificar probes activos | `lxc exec ebpf-blockchain -- cat /sys/kernel/debug/tracing/available_filter_functions | grep -E "netif_receive\|napi_consume"` |
| LpmTrie | Verificar mapa | `lxc exec ebpf-blockchain -- bpftool map list` |
| libp2p | Verificar peer ID en logs | `lxc exec ebpf-blockchain -- tail /tmp/ebpf.log | grep Peer` |
| Prometheus | Ver scrape | `curl localhost:9090/api/v1/targets` |

---

## 12. Scripts de Automatización

### 12.1 create_node.sh
```bash
# Uso
./scripts/create_node.sh <nombre_nodo> [ip_gateway]

# Ejemplo
./scripts/create_node.sh ebpf-blockchain-2 192.168.2.210
```

### 12.2 create_cluster.sh
```bash
# Uso
./scripts/create_cluster.sh <cantidad>

# Ejemplo: crear 3 nodos atacantes
./scripts/create_cluster.sh 3
```

### 12.3 setup_cluster.sh
```bash
# Uso: setup completo (nodos + Prometheus + Grafana)
./scripts/setup_cluster.sh

# Este script:
# 1. Crea ebpf-blockchain-2
# 2. Instala dependencias
# 3. Actualiza prometheus.yml
# 4. Crea dashboard de cluster
# 5. Reinicia servicios
```

### 12.4 verify_rfc001.sh
```bash
# Uso: verificar implementación de RFC 001
./scripts/verify_rfc001.sh

# Verifica:
# - Conectividad básica
# - eBPF Maps
# - Stack P2P
# - KProbes
# - Prometheus
```

### 12.5 test_blacklist.sh
```bash
# Uso: explicar mecanismo de blacklist
./scripts/test_blacklist.sh
```

### 12.6 test_attack.sh
```bash
# Uso: probar mecanismo de ATTACK
./scripts/test_attack.sh [ip_objetivo]

# Ejemplo
./scripts/test_attack.sh 192.168.2.210
```

---

## 13. Testing P2P y Blacklist

### 13.1 Arquitectura de Seguridad

```
[Mensaje Gossipsub "ATTACK"]
           │
           ▼
┌──────────────────────┐
│  libp2p Gossipsub    │
│  (Message Handler)    │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  Parsear mensaje     │
│  Extraer IP del peer  │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  NODES_BLACKLIST     │
│  (LpmTrie)          │
│  Escribir: IP -> 1   │
└──────────┬───────────┘
           │
    ┌──────┴──────┐
    │              │
    ▼              ▼
[Futuros      [Futuros
 paquetes]     paquetes]
    │              │
    │              ▼
    │    ┌──────────────────┐
    │    │  XDP (eBPF)      │
    │    │  LpmTrie Lookup   │
    │    └────────┬─────────┘
    │             │
    │        ┌────┴────┐
    │        │         │
    │        ▼         ▼
    │   XDP_DROP   XDP_PASS
    │   (bloqueado) (procesado)
    ▼
[Paquete
 bloqueado]
```

### 13.2 Conectar Dos Nodos P2P

```bash
# Nodo 1: Obtener Peer ID
lxc exec ebpf-blockchain -- bash -c "
grep 'Peer' /tmp/ebpf.log | tail -1
"

# Output ejemplo:
# [INFO  ebpf_node] Local Peer ID: 12D3KooWGVPbU6Aqpj7YgqhZUAoi485GSnzS8vxtVz7ZSu71b1Qn

# Nodo 2: Dial al Nodo 1 (requiere código adicional o modificar main.rs)
# Por ahora, los nodos se descubren automáticamente via Gossipsub
```

### 13.3 Generar Tráfico para Activar Métricas

```bash
# Desde el host, generar tráfico ICMP
ping -c 100 192.168.2.210

# Verificar que las métricas cambian
curl -s http://192.168.2.210:9090/metrics | grep ebpf_node_latency_buckets
```

### 13.4 Simular Ataque (Blacklist)

```bash
# Ejecutar script de test
./scripts/test_attack.sh 192.168.2.210

# Verificar blacklist en el nodo
lxc exec ebpf-blockchain -- bash -c "
bpftool map dump id <id_de_NODES_BLACKLIST>
"

# El mapa debe contener la IP 1.2.3.4 bloqueada
```

---

## 14. Solución de Problemas

### 14.1 Cloud-Init No Funciona

**Problema**: `runcmd` se ejecuta antes de que la red esté disponible.

**Solución**: No usar cloud-init para instalar. Instalar manualmente dentro del contenedor después de que esté corriendo.

```bash
# Verificar estado de cloud-init
lxc exec ebpf-blockchain -- cloud-init status

# Si está atascado, eliminar y recrear el contenedor
lxc delete ebpf-blockchain --force
```

### 14.2 DHCP No Asigna IP

**Problema**: El contenedor no obtiene dirección IPv4.

**Solución**: Usar IP estática.

```bash
lxc exec ebpf-blockchain -- bash -c "
ip addr flush dev eth0
ip addr add 192.168.2.210/24 dev eth0
ip route add default via 192.168.2.200
"
```

### 14.3 DNS No Funciona

**Problema**: `Temporary failure in name resolution`.

**Solución**: Configurar DNS manualmente.

```bash
echo 'nameserver 8.8.8.8' > /etc/resolv.conf
# o
echo 'nameserver 1.1.1.1' >> /etc/resolv.conf
```

### 14.4 Prometheus No Alcanza el Contenedor

**Problema**: `no route to host`.

**Causa**: Prometheus (en Docker) y el contenedor LXC están en subredes diferentes.

**Solución**: Usar `network_mode: host` en docker-compose.yml y asegurar que la red lxdbr1 tenga NAT habilitado.

```bash
# Verificar NAT en el bridge
sudo lxc network show lxdbr1 | grep nat

# Si no está habilitado
sudo lxc network set lxdbr1 ipv4.nat=true
```

### 14.5 Grafana Muestra "Login Failed"

**Problema**: Autenticación rota en Grafana 12.x.

**Solución**: Usar Grafana 10.4.0.

```yaml
# En docker-compose.yml
grafana:
  image: grafana/grafana:10.4.0  # No usar versiones >= 11
```

### 14.6 Firewall Bloquea Tráfico

**Problema en SUSE**: El firewall bloquea tráfico del bridge.

**Solución**:
```bash
sudo iptables -I FORWARD -i lxdbr1 -j ACCEPT
sudo iptables -I FORWARD -o lxdbr1 -j ACCEPT
```

### 14.7 eBPF No Se Carga

**Problema**: Error al cargar el programa XDP.

**Verificaciones**:
```bash
# Verificar que el contenedor es privilegiado
lxc config get ebpf-blockchain security.privileged
# Debe retornar: true

# Verificar syscalls de BPF
lxc config get ebpf-blockchain security.syscalls.intercept.bpf
# Debe retornar: true

# Verificar bpf() en /proc/sys/kernel/unprivileged_bpf_disabled
cat /proc/sys/kernel/unprivileged_bpf_disabled
# 0 = permite, 1 = restringe, 2 = deshabilita
```

### 14.8 Rustup Falla con DNS

**Problema**: `Temporary failure in name resolution` durante instalación de Rust.

**Solución**:
```bash
# Verificar DNS antes de instalar
lxc exec ebpf-blockchain -- ping -c 1 rustup.rs

# Si falla, configurar DNS
lxc exec ebpf-blockchain -- bash -c "
echo 'nameserver 8.8.8.8' > /etc/resolv.conf
"

# Luego reintentar
lxc exec ebpf-blockchain -- bash -c "
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
"
```

---

## Apéndice A: Comandos Útiles de LXC

```bash
# Listar contenedores
lxc list

# Información de un contenedor
lxc info ebpf-blockchain

# Ejecutar comando
lxc exec ebpf-blockchain -- <comando>

# Abrir shell
lxc exec ebpf-blockchain -- bash

# Iniciar/Detener
lxc start ebpf-blockchain
lxc stop ebpf-blockchain

# Eliminar
lxc delete ebpf-blockchain --force

# Clonar
lxc copy ebpf-blockchain ebpf-blockchain-backup

# Configuración
lxc config show ebpf-blockchain
lxc config set ebpf-blockchain limits.cpu 2
lxc config set ebpf-blockchain limits.memory 4GiB

# Montar directorio
lxc config device add ebpf-blockchain workspace disk source=/path/to/code path=/root/code

# Ver archivos del contenedor
lxc file pull ebpf-blockchain/etc/hosts ./

# Copiar archivos al contenedor
lxc file push ./file.txt ebpf-blockchain/root/
```

## Apéndice B: Referencias

- [AYA eBPF Framework](https://aya-rs.dev/)
- [LXD Documentation](https://documentation.ubuntu.com/lxd/en/latest/)
- [libp2p Documentation](https://docs.libp2p.io/)
- [Prometheus](https://prometheus.io/docs/)
- [Grafana](https://grafana.com/docs/)
- [eBPF Documentation](https://ebpf.io/)

---

## Historial de Versiones

| Versión | Fecha | Cambios |
|---------|-------|---------|
| 1.0 | Marzo 2026 | Versión inicial del documento |
