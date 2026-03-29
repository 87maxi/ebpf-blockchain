# Informe de Observabilidad P2P y Resiliencia del Clúster

## 1. Resumen Ejecutivo
Se ha estabilizado la capa de red del clúster eBPF Blockchain, permitiendo la formación de un mesh P2P robusto. Se implementó un protocolo de **Sincronización Histórica** que permite a los nodos recuperar transacciones perdidas durante desconexiones, garantizando la consistencia eventual de la base de datos RocksDB en toda la red.

## 2. Problemas Identificados y Soluciones

### A. Aislamiento de Red (Bridge Isolation)
*   **Contexto**: Los nodos LXC en la red `lxdbr1` no podían comunicarse entre sí (Packet Filtered en el gateway).
*   **Causa Raíz**: El módulo del kernel `br_netfilter` forzaba que el tráfico interno del puente pasara por el firewall del host (iptables/nftables), el cual lo bloqueaba por seguridad.
*   **Solución**: 
    1. Se aplicó una regla de aceptación explícita en el chain `FORWARD` de iptables para la interfaz `lxdbr1`.
    2. Se desactivó el filtrado de iptables para puentes mediante `sysctl`:
       ```bash
       sudo sysctl -w net.bridge.bridge-nf-call-iptables=0
       ```

### B. Falta de Sincronización Post-Desconexión
*   **Contexto**: Si un nodo caía, solo recibía transacciones nuevas por Gossip al volver, perdiendo el historial intermedio.
*   **Solución**: Se implementó el protocolo **libp2p Request-Response** con serialización CBOR.
    *   **Handshake**: Ahora los nodos usan `Identify` para detectarse y disparar un `SyncRequest`.
    *   **Reconciliación**: El nodo par escanea su **RocksDB** y envía el histórico completo al nodo solicitante.
    *   **Resultado**: En las pruebas, Node 3 recuperó **201 transacciones** instantáneamente tras reconectarse.

## 3. Estado de la Infraestructura
*   **Nodos**: 3 nodos operativos (LXC).
*   **DB**: RocksDB persistente con sincronización idempotente.
*   **Observabilidad**: 
    *   **Loki**: Captura eventos estructurados de sincronización (`sync_request_sent`, `sync_response_received`).
    *   **Grafana**: Dashboard actualizado con la fila "Transaction Ledger & Consensus" para monitoreo en tiempo real.

## 4. Verificación de Funcionamiento
Se realizaron pruebas de "Carga Offline":
1. Node 3 fuera de línea.
2. Inyección de ráfaga de transacciones (h1, h2, ..., h200) en Node 1.
3. Reinicio de Node 3.
4. **Validación**: Los logs confirman la recepción de `SyncResponse` con el conteo exacto de bloques faltantes y su inserción exitosa en la DB local.

---
**Reporte generado por Antigravity (Advanced Agentic Coding - Google Deepmind)**
