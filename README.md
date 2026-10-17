# Soto

[![Rust y Docker](https://github.com/mateoschreiber/Soto/actions/workflows/check.yml/badge.svg)](https://github.com/mateoschreiber/Soto/actions/workflows/check.yml)
[![Licencia MIT](https://img.shields.io/badge/licencia-MIT-green.svg)](LICENSE)

Aplicación multimedia para un homelab: biblioteca de archivos locales, descubrimiento de películas y series, fuentes de torrents y conexión a Real-Debrid, con una interfaz web y un futuro cliente instalable en Android TV.

**Estado: etapa 1, interfaz de demostración en Rust.** Incluye navegación, bibliotecas separadas, búsqueda, fichas, colecciones de ejemplo, estados visuales, vista TV web y servidor estático para Docker. No hay escaneo de archivos, autenticación, reproducción de películas, descargas, Real-Debrid conectado ni APK. Las siguientes etapas siguen pendientes.

Todo el código propio de aplicación y pruebas está escrito en Rust. Dioxus compila la interfaz a WebAssembly y genera los enlaces JavaScript necesarios para el navegador. CSS y archivos declarativos configuran la presentación y el despliegue; no hay una aplicación React, TypeScript o Node.js.

## Ejecutar la primera etapa

### Homelab con Docker

Con Docker Engine y Compose disponibles, descarga la versión para Linux Intel/AMD:

```sh
git clone https://github.com/mateoschreiber/Soto.git
cd Soto
git checkout v0.1.0-alpha.1
cp .env.example .env
docker compose pull
docker compose up -d --wait
docker compose ps
```

Abrir [Soto local](http://127.0.0.1:3000). Se descarga una imagen ya compilada; el homelab no necesita Rust, Cargo ni Node.js. Para construir desde el código: `docker compose -f compose.yaml -f compose.build.yaml up -d --build --wait`.

El puerto se publica en `127.0.0.1` por defecto. Para acceder desde la LAN, cambia `SOTO_HOST_IP` en `.env` por la IP LAN de tu servidor. Portainer puede desplegar directamente `compose.yaml` y definir esas mismas variables. Esta etapa solo expone datos de demostración y no solicita credenciales ni monta carpetas multimedia.

Consulta la **[guía de instalación y operación](docs/installation.md)** para Portainer, proxy, recursos, actualizaciones, reversión, diagnóstico e instalación portátil sin registro. [Notas de la versión](docs/releases/0.1.0-alpha.1.md).

La descarga anónima y el arranque temporal en un homelab Intel están [verificados](docs/distribution-verification.md).

```sh
docker compose logs --tail=50 web
docker compose down
```

### Desarrollo local en Rust

Instalar Rust con rustup y [Dioxus CLI 0.7.10](https://github.com/DioxusLabs/dioxus/releases/tag/v0.7.10). `rust-toolchain.toml` fija Rust 1.99.1, LLVM y el destino WebAssembly. En Windows, Rust también requiere el enlazador de su toolchain: Visual Studio Build Tools para MSVC o GCC/MinGW para GNU.

```sh
dx serve -p soto-web --platform web
```

Para compilar y servir la misma interfaz con Axum:

```sh
dx build -p soto-web --platform web --profile wasm-release --release --locked
cargo run --release --locked -p soto-server
```

Abrir [Soto local](http://127.0.0.1:3000). La ruta de salida se puede cambiar mediante `SOTO_STATIC_DIR`.

### Comprobaciones

```sh
cargo fmt --all --check
cargo test --locked --workspace
cargo clippy --locked -p soto-core -p soto-server --all-targets -- -D warnings
cargo clippy --locked -p soto-web --target wasm32-unknown-unknown -- -D warnings
```

Las pruebas de Rust cubren orígenes, filtros, enlaces, preferencias, foco espacial, estado del servidor y cabeceras HTTP. La revisión visual y los recorridos de navegador se describen en [la verificación de la etapa 1](docs/stage-1-verification.md).

### Configuración implementada

| Variable | Valor predeterminado | Alcance |
| --- | --- | --- |
| `SOTO_BIND` | `127.0.0.1:3000` | Escucha del servidor; Compose usa `0.0.0.0:3000` dentro del contenedor |
| `SOTO_STATIC_DIR` | `target/dx/soto-web/release/web/public` | Recursos compilados; Docker usa `/app/public` |
| `SOTO_PORT` | `3000` | Puerto publicado por Compose, no cambia el puerto interno |
| `SOTO_HOST_IP` | `127.0.0.1` | Interfaz del host donde se publica el puerto; configurable para LAN |
| `SOTO_IMAGE` | `ghcr.io/mateoschreiber/soto:0.1.0-alpha.1` | Imagen de instalación; admite tag o digest |
| `SOTO_MEMORY_LIMIT` | `128m` | Límite de memoria del contenedor |
| `SOTO_CPU_LIMIT` | `0.50` | Límite de CPU del contenedor |
| `RUST_LOG` | `soto_server=info,tower_http=warn` | Registro del servidor |

`GET /api/health` devuelve la etapa y confirma que los servicios multimedia no están conectados. Los demás endpoints de la API no están implementados y responden 404.

Las preferencias, el perfil de demostración y las colecciones se guardan en el navegador, con formato versionado y recuperación ante datos corruptos. No son cuentas reales y las colecciones de ejemplo no están aisladas por perfil. En Servidor puedes activar la vista TV, desactivar imágenes remotas y explorar estados de carga, vacío, error o desconexión. Las imágenes de la maqueta se solicitan a Metahub y no se redistribuyen; sin conexión se muestran fondos alternativos propios.

## Objetivo

Reunir la organización de una biblioteca personal y el descubrimiento online en una sola experiencia, tomando como referencias de producto Plex y Stremio. Soto será una implementación independiente; no presupone la reutilización de código propietario, marcas ni interfaces privadas de esas aplicaciones.

La interfaz aprobada en FigCraft utiliza fondo claro, verdes mate, una barra lateral estrecha, iconos grandes y centrados, una imagen protagonista amplia y filas de portadas rectangulares. El diseño contempla escritorio, pantallas pequeñas y navegación mediante mando de TV.

## Reglas del producto

- **Local y Torrents son espacios separados.** Cada uno tiene inicio, biblioteca, búsqueda, fichas y fuentes propias.
- Un archivo descargado desde Torrents conserva su origen y permanece en esa pestaña. No se importa automáticamente a Local.
- Los metadatos de una obra pueden compartirse entre espacios, pero sus fuentes, disponibilidad y estado de reproducción deben identificarse explícitamente.
- Una cuenta de Real-Debrid conectada no implica que una película tenga una fuente disponible. La disponibilidad se comprobará al resolver la fuente.
- Los catálogos describen obras; los proveedores aportan fuentes. Ninguno de los dos se confundirá con la biblioteca local.
- Los datos de demostración estarán identificados y separados de los archivos y cuentas reales.
- La administración del servidor estará separada de los permisos de reproducción de los perfiles.

## Experiencia y pantallas

| Área | Recorrido previsto |
| --- | --- |
| Acceso | Configuración inicial, acceso al servidor, selección de perfil y vinculación de TV |
| Inicio | Destacado, continuar viendo, categorías y recomendaciones dentro del origen seleccionado |
| Local | Bibliotecas, filtros, búsqueda, ficha, versiones del archivo y reproducción |
| Torrents | Descubrimiento, búsqueda, ficha, selección de fuente, preparación y seguimiento |
| Colecciones | Listas personales con identificación del origen de cada elemento |
| Reproductor | Progreso, pausa, búsqueda temporal, audio, subtítulos y siguiente episodio |
| Administración | Carpetas, escaneo, tareas, proveedores, Real-Debrid y dispositivos |
| Estados | Carga, biblioteca vacía, fuente no disponible, error de reproducción y servidor desconectado |

El prototipo de FigCraft contiene 31 pantallas web, 17 de TV y dos paneles de referencia. Se convertirán en rutas, componentes y estados reutilizables; no en 50 páginas independientes con lógica duplicada. Las imágenes y el documento de FigCraft no se redistribuyen en este repositorio mientras no se revise la licencia de todos sus recursos.

### Interacción en TV

- Foco visible y navegación espacial mediante cruceta; OK selecciona y Atrás retorna al contexto anterior.
- Restauración del foco al cerrar una ficha o un diálogo.
- Acciones principales grandes, textos legibles a distancia y ausencia de funciones dependientes del hover.
- Vinculación del dispositivo mediante un código temporal, con confirmación desde una sesión autorizada.
- Pruebas en un dispositivo Android TV real antes de afirmar compatibilidad.

## Arquitectura propuesta

```text
Navegador / cliente Android TV
             |
        API de Soto
         /   |   \
   SQLite  Trabajos  Conectores
             |      /       \
          Multimedia     Catálogo / Real-Debrid / Torrents
             |
       Carpetas del homelab
```

| Módulo | Responsabilidad | Tecnología propuesta |
| --- | --- | --- |
| Web | Interfaz, navegación, estados y controles de reproducción | Rust, Dioxus y WebAssembly |
| API | Sesiones, permisos, bibliotecas, fuentes y contratos de clientes | Rust, Axum y Tokio |
| Persistencia | Perfiles, metadatos, colecciones, trabajos y progreso | SQLite y migraciones desde Rust; pendiente |
| Trabajos | Escaneo, actualización de metadatos y tareas multimedia | Worker Rust separado con trabajos persistentes; pendiente |
| Multimedia | Inspección de formatos y conversión cuando sea necesaria | FFprobe y FFmpeg |
| Conectores | Adaptadores independientes para cada servicio | Traits y contratos Rust en el servidor; pendiente |
| Android TV | Aplicación instalable, mando y reproducción nativa | Rust y Dioxus; integración con el SDK Android por validar |
| Despliegue | Servicios, volúmenes y actualizaciones del homelab | Docker Compose |

Cargo gestiona el workspace. Las dependencias resueltas están fijadas en `Cargo.lock` y la toolchain en `rust-toolchain.toml`. No se adoptarán automáticamente nuevas versiones mayores. El SDK Android y las herramientas multimedia externas se integrarán desde Rust cuando llegue su etapa.

La reutilización de Dioxus en Android TV se validará con una prueba de navegación y reproducción. El puente al reproductor del dispositivo se definirá desde Rust y el SDK Android. La vista TV web actual no sustituye al APK ni verifica su compatibilidad.

### Estructura

```text
crates/
  soto-core/           Dominio, orígenes, navegación y preferencias
  soto-web/            Componentes Rust, estado de la interfaz y CSS
  soto-server/         Axum, archivos estáticos y comprobación de salud
docs/                  Plan y resultados de verificación
Dockerfile             Compilación y runtime separados
compose.yaml           Instalación con imagen precompilada y parámetros
compose.build.yaml     Compilación optativa desde el código
.env.example           Variables de instalación sin secretos
.github/workflows/     Comprobaciones automatizadas
```

Los workers, conectores, persistencia del homelab y el cliente Android se incorporarán en sus etapas, sin generar módulos vacíos ni dependencias innecesarias anticipadamente.

## Modelo de datos y contratos

Entidades principales: `User`, `Profile`, `Device`, `Library`, `MediaItem`, `MediaSource`, `Collection`, `PlaybackProgress`, `ProviderConfiguration` y `Job`.

`MediaItem` identifica una película, serie o episodio. `MediaSource` identifica un archivo o una fuente reproducible y conserva un `origin` obligatorio: `local` o `torrent`. Varias fuentes pueden pertenecer a una obra. Un cambio de ubicación del archivo no cambia su origen. El progreso se registra por perfil, obra y episodio; la selección de fuente se mantiene diferenciada.

Familias de API previstas:

| Familia | Finalidad |
| --- | --- |
| `/api/session`, `/api/profiles`, `/api/devices` | Acceso, perfiles y vinculación |
| `/api/libraries`, `/api/local` | Bibliotecas, escaneo y archivos locales |
| `/api/catalog`, `/api/search` | Metadatos y búsqueda con origen explícito |
| `/api/torrents`, `/api/jobs` | Fuentes online y estado de trabajos |
| `/api/playback`, `/api/progress` | Resolución de reproducción y progreso |
| `/api/collections`, `/api/providers` | Colecciones e integraciones |
| `/api/health` | Estado básico del servicio |

Los esquemas de solicitudes, respuestas, paginación y errores se documentarán antes de implementar cada familia. El navegador no recibirá rutas absolutas del servidor ni credenciales de proveedores.

## Configuración del homelab

**Configuración futura para multimedia; aún no implementada.** La interfaz se ejecuta con los comandos anteriores. Al incorporar bibliotecas y servicios, el despliegue deberá proporcionar:

1. Un host con Docker y Compose, espacio para datos persistentes y acceso a las carpetas multimedia.
2. Un volumen escribible para la base de datos y la configuración.
3. Carpetas Local montadas en modo de solo lectura por defecto.
4. Un volumen independiente para los archivos obtenidos desde Torrents.
5. Un directorio temporal con límite de espacio para las conversiones.
6. Un proxy con HTTPS para el acceso desde la red, y un dominio o nombre local estable.
7. Una cuenta personal de Real-Debrid solamente si se habilita esa integración.

### Variables propuestas

| Variable | Valor inicial o formato | Uso |
| --- | --- | --- |
| `SOTO_PUBLIC_URL` | `https://soto.example.com` | URL real configurada por el administrador |
| `SOTO_DATA_DIR` | `/data` | Base de datos y configuración persistentes |
| `SOTO_LOCAL_ROOTS` | `["/media/local"]` | Lista JSON de directorios permitidos para Local |
| `SOTO_TORRENTS_DIR` | `/media/torrents` | Archivos obtenidos mediante Torrents |
| `SOTO_TRANSCODE_DIR` | `/cache/transcode` | Trabajo temporal de FFmpeg |
| `SOTO_SESSION_SECRET_FILE` | `/run/secrets/soto_session` | Archivo de secreto para sesiones |
| `SOTO_REALDEBRID_TOKEN_FILE` | `/run/secrets/realdebrid_token` | Token del proveedor, opcional y solo en el servidor |
| `SOTO_TRUST_PROXY` | `false` | Activar únicamente con la cadena de proxies definida |
| `TZ` | `America/Asuncion` | Zona horaria de operación |

Los valores son ejemplos. La URL, las rutas y los secretos se adaptarán a cada instalación. No se guardarán tokens, contraseñas ni archivos `.env` reales en Git. Los proveedores de catálogo y de torrents se configurarán mediante adaptadores explícitos; no se incluirá un proveedor arbitrario como dependencia obligatoria.

### Reproducción y fuentes

- Priorizar reproducción directa de formatos compatibles con el cliente.
- Habilitar lectura por rangos para los archivos locales y reanudación del progreso.
- Inspeccionar audio, vídeo y subtítulos antes de decidir una conversión.
- Tratar FFmpeg y la aceleración por hardware como capacidades del servidor, no como garantías universales.
- Resolver Real-Debrid en el servidor, contemplando autenticación caducada, límites, archivos pendientes y fuentes inexistentes.
- Incorporar torrents directos como una etapa independiente, con motor, límites de disco, concurrencia y cancelación definidos antes de implementarla.
- Mantener los archivos de Torrents separados de Local durante todo el ciclo de vida.

## Seguridad y operación

- Sesiones con cookies HttpOnly y protección de solicitudes que modifican datos; acceso de administración restringido.
- Aislamiento entre perfiles y validación de toda entrada de API.
- Acceso a archivos limitado a las raíces configuradas, con comprobación de rutas reales y enlaces simbólicos.
- Conectores con destinos controlados, tiempos de espera y límites de respuesta; sin proxy abierto a URLs del usuario.
- Secretos del proveedor únicamente en el servidor, con filtrado de registros y respuestas.
- Trabajos persistentes, límites de recursos, cancelación y recuperación tras reinicios.
- Copias de seguridad de datos y configuración; los directorios temporales no forman parte del respaldo.
- Migraciones versionadas y procedimiento de restauración antes de actualizar instalaciones con datos reales.

El servicio no se expondrá directamente a Internet por defecto. La primera configuración deberá crear el administrador sin una contraseña predeterminada publicada.

## Plan de implementación

| Etapa | Entregable | Criterio de aceptación |
| --- | --- | --- |
| 1. Contratos y base visual | Estructura del proyecto, tokens, componentes, rutas y estados | Correspondencia visual con FigCraft, Local/Torrents separados y navegación por teclado |
| 2. Acceso y biblioteca local | Sesiones, perfiles, escaneo e indexación persistente | Archivos fuera de las raíces inaccesibles; escaneo recuperable y errores visibles |
| 3. Reproducción local | Reproducción directa, rangos, audio/subtítulos disponibles y progreso | Reanudar un archivo real y conservar el progreso al reiniciar el servidor |
| 4. Catálogo y Real-Debrid | Búsqueda, fichas, conectores y resolución de fuentes | Pruebas controladas de éxito, fuente ausente y credencial caducada |
| 5. Torrents y multimedia | Motor directo, trabajos, límites y conversión | Cancelación y recuperación verificadas, sin mezclar orígenes ni llenar el disco sin control |
| 6. Android TV | Vinculación, foco, mando y reproductor nativo | APK instalado y probado en un equipo real |
| 7. Distribución | Compose, documentación operativa y automatización | Instalación reproducible en un homelab limpio y restauración de una copia de seguridad |

Cada etapa entregará funcionalidad verificable. No se marcará una integración como operativa únicamente por mostrar su pantalla.

## Rendimiento y Docker

La primera etapa usa un único proceso Axum con runtime asíncrono de un hilo para servir archivos y salud. No hay base de datos, escaneos ni trabajo periódico. La compilación de release aplica LTO, elimina símbolos y ofrece un perfil WebAssembly orientado al tamaño. Los recursos tienen nombres con hash, caché inmutable y versiones gzip precomprimidas en Docker; el HTML se sirve sin caché persistente.

Compose limita el runtime a 128 MiB, 0,5 CPU y 64 procesos, utiliza un usuario sin privilegios, filesystem de solo lectura y elimina capabilities. Estos límites corresponden a la demostración; los workers multimedia necesitarán presupuestos propios medidos con archivos reales. El rendimiento se documentará con su entorno y medición, sin afirmar una optimización máxima no demostrada.

## Pruebas y GitHub

GitHub Actions comprobará formato, Clippy, pruebas, compilación WebAssembly y arranque de Docker. Las credenciales personales no se necesitarán para las pruebas normales: los futuros proveedores se probarán con respuestas controladas y habrá pruebas reales optativas para el propietario.

Recorridos prioritarios: mantener el origen al descargar, buscar en cada espacio, reproducir y reanudar, cancelar un trabajo, recuperar un reinicio, rechazar acceso no autorizado y navegar con mando. Se revisarán escritorio, móvil y TV, además de formatos y dispositivos reales para las capacidades multimedia.

La rama principal será `main`; los cambios de implementación se propondrán en ramas con revisión y comprobaciones. No habrá despliegue automático al homelab ni publicación automática de APK sin configurar ese proceso expresamente.

## Licencia y comunidad

El código propio se publica bajo [MIT](LICENSE). Las portadas y dependencias mantienen sus derechos y licencias: [avisos de terceros](THIRD_PARTY_NOTICES.md). Para contribuir consulta [CONTRIBUTING.md](CONTRIBUTING.md); para vulnerabilidades, [SECURITY.md](SECURITY.md).

Los tags `v*` publican una imagen comprobada en GHCR y una versión preliminar con imagen portátil, Compose y sumas SHA-256. La publicación manual desde `main` utiliza el tag de desarrollo `edge`. No modifica ninguna instalación existente ni actualiza automáticamente tu homelab.

## Decisiones pendientes antes de implementar

- Seleccionar los proveedores de catálogo y búsqueda de fuentes y revisar sus condiciones de uso.
- Elegir el motor para torrents directos y fijar cuotas de disco y concurrencia.
- Confirmar hardware del homelab, formatos prioritarios y modelo de Android TV para las pruebas.
- Validar el contenedor web frente a una presentación nativa para TV mediante una prueba técnica.
- Mantener actualizados los avisos de terceros al cambiar dependencias o incorporar recursos redistribuibles; el código propio ya usa MIT.

## Referencias técnicas

- [Dioxus: interfaz y estado en Rust](https://dioxuslabs.com/learn/0.7/)
- [Axum: servidor HTTP](https://docs.rs/axum/latest/axum/)
- [Docker: compilación en varias etapas](https://docs.docker.com/build/building/multi-stage/)
- [Real-Debrid: documentación de la API](https://api.real-debrid.com/)
- [Stremio: contrato de catálogos](https://github.com/Stremio/stremio-addon-sdk/blob/master/docs/api/requests/defineCatalogHandler.md)

Las referencias orientan el diseño. Cada integración se contrastará con la documentación vigente al implementarla.
