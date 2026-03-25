# Ejercicios de Monitoreo y Debugging - eBPF Blockchain

Este documento contiene ejercicios prácticos para aprender a extender el proyecto ebpf-blockchain con capacidades de monitoreo, debugging y seguridad adicionales basadas en eBPF.

---

## Ejercicio 1: Contador de Paquetes por Protocolo

### Objetivo
Agregar métricas detalladas por tipo de protocolo (TCP/UDP/ICMP) usando eBPF.

### Implementación

**Paso 1: Agregar mapa de contadores**

```rust
// ebpf-node-ebpf/src/main.rs

#[map]
static PROTOCOL_COUNTERS: PerCPUArray<ProtoStats> = 
    PerCPUArray::with_max_entries(1, 0);

#[derive(Clone, Copy)]
#[repr(C)]
struct ProtoStats {
    tcp_packets: u64,
    tcp_bytes: u64,
    udp_packets: u64,
    udp_bytes: u64,
    icmp_packets: u64,
    icmp_bytes: u64,
    other_packets: u64,
    other_bytes: u64,
}
```

**Paso 2: Modificar función de procesamiento**

```rust
fn process_with_protocol_stats(ctx: &XdpContext) -> Result<u32, ()> {
    let ethhdr: *const EthHdr = unsafe { ptr_at(ctx, 0)? };
    match unsafe { (*ethhdr).ether_type } {
        EtherType::Ipv4 => {}
        _ => return Ok(xdp_action::XDP_PASS),
    }

    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(ctx, ETH_HDR_LEN)? };
    let protocol = unsafe { (*ipv4hdr).protocol };
    let total_len = u16::from_be(unsafe { (*ipv4hdr).total_len }) as u64;

    // Actualizar contador del protocolo correspondiente
    let stats_ptr = unsafe { PROTOCOL_COUNTERS.get_ptr_mut(0) }.ok_or(())?;
    
    match protocol {
        6 => { // TCP
            unsafe {
                (*stats_ptr).tcp_packets += 1;
                (*stats_ptr).tcp_bytes += total_len;
            }
        },
        17 => { // UDP
            unsafe {
                (*stats_ptr).udp_packets += 1;
                (*stats_ptr).udp_bytes += total_len;
            }
        },
        1 => { // ICMP
            unsafe {
                (*stats_ptr).icmp_packets += 1;
                (*stats_ptr).icmp_bytes += total_len;
            }
        },
        _ => {
            unsafe {
                (*stats_ptr).other_packets += 1;
                (*stats_ptr).other_bytes += total_len;
            }
        }
    }

    Ok(xdp_action::XDP_PASS)
}
```

**Paso 3: Exportar métricas a Prometheus**

```rust
// ebpf-node/src/main.rs

use prometheus::{IntCounterVec, IntGaugeVec};

lazy_static! {
    static ref TCP_PACKETS: IntCounterVec = register_int_counter_vec!(
        "ebpf_tcp_packets_total", 
        "TCP packets counter", 
        &["instance"]
    ).unwrap();
    static ref UDP_PACKETS: IntCounterVec = register_int_counter_vec!(
        "ebpf_udp_packets_total",
        "UDP packets counter",
        &["instance"]
    ).unwrap();
    static ref ICMP_PACKETS: IntCounterVec = register_int_counter_vec!(
        "ebpf_icmp_packets_total",
        "ICMP packets counter",
        &["instance"]
    ).unwrap();
}

async fn sync_protocol_stats(ebpf: &mut Ebpf) {
    let stats: PerCPUArray<_, ProtoStats> = 
        PerCPUArray::try_from(ebpf.map("PROTOCOL_COUNTERS").unwrap()).unwrap();
    
    if let Some(ptr) = unsafe { stats.get_ptr_mut(0) } {
        let stats = unsafe { *ptr };
        
        TCP_PACKETS.with_label_values(&["node1"]).inc_by(stats.tcp_packets);
        UDP_PACKETS.with_label_values(&["node1"]).inc_by(stats.udp_packets);
        ICMP_PACKETS.with_label_values(&["node1"]).inc_by(stats.icmp_packets);
    }
}
```

