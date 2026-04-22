# Block Generator Service

Servicio para generar transacciones automáticamente en la red eBPF Blockchain, triggerando la creación de bloques.

## Descripción

Este servicio envía transacciones periódicamente a los nodos eBPF del cluster para:
- Mantener activa la red con transacciones en tiempo real
- Generar bloques continuamente para testing y monitoreo
- Simular actividad de producción

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
  --batch-size INTEGER  Number of transactions per batch (default: 3)
  --sender TEXT         Unique sender ID for nonce tracking (default: block-generator)
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

# Custom sender ID
python3 block_generator.py --nodes 192.168.2.210 --sender test-generator --interval 2
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
    "nonce": 42,
    "total_sent": 150,
    "total_failed": 3,
    "total_confirmed": 147,
    "last_updated": "2026-04-22T20:59:40.123456Z"
}
```

**Nota**: El nonce y los contadores se actualizan automáticamente para persistir entre reinicios y evitar transacciones duplicadas.

## Estado Actual

- ✅ Prometheus scrapeando los 3 nodos (192.168.2.210-212) en puerto 9090
- ✅ Grafana dashboards cargando correctamente
- ✅ API REST disponible en puerto 9091
- ✅ Block generator enviando transacciones exitosamente

## Dependencias

- Python 3.8+
- `requests` library: `pip install requests`
- Nodos eBPF corriendo en los puertos:
  - Metrics: 9090
  - API (RPC): 9091

## Estructura de transacciones

Cada transacción generada tiene la siguiente estructura:

```json
{
    "id": "a1b2c3d4e5f6g7h8",
    "data": "Transfer 100 tokens to 0x1234...",
    "nonce": 42,
    "timestamp": 1713820800
}
```

## Tipos de transacciones generadas

- Transferencias de tokens
- Updates de smart contracts
- Registry events (create/update/delete)
- Votes (yes/no/abstain)
- Token swaps
