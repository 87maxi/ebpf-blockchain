# Instalación de Nodos eBPF con LXC - Reporte de Problemas

## Resumen Ejecutivo

Se identificaron múltiples problemas durante la instalación de nodos eBPF en contenedores LXC. A continuación se documenta el proceso, los problemas encontrados y las soluciones aplicadas.

---

## 1. Problemas Identificados

### 1.1 Cloud-Init: Permisos de Archivos (Schema Validation)

**Problema**: Cloud-init rechazaba archivos con permisos numéricos.

```
Error: cloud-config failed schema validation!
write_files.0.permissions: 493 is not of type 'string'
```

**Causa**: En cloud-config, el campo `permissions` debe ser string, no número octal.

**Solución**: Cambiar `permissions: 0755` a `permissions: "0755"` en el YAML.

```yaml
write_files:
  - path: /opt/install-deps.sh
    permissions: "0755"  # String, no número
```

---

### 1.2 Cloud-Init: Ejecución Prematura de runcmd

**Problema**: Los comandos en `runcmd` se ejecutaban antes de que la red estuviera disponible.

**Síntoma**: Bucle infinito de `ping google.com` fallando.

```
ping: google.com: Temporary failure in name resolution
```

**Causa**: En contenedores LXC, `runcmd` se ejecuta durante la fase `init` de cloud-init, antes de que la red esté completamente configurada.

**Intento de Solución**: Se creó un servicio systemd que espera la red:

```yaml
write_files:
  - path: /etc/systemd/system/install-deps.service
    permissions: "0644"
    content: |
      [Unit]
      Description=eBPF Blockchain Dependencies Installation
      After=network-online.target
      Wants=network-online.target
      ConditionPathExists=!/opt/.deps-installed

      [Service]
      Type=oneshot
      ExecStart=/opt/install-deps.sh
      RemainAfterExit=yes

      [Install]
      WantedBy=multi-user.target

runcmd:
  - systemctl daemon-reload
  - systemctl enable install-deps.service
  - systemctl start install-deps.service
```

**Resultado**: El servicio se creaba pero no se ejecutaba debido a cloud-init atascado.

---

### 1.3 Cloud-Init: Fase modules-config Atascada

**Problema**: Cloud-init se quedaba permanentemente en estado `modules-config`.

**Síntoma**:
```bash
$ cloud-init status
status: running
```

**Estado JSON**:
```json
{
    "v1": {
        "modules-config": {
            "errors": [],
            "finished": null,
            "start": null
        }
    }
}
```

**Causa**: Sin conectividad de red, los módulos de cloud-init (packages, runcmd) no progresaban.

---

### 1.4 Red LXC: DHCP No Funciona en lxdbr0

**Problema**: Los contenedores LXC en el bridge `lxdbr0` no obtenían direcciones IPv4.

**Síntoma**:
```bash
$ lxc exec ebpf-blockchain -- ip addr show eth0
inet6 fd42:af1e:292c:e285:216:3eff:fe4e:8d44/64 scope global mngtmpaddr
# Sin inet IPv4
```

**Verificación**:
```bash
# dnsmasq está corriendo
$ ps aux | grep dnsmasq | grep lxdbr0
nobody    dnsmasq --dhcp-range 10.137.31.100-10.137.31.200 ...

# Pero no hay leases
$ cat /var/lib/lxd/networks/lxdbr0/dnsmasq.leases
# Archivo vacío o no existe

# DHCP no recibe respuestas
$ lxc exec ebpf-blockchain -- dhclient -v eth0
DHCPDISCOVER on eth0 to 255.255.255.255 port 67 interval 3
# Sin respuesta del servidor
```

**Causa**: El servidor DHCP (dnsmasq) no recibe las solicitudes DHCP de los contenedores.

**Intento de Solución**: Forzar IP estática:
```bash
lxc exec ebpf-blockchain -- ip addr add 10.137.31.100/24 dev eth0
lxc exec ebpf-blockchain -- ip route add default via 10.137.31.1
```

