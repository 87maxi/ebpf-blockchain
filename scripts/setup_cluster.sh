#!/bin/bash
# =============================================================================
# Setup completo de cluster para testing P2P + ATTACK
# Crea nodos, configura Prometheus y Grafana
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
GRAFANA_DIR="$PROJECT_DIR/ebpf-blockchain"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Setup Completo Cluster eBPF${NC}"
echo -e "${BLUE}========================================${NC}"

# =============================================================================
# 1. Crear segundo nodo (attacker/victim)
# =============================================================================
echo ""
echo -e "${YELLOW}1. Creando segundo nodo para testing P2P...${NC}"

if lxc info ebpf-blockchain-2 &>/dev/null; then
    echo "ebpf-blockchain-2 ya existe, verificando estado..."
    lxc start ebpf-blockchain-2 2>/dev/null || true
else
    echo "Creando ebpf-blockchain-2..."
    
    # Crear perfil si no existe
    if ! lxc profile list | grep -q ebpf-blockchain; then
        cat << 'EOF' | lxc profile edit ebpf-blockchain
name: ebpf-blockchain
config:
  limits.cpu: "2"
  limits.memory: 4GiB
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
    fi
    
    lxc launch ubuntu:22.04 ebpf-blockchain-2 -p ebpf-blockchain
fi

# Configurar red del segundo nodo
sleep 5
echo "Configurando red de ebpf-blockchain-2..."
lxc exec ebpf-blockchain-2 -- bash -c "ip addr flush dev eth0 || true"
lxc exec ebpf-blockchain-2 -- bash -c "ip addr add 192.168.2.211/24 dev eth0"
lxc exec ebpf-blockchain-2 -- bash -c "ip route add default via 192.168.2.200"
lxc exec ebpf-blockchain-2 -- bash -c "echo 'nameserver 8.8.8.8' > /etc/resolv.conf"

# Instalar dependencias si no están
echo "Verificando dependencias en ebpf-blockchain-2..."
if ! lxc exec ebpf-blockchain-2 -- which rustc &>/dev/null; then
    echo "Instalando dependencias..."
    lxc exec ebpf-blockchain-2 -- bash -c "apt update && apt install -y build-essential clang llvm libelf-dev libbpf-dev curl git"
    lxc exec ebpf-blockchain-2 -- bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
    lxc exec ebpf-blockchain-2 -- bash -c "/root/.cargo/bin/rustup toolchain install nightly --component rust-src"
    lxc exec ebpf-blockchain-2 -- bash -c "/root/.cargo/bin/cargo install bpf-linker cargo-watch"
fi

# Montar proyecto
lxc config device add ebpf-blockchain-2 workspace disk source=/home/maxi/Documentos/source/ebpf-blockchain path=/root/ebpf-blockchain 2>/dev/null || true

# =============================================================================
# 2. Actualizar prometheus.yml
# =============================================================================
echo ""
echo -e "${YELLOW}2. Actualizando Prometheus...${NC}"

cat > "$PROJECT_DIR/ebpf-blockchain/prometheus.yml" << 'EOF'
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
EOF

echo "Prometheus actualizado"

# Reiniciar Prometheus
cd "$GRAFANA_DIR" && docker compose restart prometheus 2>/dev/null || docker restart prometheus

# =============================================================================
# 3. Crear dashboard unificado para Grafana
# =============================================================================
echo ""
echo -e "${YELLOW}3. Creando dashboard unificado...${NC}"

cat > "$GRAFANA_DIR/provisioning/dashboards/ebpf-cluster.json" << 'EOF'
{
  "annotations": { "list": [] },
  "editable": true,
  "fiscalYearStartMonth": 0,
  "graphTooltip": 0,
  "id": null,
  "links": [],
  "liveNow": false,
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
          "expr": "sum by (job) (ebpf_node_latency_buckets)",
          "legendFormat": "{{job}}",
          "refId": "A"
        }
      ],
      "title": "Latencia por Nodo",
      "type": "timeseries"
    },
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "fieldConfig": { "defaults": { "color": { "mode": "thresholds" } } },
      "gridPos": { "h": 4, "w": 6, "x": 0, "y": 8 },
      "id": 2,
      "targets": [{ "expr": "count(ebpf_node_latency_buckets > 0)", "refId": "A" }],
      "title": "Nodos Activos",
      "type": "stat"
    },
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "fieldConfig": { "defaults": { "color": { "mode": "thresholds" } } },
      "gridPos": { "h": 4, "w": 6, "x": 6, "y": 8 },
      "id": 3,
      "targets": [{ "expr": "sum(ebpf_node_latency_buckets)", "refId": "A" }],
      "title": "Total Paquetes",
      "type": "stat"
    }
  ],
  "refresh": "5s",
  "schemaVersion": 39,
  "tags": ["ebpf", "cluster", "blockchain"],
  "templating": { "list": [] },
  "time": { "from": "now-15m", "to": "now" },
  "timepicker": {},
  "timezone": "browser",
  "title": "eBPF Cluster Dashboard",
  "uid": "ebpf-cluster-1",
  "version": 1
}
EOF

# Reiniciar Grafana
docker restart grafana 2>/dev/null

# =============================================================================
# 4. Resumen
# =============================================================================
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Setup Completo${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Nodos creados:"
lxc list --format csv -c n,4 | grep -E "ebpf-blockchain"
echo ""
echo -e "${YELLOW}URLs:${NC}"
echo "  Grafana: http://localhost:3000"
echo "  Prometheus: http://localhost:9090"
echo ""
echo -e "${YELLOW}Dashboards:${NC}"
echo "  - http://localhost:3000/d/ebpf-node-1 (Nodo 1)"
echo "  - http://localhost:3000/d/ebpf-cluster-1 (Cluster)"
echo ""
echo -e "${YELLOW}Para iniciar el segundo nodo:${NC}"
echo "  lxc exec ebpf-blockchain-2 bash"
echo "  cd /root/ebpf-blockchain/ebpf-node"
echo "  source /root/.cargo/env"
echo "  RUST_LOG=info ./target/debug/ebpf-node --iface eth0 &"
