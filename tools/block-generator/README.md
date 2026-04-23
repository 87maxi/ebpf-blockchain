# Block Generator Service

Servicio para generar transacciones automáticamente en la red eBPF Blockchain, simulando actividad de red productiva realista.

## Descripción

Este servicio envía transacciones periódicamente a los nodos eBPF del cluster para:
- Simular tráfico de red realista con patrones de burst y períodos de calma
- Generar bloques continuamente para testing y monitoreo
- Exponer métricas Prometheus para observabilidad del generador
- Distribuir transacciones entre múltiples senders y tipos

### Características Avanzadas

- **Burst patterns**: Períodos de alta actividad alternados con calma (simular picos reales)
- **Distribución de tipos**: 70% transfers, 15% contracts, 10% votes, 5% swaps
- **Latencia variable**: Simula tiempo de procesamiento real de red
- **Múltiples senders**: Rotación entre 10 IDs de sender diferentes (usuarios reales y bots)
- **Direcciones destino realistas**: Reutilización de addresses comunes + generación de nuevas
- **Distribución logarítmica de montos**: Muchos montos pequeños, pocos grandes
- **Fallos simulados**: 2-5% de transacciones fallan intencionalmente para testing

## Métricas Prometheus

El block-generator expone métricas en el puerto `9101` (configurable):

| Métrica | Tipo | Descripción |
|---------|------|-------------|
| `ebpf_blockgen_transactions_total{sender,type,node}` | counter | Transacciones totales por sender, tipo y nodo |
| `ebpf_blockgen_transactions_successful{node}` | counter | Transacciones exitosas |
| `ebpf_blockgen_transactions_failed{node}` | counter | Transacciones fallidas |
| `ebpf_blockgen_transaction_seconds{node}` | histogram | Latencia de transacción en segundos |
| `ebpf_blockgen_batch_duration_seconds{node}` | histogram | Duración de batches |
| `ebpf_blockgen_active_senders{node}` | gauge | Senders activos actuales |
| `ebpf_blockgen_current_batch_size{node}` | gauge | Tamaño del batch actual |
| `ebpf_blockgen_batches_total{node}` | counter | Batches totales generados |
| `ebpf_blockgen_uptime_seconds{node}` | gauge | Uptime del servicio |
| `ebpf_blockgen_success_rate_percent{node}` | gauge | Tasa de éxito en porcentaje |
| `ebpf_blockgen_transactions_per_second{node}` | gauge | Transacciones por segundo |

### Endpoints

- `http://localhost:9101/metrics` - Métricas Prometheus
- `http://localhost:9101/health` - Health check JSON

## Requisitos Previos

**Los nodos eBPF deben estar corriendo antes de iniciar el block generator.**

Los nodos exponen dos puertos:
- **Puerto 9090**: Métricas Prometheus (`/metrics`)
- **Puerto 9091**: API REST (`/api/v1/transactions`, `/api/v1/node/info`, etc.)

Verificar que los nodos están corriendo:

```bash
# Verificar métricas (puerto 9090)
curl http://192.168.2.210:9090/metrics | head -5
curl http://192.168.2.211:9090/metrics | head -5
curl http://192.168.2.212:9090/metrics | head -5

# Verificar API (puerto 9091)
curl http://192.168.2.210:9091/api/v1/node/info
curl http://192.168.2.211:9091/api/v1/node/info
curl http://192.168.2.212:9091/api/v1/node/info

# Si los nodos no responden, iniciarlos desde ansible:
cd /home/maxi/Documentos/source/ebpf-blockchain
ansible-playbook -i ansible/inventory/hosts.yml ansible/playbooks/deploy.yml
```

## Instalación

### Opción 1: Como servicio systemd (Recomendado)

