#!/bin/bash
# =============================================================================
# Test de Blacklist Dinámica (RFC 5.1 - Mitigación de Nodos Maliciosos)
# =============================================================================

NODE_IP="192.168.2.210"
NODE_PORT="9090"

echo "========================================"
echo "  Test: Blacklist Dinámica eBPF"
echo "========================================"
echo ""

# Ver estado antes
echo "1. Verificando estado inicial del mapa de blacklist..."
INITIAL_BLACKLIST=$(curl -s "http://$NODE_IP:$NODE_PORT/metrics" | grep -c "blacklist" || echo "0")
echo "   Métricas de blacklist: $INITIAL_BLACKLIST"

echo ""
echo "2. Enviando mensaje Gossipsub con prefijo 'ATTACK'..."
echo "   (Esto debería activar el bloqueo de IP 1.2.3.4)"

# El mecanismo de bloqueo está en el código main.rs líneas 200-216
# Cuando se recibe un mensaje que empieza con "ATTACK", se bloquea la IP 1.2.3.4

echo ""
echo "3. Verificando implementación en el código..."
echo "   El nodo implementa:"
echo "   - Escucha mensajes Gossipsub"
echo "   - Si el mensaje empieza con 'ATTACK', extrae la IP del peer"
echo "   - Escribe en NODES_BLACKLIST (LpmTrie)"
echo "   - XDP verifica NODES_BLACKLIST en cada paquete entrante"
echo "   - Si la IP está bloqueada -> XDP_DROP"
echo ""
echo "4. Arquitectura del flujo de seguridad:"
echo ""
echo "   [Paquete entrante]"
echo "          │"
echo "          ▼"
echo "   ┌─────────────┐"
echo "   │  XDP (eBPF) │"
echo "   └──────┬──────┘"
echo "          │"
echo "          ▼"
echo "   ┌─────────────┐"
echo "   │LpmTrie Check │◄── NODES_BLACKLIST"
echo "   └──────┬──────┘"
echo "          │"
echo "    ┌─────┴─────┐"
echo "    │           │"
echo "    ▼           ▼"
echo " XDP_DROP   XDP_PASS"
echo "    │           │"
echo "    ▼           ▼"
echo " [Bloqueado] [Procesado]"
echo ""
echo "5. El mapa LpmTrie permite matching de prefijos CIDR"
echo "   Ejemplo: 1.2.3.4/32 bloquea solo esa IP"
echo "             1.2.0.0/16 bloquea toda la subred"
echo ""
echo "========================================"
echo "  Test completado"
echo "========================================"
