# Estructura del Proyecto - Organización y Módulos Rust

## Visión General

Este documento define la estructura organizativa del proyecto **eBPF Blockchain**, un sistema distribuido que combina:
- **eBPF** para observabilidad y seguridad a nivel de kernel
- **libp2p** para comunicación P2P descentralizada
- **Rust** para implementación de alto rendimiento y seguridad de memoria
- **RocksDB** para persistencia de datos
- **API HTTP/WebSockets** para exposición de funcionalidades

La estructura está diseñada para ser:
- **Modular**: Separación clara de responsabilidades
- **Escalable**: Fácil de expandir con nuevas funcionalidades
- **Maintenable**: Código organizado y documentado
- **Testable**: Estructura que facilita pruebas unitarias e integration tests

---

## 1. Estructura del Repositorio

### 1.1 Organización Actual (Mejorada)

El repositorio sigue una organización basada en features y responsabilidades:

```
ebpf-blockchain/
├── .github/              # Automatización CI/CD
├── ansible/              # Automatización de infraestructura
├── ebpf-node/            # Binary principal y módulos Rust
├── monitoring/           # Stack de observabilidad
├── tools/                # Herramientas de desarrollo y testing
├── docs/                 # Documentación del proyecto
├── plan_mejora/          # Planes de mejora y arquitectura
├── scripts/              # Scripts utilitarios
├── tests/                # Tests de integración y E2E
├── Cargo.toml            # Workspace root
├── README.md             # Documentación principal
└── LICENSE               # Licencia del proyecto
```

---

## 2. Módulos Rust Detallados

### 2.1 Workspace Cargo

El workspace de Cargo organiza múltiples crates en un solo proyecto:

```toml
[workspace]
members = [
    "ebpf-node",              # Binary principal y lógica de negocio
    "ebpf-node-ebpf",         # Código eBPF compilable
]
resolver = "2"

[workspace.package]
version = "0.3.0"
edition = "2021"
authors = ["eBPF Blockchain Team"]
license = "MIT"
repository = "https://github.com/ebpf-blockchain/ebpf-blockchain"

[workspace.dependencies]
# eBPF
aya = "0.12"
aya-log = "0.12"
aya-ebpf-bindings = "0.12"

# Networking
libp2p = { version = "0.53", features = ["full"] }
libp2p-gossipsub = "0.46"
libp2p-identify = "0.44"
libp2p-mdns = "0.45"
libp2p-request-response = "0.26"

# Tokio runtime
tokio = { version = "1.37", features = ["full"] }

# HTTP/API
axum = "0.7"
tower = "0.4"

# Storage
rocksdb = "0.22"

# Monitoring
prometheus = "0.23"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# CLI
clap = { version = "4.4", features = ["derive"] }

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"

# Criptografía
sha2 = "0.10"
ed25519-dalek = "2.0"
```

---

## 3. Organización del Código Rust

### 3.1 Estructura del Binary Principal

El binary principal (`ebpf-node`) orquesta todos los componentes del nodo:

```rust
// src/bin/ebpf-node.rs
use ebpf_node::node::Node;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Parsear argumentos CLI
    let config = NodeConfig::from_cli();
    
    // 2. Inicializar logger y tracing
    tracing_subscriber::fmt::init();
    
    // 3. Crear instancia del nodo
    let mut node = Node::new(config).await?;
    
    // 4. Iniciar ejecución
    node.run().await?;
    
    Ok(())
}
```

### 3.2 Módulo Node

El módulo `node` es el corazón del sistema, coordinando todos los componentes:

