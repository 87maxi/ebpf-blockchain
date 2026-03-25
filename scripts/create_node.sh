#!/bin/bash
# =============================================================================
# Script para crear y levantar nodos eBPF adicionales
# Uso: ./scripts/create_node.sh <nombre> [puerta_de_enlace]
# Ejemplo: ./scripts/create_node.sh ebpf-blockchain-2 192.168.2.210
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

if [ -z "$1" ]; then
    echo -e "${RED}Error: Nombre del nodo requerido${NC}"
    echo "Uso: $0 <nombre> [ip_gateway]"
    echo "Ejemplo: $0 ebpf-blockchain-2 192.168.2.210"
    exit 1
fi

NODE_NAME="$1"
GATEWAY="${2:-192.168.2.200}"
NETWORK="192.168.2.0/24"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Creando nodo: $NODE_NAME${NC}"
echo -e "${BLUE}========================================${NC}"

# Verificar que el nodo no exista
if lxc info "$NODE_NAME" &>/dev/null; then
    echo -e "${YELLOW}El nodo $NODE_NAME ya existe. Opciones:${NC}"
    echo "  1. Iniciar existente (s)"
    echo "  2. Eliminar y recrear (r)"
    echo "  3. Salir (n)"
    read -p "Opción [3]: " opcion
    opcion="${opcion:-3}"
    
    case $opcion in
        1)
            echo "Iniciando nodo existente..."
            lxc start "$NODE_NAME"
            ;;
        2)
            echo "Eliminando nodo existente..."
            lxc delete "$NODE_NAME" --force
            ;;
        *)
            echo "Saliendo..."
            exit 0
            ;;
    esac
fi

# Buscar una IP disponible
USED_IPS=$(lxc list --format csv -c 4 | grep -oE "192\.168\.2\.[0-9]+" | sort -u)
for i in $(seq 211 250); do
    NODE_IP="192.168.2.$i"
    if ! echo "$USED_IPS" | grep -q "$NODE_IP"; then
        break
    fi
done

echo "IP asignada: $NODE_IP"

# Clonar desde el nodo base si existe, si no crear nuevo
if lxc info ebpf-blockchain &>/dev/null; then
    echo "Clonando desde ebpf-blockchain..."
    lxc copy ebpf-blockchain "$NODE_NAME"
else
    echo "Creando nuevo contenedor..."
    lxc launch ubuntu:22.04 "$NODE_NAME" -p ebpf-blockchain
fi

# Esperar a que esté corriendo
echo "Esperando a que el nodo esté disponible..."
sleep 5

# Configurar red
echo "Configurando red..."
lxc exec "$NODE_NAME" -- bash -c "ip addr flush dev eth0 || true"
lxc exec "$NODE_NAME" -- bash -c "ip addr add $NODE_IP/24 dev eth0"
lxc exec "$NODE_NAME" -- bash -c "ip route add default via $GATEWAY"

# Configurar DNS
lxc exec "$NODE_NAME" -- bash -c "echo 'nameserver 8.8.8.8' > /etc/resolv.conf"

# Verificar conectividad
echo "Verificando conectividad..."
if lxc exec "$NODE_NAME" -- ping -c 2 8.8.8.8 &>/dev/null; then
    echo -e "${GREEN}Conectividad OK${NC}"
else
    echo -e "${RED}Error: Sin conectividad${NC}"
    exit 1
fi

# Agregar al Prometheus
echo ""
echo -e "${YELLOW}Para agregar este nodo a Prometheus, añade a prometheus.yml:${NC}"
echo "  - job_name: \"$NODE_NAME\""
echo "    static_configs:"
echo "      - targets: [\"$NODE_IP:9090\"]"

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Nodo $NODE_NAME creado exitosamente${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Comandos útiles:"
echo "  lxc exec $NODE_NAME bash"
echo "  lxc stop $NODE_NAME"
echo "  lxc start $NODE_NAME"
echo "  lxc delete $NODE_NAME"
echo ""
echo "IP: $NODE_IP"
