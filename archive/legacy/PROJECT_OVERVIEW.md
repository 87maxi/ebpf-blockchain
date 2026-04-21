# 🚀 eBPF Blockchain: Un Laboratorio de Observabilidad y Redes P2P de Alto Rendimiento

Este repositorio es un entorno de experimentación avanzado que fusiona la potencia de **eBPF (Extended Berkeley Packet Filter)** con redes descentralizadas **P2P**, ofreciendo una visibilidad sin precedentes sobre el tráfico de red en arquitecturas blockchain.

---

## 🛠️ Stack Tecnológico

El proyecto está construido sobre un stack robusto y moderno:

*   **Lenguaje**: [Rust](https://www.rust-lang.org/) (Seguridad y rendimiento garantizados).
*   **eBPF Framework**: [Aya](https://aya-rs.dev/) (eBPF escrito íntegramente en Rust, sin dependencias de C).
*   **Networking P2P**: [libp2p](https://libp2p.io/) (Protocolos Gossipsub, mDNS, Identify y QUIC/TCP).
*   **Almacenamiento**: [RocksDB](https://rocksdb.org/) (Base de datos clave-valor de alto rendimiento para el estado local).
*   **API & Web**: [Axum](https://github.com/tokio-rs/axum) (Framework web asíncrono para JSON-RPC, WebSockets y métricas).
*   **Observabilidad**: [Prometheus](https://prometheus.io/) & [Grafana](https://grafana.com/).
*   **Infraestructura**: [Ansible](https://www.ansible.com/) & [LXC](https://linuxcontainers.org/) (Contenedores aislados para simulación de clúster).

---

## 🏗️ Arquitectura del Sistema

### 1. El Nodo eBPF (Kernel & User Space)
El nodo se divide en dos planos de ejecución:
*   **Kernel Space (Aya/eBPF)**: Implementa programas **XDP** para interceptar paquetes en la tarjeta de red antes de que lleguen al stack de red del kernel, y **Kprobes** para medir latencias de procesamiento (`netif_receive_skb` -> `napi_consume_skb`).
*   **User Space (Rust/Tokio)**: Orquestra la lógica P2P, expone los endpoints de control y recolecta los mapas eBPF para servirlos como métricas de Prometheus.

### 2. Consenso "Solana-lite" vía Gossip
Aunque no implementamos la complejidad de *Turbine*, el sistema utiliza un mecanismo de **Votación por Gossip** inspirado en Solana:
1.  **Ingesta**: Mediante **JSON-RPC**, se inyecta una transacción en cualquier nodo.
2.  **Propagación**: El nodo emite una `TxProposal` a través de **Gossipsub**.
3.  **Votación**: Los peers validan el paquete y emiten un `Vote`.
4.  **Confirmación**: Al alcanzar el umbral de votos, la transacción se marca como aprobada, se persiste en **RocksDB** y se notifica en tiempo real vía **WebSockets**.

---

## 📊 Observabilidad y Debugging Avanzado

La joya de la corona es el ecosistema de monitoreo, diseñado específicamente para debugging de redes complejas:

*   **Pistas de Paquetes Detalladas**: Métricas multidimensionales que trazan exactamente quién emite y quién recibe cada mensaje Gossip (`ebpf_node_gossip_packets_trace_total`).
*   **Histogramas de Latencia**: Visualización en tiempo real de cuánto tiempo tarda un paquete en cruzar el stack del kernel.
*   **Dashboard de Grafana Custom**: Un centro de mando que integra tablas de trace de red con gráficos de estado de salud del clúster (Peers, Uptime, Mensajes/seg).

---

## 🚀 Despliegue y Gestión del Laboratorio (IaC)

El entorno se gestiona como **Infraestructura como Código (IaC)** mediante Ansible:

*   **LXC Isolation**: Cada nodo corre en un contenedor LXC dedicado, permitiendo simular condiciones de red reales en un solo host.
*   **Ansible Automation**: 
    - `deploy_cluster.yml`: Levanta todo el entorno desde cero.
    - `rebuild_and_restart.yml`: Flujo de desarrollo rápido que recompila, reinicia y verifica la salud de los nodos en segundos.
*   **Docker Integration**: Los servicios de monitoreo (Prometheus/Grafana) corren en Docker, facilitando su orquestación y persistencia de datos.

---

## 📈 Casos de Uso

1.  **Investigación de Seguridad P2P**: Detección y bloqueo de ataques a nivel kernel (XDP Blacklisting).
2.  **Optimización de Latencia**: Pruebas de performance en protocolos de propagación de bloques.
3.  **Educación Blockchain**: Una base transparente para entender cómo fluye la información en una red descentralizada bajo el capó.

---

> **Nota**: Este proyecto es un entorno de laboratorio diseñado para facilitar la iteración rápida y el aprendizaje profundo sobre el stack eBPF/P2P.
