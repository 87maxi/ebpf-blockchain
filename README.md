# eBPF Blockchain Lab: P2P Resilience & Consensus

Este proyecto es un laboratorio de R+D enfocado en la creación de una red descentralizada de alto rendimiento que utiliza **eBPF (Extended Berkeley Packet Filter)** para seguridad y observabilidad nativa, y **libp2p** para una red P2P resiliente.

## 🚀 Pilares Arquitectónicos

*   **libp2p Networking**: Implementación robusta de P2P utilizando `Gossipsub 1.1` para propagación de transacciones y `Identify/Request-Response` para sincronización de historial.
*   **Consenso por Quórum**: Lógica de validación descentralizada que requiere una mayoría (2/3) de votos de los pares antes de confirmar una transacción en el ledger.
*   **eBPF Security & Observability**: 
    -   **XDP (eXpress Data Path)**: Filtrado de paquetes a nivel de kernel y blacklisting de IPs maliciosas a tasa de línea.
    -   **KProbes**: Medición de latencia interna del stack TCP/IP del kernel mediante histogramas de potencia de 2.
*   **Persistencia Aislada**: Almacenamiento persistente mediante **RocksDB** con rutas de datos únicas por contenedor para evitar colisiones de bloqueos en entornos compartidos.
*   **Observabilidad Full-Stack**: Integración nativa con Prometheus (métricas), Loki (logs JSON estructurados) y Grafana.

---

## 📋 Requisitos Previos

Antes de iniciar, asegúrate de tener instalado en el host:
1.  **LXD/LXC**: Para la orquestación de los nodos (contenedores).
2.  **Docker & Docker Compose**: Para la pila de monitoreo (Grafana/Loki/Prometheus).
3.  **Ansible**: Para la automatización del despliegue.
4.  **Rust (Nightly)**: El nodo requiere Rust nightly para la compilación de los programas eBPF (Aya).

```bash
# Instalación rápida de dependencias (Ubuntu/Debian)
sudo apt update && sudo apt install -y lxd docker-compose ansible
```

---

## 🛠️ Cómo Iniciar el Sistema

### 1. Despliegue Inicial del Clúster
Este comando crea la red de bridge, los perfiles de LXC y despliega 3 nodos iniciales.
```bash
cd ansible
ansible-playbook playbooks/deploy_cluster.yml -K
```

### 2. Reparación y Reinicio Rápido
Si realizas cambios en el código de Rust o necesitas limpiar bloqueos de base de datos:
```bash
ansible-playbook playbooks/repair_and_restart.yml
```

---

## 🔍 Simulación y Monitoreo

### Generación de Carga
Para simular transacciones y ver el consenso en tiempo real, utiliza la herramienta de simulación:
```bash
cd tools/ebpf-simulation
cargo run
```

### Visualización de Datos
Accede a los dashboards de monitoreo en tu navegador:
- **Grafana**: [http://localhost:3000](http://localhost:3000) (User/Pass: `admin`/`admin`)
- **Prometheus**: [http://localhost:9090](http://localhost:9090)
- **Loki Explorer**: Disponible dentro de Grafana para inspeccionar logs P2P.

---

## 🐞 Depuración y Análisis

-   **Logs de los Nodos**: Los logs de cada nodo se redirigen en el host a `/tmp/ebpf-node-X.log`.
-   **Inspección eBPF**: Puedes verificar los mapas de eBPF (latencia/blacklist) usando `bpftool` dentro de los contenedores:
    ```bash
    lxc exec ebpf-node-1 -- bpftool map dump name LATENCY_STATS
    ```
-   **Estado de RocksDB**: Los datos persistentes se encuentran en `/root/ebpf-blockchain/data/<hostname>`.

```bash
# Inspeccionar RocksDB
lxc exec ebpf-node-1 -- bash -ic "rocksdb-inspect scan"
```

---

## 📚 Documentación de Referencia

Durante el desarrollo se han creado documentos detallados sobre componentes específicos:

| Documento | Descripción |
| :--- | :--- |
| [AUDIT_ANALYSIS.md](./AUDIT_ANALYSIS.md) | Análisis profundo de fallas de consenso y seguridad. |
| [P2P_OBSERVABILITY_REPORT.md](./P2P_OBSERVABILITY_REPORT.md) | Reporte de métricas y propagación del protocolo. |
| [RESOLUCION_LAB.md](./RESOLUCION_LAB.md) | Guía de resolución de problemas de red y bridge. |
| [RPC_DOCUMENTATION.md](./RPC_DOCUMENTATION.md) | Detalle de la interfaz de comunicaciones del nodo. |

---

> [!IMPORTANT]
> El proyecto utiliza una configuración de quórum de 2 nodos. Si el clúster tiene menos de 2 nodos activos, las transacciones permanecerán en estado pendiente y no se confirmarán en la base de datos.
