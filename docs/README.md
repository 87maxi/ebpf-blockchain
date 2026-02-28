instalacion de lxc lxd 

lxc profile create ebpf-blockchain

lxc profile edit ebpf-blockchain

lxc launch ubuntu:22.04 blockchain-node -p ebpf-blockchain




# Detener, renombrar y reiniciar para aplicar cambios de configuración
lxc stop blockchain-node
lxc rename blockchain-node ebpf-blockchain
lxc start ebpf-blockchain

# Montar el directorio del host dentro del contenedor para desarrollo persistente
lxc config device add ebpf-blockchain project disk \
    source=/home/maxi/Documentos/source/codecrypto/rust/ebpf-blockchain \
    path=/root/ebpf-blockchain


lxc exec ebpf-blockchain -- bash -c "apt update && apt install -y \
    build-essential curl git libelf-dev zlib1g-dev \
    clang llvm pkg-config libssl-dev libbpf-dev"


# Instalar Rust (Stable)
lxc exec ebpf-blockchain -- bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"

# Instalar Nightly (necesario para compilación de core en BPF) y componentes
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && rustup toolchain install nightly"
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && rustup component add rust-src --toolchain nightly"

# Instalar Linker de BPF y generador de proyectos
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && cargo install bpf-linker"
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && cargo install cargo-generate"


# Generar el proyecto usando el template de Aya (modo silencioso para evitar errores de TTY)
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && cd /root/ebpf-blockchain && \
    cargo generate --git https://github.com/aya-rs/aya-template \
    --name ebpf-node -d program_type=xdp -d default_iface=eth0 --silent"

# Compilar el proyecto completo (User Space + eBPF Kernel Space)
lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && cd /root/ebpf-blockchain/ebpf-node && cargo build"


# Finalmente, el comando para poner en marcha el nodo con monitoreo de logs:

lxc exec ebpf-blockchain -- bash -c "source \$HOME/.cargo/env && cd /root/ebpf-blockchain/ebpf-node && \
    RUST_LOG=info ./target/debug/ebpf-node --iface eth0"


**Nota técnica:** La configuración del archivo `ebpf-blockchain.yaml` fue clave para que estos comandos funcionen, ya que otorga los privilegios necesarios (`security.privileged: "true"`) y permite el acceso a los mapas de BPF mediante el montaje de `/sys/fs/bpf`.
