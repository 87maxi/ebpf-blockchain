# Auditoría y Crítica Profunda: eBPF Blockchain

Este documento detalla los puntos críticos, fallos de diseño y áreas de mejora identificadas tras un análisis exhaustivo del código actual.

## 1. Consenso: El Eslabón Más Débil (Crítico)
**Problema**: Actualmente, una sola "Votación" (`Vote`) recibida por Gossip marca una transacción como "Aprobada" en RocksDB. 
- **Riesgo**: Un solo nodo malicioso (Sybil Attack) puede inundar la red con votos falsos y forzar la aprobación de transacciones inválidas en todos los nodos.
- **Implementación Incorrecta**: No hay concepto de **Quórum** (N/2 + 1) ni de **Altura de Bloque**.
- **Solución Recomendada**: 
    - Implementar un sistema de quórum simple. Una transacción solo se marca como `Approved` si recibe votos de al menos el 66% de los pares conocidos.
    - Introducir una estructura de **Bloque** o al menos un **SeqNum** para evitar ataques de replay.

## 2. Persistencia Volátil (Alto)
**Problema**: La base de datos RocksDB se inicializa en `/tmp/rocksdb_{pid}`.
- **Riesgo**: 
    1. Si el nodo se reinicia, el PID cambia y los datos anteriores se ignoran (nueva DB).
    2. `/tmp` es limpiado por el sistema operativo, lo que puede borrar la "cadena" arbitrariamente.
- **Implementación Incorrecta**: Los datos de un blockchain deben ser persistentes y estar en una ruta estándar (ej. `/var/lib/ebpf-blockchain/`).
- **Solución Recomendada**: Estabilizar la ruta de la DB en el `Opt` de clap y persistir los votos por transacción en lugar de sobreescribir el valor.

## 3. Seguridad eBPF: Blacklist vs Whitelist (Alto)
**Problema**: El programa XDP usa un `NODES_BLACKLIST` que solo se activa tras detectar una cadena de texto "ATTACK" en el userspace.
- **Riesgo**: El atacante ya consumió recursos de CPU de la pila de red y de la aplicación antes de ser bloqueado. Es una defensa reactiva y débil.
- **Implementación Incorrecta**: eBPF debería ser una barrera proactiva.
- **Solución Recomendada**: Implementar un **Whitelist** en XDP. Solo permitir paquetes de IPs que hayan completado exitosamente el handshake de `libp2p`. El resto del tráfico debe ser descartado en el kernel (`XDP_DROP`) a tasa de línea.

## 4. Fugas de Memoria en el Kernel (Medio)
**Problema**: El mapa `START_TIMES` usa un `HashMap` estándar de eBPF.
- **Riesgo**: Si un paquete entra al stack (`netif_receive_skb`) pero nunca llega a `napi_consume_skb` (por un drop intermedio), el valor queda en el mapa para siempre. Con tráfico real, el mapa de 10k entradas se llenará y el sistema dejará de medir latencia.
- **Solución Recomendada**: Cambiar `HashMap` por `LruHashMap` para que el kernel limpie automáticamente las entradas más viejas.

## 5. Protocolo de Sincronización Ineficiente (Medio)
**Problema**: El `SyncRequest` pide *toda* la base de datos cada vez que se detecta un nuevo par.
- **Riesgo**: No escalará. Si la DB tiene 1GB de transacciones, el tráfico de sincronización saturará el nodo.
- **Solución Recomendada**: Implementar sincronización basada en "Deltas". El nodo pide transacciones desde el último ID que conoce.

## 6. Automatización y Despliegue (Bajo)
**Problema**: El playbook de Ansible requiere correcciones manuales de `iptables` en el host.
- **Mejora**: El entorno de laboratorio debería ser totalmente autocontenido. El uso de `lxdbr1` con NAT es funcional pero frágil si no se configuran correctamente las reglas de forwarding en el host de manera persistente.

---
### Propuesta de Acción Inmediata:
1.  **Refactor de RocksDB**: Migración a ruta persistente.
2.  **Consenso por Quórum**: Implementar lógica de conteo de votos antes de confirmar en DB.
3.  **XDP LruMap**: Evitar la saturación del mapa de latencia.