**Resultado**: El contenedor tiene IP pero sin conectividad (bridge isolado).

---

### 1.5 Macvlan: Aislamiento de Red

**Problema**: Al usar macvlan, el host no puede comunicarse con los contenedores.

**Síntoma**:
```bash
$ ping 192.168.0.84
Origen 192.168.0.100 icmp_seq=1 Huésped Destino No Alcanzable
```

**Causa**: Macvlan crea interfaces aisladas. El tráfico desde el host no puede alcanzar los contenedores macvlan porque macvlan opera en capa 2 aislada.

---

### 1.6 Prometheus: Sin Ruta al Contenedor LXC

**Problema**: Prometheus (en Docker) no puede hacer scrape de métricas del contenedor LXC.

**Error**:
```
Get "http://192.168.0.84:9090/metrics": dial tcp 192.168.0.84:9090: connect: no route to host
```

**Causa**: 
1. Docker usa su propia subred (172.17.0.0/16)
2. El contenedor LXC está en una subred diferente
3. Docker con `network_mode: host` no resuelve el problema de routing entre subredes

---

## 2. Configuraciones Probadas

### 2.1 Configuración con lxdbr0 (Bridge LXD)

```yaml
# ebpf-blockchain.yaml
name: ebpf-blockchain
config:
  limits.cpu: "4"
  limits.memory: 8GiB
  security.privileged: "true"
  security.syscalls.intercept.bpf: "true"
  security.syscalls.intercept.bpf.devices: "true"
  cloud-init.user-data: |
    #cloud-config
    package_update: true
    packages:
      - build-essential
      - clang
      - llvm
      - libelf-dev
      - libbpf-dev
      - curl
      - git
      - iputils-ping

    runcmd:
      - [ bash, -c, "..." ]

devices:
  eth0:
    name: eth0
    network: lxdbr0
    type: nic
```

**Resultado**: Sin DHCP IPv4 funcional.

---

### 2.2 Configuración con Macvlan

```bash
lxc profile set ebpf-blockchain devices.eth0.nictype=macvlan
lxc profile set ebpf-blockchain devices.eth0.parent=enp5s0
```

**Resultado**: Contenedor obtiene IP de DHCP del router, pero host no puede comunicarse.

---

### 2.3 Configuración con Red Física (Physical)

```yaml
devices:
  eth0:
    name: eth0
    nictype: physical
    parent: enp6s0
    type: nic
```

**Error**: 
```
Device validation failed for "eth0": Failed loading device "eth0": 
Unsupported device type
```

**Causa**: `physical` no es un nictype válido en LXD.

---

## 3. Solución Funcional Parcial

### 3.1 Instalación Manual (Funcionó)

Dado que cloud-init no funcionaba correctamente, se instalaron las dependencias manualmente:

```bash
# 1. Paquetes del sistema
apt update && apt install -y build-essential clang llvm libelf-dev libbpf-dev curl git

# 2. Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# 3. Toolchain nightly
/root/.cargo/bin/rustup toolchain install nightly --component rust-src

# 4. Herramientas eBPF
/root/.cargo/bin/cargo install bpf-linker cargo-watch
```

### 3.2 Compilación del Proyecto

```bash
source /root/.cargo/env
cd /root/ebpf-blockchain/ebpf-node
cargo build
```

**Resultado**: Compilación exitosa en ~2 minutos.

### 3.3 Ejecución del Nodo

```bash
source /root/.cargo/env
RUST_LOG=info ./target/debug/ebpf-node --iface eth0
```

**Salida**:
```
[WARN  ebpf_node] failed to initialize eBPF logger: AYA_LOGS not found
[INFO  libp2p_swarm] local_peer_id=12D3KooWGVPbU6Aqpj7YgqhZUAoi485GSnzS8vxtVz7ZSu71b1Qn
[INFO  ebpf_node] Prometheus metrics server listening on 0.0.0.0:9090/metrics
--- Latency Histogram (nanoseconds, power of 2 buckets) ---
Bucket 2^24: 1 packets
Bucket 2^31: 2 packets
Bucket 2^32: 3 packets
```

