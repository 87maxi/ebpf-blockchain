# Migración a CO-RE (Compile Once Run Everywhere)

## Visión General

La migración a CO-RE (Compile Once Run Everywhere) es una mejora fundamental que permite que los programas eBPF sean portables y puedan ejecutarse en diferentes versiones del kernel sin necesidad de recompilación. Esto mejora significativamente la compatibilidad y facilidad de despliegue del sistema.

## ¿Qué es CO-RE?

CO-RE es una técnica que permite compilar programas eBPF de manera que puedan ejecutarse en múltiples versiones del kernel sin requerir recompilación. Esto se logra mediante:

1. **BTF (BPF Type Format)**: Información de tipos que permite la compatibilidad entre diferentes versiones del kernel
2. **Relocation**: Mecanismo para ajustar direcciones y referencias en tiempo de carga
3. **Map pinning**: Para mantener la persistencia de mapas entre cargas

## Beneficios de CO-RE

1. **Portabilidad**: Los programas pueden ejecutarse en diferentes versiones del kernel
2. **Facilidad de despliegue**: No se requiere recompilación para nuevas versiones del kernel
3. **Menor mantenimiento**: Reducción de la complejidad de gestión de diferentes compilaciones
4. **Compatibilidad**: Mejor soporte para diferentes distribuciones Linux

## Implementación

### Uso de bpf-linker

Se ha configurado el sistema de compilación para usar `bpf-linker` que es necesario para generar archivos BTF y permitir la compatibilidad CO-RE.

### Configuración del Build

El archivo `build.rs` ha sido actualizado para:
1. Verificar la presencia de `bpf-linker`
2. Usar `aya-build` para compilar correctamente los programas eBPF

### Mapas y Estructuras

Los mapas y estructuras han sido configurados para ser compatibles con CO-RE:
1. Uso de estructuras de mapas que soportan BTF
2. Nombres de mapas consistentes
3. Tipos de datos compatibles con BTF

## Requisitos

1. Instalar `bpf-linker`:
   ```bash
   cargo install bpf-linker
   ```

2. Asegurarse de tener las dependencias de Aya configuradas correctamente

## Próximos Pasos

1. Probar la compilación con CO-RE en diferentes versiones del kernel
2. Validar el funcionamiento en entornos de producción
3. Documentar el proceso de despliegue con CO-RE
4. Implementar mecanismos de fallback para versiones antiguas del kernel