```rust
pub mod swarm;
pub mod consensus;
pub mod peer_manager;
pub mod gossip;
pub mod storage;

pub struct Node {
    pub config: NodeConfig,
    pub state: NodeState,
    pub channels: NodeChannels,
    pub components: NodeComponents,
}

pub struct NodeConfig {
    pub iface: String,
    pub listen_addresses: Vec<String>,
    pub bootstrap_peers: Vec<String>,
    pub db_path: PathBuf,
    pub consensus: ConsensusConfig,
}

pub enum NodeState {
    Initializing,
    Running,
    Stopped,
    Error,
}

pub struct NodeChannels {
    pub tx_rpc: tokio::sync::mpsc::Sender<RpcRequest>,
    pub tx_ws: tokio::sync::broadcast::Sender<WsMessage>,
    pub tx_consensus: tokio::sync::mpsc::Sender<ConsensusEvent>,
}

pub struct NodeComponents {
    pub swarm: SwarmHandle,
    pub ebpf: EbpfPrograms,
    pub storage: DBHandle,
    pub monitoring: MonitoringSystem,
}

impl Node {
    pub async fn new(config: NodeConfig) -> Result<Self> {
        // Inicializar componentes en orden de dependencia
        let storage = DBHandle::open(&config.db_path)?;
        let swarm = SwarmHandle::new(&config).await?;
        let ebpf = EbpfPrograms::load(&config)?;
        let monitoring = MonitoringSystem::new()?;
        
        // Crear canales de comunicación
        let (tx_rpc, rx_rpc) = tokio::sync::mpsc::channel(100);
        let (tx_ws, rx_ws) = tokio::sync::broadcast::channel(100);
        let (tx_consensus, rx_consensus) = tokio::sync::mpsc::channel(100);
        
        Ok(Self {
            config,
            state: NodeState::Initializing,
            channels: NodeChannels { tx_rpc, tx_ws, tx_consensus },
            components: NodeComponents { swarm, ebpf, storage, monitoring },
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.state = NodeState::Running;
        
        // Iniciar todos los componentes en paralelo
        let swarm_handle = tokio::spawn(async move {
            self.components.swarm.run().await
        });
        
        let api_handle = tokio::spawn(async move {
            self.components.api.run().await
        });
        
        // Esperar a que se detenga
        tokio::select! {
            result = swarm_handle => result?,
            result = api_handle => result?,
        }
        
        Ok(())
    }

    async fn shutdown(&mut self) {
        // Limpiar recursos en orden inverso
        self.components.ebpf.unload();
        self.components.swarm.close();
        self.components.storage.close();
        self.state = NodeState::Stopped;
    }
}
```

### 3.3 Módulo Swarm (P2P Networking)

El módulo `swarm` maneja la comunicación P2P usando libp2p:

```rust
pub struct SwarmBehaviour {
    pub gossipsub: Gossipsub,
    pub identify: Identify,
    pub mdns: Mdns,
    pub sync: RequestResponse,
}

pub struct SwarmHandle {
    pub swarm: Swarm<SwarmBehaviour>,
    pub local_peer_id: PeerId,
    pub topic: Topic,
}

impl SwarmHandle {
    pub fn new(config: &NodeConfig) -> Result<Self> {
        // Generar keypair para identidad
        let keypair = generate_ed25519()?;
        let peer_id = keypair.public().to_peer_id();
        
        // Configurar transporte (TCP + QUIC)
        let tcp_transport = TokioTcpTransport::new(TokioExecutor::new());
        let quic_transport = TokioQuicTransport::new(TokioExecutor::new());
        let transport = tcp_transport
            .or_transport(quic_transport)
            .with_max_concurrency(10);
        
        // Configurar gossipsub
        let gossipsub_config = GossipsubConfigBuilder::default()
            .validate_messages()
            .heartbeat_interval(Duration::from_secs(10))
            .build()?;
        
        let gossipsub = Gossipsub::new(
            MessageAuthenticity::Signed(keypair),
            gossipsub_config,
        )?;
        
        // Configurar identify
        let identify = Identify::new(
            IdentifyConfig::new("ipfs/1.0.0", keypair.public())
                .with_agent_version("ebpf-node/0.3.0"),
        );
        
        // Configurar mDNS
        let mdns = MdnsConfig::builder()
            .ttl(Duration::from_secs(60))
            .build();
        
        Ok(Self {
            swarm: Swarm::new(transport, SwarmBehaviour {
                gossipsub,
                identify,
                mdns,
                sync: RequestResponse::new(/* config */),
            }, peer_id, TokioExecutor::new()),
            local_peer_id: peer_id,
            topic: Topic::new("ebpf-blockchain/transactions/v1"),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Escuchar en direcciones configuradas
        for addr in &self.config.listen_addresses {
            self.swarm.listen_on(addr.parse()?)?;
        }
        
        // Conectar a bootstrap peers
        for bootstrap in &self.config.bootstrap_peers {
            let (peer_id, addr) = parse_peer_addr(bootstrap)?;
            self.swarm.dial(addr)?;
            self.swarm.add_connected_peer(peer_id);
        }
        
        // Subscribirse a topic gossipsub
        self.swarm.gossipsub.subscribe(&self.topic)?;
        
        // Loop principal del swarm
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    tracing::info!("Escuchando en: {}", address);
                }
                SwarmEvent::Behaviour(GossipsubEvent::Message { message, .. }) => {
                    self.handle_gossip_message(message).await?;
                }
                SwarmEvent::OutgoingConnectionError { error, .. } => {
                    tracing::warn!("Error de conexión: {:?}", error);
                }
                _ => {}
            }
        }
    }
}
```

