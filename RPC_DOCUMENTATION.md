# Documentación de API JSON-RPC y WebSockets (eBPF Blockchain)

El nodo eBPF ahora incluye soporte para ingesta de transacciones vía RPC y un flujo de eventos en tiempo real mediante WebSockets, permitiendo interacciones programáticas directas con la red Gossipsub y la base de datos RocksDB.

Ambos servicios se exponen en el mismo puerto que las métricas de Prometheus (`9090`).

---

## 1. JSON-RPC (HTTP POST `/rpc`)

Utiliza este endpoint para inyectar transacciones en la red P2P. Cuando un nodo recibe este payload, lo encapsula en un objeto `TxProposal` y lo transmite (Gossip) al resto de los peers.

### Endpoint
`POST http://<IP_DEL_NODO>:9090/rpc`

### Formato del Payload (JSON)
El cuerpo de la petición debe ser un objeto JSON plano que represente la transacción:
```json
{
  "id": "tx_abc123",
  "data": "transfer:100"
}
```

### Ejemplo con `cURL`
Para interactuar con el Nodo 1 (`192.168.2.11`):
```bash
curl -X POST http://192.168.2.11:9090/rpc \
     -H "Content-Type: application/json" \
     -d '{"id": "test_1", "data": "Hello P2P Network"}'
```
**Respuesta exitosa:** Código HTTP `202 ACCEPTED` (cuerpo vacío).

---

## 2. WebSockets de Consenso en Tiempo Real (HTTP GET `/ws`)

Al tratarse de una validación descentralizada, usar simplemente `/rpc` no garantiza que el paquete persista; solo lo transmite. Para saber si la transacción fue **aprobada y guardada en RocksDB**, debes suscribirte al WebSocket.

### Endpoint
`ws://<IP_DEL_NODO>:9090/ws`

### Flujo de Eventos
Cuando cualquier nodo aprueba una transacción por haber recibido un voto de la red Gossip, escribe el dato de la transacción en su instancia local de RocksDB y despacha un evento JSON a todos los clientes Websocket conectados.

### Formato de Salida
Los mensajes transmitidos por el WebSocket tienen el siguiente formato:
```json
{
  "event": "BlockApproved",
  "tx_id": "test_1",
  "voter": "12D3KooWNodoEjemploId..."
}
```

### Ejemplo usando `wscat`
(Si no tienes wscat, instálalo vía `npm install -g wscat`)

1. Conéctate a cualquier nodo (ej. Nodo 2 o Nodo 3):
```bash
wscat -c ws://192.168.2.12:9090/ws
```
2. Mantenlo abierto.
3. Envía una transacción por `/rpc` a *otro* nodo. 
4. Inmediatamente verás aparecer el evento JSON `BlockApproved` en la terminal del wscat certificando el Consenso Gossip y almacenamiento del paquete.