```bash
# Copiar el script y servicio
sudo cp block_generator.py /usr/local/bin/block_generator.py
sudo chmod +x /usr/local/bin/block_generator.py
sudo cp block-generator.service /etc/systemd/system/

# Recargar systemd y habilitar servicio
sudo systemctl daemon-reload
sudo systemctl enable block-generator.service

# Iniciar servicio
sudo systemctl start block-generator.service
```

### Opción 2: Ejecución manual con venv

```bash
# Crear entorno virtual
cd tools/block-generator
python3 -m venv venv
source venv/bin/activate
pip install requests

# Ejecutar directamente
python3 block_generator.py --nodes 192.168.2.210,192.168.2.211,192.168.2.212 --interval 5 --batch-size 3

# Ejecutar en background
nohup python3 block_generator.py --nodes 192.168.2.210,192.168.2.211,192.168.2.212 --interval 5 > /var/log/ebpf-block-generator.log 2>&1 &
```

## Uso

### Línea de comandos

```bash
python3 block_generator.py [OPTIONS]

Options:
  --nodes TEXT          Comma-separated list of eBPF node IPs (default: 192.168.2.210)
  --interval INTEGER    Seconds between transaction batches (default: 5)
  --batch-size INTEGER  Base number of transactions per batch (default: 3)
  --sender TEXT         Unique sender ID for nonce tracking (default: block-generator)
  --failure-rate FLOAT  Simulated failure rate 0.0-1.0 (default: 0.03)
  --metrics-port INTEGER  Prometheus metrics port (default: 9101)
  --node-id TEXT        Custom node ID for metrics (default: derived from first node IP)
  --config TEXT         Configuration file path (default: ~/.ebpf-blockchain/block-generator.conf)
  --daemon              Run as daemon (background process)
  --verbose             Enable debug logging
```

### Ejemplos

```bash
# Single node, 5 seconds interval
python3 block_generator.py --nodes 192.168.2.210 --interval 5

# Multi-node cluster, 10 seconds interval, 5 transactions per batch
python3 block_generator.py --nodes 192.168.2.210,192.168.2.211,192.168.2.212 --interval 10 --batch-size 5

# Custom failure rate and metrics port
python3 block_generator.py --nodes 192.168.2.210 --failure-rate 0.05 --metrics-port 9102

# Custom node ID for metrics
python3 block_generator.py --nodes 192.168.2.210,192.168.2.211,192.168.2.212 --node-id block-gen-prod
```

## Gestión del servicio

```bash
# Ver estado
sudo systemctl status block-generator.service

# Ver logs
sudo journalctl -u block-generator.service -f

# Reiniciar
sudo systemctl restart block-generator.service

# Detener
sudo systemctl stop block-generator.service

# Deshabilitar
sudo systemctl disable block-generator.service
```

## Configuración

El archivo de estado se guarda en `~/.ebpf-blockchain/block-generator.conf` y se actualiza automáticamente:

```json
{
    "nodes": "192.168.2.210,192.168.2.211,192.168.2.212",
    "interval": 5,
    "batch_size": 3,
    "sender": "block-generator",
    "failure_rate": 0.03,
    "metrics_port": 9101,
    "node_id": "block-gen-01",
    "nonce": 42,
    "total_sent": 150,
    "total_failed": 3,
    "total_confirmed": 147,
    "last_updated": "2026-04-22T20:59:40.123456Z"
}
```

**Nota**: El nonce y los contadores se actualizan automáticamente para persistir entre reinicios y evitar transacciones duplicadas.

## Estructura de Transacciones

Cada transacción generada tiene la siguiente estructura:

```json
{
    "id": "a1b2c3d4e5f6g7h8",
    "data": "Transfer 100.5 EBPF to 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
    "type": "transfer",
    "sender": "user-42",
    "nonce": 42,
    "timestamp": 1713820800,
    "metadata": {
        "amount": 100.5,
        "destination": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
        "token": "EBPF",
        "fee_estimated": 0.0234
    }
}
```

### Tipos de Transacciones