### 3.4 Módulo de Consenso

El módulo `consensus` implementa el algoritmo de consenso para validación de transacciones:

```rust
pub struct ConsensusConfig {
    pub quorum_threshold: u32,      // % mínimo de votos requeridos
    pub max_voters_per_tx: u32,     // Máximo de votantes por transacción
    pub vote_timeout: Duration,     // Timeout para recoger votos
}

pub struct ConsensusEngine {
    pub db: DBHandle,
    pub config: ConsensusConfig,
    pub known_peers: Arc<RwLock<HashSet<PeerId>>>,
    pub sequence_history: Arc<RwLock<SequenceBuffer>>,
    pub voter_blacklist: Arc<RwLock<HashSet<PeerId>>},
    pub metrics: ConsensusMetrics,
}

pub struct Transaction {
    pub id: TxId,
    pub data: TxData,
    pub sequence: u64,
    pub timestamp: UnixTimestamp,
    pub signer: PublicKey,
}

pub struct Vote {
    pub tx_id: TxId,
    pub peer_id: PeerId,
    pub signature: Signature,
    pub timestamp: UnixTimestamp,
}

impl ConsensusEngine {
    pub fn new(db: DBHandle, config: ConsensusConfig) -> Self {
        Self {
            db,
            config,
            known_peers: Arc::new(RwLock::new(HashSet::new())),
            sequence_history: Arc::new(RwLock::new(SequenceBuffer::new())),
            voter_blacklist: Arc::new(RwLock::new(HashSet::new())),
            metrics: ConsensusMetrics::new(),
        }
    }

    pub async fn validate_transaction(&self, tx: Transaction) -> ConsensusResult {
        // 1. Verificar que no sea un replay attack
        if self.sequence_history.read().contains(tx.id) {
            return ConsensusResult::Rejected(RejectionReason::ReplayAttack);
        }
        
        // 2. Verificar firma
        if !self.verify_signature(&tx)? {
            return ConsensusResult::Rejected(RejectionReason::InvalidSignature);
        }
        
        // 3. Verificar que el signatario no esté blacklist
        if self.voter_blacklist.read().contains(&tx.signer) {
            return ConsensusResult::Rejected(RejectionReason::Blacklisted);
        }
        
        // 4. Guardar en pending
        self.db.store_pending_tx(tx).await?;
        
        ConsensusResult::Pending
    }

    pub async fn handle_tx_proposal(&self, proposal: TxProposal) -> ConsensusResult {
        // Validar transacción
        match self.validate_transaction(proposal.tx).await {
            ConsensusResult::Pending => {
                // Enviar a todos los peers conocidos
                self.propose_to_peers(&proposal).await?;
                ConsensusResult::Voted
            }
            other => other,
        }
    }

    pub async fn handle_vote(&self, vote: Vote) -> ConsensusResult {
        // Verificar que el votante no esté blacklist
        if self.voter_blacklist.read().contains(&vote.peer_id) {
            return ConsensusResult::Rejected(RejectionReason::Blacklisted);
        }
        
        // Registrar voto
        self.register_vote(&vote).await?;
        
        // Verificar si se alcanzó quórum
        if self.check_quorum(&vote.tx_id).await? {
            self.confirm_transaction(&vote.tx_id).await?;
            ConsensusResult::Confirmed
        } else {
            ConsensusResult::Voted
        }
    }

    async fn confirm_transaction(&self, tx_id: &TxId) -> Result<()> {
        // 1. Guardar como confirmado en DB
        self.db.confirm_transaction(tx_id).await?;
        
        // 2. Eliminar de pending
        self.db.remove_pending_tx(tx_id).await?;
        
        // 3. Añadir a sequence history
        self.sequence_history.write().add(*tx_id)?;
        
        // 4. Emitir evento de confirmación
        self.emit_confirmation_event(tx_id).await?;
        
        Ok(())
    }

    pub async fn add_known_peer(&self, peer_id: PeerId) {
        self.known_peers.write().insert(peer_id);
    }

    pub async fn blacklist_voter(&self, peer_id: PeerId) {
        self.voter_blacklist.write().insert(peer_id);
        
        // Emitir métrica de blacklist
        self.metrics.blacklist_count.inc();
    }
}

pub enum ConsensusResult {
    Rejected(RejectionReason),
    Voted,
    LimitReached,
    Confirmed,
    Pending,
}
```

