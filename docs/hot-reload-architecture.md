# Arquitectura de Hot-Reload para eBPF Programs

## Visión General

La arquitectura de hot-reload permite recargar dinámicamente los programas eBPF sin necesidad de reiniciar el nodo completo. Esto es especialmente útil para:

- Actualización de políticas de seguridad en tiempo real
- Despliegue de nuevas reglas de filtrado sin interrupciones
- Actualización de módulos de monitoreo sin detener el tráfico

## Componentes Principales

### 1. EbpfHotReloadManager

El `EbpfHotReloadManager` es el componente central que gestiona el ciclo de vida de los programas eBPF:

```rust
pub struct EbpfHotReloadManager {
    /// Instancia actual de eBPF
    ebpf: Arc<Mutex<Ebpf>>,
    /// Nombre de la interfaz para el programa XDP
    iface: String,
}
```

### 2. Métodos Principales

- `new(iface: String)`: Crea un nuevo gestor de hot-reload
- `init()`: Inicializa los programas eBPF
- `reload()`: Recarga completamente los programas eBPF
- `get_ebpf()`: Obtiene una referencia al estado actual de eBPF

### 3. Proceso de Reload

El proceso de reload sigue estos pasos:

1. **Desvinculación**: Se desvinculan todos los programas eBPF existentes
2. **Carga**: Se cargan nuevos programas desde el binario compilado
3. **Vinculación**: Se vinculan los nuevos programas a las interfaces correspondientes

## Integración con la API

Se ha añadido un endpoint REST para el hot-reload:

```
POST /api/v1/ebpf/reload
```

Este endpoint permite recargar los programas eBPF mediante una llamada HTTP.

## Beneficios

- **Disponibilidad**: Los programas pueden actualizarse sin interrupción del tráfico
- **Flexibilidad**: Permite actualizaciones dinámicas de políticas de seguridad
- **Seguridad**: Se puede actualizar el código eBPF sin afectar la operación del nodo
- **Monitoreo**: Se puede verificar el estado de los programas antes y después del reload

## Consideraciones de Implementación

- El sistema requiere que los programas eBPF sean compilables y compatibles con el hot-reload
- Se debe manejar correctamente el estado de los mapas compartidos entre recargas
- El proceso de reload debe ser seguro y no dejar estados inconsistentes
- Se debe implementar manejo de errores adecuado para evitar fallos en producción

## Uso Práctico

Para recargar los programas eBPF:

```bash
curl -X POST http://localhost:8080/api/v1/ebpf/reload
```

O mediante una aplicación cliente que haga la llamada HTTP al endpoint.