# Verificación de la etapa 1

Fecha: 17 de octubre de 2026. Entorno local: Windows x86_64, Rust 1.99.1 GNU, Dioxus CLI 0.7.10 y Chromium. El compilador y sus enlazadores están fuera del repositorio.

## Evidencia obtenida

- Dominio: seis pruebas de separación de orígenes, búsqueda y filtros, enlaces inválidos, foco espacial y recuperación de preferencias.
- HTTP: tres pruebas de salud, cabeceras y rutas de API no implementadas; una prueba del CSP generado para scripts del build.
- Interfaz: una prueba Rust SSR de metadatos y origen en tarjetas. Se verificó su fallo antes de corregir el defecto.
- Compilación WebAssembly de release mediante Dioxus.
- Auditoría de Cargo.lock con cargo-audit 0.22.2 y RustSec: cero vulnerabilidades y cero advertencias en la consulta realizada.
- Navegador: Local muestra 20 ejemplos sin La llegada; Torrents muestra 10 y conserva La llegada descargada en su origen. Buscar «llegada» en Torrents devuelve un resultado.
- Colecciones: guardar La llegada en Torrents conserva el elemento en ese origen y no lo muestra en Local.
- La aplicación carga con CSP sin `unsafe-eval`. Los estilos y metadatos del documento se configuran mediante APIs tipadas desde Rust.

## Validaciones finales

La revisión de móvil, foco TV, restauración de diálogos y todos los estados visuales está en curso. El arranque del contenedor se verificará mediante GitHub Actions, porque este equipo no tiene un daemon Docker. El resultado se actualizará antes de cerrar la etapa.

## Límites de estas pruebas

No verifican reproducción de películas, escaneo del homelab, cuentas, fuentes de proveedores, descargas ni un APK: esas funciones no están implementadas. La vista TV web se verifica con teclado de navegador, no con un televisor real.