### 3.5 Módulo eBPF

El módulo `ebpf` carga y gestiona programas eBPF para observabilidad y seguridad:

```rust
pub mod xdp;
pub mod kprobes;
pub mod maps;
pub mod security;

pub struct EbpfPrograms {
    pub xdp: XdpProgram,
    pub kprobe_in: KprobeProgram,
    pub kprobe_out: KprobeProgram,
    pub maps: EbpfMaps,
}

pub struct EbpfMaps {
    pub nodes_blacklist: Hashmap<NodeId>,
    pub latency_stats: Array<LatencySample>,
    pub whitelist: Hashmap<NodeId>,
    pub rate_limit: LruHashMap<NodeId, RateInfo>,
}

pub struct RateLimitConfig {
    pub max_messages_per_second: u32,
    pub max_bytes_per_second: u64,
    pub burst_size: u32,
}

impl EbpfPrograms {
    pub fn load(config: &NodeConfig) -> Result<Self> {
        // Cargar bytecode eBPF
        let xdp_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/xdp.o"));
        let kprobe_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/kprobe.o"));
        
        // Crear programa XDP para blacklist reactiva
        let xdp = XdpProgram::load(xdp_bytes, &config.iface)?;
        
        // Crear kprobes para tracking de latencia
        let kprobe_in = KprobeProgram::load(
            kprobe_bytes, 
            "kprobe___sys_sendmsg"?
        )?;
        
        let kprobe_out = KprobeProgram::load(
            kprobe_bytes,
            "kretprobe___sys_sendmsg"?
        )?;
        
        Ok(Self {
            xdp,
            kprobe_in,
            kprobe_out,
            maps: EbpfMaps::new()?,
        })
    }

    pub fn add_to_whitelist(&mut self, node_id: NodeId) -> Result<()> {
        self.maps.whitelist.insert(node_id, &1u8, 0)?;
        Ok(())
    }

    pub fn add_to_blacklist(&mut self, node_id: NodeId) -> Result<()> {
        self.maps.nodes_blacklist.insert(node_id, &1u8, 0)?;
        
        // Actualizar métricas
        self.metrics.blacklist_count.inc();
        
        Ok(())
    }

    pub fn get_latency_stats(&self) -> Vec<LatencySample> {
        self.maps.latency_stats.iter().collect()
    }
}
```

### 3.6 Módulo de Almacenamiento

El módulo `storage` gestiona la persistencia de datos usando RocksDB:

```rust
pub mod rocksdb;
pub mod transaction;

pub struct DBHandle {
    pub db: DB,
    pub path: PathBuf,
}

impl DBHandle {
    pub fn open(path: &Path) -> Result<Self> {
        let db = DB::open_default(path)?;
        Ok(Self { db, path: path.to_path_buf() })
    }

    pub fn get_transaction(&self, id: &TxId) -> Result<Option<Transaction>> {
        let key = format!("tx:{}", hex::encode(id));
        let value = self.db.get(key)?;
        
        value.map(|v| bincode::deserialize(&v))
            .transpose()
    }

    pub fn put_transaction(&self, tx: &Transaction) -> Result<()> {
        let key = format!("tx:{}", hex::encode(tx.id));
        let value = bincode::serialize(tx)?;
        self.db.put(key, value)?;
        Ok(())
    }

    pub fn get_voters(&self, tx_id: &TxId) -> Result<Vec<Vote>> {
        let prefix = format!("vote:{}:", hex::encode(tx_id));
        let mut voters = Vec::new();
        
        for entry in self.db.iterator(IterationMode::From(&prefix)) {
            let (_, value) = entry?;
            if let Ok(vote) = bincode::deserialize(&value) {
                voters.push(vote);
            }
        }
        
        Ok(voters)
    }

    pub fn put_voters(&self, tx_id: &TxId, votes: &[Vote]) -> Result<()> {
        let prefix = format!("vote:{}:", hex::encode(tx_id));
        
        for (i, vote) in votes.iter().enumerate() {
            let key = format!("{}{}", prefix, i);
            let value = bincode::serialize(vote)?;
            self.db.put(key, value)?;
        }
        
        Ok(())
    }

    pub fn confirm_transaction(&self, tx_id: &TxId) -> Result<()> {
        // Marcar como confirmado
        let key = format!("confirmed:{}", hex::encode(tx_id));
        self.db.put(key, b"confirmed")?;
        
        // Eliminar de pending
        let pending_key = format!("pending:{}", hex::encode(tx_id));
        self.db.delete(pending_key)?;
        
        Ok(())
    }

    pub fn list_transactions(&self) -> Result<Vec<Transaction>> {
        let mut txs = Vec::new();
        
        for entry in self.db.iterator(IterationMode::From(b"tx:")) {
            let (_, value) = entry?;
            if let Ok(tx) = bincode::deserialize(&value) {
                txs.push(tx);
            }
        }
        
        Ok(txs)
    }
}
```