### Verificación

```bash
# Compilar
cargo build

# Ejecutar
RUST_LOG=info ./target/debug/ebpf-node --iface eth0

# En otro terminal, generar tráfico
ping -c 100 192.168.2.210
curl http://192.168.2.210:9090/metrics | grep -E "tcp|udp|icmp"

# Ver en Grafana
# http://localhost:3000
# Query: rate(ebpf_tcp_packets_total[1m])
```

---

## Ejercicio 2: Latencia End-to-End con Timestamps

### Objetivo
Medir la latencia desde que un paquete entra a la NIC hasta que es procesado por la aplicación.

### Implementación

**Paso 1: Crear mapa de timestamps**

```rust
// ebpf-node-ebpf/src/main.rs

#[map]
static PACKET_TIMESTAMPS: HashMap<u64, u64> = 
    HashMap::with_max_entries(10240, BPF_F_NO_PREALLOC);

#[map]
static LATENCY_HISTOGRAM: PerCPUArray<u64> = 
    PerCPUArray::with_max_entries(64, 0);
```

**Paso 2: XDP captura timestamp de entrada**

```rust
#[xdp]
pub fn ebpf_node(ctx: XdpContext) -> u32 {
    let skb = ctx.skb();
    let skb_ptr = skb as u64;
    let now = unsafe { bpf_ktime_get_ns() };
    
    // Solo procesar TCP
    let ethhdr: *const EthHdr = unsafe { ptr_at(&ctx, 0).ok()? };
    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(&ctx, ETH_HDR_LEN).ok()? };
    let protocol = unsafe { (*ipv4hdr).protocol };
    
    if protocol == 6 { // TCP
        unsafe { 
            PACKET_TIMESTAMPS.insert(&skb_ptr, &now, 0).ok(); 
        }
    }
    
    match try_ebpf_node(ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}
```

**Paso 3: KProbe captura timestamp de salida**

```rust
#[kprobe]
pub fn tcp_send_skb(ctx: ProbeContext) -> u32 {
    let _ = try_tcp_send_skb(ctx);
    0
}

fn try_tcp_send_skb(ctx: ProbeContext) -> Result<(), ()> {
    let skb_ptr: u64 = ctx.arg(0).ok_or(())?;
    let now = unsafe { bpf_ktime_get_ns() };
    
    if let Some(start_time) = unsafe { PACKET_TIMESTAMPS.get(&skb_ptr).copied() } {
        let latency = now - start_time;
        
        // Calcular bucket (power of 2)
        let bucket = 64 - latency.leading_zeros() as u64;
        
        let hist_ptr = unsafe { LATENCY_HISTOGRAM.get_ptr_mut(bucket as u32) }.ok_or(())?;
        unsafe { *hist_ptr += 1 };
        
        // Limpiar timestamp
        let _ = unsafe { PACKET_TIMESTAMPS.remove(&skb_ptr) };
    }
    
    Ok(())
}
```

**Paso 4: Exportar histograma a Prometheus**

```rust
// ebpf-node/src/main.rs

lazy_static! {
    static ref LATENCY_HIST: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_packet_latency_us",
        "Packet latency in microseconds",
        &["bucket"]
    ).unwrap();
}

async fn sync_latency_stats(ebpf: &mut Ebpf) {
    let hist: PerCPUArray<_, u64> = 
        PerCPUArray::try_from(ebpf.map("LATENCY_HISTOGRAM").unwrap()).unwrap();
    
    for bucket in 0..64 {
        if let Some(ptr) = unsafe { hist.get_ptr_mut(bucket) } {
            let value = unsafe { *ptr };
            if value > 0 {
                LATENCY_HIST.with_label_values(&[&bucket.to_string()]).set(value as i64);
            }
        }
    }
}
```

### Verificación

```bash
# Generar tráfico TCP
iperf3 -c 192.168.2.210 -t 60

# Ver latencia
curl http://192.168.2.210:9090/metrics | grep latency

# Calcular percentiles
# P50 = bucket donde sum(counts) >= 0.5 * total
# P99 = bucket donde sum(counts) >= 0.99 * total
```

