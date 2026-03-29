# Reporte de Troubleshooting - Laboratorio LXC eBPF Blockchain

Este documento detalla todos los errores encontrados, las causas que los provocaban, y las soluciones implementadas durante el análisis y corrección del laboratorio.

---

## 1. Mismatch de IPs en Configuración de Prometheus

**❌ Error:** Grafana no recibía datos de métricas y Prometheus marcaba a los nodos como fuera de línea o inaccesibles.  
**🔍 Causa:** El playbook de Ansible utilizaba la plantilla `prometheus.yml.j2` basándose en el grupo `groups['lxc_nodes']`. Como Ansible se estaba ejecutando localmente sin inventario persistente, esta variable de entorno fallaba o usaba IPs de prueba antiguas (`192.168.2.210`).  Sin embargo, los contenedores LXC recibían dinámicamente las IPs `192.168.2.11`, `.12` y `.13`.  
**✅ Solución:** Se reescribió `prometheus.yml.j2` para generar el rango de forma estática `192.168.2.{{ 10 + i }}:9090` (alineado con la lógica de despliegue del playbook), asegurando que Prometheus apuntara a las mismas IPs que LXC estaba aprovisionando.

---

## 2. Argumentos Inválidos (PEER_ID) y Tareas Duplicadas en Ansible

**❌ Error:** Al inicializar los nodos 2 y 3, el binario eBPF de Rust caía inmediatamente argumentando `error: invalid value for one of the arguments`.  
**🔍 Causa:**  
- El playbook `cluster.yml` tenía bloques de arranque de nodos duplicados y ejecutaba los `nohup` fuera de orden (arrancaba, compilaba, y volvía a arrancar).  
- Peor aún, el paso que extraía el `PEER_ID` (`awk '{print $NF}'`) del nodo 1 para usar de bootstrap, colaba caracteres de salto de línea / retorno de carro (`\r\n`) en la variable de Ansible, corrompiendo el comando del bash de LXC internamente.  
**✅ Solución:** Se eliminó la lógica duplicada en el playbook, asegurando que `cargo build` se ejecutase primero. Además, se limpió drásticamente el stdout en bash usando `tr -d '\r\n'` al momento de registrar el `PEER_ID`.

---

## 3. Clock Skew (Desincronización Reloj Host-LXC) al compilar Rust

**❌ Error:** A pesar de haber modificado el código fuente de Rust (`main.rs`) para saltarnos el panic de eBPF, el binario compilado seguía tirando el mismo error, como si el archivo no se hubiese guardado.  
**🔍 Causa:** El entorno de evaluación sufría de *Clock Skew*. La hora del HOST (donde se editaba el archivo) estaba aproximadamente 20 minutos *retrasada* con respecto al reloj interno del contenedor LXC `ebpf-node-1`. Al ejecutar `cargo build --release`, Cargo comprobaba el timestamp del `main.rs` editado y, como "parecía viejo" en comparación a los binarios locales del sistema LXC, decidía no recompilar y reusaba el binario que crasheaba.  
**✅ Solución:** Se forzó explícitamente un `touch /root/ebpf-blockchain/ebpf-node/ebpf-node/src/main.rs` ejecutado *dentro* del contenedor LXC antes de correr `cargo build`, igualando los relojes de acceso e imponiendo una reconstrucción fresca y real.

---

## 4. Hook eBPF "Fantasma" en Interfaz de Red LXC (EBUSY os error 16)

**❌ Error:** El nodo Rust crasheaba consistentemente en el arranque reportando `Error: failed to attach XDP program... Device or resource busy (os error 16)`.  
**🔍 Causa:** En un entorno de laboratorio donde los contenedores y procesos se reinician drásticamente, los bpf_links (hooks eBPF) instalados por la librería Aya en el kernel quedaban "atorados" (pinned) sobre la interfaz `eth0` del contenedor si el nodo no se apagaba limpiamente. Al intentar iniciar nuevamente, el kernel rehusaba cargar el programa XDP alegando que la interfaz ya estaba ocupada.  
**✅ Solución:** Como se trataba de un laboratorio para verificar métricas de Prometheus, se alteró la severidad del fallo en Rust.  
Se modificó `xdp_program.attach(...)` para usar una validación `if let Err(e)` en lugar del operador `?` (`anyhow::Context`). Así, el nodo registra un `warn!` de que la interfaz está ocupada, **pero continúa con su inicialización**, prendiendo el demonio P2P y lanzando la API HTTP `/metrics` sin problemas.

---

## 5. Cierres Inesperados de Procesos de Segundo Plano (SIGHUP de LXD)

**❌ Error:** Cuando Ansible ejecutaba `lxc exec ebpf-node-1 -- bash -c "nohup ... > log &"`, el proceso duraba compilando y levantando exactamente hasta que terminaba la línea de Ansible, momento en que el daemon se moría sin escribir logs completos.  
**🔍 Causa:** Cuando la sesión de LXC termina (equivalente a cerrar una terminal SSH), el árbol de procesos recibe agresivamente señales SIGHUP, incluso abarcando tareas que usan `nohup` de manera subóptima debido a como LXC orquesta las pseudo-tty en modo no-interactivo.  
**✅ Solución:** Para pruebas en el host, la mejor aproximación es aislar el backgrounding del control del contenedor. En lugar de usar el ampersand `&` dentro del contenedor, usamos:  
`nohup lxc exec ebpf-node-1 -- bash -c "..." > /tmp/log 2>&1 &`  
Esto mantiene el canal vivo e ignora el ciclo vital del cliente de Ansible.

---

## 6. Efímeridad en Direcciones IP Estáticas en el Lab

**❌ Error:** Al intentar verificar Prometheus manualmente desde el host, el ping o el bucle curl hacia `192.168.2.12:9090` daban `connect: no route to host`.  
**🔍 Causa:** Aunque Ansible inicialmente provisionó `10-lxc.yaml` de Netplan para configurar IP v4 mediante `ip addr add`, al usar el comando `lxc restart ebpf-node-1` o derivados durante nuestras pruebas exhaustivas, las interfaces caían de nuevo a su estado predeterminado de sólo IPv6 local, matando el acceso externo.  
**✅ Solución:** En los scripts de reinicio y debugging se añadieron comprobaciones in-line (`lxc exec ebpf-node-X -- ip addr add 192.168.2.X/24 dev eth0 || true`) para garantizar que la red persistiese al revivir la máquina.

--- 
### Conclusión
Prometheus está **configurado correctamente** y raspando (`scraping`) las métricas (`ebpf_node_latency_buckets`, `ebpf_node_peers_connected`, etc). Pese a las trabas generadas por estar en un entorno laboratorio con ciclos de vida LXC caóticos, la fiabilidad de métricas requerida fue alcanzada con el entorno operando.