### 3.7 Módulo de API HTTP

El módulo `api` expone funcionalidades del nodo vía HTTP y WebSockets:

```rust
pub mod metrics;
pub mod rpc;
pub mod ws;

pub struct ApiState {
    pub tx_rpc: tokio::sync::mpsc::Sender<RpcRequest>,
    pub tx_ws: tokio::sync::broadcast::Sender<WsMessage>,
    pub db: DBHandle,
    pub consensus: ConsensusEngine,
}

pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // RPC endpoints
        .route("/rpc/tx_propose", post(propose_transaction))
        .route("/rpc/tx_status", get(get_transaction_status))
        .route("/rpc/peer_list", get(list_peers))
        
        // WebSocket endpoints
        .route("/ws", get(websocket_handler))
        
        // Metrics
        .route("/metrics", get(metrics_handler))
        
        // Health check
        .route("/health", get(health_check))
        .with_state(state)
}

async fn metrics_handler(
    State(state): State<ApiState>,
) -> impl IntoResponse {
    prometheus::gather()
        .into_iter()
        .map(metric_family_to_bytes)
        .collect::<Result<Vec<_>, _>>()
        .map(|v| {
            Response::builder()
                .header("Content-Type", prometheus::TEXT_FORMAT)
                .body(v.join("\n\n").into_bytes())
        })
        .unwrap_or_else(|e| {
            Response::builder()
                .status(500)
                .body(format!("Error gathering metrics: {}", e))
        })
}
```

---

## 4. Convenios de Código

### 4.1 Nomenclatura

**Tipos:**
- `PascalCase`: Structs, Enums, Traits (`NodeConfig`, `ConsensusEngine`)
- `camelCase`: Tipos de alias y tipos de datos de terceros

**Variables y funciones:**
- `snake_case`: Variables, funciones, métodos (`node_config`, `handle_message`)

**Constantes:**
- `SCREAMING_SNAKE_CASE`: Constantes (`MAX_PEER_COUNT`, `DEFAULT_PORT`)

**Módulos:**
- `snake_case`: Nombres de módulos (`peer_manager`, `consensus_engine`)

**Archivos:**
- `snake_case`: Nombres de archivos (`consensus_engine.rs`)

### 4.2 Manejo de Errores

Patrones recomendados para manejo de errores:

```rust
// Usar anyhow para errores de alto nivel
pub fn handle_transaction(data: TxData) -> Result<Transaction> {
    let tx = Transaction::from_data(data)?;
    tx.validate()?;
    Ok(tx)
}

// Usar thiserror para errores específicos del dominio
#[derive(thiserror::Error, Debug)]
pub enum TransactionError {
    #[error("transaction not found: {0}")]
    NotFound(String),
    
    #[error("invalid signature for transaction")]
    InvalidSignature,
    
    #[error("replay attack detected")]
    ReplayAttack,
}

// Propagar errores con ?
pub fn process_transaction(tx: Transaction) -> Result<()> {
    let validated = tx.validate()?;
    self.storage.store(&validated)?;
    self.consensus.propose(validated)?;
    Ok(())
}
```

### 4.3 Documentación

**Todos los elementos públicos deben estar documentados:**

```rust
/// Gestiona el ciclo de vida del nodo P2P
///
/// El `Node` coordina todos los componentes:
/// - Swam P2P para comunicación
/// - ConsensusEngine para validación
/// - Storage para persistencia
/// - API para exposición pública
pub struct Node {
    /// Configuración del nodo
    pub config: NodeConfig,
    
    /// Estado actual del nodo
    pub state: NodeState,
    
    /// Canales de comunicación entre componentes
    pub channels: NodeChannels,
    
    /// Componentes del nodo
    pub components: NodeComponents,
}
```

---

## 5. Archivos de Configuración

### 5.1 .cargo/config.toml

```toml
[build]
# Usar rustflags para optimizaciones
rustflags = ["-C", "target-cpu=native"]

[env]
# Habilitar backtrace para debugging
RUST_BACKTRACE = "1"

[alias]
# Alias para comandos comunes
clippy = "clippy -- -D warnings"
fmt = "fmt -- --check"
```

