#!/bin/bash
# =============================================================================
# Configure LXD ports for ebpf-blockchain nodes
# Expone los puertos RPC, Metrics y P2P en los contenedores LXD
# =============================================================================

set -euo pipefail

# Configuración de nodos
NODES=("ebpf-node-1" "ebpf-node-2" "ebpf-node-3")
RPC_PORT=8080
METRICS_PORT=9090
P2P_START=50000

# Colores para salida
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Verificar que LXD está disponible
if ! command -v lxc &> /dev/null; then
    log_error "LXD no está instalado o no está en el PATH"
    exit 1
fi

# Verificar que hay al menos un nodo
if [ ${#NODES[@]} -eq 0 ]; then
    log_error "No se configuraron nodos"
    exit 1
fi

log_info "Comenzando configuración de puertos para ebpf-blockchain nodes..."
log_info "=================================================="

for i in "${!NODES[@]}"; do
    NODE="${NODES[$i]}"
    P2P_PORT=$((P2P_START + i))
    
    log_info "Configurando puertos para $NODE..."
    log_info "  RPC:   $RPC_PORT/tcp"
    log_info "  Metrics: $METRICS_PORT/tcp"
    log_info "  P2P:   $P2P_PORT/tcp"
    
    # Verificar que el nodo existe
    if ! lxc info "$NODE" &> /dev/null; then
        log_warn "El nodo $NODE no existe, omitiendo..."
        continue
    fi
    
    # Verificar que el nodo está ejecutándose
    if [ "$(lxc info "$NODE" | grep -o 'Status: [a-zA-Z]*' | awk '{print $2}')" != "RUNNING" ]; then
        log_warn "El nodo $NODE no está RUNNING, intentando configurar de todos modos..."
    fi
    
    # API HTTP port (RPC)
    if lxc config device show "$NODE" rpc &> /dev/null; then
        lxc config device set "$NODE" rpc host-port="$RPC_PORT" 2>/dev/null || \
        log_warn "No se pudo actualizar dispositivo rpc en $NODE"
    else
        lxc config device add "$NODE" rpc proxy 0.0.0.0:$RPC_PORT 127.0.0.1:$RPC_PORT tcp 2>/dev/null || \
        log_warn "No se pudo crear dispositivo rpc en $NODE"
    fi
    
    # Metrics port
    if lxc config device show "$NODE" metrics &> /dev/null; then
        lxc config device set "$NODE" metrics host-port="$METRICS_PORT" 2>/dev/null || \
        log_warn "No se pudo actualizar dispositivo metrics en $NODE"
    else
        lxc config device add "$NODE" metrics proxy 0.0.0.0:$METRICS_PORT 127.0.0.1:$METRICS_PORT tcp 2>/dev/null || \
        log_warn "No se pudo crear dispositivo metrics en $NODE"
    fi
    
    # P2P port
    if lxc config device show "$NODE" p2p &> /dev/null; then
        lxc config device set "$NODE" p2p host-port="$P2P_PORT" 2>/dev/null || \
        log_warn "No se pudo actualizar dispositivo p2p en $NODE"
    else
        lxc config device add "$NODE" p2p proxy 0.0.0.0:$P2P_PORT 127.0.0.1:$P2P_PORT tcp 2>/dev/null || \
        log_warn "No se pudo crear dispositivo p2p en $NODE"
    fi
    
    log_info "Puertos configurados exitosamente para $NODE"
    echo ""
done

log_info "=================================================="
log_info "Todos los puertos fueron configurados exitosamente."
log_info ""
log_info "Resumen de puertos:"
for i in "${!NODES[@]}"; do
    NODE="${NODES[$i]}"
    P2P_PORT=$((P2P_START + i))
    log_info "  $NODE: RPC=$RPC_PORT, Metrics=$METRICS_PORT, P2P=$P2P_PORT"
done
log_info ""
log_info "Para verificar, usa: lxc config device show <node> <device>"