---

## Ejercicio 3: Detección de Port Scanning

### Objetivo
Detectar patrones de port scanning usando eBPF para identificar ataques.

### Implementación

```rust
// ebpf-node-ebpf/src/main.rs

#[map]
static SCAN_DETECTOR: HashMap<u32, ScanState> = 
    HashMap::with_max_entries(10240, BPF_F_NO_PREALLOC);

#[derive(Clone, Copy)]
#[repr(C)]
struct ScanState {
    ports_scanned: u32,      // Número de puertos únicos
    last_port: u16,          // Último puerto escaneado
    first_seen: u64,         // Timestamp inicial
    last_seen: u64,          // Timestamp del último intento
    syn_count: u32,          // Count de SYN sin respuesta
}

const SCAN_THRESHOLD: u32 = 20;  // Puertos en 10 segundos = sospechoso
const TIME_WINDOW: u64 = 10_000_000_000; // 10 segundos en ns

#[xdp]
pub fn scan_detector(ctx: XdpContext) -> u32 {
    let _ = try_scan_detector(ctx);
    match try_ebpf_node(ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

fn try_scan_detector(ctx: XdpContext) -> Result<(), ()> {
    let ethhdr: *const EthHdr = unsafe { ptr_at(&ctx, 0)? };
    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(&ctx, ETH_HDR_LEN)? };
    
    let src_ip = unsafe { (*ipv4hdr).src_addr };
    let dst_ip = unsafe { (*ipv4hdr).dst_addr };
    let protocol = unsafe { (*ipv4hdr).protocol };
    
    if protocol != 6 { // Solo TCP
        return Ok(());
    }
    
    // Extraer puerto destino (TCP header)
    let tcp_hdr: *const TcpHdr = unsafe { ptr_at(&ctx, ETH_HDR_LEN + 20)? };
    let dst_port = u16::from_be(unsafe { (*tcp_hdr).dest });
    
    let now = unsafe { bpf_ktime_get_ns() };
    let mut state = unsafe { SCAN_DETECTOR.get(&src_ip).copied() }
        .unwrap_or(ScanState {
            ports_scanned: 0,
            last_port: 0,
            first_seen: now,
            last_seen: now,
            syn_count: 0,
        });
    
    // Verificar si es SYN
    let syn_flag = (unsafe { (*tcp_hdr).data_offset } & 0x02) != 0;
    
    if syn_flag {
        state.syn_count += 1;
    }
    
    // Detectar nuevo puerto
    if dst_port != state.last_port {
        state.ports_scanned += 1;
        state.last_port = dst_port;
    }
    
    state.last_seen = now;
    
    // Detectar si es scanning
    if state.ports_scanned >= SCAN_THRESHOLD {
        // Reset y reportar
        let elapsed = now - state.first_seen;
        if elapsed < TIME_WINDOW {
            // SCAN DETECTED! Emitir evento
            emit_scan_alert(src_ip, state.ports_scanned, elapsed);
        }
        state.ports_scanned = 0;
        state.first_seen = now;
    }
    
    unsafe { SCAN_DETECTOR.insert(&src_ip, &state, 0) };
    
    Ok(())
}

fn emit_scan_alert(src_ip: u32, ports: u32, elapsed: u64) {
    #[map]
    static ALERTS: RingBuf<ScanAlert> = RingBuf::with_size(256);
    
    #[repr(C)]
    struct ScanAlert {
        timestamp: u64,
        src_ip: u32,
        ports_scanned: u32,
        duration_ns: u64,
    }
    
    let alert = ScanAlert {
        timestamp: unsafe { bpf_ktime_get_ns() },
        src_ip,
        ports_scanned: ports,
        duration_ns: elapsed,
    };
    
    unsafe { ALERTS.output(&alert, 0) };
}
```

### Verificación

```bash
# Simular port scan (desde otra máquina)
nmap -sT -p 1-100 192.168.2.210

# Ver alertas
sudo bpftool map dump name SCAN_DETECTOR

# Ver eventos en user space
# (implementar consumer de RingBuf en main.rs)
```

---