### 5.2 rustfmt.toml

```toml
max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Unix"
use_small_heuristics = "Default"
reorder_imports = true
reorder_modules = true
remove_nested_parens = true
edition = "2021"
merge_derives = true
use_try_shorthand = true
use_field_init_shorthand = true
force_explicit_abi = true
```

### 5.3 clippy.toml

```toml
# Tipos desaconsejados
disallowed-types = [
    { path = "std::collections::HashMap", reason = "Usar dashmap para acceso concurrente" },
]

# Métodos desaconsejados
disallowed-methods = [
    { path = "Vec::new", reason = "Usar Vec::with_capacity si se conoce el tamaño" },
]
```

---

## 6. Guía de Estilo Rust

### 6.1 Prioridades de Estilo

1. **Legibilidad**: Código claro y auto-documentado
2. **Consistencia**: Seguir conventions de Rust y este documento
3. **Eficiencia**: Evitar allocations innecesarias
4. **Seguridad**: Preferir código que el compiler puede verificar

### 6.2 Patrones Preferidos

**Uso de Option y Result:**

```rust
// Preferir unwrap solo cuando es imposible el error
let value = possibly_failing_operation().expect("Operation should never fail");

// Usar ok() para convertir Result a Option
let opt = result.ok();

// Usar match para manejar múltiples casos
match value {
    Some(v) => process(v),
    None => handle_missing(),
}
```

**Manejo de recursos:**

```rust
// Usar RAII para gestión de recursos
struct Resource {
    handle: FileHandle,
}

impl Drop for Resource {
    fn drop(&mut self) {
        self.handle.close();
    }
}

// Usar scoped threads cuando sea posible
tokio::spawn(async move {
    // Código que usa recursos
});
```

### 6.3 Patrones a Evitar

```rust
// ❌ Evitar: Uso de unwrap() sin contexto
let value = risky_operation().unwrap();

// ✅ Preferir: Manejo explícito de errores
let value = risky_operation().context("Failed to process data")?;

// ❌ Evitar: Clonaciones innecesarias
let cloned = data.clone();
process(&cloned);

// ✅ Preferir: Pasar referencias
process(data);

// ❌ Evitar: Bloquear threads en async context
let sync_data = blocking_read();
async_process(&sync_data).await;

// ✅ Preferir: Usar tokio::task::spawn_blocking
let sync_data = tokio::task::spawn_blocking(blocking_read).await?;
```

---

## 7. Estructura de Directorios Final

