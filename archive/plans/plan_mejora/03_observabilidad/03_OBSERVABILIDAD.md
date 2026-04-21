# ETAPA 3: OBSERVABILIDAD

**Estado:** Pendiente  
**Duración estimada:** 2 semanas  
**Prioridad:** 📊 MEDIA  
**Meta:** Alcanzar 100% de métricas implementadas y documentadas

---

## 1. Resumen Ejecutivo

Esta etapa se enfoca en mejorar el sistema de observabilidad para producción, implementando un stack completo de monitoreo con Prometheus, Grafana y Loki. Actualmente se tiene el 80% de completitud en métricas, pero se requiere llegar al 100% con documentación completa y dashboards funcionales.

### Métricas Actuales vs. Objetivo

| Área | Estado Actual | Meta PoC | Crítica |
|------|---------------|----------|---------|
| Métricas completas | 80% | 100% | 🟠 |
| Dashboards Grafana | 0% | 100% | 🔴 |
| Logging estructurado | 50% | 100% | 🟠 |
| Distributed tracing | 0% | 100% | 🟡 |
| Alertas configuradas | 0% | 100% | 🟠 |

---

## 2. Problemas de Observabilidad Identificados

### 2.1 Métricas Incompletas (80% completitud)

**Problema:** Los peers y mensajes no se reportan correctamente en Prometheus.

**Impacto:**
- Métricas incompletas de red
- Dificultad para monitorear el estado de la red
- Falta de visibilidad en el tráfico P2P
- Imposibilidad de detectar problemas de performance

**Ubicación del código:**
```
ebpf-node/src/metrics/
ebpf-node/src/metrics/prometheus.rs
```

**Requisitos:**
- Corregir contadores de peers
- Implementar métricas de mensajes por tipo
- Agregar métricas de latencia por peer
- Documentar todas las métricas expuestas

### 2.2 Ausencia de Dashboards (0% completitud)

**Problema:** No existen dashboards de Grafana para visualizar las métricas.

**Impacto:**
- Dificultad para monitorear el estado del sistema
- Requiere consultar Prometheus manualmente
- No hay visibilidad unificada del sistema

**Requisitos:**
- Crear dashboards para cada componente
- Dashboard unificado de salud del sistema
- Alertas configuradas en Grafana

### 2.3 Logging No Estructurado (50% completitud)

**Problema:** Los logs no están estructurados y centralizados.

**Impacto:**
- Dificultad para buscar y analizar logs
- No hay correlación entre logs de diferentes componentes
- Imposibilidad de hacer query sobre historial de logs

**Requisitos:**
- Implementar logging estructurado (JSON)
- Configurar Loki para centralización
- Agregar correlation IDs para tracing

### 2.4 Ausencia de Distributed Tracing (0% completitud)

**Problema:** No hay seguimiento de requests a través del sistema.

**Impacto:**
- Dificultad para diagnosticar problemas de performance
- Imposibilidad de identificar cuellos de botella
- Falta de visibilidad en flujos de trabajo complejos

**Requisitos:**
- Implementar OpenTelemetry
- Configurar Jaeger o Tempo
- Agregar spans para operaciones críticas

---

## 3. Soluciones de Observabilidad Propuestas

### 3.1 Stack de Observabilidad Completo

```
┌─────────────────────────────────────────────────────────┐
│                    Grafana                              │
│              (Visualización y Alertas)                   │
└─────────────────────┬───────────────────────────────────┘
                      │
         ┌────────────┼────────────┐
         │            │            │
    ┌────▼────┐  ┌───▼───┐  ┌────▼────┐
    │Prometheus│  │ Loki  │  │ Tempo   │
    │ Métricas │  │ Logs  │  │ Tracing │
    └────┬────┘  └───┬───┘  └────┬────┘
         │           │           │
    ┌────▼───────────▼───────────▼────┐
    │         eBPF Blockchain Node    │
    │   (OpenTelemetry Exporter)      │
    └─────────────────────────────────┘
```

### 3.2 Métricas Prometheus

**Categorías de métricas:**

#### 3.2.1 Métricas de Red

```rust
// ebpf-node/src/metrics/network.rs
use prometheus::{IntGauge, Histogram, Counter};

pub struct NetworkMetrics {
    pub peers_connected: IntGauge,
    pub peers_connected_by_transport: IntGaugeVec,
    pub messages_sent: Counter,
    pub messages_received: Counter,
    pub messages_by_type: CounterVec,
    pub network_latency: Histogram,
    pub bandwidth_used: Counter,
}
```