## Ejercicio 4: Top Talkers - IPs con Más Tráfico

### Objetivo
Mantener un ranking de las IPs que generan más tráfico.

### Implementación

```rust
// ebpf-node-ebpf/src/main.rs

#[map]
static TOP_TALKERS: PerCPUHashMap<u32, TrafficStats> = 
    PerCPUHashMap::with_max_entries(256, 0);

#[derive(Clone, Copy)]
#[repr(C)]
struct TrafficStats {
    packets_in: u64,
    bytes_in: u64,
    packets_out: u64,
    bytes_out: u64,
}

#[xdp]
pub fn traffic_tracker(ctx: XdpContext) -> u32 {
    let _ = try_traffic_tracker(ctx);
    match try_ebpf_node(ctx) {
        Ok(ret) => ret,
        Err(_) => xpf_action::XDP_ABORTED,
    }
}

fn try_traffic_tracker(ctx: XdpContext) -> Result<(), ()> {
    let ethhdr: *const EthHdr = unsafe { ptr_at(&ctx, 0)? };
    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(&ctx, ETH_HDR_LEN)? };
    
    let src_ip = unsafe { (*ipv4hdr).src_addr };
    let dst_ip = unsafe { (*ipv4hdr).dst_addr };
    let total_len = u16::from_be(unsafe { (*ipv4hdr).total_len }) as u64;
    
    // Incrementar contador de entrada para destino
    let dst_stats = unsafe { TOP_TALKERS.get_ptr_mut(src_ip) }.ok_or(())?;
    unsafe { 
        (*dst_stats).packets_in += 1;
        (*dst_stats).bytes_in += total_len;
    }
    
    // Incrementar contador de salida para fuente
    let src_stats = unsafe { TOP_TALKERS.get_ptr_mut(dst_ip) }.ok_or(())?;
    unsafe { 
        (*src_stats).packets_out += 1;
        (*src_stats).bytes_out += total_len;
    }
    
    Ok(())
}
```

### Verificación

```bash
# Generar tráfico diverso
for i in {1..10}; do
    curl http://192.168.2.210:$((8000 + i)) &
done

# Ver top talkers
# Implementar lectura periódica en user space:
while true; do
    clear
    curl -s http://192.168.2.210:9090/metrics | grep top_talkers
    sleep 5
done
```

---

## Ejercicio 5: Packet Capture con Filtering

### Objetivo
Implementar captura selectiva de paquetes para debugging.

### Implementación

```rust
// ebpf-node-ebpf/src/main.rs

#[map]
static CAPTURE_FILTER: HashMap<u32, CaptureRule> = 
    HashMap::with_max_entries(64, BPF_F_NO_PREALLOC);

#[map]
static PACKET_CAPTURE: RingBuf<CapturedPacket> = 
    RingBuf::with_size(4096);

#[derive(Clone, Copy)]
#[repr(C)]
struct CaptureRule {
    ip_prefix: u32,
    prefix_len: u8,
    protocol: u8,         // 0 = any, 6 = TCP, 17 = UDP
    port: u16,            // 0 = any
    enabled: u8,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct CapturedPacket {
    timestamp: u64,
    src_ip: u32,
    dst_ip: u32,
    src_port: u16,
    dst_port: u16,
    protocol: u8,
    length: u16,
    payload_preview: [u8; 32],
}

fn should_capture(src_ip: u32, dst_ip: u32, protocol: u8, port: u16) -> bool {
    // Iterar sobre reglas (simplificado - en producción usar arraymap)
    let mut i = 0u32;
    loop {
        if i >= 64 { break; }
        
        let rule = unsafe { CAPTURE_FILTER.get(&i).copied() };
        if let Some(r) = rule {
            if r.enabled == 0 {
                i += 1;
                continue;
            }
            
            // Verificar si la IP matchea
            let masked_src = src_ip & (u32::MAX << (32 - r.prefix_len));
            if masked_src != r.ip_prefix {
                i += 1;
                continue;
            }
            
            // Verificar protocolo
            if r.protocol != 0 && r.protocol != protocol {
                i += 1;
                continue;
            }
            
            // Verificar puerto
            if r.port != 0 && r.port != port {
                i += 1;
                continue;
            }
            
            // Match! Capturar
            return true;
        }
        i += 1;
    }
    false
}

fn capture_packet(ctx: &XdpContext, src_ip: u32, dst_ip: u32, 
                  protocol: u8, src_port: u16, dst_port: u16) -> Result<(), ()> {
    
    let data = ctx.data();
    let data_end = ctx.data_end();
    let len = (data_end - data) as u16;
    
    let mut pkt = CapturedPacket {
        timestamp: unsafe { bpf_ktime_get_ns() },
        src_ip,
        dst_ip,
        src_port,
        dst_port,
        protocol,
        length: len.min(32), // Preview size
        payload_preview: [0; 32],
    };
    
    // Copiar preview del payload
    let payload_len = (len as usize).min(32);
    let payload_start = ETH_HDR_LEN + 20 + 4; // Eth + IP + TCP offset
    for i in 0..payload_len {
        let offset = payload_start + i;
        if offset < data_end {
            pkt.payload_preview[i] = unsafe { *((data + offset) as *const u8) };
        }
    }
    
    unsafe { PACKET_CAPTURE.output(&pkt, 0) };
    
    Ok(())
}
```