```
ebpf-blockchain/
├── .github/
│   └── workflows/
│       ├── ci.yml           # Tests automáticos
│       ├── cd.yml           # Deploy automático
│       ├── lint.yml         # Linting y formateo
│       └── security-scan.yml # Escaneo de seguridad
│
├── ansible/
│   ├── inventory/
│   │   ├── hosts.yml         # Definición de hosts LXC
│   │   └── group_vars/       # Variables por grupo
│   ├── playbooks/
│   │   ├── deploy_node.yml   # Deploy de nodos
│   │   ├── setup_lxc.yml     # Configuración LXC
│   │   └── monitor.yml       # Setup de monitoreo
│   ├── roles/
│   │   ├── ebpf-node/
│   │   ├── lxc-setup/
│   │   └── monitoring/
│   ├── ansible.cfg          # Configuración Ansible
│   └── requirements.yml     # Dependencias Ansible
│
├── ebpf-node/
│   ├── Cargo.toml           # Definición del workspace
│   ├── ebpf-node-ebpf/      # Crate eBPF
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs      # Programa XDP
│   │       └── lib.rs       # Funciones kprobe
│   │
│   └── ebpf-node/           # Binary principal
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs      # Entry point
│           ├── lib.rs       # Módulos públicos
│           ├── bin/
│           │   ├── ebpf-node.rs    # Binary principal
│           │   └── ebpf-debug.rs   # Binary de debugging
│           ├── node/
│           │   ├── mod.rs        # Módulos del nodo
│           │   ├── swarm.rs      # P2P networking
│           │   ├── consensus.rs  # Algoritmo consenso
│           │   ├── peer_manager.rs # Gestión de peers
│           │   └── gossip.rs     # Protocolo gossip
│           ├── ebpf/
│           │   ├── mod.rs        # Módulos eBPF
│           │   ├── xdp.rs        # Programas XDP
│           │   ├── kprobes.rs    # Kprobes tracking
│           │   ├── maps.rs       # Maps eBPF
│           │   └── security.rs   # Seguridad eBPF
│           ├── storage/
│           │   ├── mod.rs        # Módulos storage
│           │   ├── rocksdb.rs    # Implementación RocksDB
│           │   └── transaction.rs # Modelo de transacciones
│           ├── api/
│           │   ├── mod.rs        # Módulos API
│           │   ├── metrics.rs    # Endpoints metrics
│           │   ├── rpc.rs        # RPC endpoints
│           │   └── ws.rs         # WebSocket handlers
│           ├── monitoring/
│           │   ├── mod.rs        # Módulos monitoring
│           │   ├── prometheus.rs # Metrics Prometheus
│           │   ├── logging.rs    # Sistema logging
│           │   └── tracing.rs    # Distributed tracing
│           └── utils/
│               ├── mod.rs        # Utilidades
│               ├── error.rs      # Tipos de error
│               └── config.rs     # Manejo configuración
│
├── monitoring/
│   ├── docker-compose.yml    # Stack observabilidad
│   ├── prometheus/
│   │   ├── prometheus.yml    # Configuración Prometheus
│   │   └── rules/            # Alerting rules
│   ├── grafana/
│   │   ├── dashboards/       # Dashboards JSON
│   │   └── datasources/      # Datasources YAML
│   └── loki/
│       └── loki.yml          # Configuración Loki
│
├── tools/
│   ├── ebpf-simulation/
│   │   ├── attack_simulator.py # Simulador de ataques
│   │   └── network_gen.py      # Generador de red
│   └── scripts/
│       ├── analyze_logs.sh     # Análisis de logs
│       └── benchmark.sh        # Benchmarks
│
├── docs/
│   ├── INSTALLATION.md       # Guía instalación
│   ├── ARCHITECTURE.md       # Arquitectura detallada
│   ├── API.md               # Documentación API
│   ├── SECURITY.md          # Consideraciones seguridad
│   ├── TROUBLESHOOTING.md   # Solución problemas
│   └── EXAMPLES.md          # Ejemplos de uso
│
├── plan_mejora/
│   ├── 00_VISION_Y_ESTADO_ACTUAL.md
│   ├── 00_EVOLUCION_ETAPAS.md
│   ├── 01_plan_estructural/
│   │   ├── 01_ESTRUCTURA_PROYECTO.md
│   │   └── 02_ORGANIZACION_MODULOS.md
│   ├── 02_ansible_lxc/
│   │   ├── 01_INFRAESTRUCTURA_LXC.md
│   │   ├── 02_ANSIBLE_PLAYBOOKS.md
│   │   └── 03_CONFIGURACION_REDES.md
│   ├── 03_rust_profundo/
│   │   ├── 01_PATRONES_RUST.md
│   │   ├── 02_ASINCRONIA_TOKIO.md
│   │   └── 03_MANEJO_ERRORES.md
│   ├── 04_laboratorio_pruebas/
│   │   ├── 01_TESTING_EBPF.md
│   │   ├── 02_SIMULACION_ATAQUES.md
│   │   └── 03_BENCHMARKS.md
│   ├── 05_consenso_seguro/
│   │   ├── 01_ALGORITMO_CONSENSO.md
│   │   ├── 02_PROTECCION_SYBIL.md
│   │   └── 03_REPLAY_PROTECTION.md
│   ├── 06_observabilidad/
│   │   ├── 01_PROMETHEUS_GRAFANA.md
│   │   ├── 02_LOKI_LOGGING.md
│   │   └── 03_DISTRIBUTED_TRACING.md
│   └── 07_seguridad/
│       ├── 01_SECURITY_AUDIT.md
│       ├── 02_ENCRYPTED_COMMUNICATION.md
│       └── 03_ACCESS_CONTROL.md
│
├── scripts/
│   ├── deploy.sh              # Deploy de nodos
│   ├── test_network.sh        # Testing de red
│   ├── simulate_attack.sh     # Simulación de ataques
│   ├── benchmark.sh           # Benchmarks
│   └── cleanup.sh             # Limpieza de recursos
│
├── tests/
│   ├── integration/
│   │   ├── consensus_test.rs  # Tests consenso
│   │   ├── gossip_test.rs     # Tests gossip
│   │   └── storage_test.rs    # Tests storage
│   ├── e2e/
│   │   ├── network_setup.rs   # Tests setup red
│   │   └── transaction_flow.rs # Flow transacciones
│   └── fixtures/
│       ├── test_transactions.json
│       └── mock_peers.json
│
├── .gitignore
├── LICENSE                    # Licencia MIT
├── README.md                  # Documentación principal
├── CONTRIBUTING.md            # Guía contribución
├── CODE_OF_CONDUCT.md         # Código conducta
└── SECURITY.md               # Política seguridad
```

