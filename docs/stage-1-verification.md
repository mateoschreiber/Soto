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

- Rust: las 11 pruebas del workspace pasan; formato y Clippy sin advertencias, tanto para servidor como para WebAssembly.
- Escritorio: revisión visual a 1600 px, iconos centrados y metadatos reales de las tarjetas, sin desbordamiento horizontal.
- Móvil: viewport emulado de 390 × 844; inicio, biblioteca, ficha y diálogo sin desbordamiento. El diálogo mide 352 px y la navegación inferior permanece disponible.
- TV: viewport emulado de 1920 × 1080; la primera fila completa termina en y=1066. Flechas desplazan el foco, Enter guarda una colección y Escape vuelve a la biblioteca. Cerrar un diálogo restaura el foco en el botón que lo abrió.
- Estados: carga, vacío, error y desconexión comprobados; recuperar el estado normal devuelve los 20 elementos de Local. El filtro de género se aplica y se puede restablecer.
- Privacidad visual: desactivar imágenes remotas elimina todos los elementos de imagen de las portadas y conserva fondos propios. La preferencia persiste al recargar.
- Se reprodujo un bloqueo CSP causado por una redirección HTTP 307 de `images.metahub.space` a `live.metahub.space`. La corrección permite explícitamente ambos dominios, sin comodín; se añadió una aserción HTTP observada fallando antes del arreglo.
- Docker Linux amd64: [ejecución 3 de GitHub Actions](https://github.com/mateoschreiber/Soto/actions/runs/38997701198) aprobada. Compila la interfaz y el servidor, arranca con Compose, responde a salud HTTP y al healthcheck binario, y verifica UID 10001 y raíz de solo lectura. Este equipo no tiene daemon Docker; la prueba se ejecutó en Ubuntu 24.04 de GitHub.

## Medidas observadas

- WebAssembly de release utilizado por el HTML local: 1.017.887 bytes; comprimido con gzip nivel 9: 380.945 bytes. Excluye JavaScript generado por las dependencias, CSS e imágenes remotas.
- Imagen Docker de la ejecución 3: 82.750.315 bytes, según `docker image inspect`.
- Muestra puntual de `docker stats` tras las peticiones de salud: 960 KiB de memoria y 0,00 % de CPU, con límite de 128 MiB. Es una medición del servidor estático en reposo; no predice el consumo del futuro procesamiento multimedia ni del navegador cliente.

## Límites de estas pruebas

No verifican reproducción de películas, escaneo del homelab, cuentas, fuentes de proveedores, descargas ni un APK: esas funciones no están implementadas. La vista TV web se verifica con teclado de navegador, no con un televisor real. El Dockerfile contempla arm64, pero esa arquitectura y el despliegue en el homelab del usuario todavía no se han probado.
