#!/bin/bash
# =============================================================================
# Script de instalación de dependencias para nodos eBPF
# Este script se ejecuta dentro de cada nodo LXC
# Uso: bash /opt/install-deps.sh [sin-internet]
# =============================================================================

set -e

# Asegurar que HOME esté configurado
export HOME="${HOME:-/root}"

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

MARKER_FILE="$HOME/.deps-installed"
LOG_FILE="$HOME/install-deps.log"
MODE="${1:-internet}"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

error() {
    echo -e "${RED}ERROR: $1${NC}" | tee -a "$LOG_FILE"
}

log "========================================"
log "  Instalando dependencias eBPF"
log "  Modo: $MODE"
log "========================================"

# Verificar si ya está instalado
if [ -f "$MARKER_FILE" ]; then
    log "Las dependencias ya están instaladas. Omitiendo..."
    exit 0
fi

# Función para verificar conectividad
check_connectivity() {
    ping -c 1 -W 2 8.8.8.8 &>/dev/null
}

# Verificar según el modo
if [ "$MODE" = "sin-internet" ]; then
    log "Modo sin-internet: saltando verificación de conectividad"
    log "Asegúrate de tener los paquetes en /opt/packages/"
elif [ -n "$HTTP_PROXY" ] || [ -n "$http_proxy" ]; then
    log "Usando proxy configurado: ${HTTP_PROXY:-$http_proxy}"
    log "Configurando apt para usar proxy..."
    echo "Acquire::http::Proxy \"${HTTP_PROXY:-$http_proxy}\";" > /etc/apt/apt.conf.d/99proxy
    log "Conectividad OK (con proxy)"
else
    log "Verificando conectividad a internet..."
    if ! check_connectivity; then
        error "=============================================="
        error "SIN CONECTIVIDAD A INTERNET"
        error "=============================================="
        error ""
        error "Los nodos no pueden acceder a internet."
        error ""
        error "Opciones para resolver:"
        error "  1. Configurar NAT en el host (requiere sudo):"
        error "       sudo iptables -t nat -A POSTROUTING -s 192.168.2.0/24 -j MASQUERADE"
        error ""
        error "  2. Usar un proxy:"
        error "       export HTTP_PROXY=http://tu-proxy:port"
        error "       lxc exec <nodo> -- bash /opt/install-deps.sh"
        error ""
        error "  3. Crear un template con dependencias pre-instaladas"
        error ""
        error "=============================================="
        error ""
        log "Saliendo..."
        exit 1
    fi
    log "Conectividad OK"
fi

# Actualizar repositorios
log "Actualizando repositorios..."
apt update -qq 2>&1 | tee -a "$LOG_FILE" || {
    log "ERROR: No se pudieron actualizar los repositorios"
    exit 1
}

# Instalar dependencias del sistema
log "Instalando dependencias del sistema..."
SYSTEM_DEPS="build-essential clang llvm libelf-dev libbpf-dev curl git"
apt install -y $SYSTEM_DEPS 2>&1 | tee -a "$LOG_FILE" || {
    log "ERROR: Falló la instalación de paquetes del sistema"
    exit 1
}
log "Paquetes del sistema instalados"

# Instalar Rust
log "Instalando Rust..."
if [ -d "$HOME/.cargo" ]; then
    log "Rust ya está instalado, omitiendo..."
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y 2>&1 | tee -a "$LOG_FILE" || {
        log "ERROR: Falló la instalación de Rust"
        exit 1
    }
fi

# Source cargo env
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
else
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# Instalar nightly toolchain con rust-src
log "Instalando Rust nightly..."
rustup toolchain install nightly --component rust-src 2>&1 | tee -a "$LOG_FILE" || {
    log "ERROR: Falló la instalación de Rust nightly"
    exit 1
}
rustup default nightly 2>&1 | tee -a "$LOG_FILE"
log "Rust nightly instalado"

# Instalar crates de Rust
log "Instalando crates de Rust..."
CRATES="bpf-linker cargo-watch"
for crate in $CRATES; do
    log "Instalando $crate..."
    cargo install $crate --force 2>&1 | tee -a "$LOG_FILE" || {
        log "ADVERTENCIA: Falló la instalación de $crate"
    }
done

# Marcar como instalado
log "Marcando dependencias como instaladas..."
touch "$MARKER_FILE"

log "========================================"
log "  Dependencias instaladas correctamente"
log "========================================"

# Verificar instalación
log "Verificando instalación..."
source "$HOME/.cargo/env"
echo ""
echo "Versiones instaladas:"
echo "  - Rust: $(rustc --version)"
echo "  - Cargo: $(cargo --version)"
echo "  - Clang: $(clang --version | head -1)"
echo "  - LLVM: $(llvm-config --version 2>/dev/null || echo 'N/A')"

# Listar crates instalados
echo ""
echo "Crates instalados:"
cargo install --list 2>/dev/null | grep -E "^[a-z].* v" || echo "  (ninguno)"

echo ""
echo "Dependencias listas para compilar proyectos eBPF"