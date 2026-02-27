# RFC 001: Arquitectura de Blockchain con Observabilidad Nativa (eBPF) y Networking P2P (libp2p)

**Estado:** DRAFT (Borrador de Investigación)
**Versión:** 1.3.0
**Fecha:** 27 de febrero de 2026
**Autor:** Maximiliano Paredes
**Categoría:** Infraestructura de Sistemas Distribuidos / Seguridad de Red

---

## 1. INTRODUCCIÓN Y OBJETIVOS
Este documento detalla la arquitectura técnica de un nodo de blockchain experimental. El objetivo es resolver la opacidad y las ineficiencias de rendimiento en las redes P2P actuales mediante la integración de **libp2p** para la comunicación descentralizada y **eBPF (Extended Berkeley Packet Filter)** para la gestión y observación de datos a nivel de kernel.

El diseño transforma el nodo de una aplicación pasiva a un sistema activo capaz de protegerse, monitorearse y debuguearse a sí mismo desde el nivel más bajo del sistema operativo.

---

## 2. ESPECIFICACIÓN DEL STACK DE NETWORKING

### 2.1. Capa P2P (libp2p)
Se utiliza libp2p por su capacidad de abstracción de red y protocolos de consenso de malla.
* **Transporte:** Prioridad a **QUIC** sobre TCP para reducir la latencia de handshake y mejorar el NAT Traversal.
* **Propagación:** Protocolo **Gossipsub v1.1**. Se utiliza para la difusión de bloques, aplicando el "Peer Scoring" para mitigar ataques de spam y nodos de baja calidad.

---

## 3. CRIPTOGRAFÍA ROTATIVA (PERFECT FORWARD SECRECY)

### 3.1. Mecanismo de Rotación de Claves
Para garantizar la confidencialidad a largo plazo, el nodo implementa una rotación de claves efímeras basadas en curvas elípticas.
* **Algoritmo:** **Curve25519 (ECDH)**.
* **Lógica:** Cada sesión entre pares genera un secreto compartido mediante claves efímeras que se rotan cada $N$ bloques.

---

## 4. COMUNICACIÓN KERNEL-USUARIO: eBPF MAPS
El núcleo de la eficiencia reside en los **eBPF Maps**, que permiten que la aplicación (espacio de usuario) y el kernel compartan información con latencia cercana a cero.



### 4.1. Definición de Mapas Técnicos:
* **nodes_blacklist (LPM_TRIE):** Bloqueo inmediato de IPs en capa 3.
* **latency_stats (HISTOGRAM):** Registra tiempos de procesamiento (NIC -> App -> NIC) usando `uprobes`.
* **ratelimit_cfg (HASH):** Control de ancho de banda por Peer.

---

## 5. OBSERVABILIDAD Y RESILIENCIA

### 5.1. Sonda de Observabilidad con eBPF (Red)
Para una visibilidad total sin impacto en el rendimiento, se implementa una sonda eBPF basada en `TC` (Traffic Control) y `kprobes`.

#### Métricas Capturadas:
1.  **Latencia de Paquete (TC):** Tiempo transcurrido desde la recepción del paquete en el driver de red hasta que es procesado por el stack TCP/IP (`TC_INGRESS`).
2.  **Rendimiento de Gossipsub (kprobes):** Monitoreo de funciones específicas en libp2p para medir cuántos bytes se propagan por segundo.
3.  **Errores de Conexión (kprobes):** Conteo de `ECONNRESET` o `ETIMEDOUT` a nivel de kernel para diagnosticar particiones de red.

#### Justificación técnica:
El uso de `kprobes` permite rastrear funciones del kernel sin modificar el código fuente de libp2p o la aplicación blockchain, ideal para debuguear comportamiento de red en entornos de producción.



### 5.2. Manejo de Errores y Carga
* **Backpressure con XDP:** Si la carga de CPU excede el 80%, el kernel descarta paquetes de nodos con baja reputación basándose en los datos de la sonda.
* **Self-Healing:** En caso de desincronización de claves elípticas, el nodo dispara un protocolo de "Handshake de Emergencia".

---

## 6. SEGURIDAD Y VULNERABILIDADES (FUZZING)
* **Fuzzing de Protocolo:** Uso de **AFL++** sobre el parser de mensajes para evitar desbordamientos de búfer en la comunicación P2P.
* **Mitigación de Nodos Maliciosos:** Los nodos que envíen bloques inválidos son baneados dinámicamente mediante la actualización del mapa `nodes_blacklist` desde la aplicación hacia el programa XDP del kernel.

---

## 7. LIMITACIONES DEL SISTEMA
1. **Dependencia del Kernel:** Requiere Linux 5.10+ con soporte para BTF (BPF Type Format).
2. **Complejidad de Debugging:** Requiere herramientas como `bpftool` o `bpftrace` para inspeccionar el estado de la sonda en tiempo real.

---

## 8. CONCLUSIÓN
Este modelo de arquitectura redefine el nodo de blockchain como una entidad de alto rendimiento capaz de auto-gestionarse. La combinación de la sonda de observabilidad eBPF con libp2p ofrece un equilibrio óptimo entre descentralización, seguridad activa y visibilidad técnica profunda.