### Verificación

```bash
# Agregar regla de captura (desde user space)
# Implementar CLI para agregar reglas

# Simular tráfico
curl http://192.168.2.210:9090/metrics

# Ver capturas
# Consumir RingBuf en user space
```

---

## Ejercicio 6: Connection Tracking

### Objetivo
Mantener estado de conexiones TCP para análisis de flujo.

### Implementación

```rust
// ebpf-node-ebpf/src/main.rs

#[map]
static CONN_TRACK: HashMap<ConnectionKey, ConnectionState> = 
    HashMap::with_max_entries(8192, BPF_F_NO_PREALLOC);

#[derive(Clone, Copy)]
#[repr(C)]
struct ConnectionKey {
    src_ip: u32,
    dst_ip: u32,
    src_port: u16,
    dst_port: u16,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct ConnectionState {
    established: u8,
    packets_forward: u64,
    packets_backward: u64,
    bytes_forward: u64,
    bytes_backward: u64,
    start_time: u64,
    last_time: u64,
    flags: u32,
}

const TCP_SYN: u8 = 0x02;
const TCP_SYN_ACK: u8 = 0x12;
const TCP_ACK: u8 = 0x10;
const TCP_FIN: u8 = 0x01;
const TCP_RST: u8 = 0x04;

fn process_connection(ctx: &XdpContext) -> Result<(), ()> {
    let ethhdr: *const EthHdr = unsafe { ptr_at(ctx, 0)? };
    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(ctx, ETH_HDR_LEN)? };
    
    let src_ip = unsafe { (*ipv4hdr).src_addr };
    let dst_ip = unsafe { (*ipv4hdr).dst_addr };
    let total_len = u16::from_be(unsafe { (*ipv4hdr).total_len }) as u64;
    
    let tcp_hdr: *const TcpHdr = unsafe { ptr_at(ctx, ETH_HDR_LEN + 20)? };
    let src_port = u16::from_be(unsafe { (*tcp_hdr).source });
    let dst_port = u16::from_be(unsafe { (*tcp_hdr).dest });
    let flags = unsafe { (*tcp_hdr).data_offset } & 0x3F;
    
    let now = unsafe { bpf_ktime_get_ns() };
    
    // Crear key en ambas direcciones
    let mut key_fwd = ConnectionKey { src_ip, dst_ip, src_port, dst_port };
    let mut key_rev = ConnectionKey { src_ip: dst_ip, dst_ip: src_ip, 
                                      src_port: dst_port, dst_port: src_port };
    
    let mut state = unsafe { CONN_TRACK.get(&key_fwd).copied() }
        .unwrap_or(ConnectionState {
            established: 0,
            packets_forward: 0,
            packets_backward: 0,
            bytes_forward: 0,
            bytes_backward: 0,
            start_time: now,
            last_time: now,
            flags: 0,
        });
    
    // Procesar flags TCP
    if flags & TCP_SYN != 0 && flags & TCP_ACK == 0 {
        // SYN - nueva conexión
        state = ConnectionState {
            established: 1,
            start_time: now,
            last_time: now,
            ..Default::default()
        };
    } else if flags & TCP_SYN != 0 && flags & TCP_ACK != 0 {
        // SYN-ACK - conexión establecida
        state.established = 2;
    } else if flags & TCP_FIN != 0 || flags & TCP_RST != 0 {
        // FIN o RST - conexión cerrada
        state.established = 0;
    }
    
    // Actualizar contadores
    state.packets_forward += 1;
    state.bytes_forward += total_len;
    state.last_time = now;
    state.flags = flags as u32;
    
    unsafe { CONN_TRACK.insert(&key_fwd, &state, 0) };
    
    // Actualizar reverse direction
    if let Some(mut rev_state) = unsafe { CONN_TRACK.get(&key_rev).copied() } {
        rev_state.packets_backward += 1;
        rev_state.bytes_backward += total_len;
        rev_state.last_time = now;
        unsafe { CONN_TRACK.insert(&key_rev, &rev_state, 0) };
    }
    
    Ok(())
}

impl Default for ConnectionState {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
```