### 3.4 Métricas Generadas

```bash
$ curl -s http://localhost:9090/metrics
# HELP ebpf_node_latency_buckets Current values of latency buckets
# TYPE ebpf_node_latency_buckets gauge
ebpf_node_latency_buckets{bucket="24"} 1
ebpf_node_latency_buckets{bucket="31"} 2
ebpf_node_latency_buckets{bucket="32"} 3
```

**El nodo eBPF está interceptando paquetes y generando métricas de latencia correctamente.**

---

## 4. Solución Implementada

### 4.1 Configuración de Red Exclusiva para LXC (Completado)

Se configuró `enp6s0` de forma exclusiva para LXD creando un bridge dedicado:

```bash
# Crear red LXD con bridge dedicado
sudo ip link delete lxdbr1 2>/dev/null
sudo lxc network create lxdbr1 --type bridge
sudo lxc network set lxdbr1 ipv4.address=192.168.2.200/24
sudo lxc network set lxdbr1 ipv4.dhcp.ranges=192.168.2.210-192.168.2.250

# Mover enp6s0 al bridge
sudo ip link set enp6s0 master lxdbr1
```

**Importante**: En SUSE, se requieren reglas de firewall adicionales:
```bash
sudo iptables -I FORWARD -i lxdbr1 -j ACCEPT
sudo iptables -I FORWARD -o lxdbr1 -j ACCEPT
```

### 4.2 Prometheus Haciendo Scrape (Completado)

El servidor Prometheus ahora puede alcanzar las métricas del contenedor LXC:

**Configuración prometheus.yml:**
```yaml
- job_name: "ebpf_node_1"
  static_configs:
    - targets: ["192.168.2.210:9090"]
```

**Verificación:**
```bash
$ curl http://localhost:9090/api/v1/targets | grep -A5 "ebpf_node"
"lastError": "",
"lastScrape": "2026-03-25T00:20:08.121779806Z",
```

### 4.3 Problema de DNS en Contenedor

**Síntoma**: apt update falla con `Temporary failure resolving archive.ubuntu.com`

**Solución**: Configurar DNS manualmente:
```bash
echo 'nameserver 8.8.8.8' > /etc/resolv.conf
```

Esto es necesario porque systemd-resolved no funciona correctamente en el contenedor.

---

## 5. Resumen de Solución Completa

### 5.1 Configuración del Host (SUSE)

```bash
# Crear bridge dedicado para LXC
sudo lxc network create lxdbr1 --type bridge
sudo lxc network set lxdbr1 ipv4.address=192.168.2.200/24
sudo lxc network set lxdbr1 ipv4.dhcp.ranges=192.168.2.210-192.168.2.250
sudo lxc network set lxdbr1 ipv4.nat=true

# Permitir tráfico en el bridge (SUSE)
sudo iptables -I FORWARD -i lxdbr1 -j ACCEPT
sudo iptables -I FORWARD -o lxdbr1 -j ACCEPT
```

### 5.2 Creación del Contenedor

```bash
# Profile LXC
lxc profile create ebpf-blockchain
cat << 'EOF' | lxc profile edit ebpf-blockchain
name: ebpf-blockchain
config:
  limits.cpu: "4"
  limits.memory: 8GiB
  security.privileged: "true"
  security.syscalls.intercept.bpf: "true"
  security.syscalls.intercept.bpf.devices: "true"
devices:
  root:
    path: /
    pool: default
    type: disk
  eth0:
    name: eth0
    network: lxdbr1
    type: nic
EOF

# Crear contenedor
lxc launch ubuntu:22.04 ebpf-blockchain -p ebpf-blockchain
```

### 5.3 Configuración de Red del Contenedor