#### 3.2.2 Métricas de Consenso

```rust
// ebpf-node/src/metrics/consensus.rs
pub struct ConsensusMetrics {
    pub blocks_proposed: Counter,
    pub blocks_validated: Counter,
    pub consensus_rounds: Counter,
    pub consensus_duration: Histogram,
    pub validator_count: IntGauge,
    pub stake_total: IntGauge,
    pub slashing_events: Counter,
}
```

#### 3.2.3 Métricas de Transacciones

```rust
// ebpf-node/src/metrics/transaction.rs
pub struct TransactionMetrics {
    pub transactions_processed: Counter,
    pub transactions_by_type: CounterVec,
    pub transaction_latency: Histogram,
    pub transaction_size: Histogram,
    pub transaction_queue_size: IntGauge,
    pub transaction_failures: Counter,
}
```

#### 3.2.4 Métricas de eBPF

```rust
// ebpf-node/src/metrics/ebpf.rs
pub struct EbpfMetrics {
    pub xdp_packets_processed: Counter,
    pub xdp_packets_dropped: Counter,
    pub xdp_blacklist_size: IntGauge,
    pub xdp_whitelist_size: IntGauge,
    pub kprobe_latencies: HistogramVec,
    pub ebpf_errors: Counter,
}
```

#### 3.2.5 Métricas del Sistema

```rust
// ebpf-node/src/metrics/system.rs
pub struct SystemMetrics {
    pub memory_usage_bytes: Gauge,
    pub cpu_usage_percent: Gauge,
    pub disk_usage_bytes: Gauge,
    pub uptime_seconds: IntGauge,
    pub thread_count: IntGauge,
    pub file_descriptor_count: IntGauge,
}
```

### 3.3 Dashboards de Grafana

#### 3.3.1 Dashboard Unificado de Salud

**Secciones:**
- Resumen general del sistema
- Estado de red P2P
- Performance de consenso
- Métricas de transacciones
- Alertas en tiempo real

**Configuración:**
```json
{
  "dashboard": {
    "title": "eBPF Blockchain - Health Overview",
    "panels": [
      {
        "title": "Resumen del Sistema",
        "type": "stat",
        "targets": [
          {"expr": "ebpf_node_uptime_seconds"},
          {"expr": "ebpf_peers_connected"},
          {"expr": "ebpf_blocks_proposed_total"}
        ]
      },
      {
        "title": "Red P2P",
        "type": "graph",
        "targets": [
          {"expr": "rate(ebpf_messages_sent[5m])"},
          {"expr": "rate(ebpf_messages_received[5m])"}
        ]
      }
    ]
  }
}
```

#### 3.3.2 Dashboard de Red P2P

**Métricas principales:**
- Número de peers conectados
- Métricas por transporte (QUIC, TCP)
- Latencia de red
- Throughput de mensajes
- Conexiones activas

#### 3.3.3 Dashboard de Consenso

**Métricas principales:**
- Bloques propuestos por validador
- Tiempo de consenso
- Stake total
- Eventos de slashing
- Eficiencia del consenso

#### 3.3.4 Dashboard de Transacciones

**Métricas principales:**
- Transacciones procesadas por segundo
- Latencia de transacciones
- Tasa de éxito/fracaso
- Tamaño de transacciones
- Colas de transacciones pendientes

### 3.4 Logging con Loki

#### 3.4.1 Formato de Logs Estructurados

```rust
// ebpf-node/src/logging.rs
use tracing::{info, error, warn};
use tracing_subscriber::{fmt, EnvFilter};

pub fn setup_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .with_target(false)
        .with_level(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .init();
}

// Ejemplo de log estructurado
#[derive(Serialize)]
struct LogEvent {
    timestamp: u64,
    level: String,
    message: String,
    component: String,
    correlation_id: String,
    metadata: HashMap<String, String>,
}
```

#### 3.4.2 Configuración de Loki

```yaml
# monitoring/loki/loki-config.yml
server:
  http_listen_port: 3100
  grpc_listen_port: 9096

common:
  instance_addr: 127.0.0.1
  path_prefix: /loki
  storage:
    filesystem:
      chunks_directory: /loki/chunks
      rules_directory: /loki/rules
  replication_factor: 1
  ring:
    kvstore:
      store: inmemory

query_range:
  results_cache:
    cache:
      embedded_cache:
        max_size_mb: 250

ruler:
  alertmanager_url: http://localhost:9093

schema_config:
  configs:
    - from: 2024-01-01
      store: tsdb
      object_store: filesystem
      schema: v13
      index:
        prefix: index_
        period: 24h
```