### Descripción de Directorios

#### `.github/workflows/`
Flujos de trabajo GitHub Actions para:
- **ci.yml**: Ejecutar tests en cada PR
- **cd.yml**: Deploy automático a entornos
- **lint.yml**: Linting y formateo de código
- **security-scan.yml**: Escaneo de vulnerabilidades

#### `ansible/`
Automatización de infraestructura con Ansible:
- **inventory**: Definición de hosts y grupos
- **playbooks**: Scripts de automatización
- **roles**: Módulos reutilizables

#### `ebpf-node/`
Código fuente principal:
- **ebpf-node-ebpf**: Código eBPF compilable para kernel
- **ebpf-node**: Binary principal con toda la lógica de negocio

#### `monitoring/`
Stack de observabilidad con Docker Compose:
- **Prometheus**: Time-series database para metrics
- **Grafana**: Dashboards y visualización
- **Loki**: Sistema de logging agregado

#### `tools/`
Herramientas de desarrollo y testing:
- **ebpf-simulation**: Simuladores de ataques y red
- **scripts**: Utilidades de análisis y benchmarking

#### `docs/`
Documentación completa del proyecto:
- **INSTALLATION.md**: Guía paso a paso de instalación
- **ARCHITECTURE.md**: Descripción detallada de arquitectura
- **API.md**: Documentación de endpoints
- **SECURITY.md**: Consideraciones de seguridad

#### `plan_mejora/`
Documentación de planes de mejora:
- Planificación estratégica del proyecto
- Arquitectura de soluciones
- Mejoras progresivas del sistema

#### `scripts/`
Scripts utilitarios para operaciones:
- **deploy.sh**: Automatización de despliegue
- **test_network.sh**: Testing de conectividad
- **simulate_attack.sh**: Simulación de ataques para testing

#### `tests/`
Tests automatizados:
- **integration**: Tests de integración entre módulos
- **e2e**: Tests end-to-end de flujo completo
- **fixtures**: Datos de prueba

---

## 8. Criterios de Calidad

### 8.1 Cobertura de Tests
- **Mínimo requerido**: 80% cobertura de línea
- **Objetivo**: 90%+ cobertura
- **Crítico**: 100% para módulos de seguridad y consenso

### 8.2 Documentación
- **Código**: Todos los elementos públicos documentados
- **API**: Documentación OpenAPI/Swagger
- **Arquitectura**: Diagramas actualizados

### 8.3 Rendimiento
- **Latencia P99**: < 100ms para operaciones críticas
- **Throughput**: > 1000 transacciones/segundo
- **Memory**: < 500MB uso promedio

### 8.4 Seguridad
- **Audit**: Revisión anual de seguridad
- **Dependencies**: Escaneo mensual de vulnerabilidades
- **Secrets**: Nunca en código o versiones

---

## 9. Roadmap de Implementación

### Fase 1: Estabilización (2 semanas)
- [ ] Corregir persistencia RocksDB
- [ ] Fix métricas de peers
- [ ] Configurar red LXC
- [ ] Fix procesos zombie

### Fase 2: Seguridad (2 semanas)
- [ ] Implementar consenso seguro
- [ ] Protección Sybil
- [ ] Replay protection
- [ ] Rate limiting

### Fase 3: Observabilidad (2 semanas)
- [ ] Mejorar métricas
- [ ] Dashboard Grafana
- [ ] Distributed tracing

### Fase 4: Automatización (2 semanas)
- [ ] Playbooks Ansible
- [ ] CI/CD pipeline
- [ ] Testing automatizado

### Fase 5: Documentación (Continua)
- [ ] Documentación completa
- [ ] Ejemplos de uso
- [ ] Guía de troubleshooting

---

## 10. Conclusión

Esta estructura está diseñada para transformar el proyecto de un laboratorio experimental a un PoC serio y presentable. Cada componente está separado por responsabilidades, facilitando el mantenimiento, testing y expansión futura.

La organización modular permite:
- **Desarrollo paralelo**: Múltiples desarrolladores trabajando en módulos independientes
- **Testing aislado**: Tests unitarios para cada módulo
- **Escalabilidad**: Fácil añadir nuevas funcionalidades
- **Mantenibilidad**: Código organizado y documentado

El proyecto está listo para avanzar a las siguientes etapas de implementación.