---

## Ejercicio 7: Prometheus Exporter Completo

### Objetivo
Crear un exporter de Prometheus que exponga todas las métricas收集.

### Implementación

```rust
// ebpf-node/src/main.rs

use prometheus::{
    Encoder, TextEncoder,
    IntCounterVec, IntGaugeVec, HistogramVec, Histogram,
};

lazy_static! {
    // Counters básicos
    static ref PACKETS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "ebpf_packets_total",
        "Total packets processed",
        &["direction", "action"]
    ).unwrap();
    
    // Gauges para mapas
    static ref BLACKLIST_SIZE: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_blacklist_entries",
        "Number of entries in blacklist",
        &["map"]
    ).unwrap();
    
    // Histograms para latencia
    static ref PACKET_LATENCY: HistogramVec = register_histogram_vec!(
        "ebpf_packet_latency_seconds",
        "Packet processing latency",
        &["protocol"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
    ).unwrap();
    
    // Métricas de conexión
    static ref CONNECTIONS_ACTIVE: IntGaugeVec = register_int_gauge_vec!(
        "ebpf_connections_active",
        "Active TCP connections",
        &["state"]
    ).unwrap();
}

async fn metrics_handler() -> String {
    // Sincronizar métricas desde eBPF maps
    sync_ebpf_metrics().await;
    
    // Generar output de Prometheus
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

async fn sync_ebpf_metrics() {
    // Esta función sincronizaría todas las métricas desde los mapas eBPF
    // Por ahora es un placeholder
    
    // Ejemplo: Leer blacklist size
    if let Ok(blacklist) = LpmTrie::<_, u32, u32>::try_from(
        ebpf.map("NODES_BLACKLIST").unwrap()
    ) {
        let count = blacklist.iter().count() as i64;
        BLACKLIST_SIZE.with_label_values(&["nodes"]).set(count);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... setup ...
    
    // Iniciar servidor de métricas
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(|| async { "OK" }));
    
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await.unwrap();
        axum::serve(listener, app).await;
    });
    
    // Loop de sync
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        sync_ebpf_metrics().await;
    }
}
```

---

## Ejercicio 8: Alertas y Notificaciones

### Objetivo
Implementar sistema de alertas cuando se detectan anomalías.

### Implementación