### 3.5 Distributed Tracing con Tempo

#### 3.5.1 Configuración de OpenTelemetry

```rust
// ebpf-node/src/tracing.rs
use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;
use opentelemetry_tempo::TempoPipelineBuilder;

pub fn setup_tracing() {
    let provider = TempoPipelineBuilder::new()
        .with_endpoint("http://localhost:4318")
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .unwrap();

    let tracer = provider.tracer("ebpf-blockchain");
    global::set_tracer_provider(provider);
    
    tracing::subscriber::set_global_default(
        tracing_opentelemetry::layer().with_tracer(tracer)
    ).unwrap();
}

// Ejemplo de span
#[tracing::instrument(skip(state), fields(block_number = tx.block_number))]
pub async fn process_transaction(
    tx: Transaction,
    state: &mut State,
) -> Result<TransactionResult> {
    // Operación de procesamiento
    tracing::debug!("Processing transaction");
    // ...
}
```

#### 3.5.2 Configuración de Tempo

```yaml
# monitoring/tempo/tempo-config.yml
server:
  http_listen_port: 3200

distributor:
  receivers:
    jaeger:
      protocols:
        thrift_http:
        grpc:
    otlp:
      protocols:
        http:
        grpc

storage:
  trace:
    backend: local
    wal:
      path: /tmp/tempo/wal
    local:
      path: /tmp/tempo/blocks
```

---

## 4. Plan de Implementación

### 4.1 Semana 1: Métricas y Dashboards

#### Día 1-2: Corregir y Completar Métricas

**Tareas:**
1. Corregir contadores de peers en network metrics
2. Implementar métricas de mensajes por tipo
3. Agregar métricas de latencia por peer
4. Documentar todas las métricas expuestas

**Código objetivo:**
```rust
// ebpf-node/src/metrics/network.rs
pub struct NetworkMetrics {
    // Corrección de contadores
    pub peers_connected: IntGauge,
    pub peers_connected_by_transport: IntGaugeVec,
    
    // Nuevas métricas
    pub messages_sent: Counter,
    pub messages_received: Counter,
    pub messages_by_type: CounterVec,  // Tipo: message, ping, pong, etc.
    pub network_latency: Histogram,
    pub latency_by_peer: HistogramVec,
    pub bandwidth_used: Counter,
}

impl NetworkMetrics {
    pub fn record_message_sent(&self, msg_type: &str) {
        self.messages_sent.inc();
        self.messages_by_type
            .with_label_values(&[msg_type])
            .inc();
    }
    
    pub fn record_latency(&self, peer_id: &str, latency_ms: f64) {
        self.network_latency.observe(latency_ms);
        self.latency_by_peer
            .with_label_values(&[peer_id])
            .observe(latency_ms);
    }
}
```

**Criterios de aceptación:**
- [ ] Todos los peers contados correctamente
- [ ] Mensajes categorizados por tipo
- [ ] Latencia medida por peer
- [ ] Documentación completa en OpenAPI

#### Día 3-4: Crear Dashboards de Grafana

**Tareas:**
1. Diseñar dashboard unificado de salud
2. Crear dashboard de red P2P
3. Crear dashboard de consenso
4. Configurar alertas básicas

**Archivos:**
```
monitoring/grafana/dashboards/
├── health-overview.json
├── network-p2p.json
├── consensus.json
├── transactions.json
└── system.json
```

**Ejemplo de dashboard:**
```json
{
  "dashboard": {
    "title": "eBPF Blockchain - Network P2P",
    "panels": [
      {
        "title": "Peers Conectados",
        "type": "gauge",
        "targets": [
          {
            "expr": "ebpf_peers_connected",
            "legendFormat": "Peers"
          }
        ],
        "options": {
          "min": 0,
          "max": 100,
          "thresholds": {
            "steps": [
              {"color": "green", "value": null},
              {"color": "yellow", "value": 10},
              {"color": "red", "value": 5}
            ]
          }
        }
      },
      {
        "title": "Throughput de Mensajes",
        "type": "timeseries",
        "targets": [
          {
            "expr": "rate(ebpf_messages_sent[5m])",
            "legendFormat": "Enviados"
          },
          {
            "expr": "rate(ebpf_messages_received[5m])",
            "legendFormat": "Recibidos"
          }
        ]
      }
    ]
  }
}
```

