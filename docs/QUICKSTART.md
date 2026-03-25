# Quick Start Guide - eBPF Blockchain Lab

## 5-Minute Setup (If Prerequisites Met)

```bash
# 1. Crear red LXC
sudo lxc network create lxdbr1 --type bridge
sudo lxc network set lxdbr1 ipv4.address=192.168.2.200/24
sudo iptables -I FORWARD -i lxdbr1 -j ACCEPT

# 2. Crear perfil
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

# 3. Lanzar contenedor
lxc launch ubuntu:22.04 ebpf-blockchain -p ebpf-blockchain

# 4. Configurar red
lxc exec ebpf-blockchain -- bash -c "ip addr flush dev eth0 && ip addr add 192.168.2.210/24 dev eth0 && ip route add default via 192.168.2.200 && echo 'nameserver 8.8.8.8' > /etc/resolv.conf"

# 5. Instalar dependencias
lxc exec ebpf-blockchain -- bash -c "apt update && apt install -y build-essential clang llvm libelf-dev libbpf-dev curl git && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && /root/.cargo/bin/rustup toolchain install nightly --component rust-src"

# 6. Montar proyecto y compilar
lxc config device add ebpf-blockchain workspace disk source=/home/maxi/Documentos/source/ebpf-blockchain path=/root/ebpf-blockchain
lxc exec ebpf-blockchain -- bash -c "source /root/.cargo/env && cd /root/ebpf-blockchain/ebpf-node && cargo build"

# 7. Iniciar nodo
lxc exec ebpf-blockchain -- bash -c "source /root/.cargo/env && nohup /root/ebpf-blockchain/ebpf-node/target/debug/ebpf-node --iface eth0 > /tmp/ebpf.log 2>&1 &"

# 8. Verificar
curl http://192.168.2.210:9090/metrics | grep ebpf_node
```

## Verificación Rápida

```bash
# Estado del nodo
curl http://192.168.2.210:9090/metrics

# Logs
lxc exec ebpf-blockchain -- tail -f /tmp/ebpf.log

# Peer ID
lxc exec ebpf-blockchain -- grep Peer /tmp/ebpf.log

# Prometheus targets
curl http://localhost:9090/api/v1/targets

# Grafana
# http://localhost:3000 (admin/admin)
```

## Comandos Diarios

```bash
# Ver nodos
lxc list

# Acceder a nodo
lxc exec ebpf-blockchain -- bash

# Ver logs
lxc exec ebpf-blockchain -- tail /tmp/ebpf.log

# Reiniciar nodo
lxc exec ebpf-blockchain -- pkill ebpf-node
lxc exec ebpf-blockchain -- bash -c "source /root/.cargo/env && nohup /root/ebpf-blockchain/ebpf-node/target/debug/ebpf-node --iface eth0 > /tmp/ebpf.log 2>&1 &"

# Crear nuevo nodo
./scripts/create_node.sh ebpf-blockchain-2 192.168.2.210

# Verificar RFC 001
./scripts/verify_rfc001.sh
```