```rust
// ebpf-node/src/main.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

struct AlertManager {
    alerts: Arc<RwLock<HashMap<String, Alert>>>,
    thresholds: HashMap<String, Threshold>,
}

struct Alert {
    name: String,
    severity: Severity,
    message: String,
    count: u64,
    first_seen: u64,
    last_seen: u64,
}

enum Severity { Low, Medium, High, Critical }

struct Threshold {
    metric: String,
    operator: Op,
    value: f64,
    window_secs: u64,
}

enum Op { Gt, Lt, Eq }

impl AlertManager {
    async fn check_and_alert(&self, metric: &str, value: f64) {
        if let Some(threshold) = self.thresholds.get(metric) {
            let triggered = match threshold.operator {
                Op::Gt => value > threshold.value,
                Op::Lt => value < threshold.value,
                Op::Eq => (value - threshold.value).abs() < 0.001,
            };
            
            if triggered {
                self.emit_alert(metric, value).await;
            }
        }
    }
    
    async fn emit_alert(&self, metric: &str, value: f64) {
        let alert = Alert {
            name: format!("alert_{}", metric),
            severity: Severity::High,
            message: format!("Metric {} exceeded threshold: {}", metric, value),
            count: 1,
            first_seen: now(),
            last_seen: now(),
        };
        
        let mut alerts = self.alerts.write().await;
        alerts.insert(alert.name.clone(), alert);
        
        // Notificar via Gossipsub
        notify_peers(&alert).await;
    }
}

// Configuración de umbrales
fn default_thresholds() -> HashMap<String, Threshold> {
    let mut t = HashMap::new();
    t.insert("port_scan".to_string(), Threshold {
        metric: "ports_scanned".to_string(),
        operator: Op::Gt,
        value: 20.0,
        window_secs: 10,
    });
    t.insert("high_latency".to_string(), Threshold {
        metric: "p99_latency_ms".to_string(),
        operator: Op::Gt,
        value: 100.0,
        window_secs: 60,
    });
    t
}
```

---

## Soluciones Paso a Paso

### Solución Ejercicio 1: Contador de Protocolos

```rust
// Archivo: ebpf-node-ebpf/src/main.rs
// Agregar después de los mapas existentes:

#[map]
static PROTOCOL_COUNTERS: PerCPUArray<ProtoStats> = 
    PerCPUArray::with_max_entries(1, 0);

#[derive(Clone, Copy)]
#[repr(C)]
struct ProtoStats {
    tcp_packets: u64,
    tcp_bytes: u64,
    udp_packets: u64,
    udp_bytes: u64,
    icmp_packets: u64,
    icmp_bytes: u64,
    other_packets: u64,
    other_bytes: u64,
}

impl Default for ProtoStats {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
```

```rust
// En try_ebpf_node(), agregar después de verificar EtherType::Ipv4:

// Actualizar contador de protocolo
if let Some(stats_ptr) = unsafe { PROTOCOL_COUNTERS.get_ptr_mut(0) } {
    match protocol {
        6 => {
            unsafe { (*stats_ptr).tcp_packets += 1 };
            unsafe { (*stats_ptr).tcp_bytes += total_len as u64 };
        },
        17 => {
            unsafe { (*stats_ptr).udp_packets += 1 };
            unsafe { (*stats_ptr).udp_bytes += total_len as u64 };
        },
        1 => {
            unsafe { (*stats_ptr).icmp_packets += 1 };
            unsafe { (*stats_ptr).icmp_bytes += total_len as u64 };
        },
        _ => {
            unsafe { (*stats_ptr).other_packets += 1 };
            unsafe { (*stats_ptr).other_bytes += total_len as u64 };
        }
    }
}
```

---

## Comandos de Verificación

```bash
# Compilar proyecto modificado
cargo build 2>&1 | head -50

# Ver errores de compilación
cargo build 2>&1 | grep "error"

# Ver warnings
cargo build 2>&1 | grep "warning"

# Ejecutar con logs detallados
RUST_LOG=debug ./target/debug/ebpf-node --iface eth0

# Ver métricas en Prometheus
curl -s localhost:9090/metrics | grep ebpf

# Ver alertas en Grafana
# Dashboard: http://localhost:3000/d/ebpf-alerts
```

---

## Referencias para Profundizar

- [AYA Maps Documentation](https://aya-rs.dev/aya/maps/)
- [Prometheus Exporter Best Practices](https://prometheus.io/docs/instrumenting/writing_exporters/)
- [eBPF Performance Tools](https://www.brendangregg.com/ebpf.html)
- [TCP Connection Tracking in eBPF](https://github.com/iovisor/bcc/blob/master/docs/reference_guide.md)
