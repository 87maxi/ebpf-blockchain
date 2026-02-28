# eBPF Blockchain Lab: Observabilidad y Seguridad Activa (RFC 001)

Este proyecto implementa un nodo de blockchain experimental que integra **eBPF (Extended Berkeley Packet Filter)** para la gestión de red en el kernel y **libp2p** para la comunicación P2P en el espacio de usuario. Basado en el [RFC 001](./rfc.md).

## 1. Arquitectura del Sistema

El sistema se divide en dos dominios principales que colaboran mediante mapas compartidos:

### Estructura de Componentes (UML)

```mermaid
graph TD
    subgraph "Espacio de Usuario (Rust + libp2p)"
        App[ebpf-node App]
        P2P[libp2p Swarm]
        Gossip[Gossipsub v1.1]
        Aya[Aya Manager]
    end

    subgraph "Espacio de Kernel (eBPF)"
        XDP[Programa XDP: Filtrado]
        Kprobes[Kprobes: Latencia]
        Maps[(eBPF Maps)]
    end

    App --> Aya
    Aya -- Carga --> XDP
    Aya -- Carga --> Kprobes
    P2P --> Gossip
    Gossip -- Peer Malicioso --> App
    App -- Actualiza Blacklist --> Maps
    XDP -- Consulta Blacklist --> Maps
    Maps -- Histograma de Latencia --> App
```

### Descripción de las Partes:
*   **ebpf-node (User Space):** Orquestador principal. Gestiona el ciclo de vida de los programas eBPF y la lógica de red descentralizada.
*   **ebpf-node-ebpf (Kernel Space):** Código Rust compilado a bytecode BPF.
    *   **XDP (eXpress Data Path):** Intercepta paquetes en la NIC. Si una IP está en la `NODES_BLACKLIST`, el paquete se descarta (`XDP_DROP`) antes de llegar al stack TCP/IP.
    *   **Kprobes:** Sondas en funciones del kernel (`netif_receive_skb`) para medir latencia con precisión de nanosegundos.
*   **ebpf-node-common:** Tipos de datos compartidos (estructuras de mapas y claves) para garantizar la coherencia entre kernel y usuario.

---

## 2. Requisitos y Dependencias

### Requisitos del Host:
*   Linux Kernel 5.10+ con BTF habilitado.
*   LXD/LXC instalado y configurado.

### Dependencias del Contenedor:
*   **Toolchain:** Rust (Nightly para eBPF), `bpf-linker`, `cargo-generate`.
*   **Librerías:** `clang`, `llvm`, `libelf-dev`, `libbpf-dev`.
*   **Crates clave:** `aya` (BPF manager), `libp2p` (Networking), `network-types` (Parsing de paquetes).

---

## 3. Preparación del Entorno (LXC)

Para estabilizar el ambiente de desarrollo, ejecute:

```bash
# Crear y configurar contenedor
lxc launch ubuntu:22.04 ebpf-blockchain --profile ebpf-blockchain
lxc config device add ebpf-blockchain project disk source=$(pwd) path=/root/ebpf-blockchain

# Instalar herramientas (dentro del contenedor)
lxc exec ebpf-blockchain -- bash -c "apt update && apt install -y build-essential clang llvm libelf-dev libbpf-dev"
lxc exec ebpf-blockchain -- bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && rustup toolchain install nightly && rustup component add rust-src --toolchain nightly"
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && cargo install bpf-linker"
```

---

## 4. Ejecución de Nodos

### Compilación:
```bash
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && cd /root/ebpf-blockchain/ebpf-node && cargo build"
```

### Nodo 1 (Bootstrap):
```bash
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && cd /root/ebpf-blockchain/ebpf-node && RUST_LOG=info ./target/debug/ebpf-node --iface eth0"
```

### Nodo 2 (Conexión):
Clone el contenedor y ejecute apuntando a la dirección del Nodo 1:
```bash
lxc copy ebpf-blockchain node-2
lxc start node-2
lxc exec node-2 -- bash -c "... ./target/debug/ebpf-node --iface eth0 --peer /ip4/[IP_NODO_1]/udp/4001/quic-v1/p2p/[PEER_ID]"
```

---

## 5. Pruebas de Funcionalidad

### Flujo de Secuencia de Pruebas:

```mermaid
sequenceDiagram
    participant N2 as Nodo 2 (Atacante)
    participant N1_K as Nodo 1 Kernel (eBPF)
    participant N1_U as Nodo 1 Usuario (libp2p)

    Note over N2, N1_U: Fase 1: Comunicación Normal
    N2->>N1_K: Envía Paquete Válido
    N1_K->>N1_U: XDP_PASS -> Procesa Transacción
    N1_U-->>N2: Responde OK

    Note over N2, N1_U: Fase 2: Ataque y Bloqueo
    N2->>N1_U: Envía mensaje "ATTACK: Spam"
    N1_U->>N1_U: Detecta comportamiento malicioso
    N1_U->>N1_K: Inserta IP de N2 en NODES_BLACKLIST (Map)
    
    Note over N2, N1_U: Fase 3: Resiliencia eBPF
    N2->>N1_K: Intenta nuevo paquete
    N1_K->>N1_K: Lookup en Blacklist -> MATCH
    N1_K--xN1_K: XDP_DROP (Paquete descartado en NIC)
    Note right of N1_K: El CPU de usuario no se entera
```

### Pasos para validar:
1.  **Validar Observabilidad:** Observe los logs cada 10 segundos. Verá el histograma de latencia:
    *   `Bucket 2^15: X packets` (indica tiempo de procesamiento en el stack).
2.  **Simular Ataque:** Envíe un mensaje de Gossipsub que inicie con la palabra `ATTACK`.
3.  **Verificar Bloqueo:**
    *   El Nodo 1 imprimirá: `IP 1.2.3.4 blocked successfully`.
    *   Use `bpftool map dump name NODES_BLACKLIST` para ver la IP bloqueada en el kernel.
    *   Cualquier paquete subsiguiente de esa IP será descartado por la tarjeta de red sin impactar el rendimiento de la aplicación.

---

## 6. Mantenimiento
Para inspeccionar el estado de los mapas de eBPF en tiempo real desde el host:
```bash
lxc exec ebpf-blockchain -- bpftool map show
lxc exec ebpf-blockchain -- bpftool prog show
```
