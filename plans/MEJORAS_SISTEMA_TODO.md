# Plan de Acciones - eBPF Blockchain

**Generado desde:** [`ANALISIS_PROFUNDO_SISTEMA.md`](docs/ANALISIS_PROFUNDO_SISTEMA.md)

---

## FASE 0: ESTABILIZACIÓN (P0 - Inmediato)

- [ ] **P0-1** Corregir métricas nunca actualizadas (`XDP_PACKETS_DROPPED`, `TRANSACTION_QUEUE_SIZE`, `CONSENSUS_DURATION`)
- [ ] **P0-2** Corregir alerta `NodeDown` con job name incorrecto en Prometheus
- [ ] **P0-3** Eliminar datos simulados de APIs de bloques y conectar con RocksDB real

## FASE 1: CONSENSO FUNCIONAL (P1 - Corto Plazo)

- [ ] **P1-1** Implementar estructura formal de bloques con pipeline completo
- [ ] **P1-2** Implementar selección de proposer (Round-Robin → VRF)
- [ ] **P1-3** Verificación de firmas Ed25519 en votos recibidos

## FASE 2: SEGURIDAD AVANZADA (P2 - Mediano Plazo)

- [ ] **P2-1** Detección de double-vote con slashing automático
- [ ] **P2-2** Detección de eclipse attack con métrica de riesgo
- [ ] **P2-3** Threat Score global calculado en tiempo real

## FASE 3: AUTOMATIZACIÓN (P3 - Paralelo)

- [ ] **P3-1** Pipeline CI/CD con GitHub Actions
- [ ] **P3-2** Suite de tests unitarios e integración
- [ ] **P3-3** Health checks automáticos con alertas
- [ ] **P3-4** Automatización de backups con retención

## FASE 4: OBSERVABILIDAD AVANZADA (P4 - Largo Plazo)

- [ ] **P4-1** Nuevas métricas de seguridad (threat score, double-vote, eclipse)
- [ ] **P4-2** Dashboard de Security Threat en Grafana

---

## Automatización Adicional

- [ ] **A1** Script de auto-scaling del cluster
- [ ] **A2** Playbooks de auto-remediation con Ansible
- [ ] **A3** Detección de configuration drift entre nodos
