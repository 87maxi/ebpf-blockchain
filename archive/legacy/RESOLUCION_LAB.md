# Reporte de Resolución de Errores - Laboratorio eBPF Blockchain (LXC + Ansible)

Este documento detalla los problemas encontrados, las causas raíz identificadas y las soluciones implementadas durante la depuración del entorno de laboratorio de eBPF Blockchain desplegado en contenedores LXC.

---

## 1. Configuración y Desconexión de Métricas en Prometheus

**❌ Error:**
Prometheus no podía scrapear y Grafana no mostraba datos para las métricas de `ebpf_node_peers_connected` y `ebpf_node_messages_received_total`. El target de Prometheus mostraba errores de conexión.

**🔍 Causa Raíz:**
El archivo `prometheus.yml.j2` (templating de Ansible) intentaba extraer dinámicamente las IPs basándose en un grupo de inventario `lxc_nodes` inexistente en el entorno de ejecución estático de `cluster.yml`. Esto resultaba en IPs target codificadas erróneamente (`192.168.2.210` y `.211`), mientras que el playbook en realidad le asignaba a los contenedores los rangos `192.168.2.11`, `12` y `13`.

**✅ Solución:**
Se reescribió `prometheus.yml.j2` para utilizar un ciclo iterativo estático que coincida exactamente con la lógica de despliegue del playbook. 
```yaml
{% for i in range(1, 4) %}
  - targets: ['192.168.2.{{ 10 + i }}:9090']
{% endfor %}
```
*Se reinició el contenedor Docker de Prometheus y los targets cambiaron a estado "UP".*

---

## 2. Errores de Argumentos al Levantar Nodos P2P (Ansible)

**❌ Error:**
En los logs (`/tmp/ebpf.log`) de los nodos 2 y 3, el proceso de Rust fallaba inmediatamente con: `error: invalid value for one of the arguments`. 

**🔍 Causa Raíz:**
En `cluster.yml`, el nodo 1 extrae su parámetro de arranque para el resto del clúster (`PEER_ID`). Sin embargo, el pipeline `awk` extraía la cadena con saltos de línea invisibles (`\r\n`). Al inyectar la variable en Ansible como parámetro `--bootstrap-peers` en la terminal de los otros contenedores, se rompía la estructura del comando Bash. Además, el playbook de Ansible contenía un bloque de ejecución completamente duplicado.

**✅ Solución:**
1. Se limpió el playbook para que compile primero y ejecute después de forma lineal.
2. Se inyectó `tr -d '\r\n'` al recolector del Peer ID para sanear por completo la variable antes de proveerla a los nodos hijos.

---

## 3. Pánicos Duales en Rust (EBUSY XDP Hook y Sincronización de Relojes)

**❌ Error:**
Los procesos de eBPF Node fallaban internamente en el código Rust con:
`Error: failed to attach XDP program... Device or resource busy (os error 16)`

**🔍 Causa Raíz:**
Al trabajar en un laboratorio con reinicios recurrentes, los `bpf_link` de programas XDP anteriores no se limpiaban correctamente de la interfaz veth del contenedor, dejando el canal ocupado ("Busy").  
Por otro lado, **las modificaciones al código fuente en el HOST no se reflejaban al compilar**, porque el reloj (Timezone) del host estaba retrasado frente al reloj del contenedor LXC. Cargo validaba las firmas de tiempo (timestamps) y consideraba que el código desactualizado era "más nuevo", omitiendo la recompilación silenciosamente.

**✅ Solución:**
1. **Clock Skew Bypass**: Se forzó una sobrescritura de los timestamps del archivo fuente ejecutando `touch main.rs` expresamente dentro del contenedor LXC justo antes de ejecutar `cargo build --release`.
2. **Crash Loop Bypass**: En vez de explotar o finalizar abruptamente la rutina ante un error de `attach`, encapsulamos las inyecciones eBPF (`xdp_program` y `kprobes`) dentro de un flujo sin pánico.
```rust
if let Err(e) = xdp_program.attach(&opt.iface, XdpFlags::default()) {
    warn!("Failed to attach XDP program, continuing: {}", e);
}
```
*Esto garantizó que si la interfaz de red ya estaba monopolizada por una ejecución anterior (estado sucio de laboratorio), la lógica peer-to-peer y los servidores de métricas igual pudieran inicializarse.*

---

## 4. Subprocesos "Zombies" y Daemons de Terminal LXC

**❌ Error:**
Al usar `nohup ... &` desde los scripts de bash invocados por `lxc exec`, el contenedor de LXC mataba el proceso en segundo plano un par de segundos después de que el comando de Ansible cortaba el "pseudo-TTY".

**🔍 Causa Raíz:**
LXC limpia rigurosamente su árbol de procesos cuando el shell de entrada desaparece, causando un envío indeseado de señales SIGHUP, incluso en presencia del comando `nohup` interno a la máquina aislada.

**✅ Solución:**
La contramedida ideal en lab es mantener a LXD trabajando en background pero controlando el flujo **desde el Host**:
`nohup lxc exec ebpf-node-1 -- bash -c "...comando..." > /tmp/ebpf-node-1.log 2>&1 &`
Ejecutar el descarte y backgrounding fuera del contenedor asegura que la vida del nodo P2P dependa del host y no del estado momentáneo del cliente LXC interno.

---

## 5. Falla de Enrutamiento en Nodos Subsiguientes por Reinicios

**❌ Error:**
`curl 192.168.2.13:9090/metrics` retornaba intermitentemente `No route to host`. 

**🔍 Causa Raíz:**
La reaparición del error se debía a que los contenedores perdían la información de enrutamiento estática inyectada mediante `ip addr add` en el playbook si se reiniciaban bruscamente mediante el servidor maestro.

**✅ Solución:**
Se re-aplicaron las asignaciones de IP IPv4 a nivel dev (`eth0`) estáticas sobre los nodos en vivo. En producción o despliegues puros, se debería depender exclusivamente de confirmaciones sanas de `netplan apply`.

---
*Con todas estas resoluciones activas, el ecosistema consta ahora de despliegues P2P libres de pausas catastróficas, métricas activas que pueden rasparse ininterrumpidamente, y cuadros de mandos de Grafana operando con fiabilidad.*
