# Instructivo de Desarrollo: Blockchain Lab (RFC 001)

## 1. Visión General de la Arquitectura
El proyecto se basa en la interacción entre el espacio de usuario (donde vive la lógica blockchain y libp2p) y el espacio de kernel (donde eBPF filtra paquetes a nivel de tarjeta de red).

---

## 2. Fase de Configuración del Entorno (Lab)

Antes de programar, debemos preparar el entorno de virtualización en Manjaro para que LXC tenga acceso directo al hardware.

### Tareas:
1.  **Instalar LXC/LXD:** Utilizar `pacman` para instalar los paquetes necesarios.
2.  **Configurar Red Física (Passthrough):**
    * Editar el perfil LXC para usar `nictype: physical`.
    * Identificar el nombre de la interfaz física del host (ej: `enp3s0`).
3.  **Montar Sistema BPF:** Asegurar que `/sys/fs/bpf` sea accesible dentro del contenedor para compartir mapas.

---

## 3. Fase de Implementación del Kernel (eBPF - Rust/Aya)

En esta fase desarrollaremos el programa BPF que se ejecutará en la tarjeta de red.



### Tareas:
1.  **Estructura del Mapa de Blacklist (Kernel Side):**
    * Implementar `BPF_MAP_TYPE_LPM_TRIE` en Rust para almacenar las IPs a bloquear.
    * *Justificación:* El uso de un Trie permite búsquedas de subredes extremadamente rápidas en el "fast path" del kernel.
2.  **Programa XDP (eBPF):**
    * Escribir la función `xdp_filter`.
    * **Lógica:**
        1.  Parsear el paquete entrante para extraer la IP de origen (IPv4).
        2.  Buscar la IP en el mapa `nodes_blacklist`.
        3.  Si la IP está en el mapa, ejecutar `XDP_DROP`.
        4.  Si no, ejecutar `XDP_PASS`.
3.  **Sonda de Latencia (Observabilidad):**
    * Implementar `BPF_MAP_TYPE_HISTOGRAM`.
    * Usar `kprobes` en la función del kernel `netif_receive_skb` para marcar el tiempo de entrada del paquete.
    * *Justificación:* Esto permite medir la latencia real de la red sin impactar el rendimiento de la aplicación.

---

## 4. Fase de Implementación del Nodo (User Space - Rust/libp2p)

Esta es la aplicación principal que gestiona la red y controla el programa eBPF.



### Tareas:
1.  **Carga del Programa BPF:**
    * Usar la biblioteca `aya` para cargar el archivo `.bpf.o` y adjuntarlo a la interfaz de red (`eth0`) del contenedor.
2.  **Implementación de libp2p:**
    * Configurar `libp2p` con transporte QUIC para alta eficiencia.
    * Configurar `Gossipsub` para la difusión de bloques.
3.  **Interacción App -> Kernel (Mapa):**
    * Implementar la lógica para que, cuando `libp2p` detecte un par malicioso (ej: firma inválida repetidamente), obtenga su IP y la escriba en el mapa `nodes_blacklist` usando `aya::maps::HashMap`.
4.  **Recolección de Métricas:**
    * Leer el mapa `latency_stats` desde la aplicación para mostrar estadísticas de latencia en la consola.

---

## 5. Fase de Resiliencia y Criptografía

### Tareas:
1.  **Perfect Forward Secrecy (Curve25519):**
    * Implementar la rotación de claves efímeras para cada conexión P2P.
    * Implementar la función de *zeroing* de memoria para sobrescribir claves antiguas.
2.  **Manejo de Carga (Backpressure):**
    * Si la aplicación detecta alto uso de CPU, enviar una señal al programa eBPF para aumentar la agresividad del filtrado de paquetes de peers con baja reputación.

---

## 6. Fase de Pruebas y Validación (Lab)

### Tareas:
1.  **Validar XDP:**
    * Desde otra máquina, hacer `ping` al contenedor.
    * Añadir la IP de la máquina de pruebas a la blacklist desde la App Rust.
    * Verificar que el `ping` se detiene y la CPU del contenedor no aumenta (confirmando `XDP_DROP`).
2.  **Validar Observabilidad:**
    * Usar `bpftool map show` para visualizar el contenido del histograma de latencia en tiempo real.
