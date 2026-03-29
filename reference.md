# Guía de Referencia: Debugging y Pruebas del Laboratorio

Este documento detalla los procedimientos para validar de manera minuciosa el funcionamiento del sistema, desde la inyección de transacciones hasta el baneo por eBPF.

---

## 1. Depuración de Transacciones (JSON-RPC)

Para inyectar una transacción y verificar su flujo:

1.  **Ejecutar Simulación**:
    ```bash
    cd tools/ebpf-simulation
    cargo run
    ```
2.  **Verificar RPC**:
    Cada transacción debe devolver un HTTP 202 (Accepted). Puedes usar `curl` manualmente:
    ```bash
    curl -X POST http://<LXC_IP>:9090/rpc -H "Content-Type: application/json" -d '{"id":"tx-123","data":"hello"}'
    ```
3.  **Rastrear en Logs (Loki)**:
    En Grafana -> Explore -> Loki, usa el filtro:
    `{job="ebpf-nodes"} |= "TxProposal"`
    Deberías ver el dump JSON con el ID `tx-123`.

---

## 2. Comprobación de Consenso (Gossip + Vote)

El consenso se basa en que los nodos al recibir un `TxProposal`, emiten un `Vote`.

1.  **Observación en Tiempo Real**:
    En Grafana, el panel **"Gossip MPS"** debe mostrar picos de actividad al enviar transacciones.
2.  **Validación de Votos (Loki)**:
    Filtra por: `{job="ebpf-nodes"} |= "Vote"`
    Verás quién votó por qué transacción y en qué timestamp.

---

## 3. Almacenamiento en Base de Datos (RocksDB)

Cada nodo almacena las transacciones aprobadas en su propia instancia de RocksDB.

1.  **Localizar la DB**:
    Dentro del contenedor LXC, la base de datos se crea con el PID actual: `/tmp/rocksdb_<PID>`.
2.  **Verificar persistencia**:
    Entra a un nodo esclavo (ej. `ebpf-node-2`):
    ```bash
    lxc exec ebpf-node-2 -- ls -l /tmp/
    ```
    Busca carpetas que empiecen con `rocksdb_`.
3.  **Dump de valores (Opcional)**:
    Si la herramienta `ldb` (RocksDB CLI) está instalada:
    ```bash
    lxc exec ebpf-node-2 -- ldb --db=/tmp/rocksdb_<PID> scan
    ```

---

## 4. Debugging eBPF con Aya y bpftool

Para verificar que el kernel está tomando las acciones correctas:

1.  **Listar Mapas Activos**:
    ```bash
    lxc exec ebpf-node-1 -- bpftool map show
    ```
2.  **Inspeccionar la Blacklist**:
    Si quieres ver qué IPs han sido baneadas dinámicamente:
    ```bash
    lxc exec ebpf-node-1 -- bpftool map dump name NODES_BLACKLIST
    ```
3.  **Verificar Estadísticas de Latencia**:
    ```bash
    lxc exec ebpf-node-1 -- bpftool map dump name LATENCY_STATS
    ```
    (Nota: Verás los buckets de 1 a 64 ms).

---

## 5. Escenarios de Prueba de Ataque (Paso a Paso)

Para probar el sistema de baneo por payload malicioso:

1.  **Simular Ataque**:
    Envía un mensaje que empiece con la palabra "ATTACK" vía Gossip o simulando un paquete crudo.
2.  **Observar Reacción**:
    El log del nodo mostrará: `Malicious message detected from peer... Blocking IP.`
3.  **Comprobar Kernel**:
    Ejecuta el comando del punto 4.2 para verificar que la IP del atacante está en el `LPM_TRIE`.
4.  **Verificar Drop**:
    Cualquier paquete subsiguiente de esa IP será descartado por el programa XDP antes de llegar a `libp2p`.

---

## 6. Ciclo de Desarrollo con Aya

Si modificas el código eBPF en `ebpf-node-ebpf/src/main.rs`:

1.  **Modificar**: Realiza tus cambios en el programa kernel.
2.  **Sincronizar y Reconstruir**:
    ```bash
    ansible-playbook ansible/playbooks/rebuild_and_restart.yml
    ```
3.  **Logs de Kernel**:
    Usa `aya-log` para ver lo que pasa en el kernel:
    Los logs del kernel aparecerán en el flujo de logs estándar del nodo si `EbpfLogger::init` está activo.