| Tipo | Distribución | Descripción |
|------|-------------|-------------|
| `transfer` | 70% | Transferencias de tokens entre direcciones |
| `contract` | 15% | Updates de smart contracts (deploy, call, initialize) |
| `vote` | 10% | Votaciones de propuesta (yes/no/abstain) |
| `swap` | 5% | Token swaps con fee del 0.1% |

### Senders Simulados

- `user-42`, `user-17`, `user-89`, `user-3`, `user-56` - Usuarios regulares
- `trader-bot-1`, `trader-bot-2` - Bots de trading
- `defi-user-7` - Usuario DeFi
- `nft-collector` - Coleccionista NFT
- `whale-account` - Cuenta whale (gran volumen)

## Dashboards de Grafana

### Block Generator Debug Dashboard

Dashboard dedicado para monitorear el block-generator: [`block-generator-debug.json`](../../monitoring/grafana/dashboards/block-generator-debug.json)

**Panels incluidos:**
1. Transactions Rate - Transacciones por segundo
2. Total Transactions - Contador total
3. Success Rate - Tasa de éxito
4. Latency p95 - Percentil 95 de latencia
5. Transaction Rate by Type - Rate por tipo (time series)
6. Transaction Types Distribution - Pie chart
7. Sender Activity - Actividad por sender
8. Success/Failure Rate - Time series de éxito/fallo
9. Latency Histogram - Histograma de latencia
10. Node Success Rate Gauge - Gauge por nodo
11. Active Senders - Senders activos
12. Batch Duration - Duración de batches
13. Node Health Table - Estado de nodos
14. Current Batch Size - Tamaño actual del batch
15. Batch Rate - Rate de batches

**Variables:**
- `$node` - Selectores de nodo (192_168_2_210, 211, 212)
- `$sender` - Selectores de sender
- `$type` - Selectores de tipo de transacción
- `$interval` - Intervalo de tiempo para rate calculations

## Estado Actual

- ✅ Prometheus scrapeando los 3 nodos (192.168.2.210-212) en puerto 9090
- ✅ Prometheus scrapeando block-generator en localhost:9101
- ✅ Grafana dashboards cargando correctamente
- ✅ API REST disponible en puerto 9091
- ✅ Block generator con patrones realistas y métricas Prometheus
- ✅ Dashboard block-generator-debug.json creado

## Dependencias

- Python 3.8+
- `requests` library: `pip install requests`
- Nodos eBPF corriendo en los puertos:
  - Metrics: 9090
  - API (RPC): 9091

## Arquitectura

```
┌─────────────────────────────────────────────────────────────┐
│                    Block Generator                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────┐    ┌─────────────────────────┐   │
│  │ TrafficPatternSim   │    │ RealisticTxGenerator    │   │
│  │ (burst/calm cycles) │    │ (types, senders, amounts)│   │
│  └─────────────────────┘    └─────────────────────────┘   │
│            │                            │                   │
│            └────────────┬───────────────┘                   │
│                         ▼                                   │
│              ┌─────────────────────┐                       │
│              │   BlockGenerator    │                       │
│              │   (orchestrator)    │                       │
│              └─────────────────────┘                       │
│                         │                                  │
│          ┌──────────────┴──────────────┐                  │
│          ▼                             ▼                  │
│  ┌──────────────────┐        ┌──────────────────┐       │
│  │ PrometheusMetrics│        │  HTTP Server     │       │
│  │ (counters,       │        │ (:9101/metrics)  │       │
│  │  histograms)     │        └──────────────────┘       │
│  └──────────────────┘                                    │
└─────────────────────────────────────────────────────────────┘
                         │
                         ▼ HTTP POST
┌─────────────────────────────────────────────────────────────┐
│                    eBPF Nodes (3x)                          │
│  192.168.2.210:9091  192.168.2.211:9091  192.168.2.212:9091│
│  /api/v1/transactions (round-robin)                         │
└─────────────────────────────────────────────────────────────┘
```