```bash
# Configurar IP estática (el DHCP puede no funcionar inmediatamente)
lxc exec ebpf-blockchain -- bash -c "ip addr add 192.168.2.210/24 dev eth0"
lxc exec ebpf-blockchain -- bash -c "ip route add default via 192.168.2.200"

# Configurar DNS
lxc exec ebpf-blockchain -- bash -c "echo 'nameserver 8.8.8.8' > /etc/resolv.conf"
```

### 5.4 Instalación de Dependencias

```bash
lxc exec ebpf-blockchain -- bash -c "apt update && apt install -y build-essential clang llvm libelf-dev libbpf-dev curl git"

# Instalar Rust
lxc exec ebpf-blockchain -- bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"

# Instalar nightly y rust-src
lxc exec ebpf-blockchain -- bash -c "/root/.cargo/bin/rustup toolchain install nightly --component rust-src"

# Instalar herramientas eBPF
lxc exec ebpf-blockchain -- bash -c "/root/.cargo/bin/cargo install bpf-linker cargo-watch"
```

### 5.5 Montar Proyecto y Compilar

```bash
# Montar directorio del proyecto
lxc config device add ebpf-blockchain workspace disk source=/home/maxi/Documentos/source/ebpf-blockchain path=/root/ebpf-blockchain

# Compilar
lxc exec ebpf-blockchain -- bash -c "source /root/.cargo/env && cd /root/ebpf-blockchain/ebpf-node && cargo build"
```

### 5.6 Ejecutar el Nodo

```bash
lxc exec ebpf-blockchain -- bash -c "source /root/.cargo/env && RUST_LOG=info nohup ./ebpf-blockchain/ebpf-node/target/debug/ebpf-node --iface eth0 > /tmp/ebpf.log 2>&1 &"
```

### 5.7 Configurar Prometheus

Actualizar `prometheus.yml` con la IP del contenedor:
```yaml
- job_name: "ebpf_node_1"
  static_configs:
    - targets: ["192.168.2.210:9090"]
```

Reiniciar Prometheus:
```bash
docker compose restart prometheus
```

---

## 6. Comandos Útiles

```bash
# Ver estado de cloud-init
lxc exec ebpf-blockchain -- cloud-init status

# Ver logs de cloud-init
lxc exec ebpf-blockchain -- cat /var/log/cloud-init.log
lxc exec ebpf-blockchain -- cat /var/log/cloud-init-output.log

# Ver estado de red
lxc exec ebpf-blockchain -- networkctl status

# Reiniciar cloud-init
lxc exec ebpf-blockchain -- cloud-init clean -r

# Ver contenedores LXC
lxc list

# Ver configuración de red LXD
lxc network show lxdbr1

# Ver dnsmasq de LXD
ps aux | grep dnsmasq | grep lxdbr1
```

---

## 7. Verificación Final

### Estado del Sistema

```bash
# Contenedor corriendo
$ lxc list
+-----------------+---------+--------------------+---------------------------+
|      NAME       |  STATE  |        IPV4        |          IPV6             |
+-----------------+---------+--------------------+---------------------------+
| ebpf-blockchain | RUNNING | 192.168.2.210/24   | fd42:cb45:...:fec1:4c4c   |
+-----------------+---------+--------------------+---------------------------+

# Nodo eBPF activo
$ lxc exec ebpf-blockchain -- curl http://localhost:9090/metrics
# HELP ebpf_node_latency_buckets Current values of latency buckets
ebpf_node_latency_buckets{bucket="16"} 1
ebpf_node_latency_buckets{bucket="30"} 1
ebpf_node_latency_buckets{bucket="32"} 1

# Prometheus scrapeando
$ curl http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | select(.job=="ebpf_node_1")'
{"health":"up","lastError":""}
```

### Componentes Funcionales

| Componente | Estado | Notas |
|------------|--------|-------|
| Nodo eBPF | ✅ Funcionando | XDP, Kprobes, métricas de latencia |
| libp2p | ✅ Funcionando | Peer ID generado, QUIC listener |
| Prometheus | ✅ Scrapeando | Recolectando métricas del nodo |
| LXC Bridge | ✅ Configurado | Red 192.168.2.0/24 dedicada |
| Compilación | ✅ Exitosa | Release + Debug builds |

