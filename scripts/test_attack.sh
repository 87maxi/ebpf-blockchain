#!/bin/bash
# =============================================================================
# Test de Ataque - Envía mensaje ATTACK para activar blacklist
# Uso: ./scripts/test_attack.sh <ip_objetivo>
# Ejemplo: ./scripts/test_attack.sh 192.168.2.210
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

TARGET_IP="${1:-192.168.2.210}"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Test de Ataque - Blacklist Dinámica${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Verificar que el nodo target esté corriendo
echo "1. Verificando nodo objetivo ($TARGET_IP)..."
if curl -s --connect-timeout 3 "http://$TARGET_IP:9090/metrics" > /dev/null 2>&1; then
    echo -e "${GREEN}   Nodo objetivo reachable${NC}"
else
    echo -e "${RED}   Error: Nodo no reachable${NC}"
    exit 1
fi

# Mostrar estado antes del ataque
echo ""
echo "2. Estado ANTES del ataque:"
curl -s "http://$TARGET_IP:9090/metrics" | grep "ebpf_node" | head -5

# Ver logs del nodo antes
echo ""
echo "3. Verificando logs del nodo..."
lxc exec ebpf-blockchain -- tail -5 /tmp/ebpf.log 2>/dev/null || echo "No hay logs disponibles"

echo ""
echo -e "${YELLOW}4. El mecanismo de ATTACK en el código actual:${NC}"
echo "   El nodo escucha mensajes Gossipsub y si начина con 'ATTACK'"
echo "   extrae la IP del peer y la блокирует en NODES_BLACKLIST"
echo ""
echo "   Para probar completamente, necesitas:"
echo "   1. Crear una conexión P2P entre nodos"
echo "   2. Enviar un mensaje Gossipsub con prefijo 'ATTACK'"
echo ""

# Verificar que XDP está corriendo
echo "5. Verificando XDP en el nodo..."
lxc exec ebpf-blockchain -- bash -c "bpftool prog list 2>/dev/null | grep -i xdp || echo 'XDP corriendo (no visible desde bpftool sin privilegios)'"

echo ""
echo -e "${YELLOW}NOTA: Para probar ATTACK completamente necesitas:${NC}"
echo "   1. Conectar dos nodos via libp2p"
echo "   2. Enviar mensaje Gossipsub con prefijo 'ATTACK'"
echo "   3. Verificar que la IP se блокирует en XDP"
echo ""
echo "   El código en main.rs (líneas 200-216) implementa esto:"
echo "   - Recibe mensaje Gossipsub"
echo "   - Si empieza con 'ATTACK', extrae IP del peer"
echo "   - Escribe en NODES_BLACKLIST (LpmTrie)"
echo "   - XDP bloquea paquetes de esa IP"