**Criterios de aceptación:**
- [ ] Dashboard unificado de salud funcionando
- [ ] Dashboard de red P2P con todas las métricas
- [ ] Dashboard de consenso mostrando validadores
- [ ] 5+ alertas configuradas

#### Día 5-6: Configurar Alertas

**Tareas:**
1. Definir reglas de alerta en Prometheus
2. Configurar Notifier de Grafana
3. Implementar notificaciones por email/slack
4. Pruebas de alertas

**Configuración de alertas:**
```yaml
# monitoring/prometheus/alerts.yml
groups:
  - name: ebpf_blockchain_alerts
    rules:
      - alert: HighPeerCount
        expr: ebpf_peers_connected > 50
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Alto número de peers conectados"
          description: "El nodo tiene {{ $value }} peers conectados"
      
      - alert: HighLatency
        expr: histogram_quantile(0.95, rate(ebpf_network_latency_bucket[5m])) > 1000
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Alta latencia de red"
          description: "Latencia al 95th percentile: {{ $value }}ms"
      
      - alert: PeerDisconnectionRate
        expr: rate(ebpf_peers_disconnected_total[5m]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Alta tasa de desconexión de peers"
      
      - alert: ConsensusSlow
        expr: histogram_quantile(0.95, rate(ebpf_consensus_duration_bucket[5m])) > 10000
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Consenso lento"
          description: "Tiempo de consenso: {{ $value }}ms"
```

**Criterios de aceptación:**
- [ ] 10+ alertas configuradas
- [ ] Notificaciones funcionando
- [ ] Alertas críticas vs warnings diferenciadas
- [ ] Pruebas de alerta exitosas

#### Día 7: Pruebas y Documentación

**Tareas:**
1. Pruebas integrales de métricas
2. Documentar todas las métricas
3. Crear guía de uso de dashboards
4. Release notes

**Criterios de aceptación:**
- [ ] Todas las métricas funcionan
- [ ] Documentación completa
- [ ] Dashboards accesibles
- [ ] Alertas configuradas

### 4.2 Semana 2: Logging y Tracing

#### Día 8-9: Implementar Logging Estructurado

**Tareas:**
1. Configurar logging JSON
2. Implementar correlation IDs
3. Integrar con Loki
4. Pruebas de logs

**Código objetivo:**
```rust
// ebpf-node/src/logging/mod.rs
mod json_formatter;
mod correlation;
mod loki_client;

use json_formatter::JsonFormatter;
use correlation::CorrelationId;

pub struct Logger {
    formatter: JsonFormatter,
    correlation_id: Option<CorrelationId>,
}

impl Logger {
    pub fn log(&self, level: Level, message: &str, metadata: HashMap<String, String>) {
        let log_entry = LogEntry {
            timestamp: Utc::now(),
            level: level.to_string(),
            message: message.to_string(),
            correlation_id: self.correlation_id.clone(),
            metadata: metadata,
        };
        // Enviar a Loki
        self.loki_client.send(log_entry);
    }
}
```

**Criterios de aceptación:**
- [ ] Todos los logs en formato JSON
- [ ] Correlation ID en todos los logs
- [ ] Logs centralizados en Loki
- [ ] Querying de logs funcionando

#### Día 10-11: Implementar Distributed Tracing

**Tareas:**
1. Configurar OpenTelemetry
2. Implementar spans para operaciones críticas
3. Configurar Tempo
4. Pruebas de tracing

**Código objetivo:**
```rust
// ebpf-node/src/tracing/operations.rs
use opentelemetry::trace::{Tracer, SpanKind, Status};

pub async fn process_block(block: Block) -> Result<BlockResult> {
    let tracer = global::tracer("ebpf-blockchain");
    let (span, ctx) = tracer.start_with_context("process_block", &global::get_tracer_provider().tracer("ebpf"));
    
    span.set_attribute("block.number", block.number);
    span.set_attribute("block.size", block.size);
    
    let result = inner_process_block(block).await;
    
    match &result {
        Ok(_) => span.set_status(Status::Ok),
        Err(e) => span.set_status(Status::error(e.to_string())),
    }
    
    span.end();
    result
}
```