---

## 8. Conclusión

El sistema está **completamente funcional**:

1. **Nodo eBPF corriendo** en contenedor LXC privilegiado
2. **Métricas de latencia** siendo generadas correctamente
3. **Prometheus scrapeando** las métricas del contenedor
4. **Red dedicada** usando enp6s0 como uplink del bridge lxdbr1

### Problemas Resueltos

- Cloud-init: Instalación manual de dependencias
- DHCP: Uso de IP estática + gateway manual
- DNS: Configuración manual de nameserver
- Firewall SUSE: Reglas iptables para bridge
- Prometheus: Red compartida entre Docker y LXC

---

## 9. Verificación RFC 001

### 9.1 Scripts de Verificación

Se crearon scripts para verificar el cumplimiento del RFC:

```bash
# Verificación general del sistema
./scripts/verify_rfc001.sh

# Test específico de blacklist dinámica
./scripts/test_blacklist.sh
```

### 9.2 Resultados de Verificación

**Componentes Implementados (RFC 001):**

| Componente | Estado | Implementación |
|-----------|--------|----------------|
| XDP (eBPF Kernel) | ✅ | `ebpf_node()` en ebpf-node-ebpf |
| nodes_blacklist (LPM_TRIE) | ✅ | `NODES_BLACKLIST` - bloqueo de IPs |
| latency_stats (HISTOGRAM) | ✅ | `LATENCY_STATS` - 64 buckets power-of-2 |
| kprobes (netif/napi) | ✅ | `netif_receive_skb`, `napi_consume_skb` |
| libp2p (QUIC) | ✅ | `with_quic()` en main.rs |
| Gossipsub v1.1 | ✅ | `gossipsub::Behaviour` |
| Prometheus Metrics | ✅ | Servidor en puerto 9090 |
| Blacklist dinámica | ✅ | Gossipsub → NODES_BLACKLIST |

**Componentes Pendientes (RFC 001):**

| Componente | Prioridad | Notas |
|-----------|----------|-------|
| Curve25519 (ECDH) PFS | Alta | Perfect Forward Secrecy |
| ratelimit_cfg (HASH) | Media | Control de ancho de banda |
| TC hooks | Media | Latencia de red completa |
| Peer Scoring | Alta | Anti-spam para Gossipsub |
| Backpressure XDP | Media | 80% CPU threshold |
| Self-Healing Handshake | Baja | Handshake de emergencia |
| Fuzzing AFL++ | Baja | Testing de seguridad |

### 9.3 Métricas Capturadas

```
ebpf_node_latency_buckets{bucket="12"} 3
ebpf_node_latency_buckets{bucket="13"} 506
ebpf_node_latency_buckets{bucket="14"} 4707
ebpf_node_latency_buckets{bucket="15"} 10419
...
Total packets monitoreados: 123,691+
```

### 9.4 Arquitectura de Seguridad Implementada

```
[Paquete entrante]
       │
       ▼
┌──────────────────┐
│   XDP (eBPF)    │ ◄── LpmTrie Lookup
│  ebpf_node()    │     NODES_BLACKLIST
└────────┬────────┘
         │
    ┌────┴────┐
    │         │
 XDP_DROP  XDP_PASS
 (bloqueado) (procesado)
```

---

## Referencias

- [Cloud-Init Documentation](https://cloudinit.readthedocs.io/)
- [LXD Networking](https://documentation.ubuntu.com/lxd/en/latest/networks/)
- [LXC Security](https://documentation.ubuntu.com/lxd/en/latest/security/)
- [AYA eBPF Framework](https://aya-rs.dev/)
- [SUSE Firewall](https://documentation.suse.com/)
- [RFC 001: Arquitectura Blockchain eBPF](./rfc.md)
