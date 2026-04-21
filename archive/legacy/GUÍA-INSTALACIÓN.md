# eBPF Blockchain POC - Guía de Instalación

## Requisitos del Sistema

### Software Requerido
- **Sistema Operativo:** Linux (Ubuntu 20.04 LTS o superior)
- **Rust:** Versión 1.70 o superior
- **Kernel Linux:** Versión 5.10 o superior (con soporte eBPF)
- **Docker/LXD:** Para contenedores
- **Ansible:** Versión 2.12 o superior
- **Build Tools:** `make`, `gcc`, `libelf-dev`

### Hardware Requerido
- **CPU:** Procesador moderno (mínimo 2 núcleos)
- **Memoria:** Mínimo 4GB RAM
- **Almacenamiento:** Mínimo 20GB de espacio libre
- **Red:** Conectividad de red para nodos

## Configuración del Entorno

### Paso 1: Instalación de Dependencias

```bash
# Actualizar el sistema
sudo apt update && sudo apt upgrade -y

# Instalar dependencias básicas
sudo apt install -y build-essential gcc libelf-dev zlib1g-dev libssl-dev

# Instalar Rust (método recomendado)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Instalar Ansible
sudo apt install -y ansible

# Verificar instalaciones
rustc --version
ansible --version
```

### Paso 2: Configuración de LXD

```bash
# Instalar LXD (si no está instalado)
sudo apt install -y lxd

# Iniciar LXD (si es necesario)
sudo systemctl start lxd
sudo systemctl enable lxd

# Configurar red para LXD
sudo lxc network create lxdbr0 ipv4.address=10.0.0.1/24 ipv4.nat=true
```

### Paso 3: Configuración de Reglas de Firewall

```bash
# Permitir comunicación en puerto 50000 para P2P
sudo iptables -A INPUT -p tcp --dport 50000 -j ACCEPT
sudo iptables -A INPUT -p udp --dport 50000 -j ACCEPT
sudo iptables -A FORWARD -d 10.0.0.0/8 -j ACCEPT

# Guardar reglas (opcional pero recomendado)
sudo iptables-save > /etc/iptables/rules.v4
```

## Clonación del Repositorio

```bash
# Clonar el repositorio
git clone https://github.com/eBPF-Blockchain/ebpf-blockchain.git
cd ebpf-blockchain

# Verificar estructura del proyecto
ls -la
```

## Compilación del Proyecto

### Paso 1: Compilación del Kernel (eBPF)

```bash
cd kernel/ebpf
make clean
make
```

### Paso 2: Compilación del Usuario (Rust)

```bash
cd ../user
cargo build --release
```

## Despliegue con Ansible

### Paso 1: Configuración del Inventario

```bash
# Crear archivo de inventario
cat > ansible/inventory << EOF
[all]
node1 ansible_host=10.0.0.10
node2 ansible_host=10.0.0.11
node3 ansible_host=10.0.0.12

[validators]
node1
node2
node3

[monitoring]
node1
EOF
```

### Paso 2: Ejecutar Playbooks

```bash
# Ejecutar playbook de configuración
ansible-playbook -i ansible/inventory ansible/site.yml

# Verificar despliegue
ansible -i ansible/inventory all -m ping
```

## Configuración Inicial del Sistema

### Paso 1: Iniciar Nodos

```bash
# Iniciar el primer nodo como bootstrap
./target/release/ebpf-blockchain --node-id node1 --bootstrap --port 50000

# Iniciar nodos adicionales
./target/release/ebpf-blockchain --node-id node2 --bootstrap node1:50000 --port 50000
./target/release/ebpf-blockchain --node-id node3 --bootstrap node1:50000 --port 50000
```

### Paso 2: Verificar Conectividad

```bash
# Verificar conectividad entre nodos
curl http://localhost:9090/metrics | grep "peers_connected"

# Verificar métricas de seguridad
curl http://localhost:9090/metrics | grep "attacks_detected"
```

## Verificación de Funcionalidad

### Prueba 1: Conectividad de Red

```bash
# Verificar que los nodos se descubran entre sí
ansible -i ansible/inventory all -a "curl -s http://localhost:9090/metrics | grep 'peers_connected'"
```

### Prueba 2: Métricas de Seguridad

```bash
# Verificar que las métricas de seguridad estén funcionando
ansible -i ansible/inventory all -a "curl -s http://localhost:9090/metrics | grep 'attacks_detected'"
```

### Prueba 3: Dashboard de Observabilidad

```bash
# Acceder al dashboard de Grafana (puerto 3000)
# http://<direccion_ip>:3000
# Usuario: admin
# Contraseña: admin
```

## Solución de Problemas Comunes

### Problema 1: Problemas de Conectividad

**Síntomas:** Los nodos no se comunican entre sí

**Solución:**
```bash
# Verificar reglas de firewall
sudo iptables -L

# Verificar configuración de red LXD
sudo lxc network list

# Reiniciar servicios de red
sudo systemctl restart networking
```

### Problema 2: Métricas no Actualizadas

**Síntomas:** Las métricas de Prometheus no se actualizan

**Solución:**
```bash
# Verificar servicios
sudo systemctl status prometheus
sudo systemctl status grafana-server

# Reiniciar servicios de métricas
sudo systemctl restart prometheus
sudo systemctl restart grafana-server
```

### Problema 3: Errores de Compilación

**Síntomas:** Errores durante la compilación

**Solución:**
```bash
# Limpiar y reconstruir
cargo clean
cargo build --release

# Verificar versión de Rust
rustc --version
```

## Recursos Adicionales

### Documentación Técnica
- [Documentación de eBPF](https://ebpf.io/)
- [Documentación de Rust](https://doc.rust-lang.org/)
- [Documentación de Ansible](https://docs.ansible.com/)

### Soporte y Ayuda
- Issue Tracker: https://github.com/eBPF-Blockchain/ebpf-blockchain/issues
- Documentación Online: https://ebpf-blockchain.readthedocs.io/

## Actualizaciones y Mantenimiento

### Actualización de Componentes

```bash
# Actualizar dependencias de Rust
cargo update

# Actualizar Ansible roles
ansible-galaxy install -r ansible/requirements.yml

# Reconstruir el sistema
cargo build --release
```

### Backups

```bash
# Hacer backup de configuraciones
tar -czf config-backup-$(date +%Y%m%d).tar.gz ansible/ config/

# Backup de datos del sistema
tar -czf data-backup-$(date +%Y%m%d).tar.gz data/
```

## Enlaces Útiles

- **Repositorio GitHub:** https://github.com/eBPF-Blockchain/ebpf-blockchain
- **Documentación:** https://ebpf-blockchain.readthedocs.io/
- **Issue Tracker:** https://github.com/eBPF-Blockchain/ebpf-blockchain/issues
- **Comunidad:** https://discord.gg/ebpf-blockchain