**Criterios de aceptación:**
- [ ] OpenTelemetry configurado
- [ ] Spans para operaciones críticas
- [ ] Tempo funcionando
- [ ] Tracing visible en dashboard

#### Día 12-13: Integración Completa

**Tareas:**
1. Integrar métricas, logs y tracing
2. Configurar stack completo
3. Pruebas de integración
4. Documentación de operación

**Criterios de aceptación:**
- [ ] Stack completo funcionando
- [ ] Correlación entre métricas, logs y tracing
- [ ] Documentación completa
- [ ] Runbook de operaciones

#### Día 14: Pruebas Finales y Documentación

**Tareas:**
1. Pruebas integrales
2. Documentación final
3. Actualización de README
4. Release notes

**Criterios de aceptación:**
- [ ] Todas las pruebas pasan
- [ ] Documentación completa
- [ ] Release notes actualizados
- [ ] Métricas 100% completas

---

## 5. Configuración del Stack de Observabilidad

### 5.1 docker-compose para Stack Completo

```yaml
# monitoring/docker-compose.yml
version: '3.8'

services:
  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus/prometheus.yml:/etc/prometheus/prometheus.yml
      - ./prometheus/alerts.yml:/etc/prometheus/alerts.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
    networks:
      - observability

  loki:
    image: grafana/loki:latest
    container_name: loki
    ports:
      - "3100:3100"
    volumes:
      - ./loki/loki-config.yml:/etc/loki/loki-config.yml
      - loki_data:/loki
    command: -config.file=/etc/loki/loki-config.yml
    networks:
      - observability

  tempo:
    image: grafana/tempo:latest
    container_name: tempo
    ports:
      - "3200:3200"
      - "4317:4317"
    volumes:
      - ./tempo/tempo-config.yml:/etc/tempo/config.yaml
      - tempo_data:/tmp/tempo
    command: -config.file=/etc/tempo/config.yaml
    networks:
      - observability

  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
    volumes:
      - grafana_data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./grafana/datasources:/etc/grafana/provisioning/datasources
    depends_on:
      - prometheus
      - loki
      - tempo
    networks:
      - observability

  ebpf-node:
    build:
      context: ..
      dockerfile: Dockerfile
    container_name: ebpf-node
    ports:
      - "9000:9000"
      - "9090:9090"  # Metrics
    environment:
      - METRICS_ENDPOINT=http://prometheus:9090
      - LOGGING_BACKEND=loki
      - LOGGING_ENDPOINT=http://loki:3100
      - TRACING_ENDPOINT=http://tempo:4317
    depends_on:
      - prometheus
      - loki
      - tempo
    networks:
      - observability

networks:
  observability:
    driver: bridge

volumes:
  prometheus_data:
  loki_data:
  tempo_data:
  grafana_data:
```

### 5.2 Configuración de Prometheus

```yaml
# monitoring/prometheus/prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'ebpf-blockchain'
    static_configs:
      - targets: ['ebpf-node:9090']
    metrics_path: '/metrics'
    
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093

rule_files:
  - /etc/prometheus/alerts.yml
```

### 5.3 Configuración de Grafana

```yaml
# monitoring/grafana/datasources/datasources.yml
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
    editable: false

  - name: Loki
    type: loki
    access: proxy
    url: http://loki:3100
    editable: false

  - name: Tempo
    type: tempo
    access: proxy
    url: http://tempo:3200
    editable: false
```

---

## 6. Tests y Validación

### 6.1 Tests de Métricas

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_collection() {
        let metrics = NetworkMetrics::new();
        
        metrics.record_message_sent("block");
        metrics.record_message_sent("transaction");
        
        assert_eq!(metrics.messages_sent.get(), 2);
    }
    
    #[test]
    fn test_latency_metrics() {
        let metrics = NetworkMetrics::new();
        
        metrics.record_latency("peer1", 100.0);
        metrics.record_latency("peer2", 200.0);
        
        // Validar que las métricas se registraron correctamente
    }
}
```

### 6.2 Tests de Logging

```bash
# tests/logging_test.sh
#!/bin/bash

# Test 1: Verificar que los logs se envían a Loki
curl -X POST 'http://localhost:3100/loki/api/v1/push' \
  -H 'Content-Type: application/json' \
  -d '{
    "streams": [{
      "stream": {"level": "info", "component": "test"},
      "values": [["2024-01-01T00:00:00Z", "Test log entry"]]
    }]
  }'

