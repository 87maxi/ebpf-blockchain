# Reestructuración de la Estructura de Ansible para Ambiente de Desarrollo

## Visión General

Este documento describe la reestructuración propuesta para la estructura de Ansible con el objetivo de crear un ambiente de desarrollo local optimizado que integre el sistema de observabilidad con Docker, manteniendo la compatibilidad con los entornos de producción existentes.

## Problemas Identificados

1. **Complejidad en la Configuración**: La configuración de los servicios de observabilidad está separada del directorio de Ansible
2. **Falta de Desarrollo Local**: No hay una configuración específica para desarrollo local
3. **Configuración de Métricas**: Las métricas de los nodos eBPF no están configuradas para ser recolectadas automáticamente en entornos de desarrollo

## Propuesta de Reestructuración

### Estructura de Directorios Actualizada

```
ansible/
├── ansible.cfg
├── inventory/
│   ├── hosts.yml
│   └── group_vars/
│       └── all.yml
├── roles/
│   ├── common/
│   ├── dependencies/
│   ├── lxc_node/
│   ├── monitoring/
│   └── dev_environment/
├── playbooks/
│   ├── setup_cluster.yml
│   ├── create_node.yml
│   ├── create_cluster.yml
│   ├── destroy_node.yml
│   ├── install_deps.yml
│   ├── configure_monitoring.yml
│   ├── cluster_status.yml
│   └── setup_dev_environment.yml
└── templates/
    └── docker-compose.dev.yml.j2
```

### Componentes Nuevos

1. **Rol `dev_environment`**: Configura el ambiente de desarrollo local con todas las herramientas necesarias
2. **Playbook `setup_dev_environment.yml`**: Configura el ambiente de desarrollo completo en un solo nodo local
3. **Plantilla `docker-compose.dev.yml.j2`**: Configuración específica para desarrollo con todos los servicios de observabilidad

## Beneficios de la Reestructuración

### Mejora en la Experiencia de Desarrollo
- Ambiente de desarrollo más fácil de configurar
- Menos dependencias externas para ejecutar el sistema
- Configuración más intuitiva para nuevos desarrolladores

### Flexibilidad en Ambientes
- Soporte para múltiples ambientes (desarrollo, testing, producción)
- Configuración modular que permite cambiar fácilmente entre ambientes
- Menos duplicación de configuraciones

### Mejora en la Observabilidad
- Métricas más completas en entornos de desarrollo
- Dashboards específicos para el desarrollo
- Alertas configurables para diferentes niveles de entorno

## Implementación Detallada

### 1. Creación del Rol `dev_environment`
- Instalación de herramientas de desarrollo necesarias
- Configuración de Docker y Docker Compose
- Configuración de servicios de observabilidad en modo desarrollo
- Configuración de variables de entorno específicas para desarrollo

### 2. Actualización de Playbooks
- Creación de `setup_dev_environment.yml` para configurar el ambiente de desarrollo
- Actualización de `configure_monitoring.yml` para soportar diferentes ambientes
- Asegurar que los playbooks puedan ejecutarse en modo local o en nodos remotos

### 3. Creación de Plantillas Específicas
- Plantilla `docker-compose.dev.yml.j2` para el entorno de desarrollo
- Plantilla `prometheus.dev.yml.j2` para configuración específica de desarrollo

## Consideraciones de Implementación

### Compatibilidad
- Mantener compatibilidad con la estructura actual para producción
- No romper la funcionalidad existente de los playbooks actuales
- Asegurar que las nuevas configuraciones sean opcionales

### Documentación
- Actualizar la documentación en `ansible/README.md`
- Crear documentación específica para el entorno de desarrollo
- Incluir ejemplos de uso para ambos ambientes

### Pruebas
- Probar el nuevo entorno de desarrollo
- Verificar que los playbooks existentes sigan funcionando
- Asegurar que la integración con el sistema de observabilidad funcione correctamente

## Próximos Pasos

1. Implementar el rol `dev_environment`
2. Implementar el playbook `setup_dev_environment.yml`
3. Crear las plantillas específicas para desarrollo
4. Actualizar la documentación
5. Probar la nueva estructura de desarrollo
6. Verificar compatibilidad con entornos existentes

## Conclusión

La reestructuración propuesta mejora significativamente la experiencia de desarrollo al proporcionar un ambiente local completo y fácil de configurar, manteniendo al mismo tiempo la compatibilidad con los entornos de producción existentes. Esta solución permite a los desarrolladores ejecutar todo el sistema en un solo nodo local, facilitando el desarrollo y pruebas sin necesidad de múltiples nodos físicos o virtuales.