#!/bin/bash
# =============================================================================
# Crear múltiples nodos eBPF para testing
# Uso: ./scripts/create_cluster.sh <cantidad>
# Ejemplo: ./scripts/create_cluster.sh 3
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

CANTIDAD="${1:-2}"
GATEWAY="192.168.2.200"
BASE_NAME="ebpf-attacker"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Creando cluster de $CANTIDAD nodos${NC}"
echo -e "${BLUE}========================================${NC}"

# Buscar IPs disponibles
START_IP=220
for i in $(seq 1 $CANTIDAD); do
    NODE_IP="192.168.2.$((START_IP + i))"
    
    if lxc info "$BASE_NAME-$i" &>/dev/null; then
        echo -e "${YELLOW}Nodo $BASE_NAME-$i ya existe, omitiendo...${NC}"
        continue
    fi
    
    echo ""
    echo -e "${BLUE}Creando nodo $BASE_NAME-$i ($NODE_IP)${NC}"
    
    # Crear contenedor
    lxc launch ubuntu:22.04 "$BASE_NAME-$i" -p ebpf-blockchain 2>/dev/null || true
    sleep 3
    
    # Configurar red
    lxc exec "$BASE_NAME-$i" -- bash -c "ip addr flush dev eth0 || true" 2>/dev/null || true
    lxc exec "$BASE_NAME-$i" -- bash -c "ip addr add $NODE_IP/24 dev eth0" 2>/dev/null || true
    lxc exec "$BASE_NAME-$i" -- bash -c "ip route add default via $GATEWAY" 2>/dev/null || true
    lxc exec "$BASE_NAME-$i" -- bash -c "echo 'nameserver 8.8.8.8' > /etc/resolv.conf" 2>/dev/null || true
    
    echo -e "${GREEN}  Nodo $BASE_NAME-$i listo${NC}"
    
    # Copiar script de instalación de dependencias
    echo -e "${BLUE}  Copiando script de instalación de dependencias...${NC}"
    lxc exec "$BASE_NAME-$i" -- mkdir -p /opt 2>/dev/null || true
    lxc file push "$SCRIPT_DIR/install-deps.sh" "$BASE_NAME-$i/opt/install-deps.sh" --mode=755 2>/dev/null || true
    
    # Ejecutar instalación de dependencias
    echo -e "${BLUE}  Instalando dependencias en $BASE_NAME-$i (esto puede tomar varios minutos)...${NC}"
    lxc exec "$BASE_NAME-$i" -- bash /opt/install-deps.sh || {
        echo -e "${YELLOW}  Advertencia: La instalación de dependencias falló${NC}"
        echo -e "${YELLOW}  Puedes ejecutar manualmente: lxc exec $BASE_NAME-$i -- bash /opt/install-deps.sh${NC}"
    }
    
    echo -e "${GREEN}  Dependencias instaladas en $BASE_NAME-$i${NC}"
done

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Cluster creado${NC}"
echo -e "${GREEN}========================================${NC}"

# Listar nodos
echo ""
echo "Nodos activos:"
lxc list --format csv -c n,4 | grep -E "$BASE_NAME|ebpf-blockchain"