# Test 2: Query logs en Loki
curl -G 'http://localhost:3100/loki/api/v1/query' \
  --data-urlencode 'query={component="ebpf-node"}'

# Test 3: Verificar formato JSON
./bin/ebpf-blockchain-cli log --format=json | jq .
```

### 6.3 Tests de Tracing

```rust
#[cfg(test)]
mod tracing_tests {
    use opentelemetry::trace::{Tracer, SpanKind};
    
    #[test]
    fn test_span_creation() {
        let tracer = global::tracer("ebpf-blockchain");
        let span = tracer.span_builder("test_operation").start(&tracer);
        
        span.set_attribute("test.key", "test.value");
        span.end();
        
        // Validar que el span fue creado correctamente
    }
}
```

### 6.4 Criterios de Aceptación

- [ ] 100% de métricas implementadas
- [ ] Todos los dashboards funcionando
- [ ] Alertas configuradas y probadas
- [ ] Logs estructurados y centralizados
- [ ] Distributed tracing funcionando
- [ ] Correlación entre métricas, logs y tracing
- [ ] Documentación completa

---

## 7. Monitoreo de Producción

### 7.1 Health Checks

```bash
# Health check para Prometheus
curl http://localhost:9090/-/healthy

# Health check para Loki
curl http://localhost:3100/loki/ready

# Health check para Tempo
curl http://localhost:3200/ready

# Health check para Grafana
curl http://localhost:3000/api/health
```

### 7.2 Comandos de Diagnóstico

```bash
# Ver estado de métricas
./bin/ebpf-blockchain-cli metrics dump

# Ver logs en tiempo real
journalctl -u ebpf-node -f

# Query de logs
curl -G 'http://localhost:3100/loki/api/v1/query' \
  --data-urlencode 'query={component="ebpf-node"} |= "error"'

# Ver tracing
curl http://localhost:3200/search?limit=10
```

### 7.3 Reportes Automatizados

```yaml
# monitoring/prometheus/rules/reports.yml
groups:
  - name: daily_reports
    interval: 24h
    rules:
      - alert: DailyMetricsReport
        runbook_url: https://github.com/your-org/ebpf-blockchain/runbooks/daily-report.md
        annotations:
          summary: "Reporte diario de métricas"
```

---

## 8. Riesgos y Mitigación

### 8.1 Riesgos Técnicos

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| Performance impact de tracing | Media | Medio | Sampling configurable |
| Almacenamiento de logs | Alta | Medio | Retention policies |
| Complejidad del stack | Media | Bajo | Documentación clara |
| Compatibilidad de versiones | Baja | Bajo | Testing exhaustivo |

### 8.2 Riesgos Operativos

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| Fallo de monitoreo | Media | Alto | Health checks independientes |
| Pérdida de métricas | Baja | Medio | Buffer local |
| Alertas falsas | Media | Bajo | Tuning de thresholds |

---

## 9. Criterios de Finalización

La Etapa 3 se considera completada cuando:

1. ✅ **Métricas:** 100% de métricas implementadas y funcionando
2. ✅ **Dashboards:** Todos los dashboards de Grafana funcionando
3. ✅ **Alertas:** Alertas configuradas y probadas
4. ✅ **Logs:** Logging estructurado y centralizado en Loki
5. ✅ **Tracing:** Distributed tracing implementado y funcionando
6. ✅ **Correlación:** Correlación entre métricas, logs y tracing
7. ✅ **Documentación:** Documentación completa de todas las métricas
8. ✅ **Tests:** Todos los tests de observabilidad pasan

---

## 10. Referencias

- [Documento 01_ESTRUCTURA_PROYECTO.md](../01_ESTRUCTURA_PROYECTO.md)
- [Etapa 1: Estabilización](../01_estabilizacion/01_ESTABILIZACION.md)
- [Etapa 2: Seguridad](../02_seguridad/02_CONSENSO_SEGURO.md)
- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Loki Documentation](https://grafana.com/docs/loki/latest/)
- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)

---

## 11. Historial de Cambios

| Versión | Fecha | Cambios | Autor |
|---------|-------|---------|-------|
| 1.0 | 2026-01-26 | Creación inicial del documento | @ebpf-dev |

---

*Documento bajo revisión para Etapa 3 de la mejora del proyecto ebpf-blockchain*