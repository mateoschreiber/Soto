# Etapa 1: interfaz editorial en Rust

La instrucción de implementar todo el código propio en Rust sustituye la propuesta inicial de React/TypeScript. No se conserva código de aplicación JavaScript/TypeScript ni un runtime Node.js.

## Arquitectura y alcance

- `soto-core`: modelos tipados, separación de Local/Torrents, búsqueda, navegación validada, selección espacial del foco y preferencias versionadas. Compartido con los futuros clientes y servicios.
- `soto-web`: Dioxus compilado a WebAssembly. Componentes para navegación, tarjetas, destacados, géneros, fichas, fuentes, diálogos, estados y administración de la demostración.
- `soto-server`: Axum sirve recursos compilados y `/api/health`. No implementa sesiones, escaneo, reproducción ni conectores todavía.
- Docker multi-stage: compilación Rust/WASM separada de runtime Debian mínimo, usuario sin privilegios, healthcheck y recursos limitados.

## Diseño aprobado

Interfaz clara de FigCraft, verde mate, lateral estrecho con símbolos grandes y centrados, héroe panorámico y filas de portadas. Escritorio y móvil comparten componentes. La vista TV aumenta tamaños y habilita foco espacial por teclado, sin presentarse como APK.

## Decisiones

1. Navegación mediante hash para admitir enlaces directos sin reescritura. Rutas e identificadores desconocidos muestran una pantalla recuperable.
2. La obra y su fuente son entidades distintas. Una fuente descargada de Torrents no se importa automáticamente a Local.
3. Preferencias visuales y colecciones de ejemplo en almacenamiento local versionado, con recuperación ante corrupción o almacenamiento bloqueado. Ningún secreto en el navegador.
4. Las imágenes no se copian al repositorio. Se permiten solicitudes a Metahub, con alternativa propia y control para desactivarlas.
5. Las acciones que necesitan backend informan qué etapa las implementará; no simulan reproducción, vinculación ni descargas.
6. Sin procesos multimedia en esta etapa. Runtime asíncrono pequeño, caché por contenido y gzip precomprimido. Las optimizaciones posteriores se apoyarán en mediciones del homelab.

## Comprobación

Pruebas de dominio y HTTP anteriores a sus implementaciones. Rustfmt, Clippy con advertencias como errores, compilación WASM de release y recorridos de navegador para ambas bibliotecas, búsqueda, colecciones, diálogos, estados, móvil y foco TV. Docker se verificará donde exista un daemon; no inferir éxito del contenedor a partir de una compilación nativa en Windows.

## Fuentes

- https://dioxuslabs.com/learn/0.7/tutorial/rsx/
- https://dioxuslabs.com/learn/0.7/tutorial/state/
- https://dioxuslabs.com/learn/0.7/tutorial/bundle/
- https://docs.rs/tower-http/latest/tower_http/services/struct.ServeDir.html
- https://docs.docker.com/build/building/multi-stage/
- https://docs.docker.com/reference/compose-file/services/
