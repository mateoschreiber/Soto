# Soto

[![Rust y Docker](https://github.com/mateoschreiber/Soto/actions/workflows/check.yml/badge.svg)](https://github.com/mateoschreiber/Soto/actions/workflows/check.yml)
[![Licencia MIT](https://img.shields.io/badge/licencia-MIT-green.svg)](LICENSE)

Soto organiza medios de un homelab con bibliotecas locales, catálogo persistente, reproducción con progreso, conversión multimedia y fuentes HTTP, Real-Debrid o P2P. Local y Torrents conservan sus orígenes separados.

**Estado: 0.2.0-alpha.1, alfa funcional en validación.** Las comprobaciones de CI y distribución son requisitos de publicación; este documento no certifica una instalación limpia ya verificada. Las cuentas Real-Debrid y los proveedores externos requieren configuración y comprobación propias. No hay APK ni compatibilidad Android TV certificada.

## Instalación

Requiere Linux amd64, Docker Engine y Compose v2. El homelab utiliza dos imágenes precompiladas de la misma versión; no necesita Rust ni Node.js.

```sh
git clone https://github.com/mateoschreiber/Soto.git
cd Soto
git checkout v0.2.0-alpha.1
cp .env.example .env
```

Edita `.env`: configura `SOTO_LOCAL_PATH` con una carpeta absoluta del host que contenga tus medios. Una carpeta SMB/NFS debe estar montada primero en el host. Si la versión todavía no está publicada, usa la rama de desarrollo y el comando de compilación indicado más abajo.

```sh
docker compose pull
docker compose up -d --wait
docker compose exec web /app/soto-server --print-setup-token
```

Abre [Soto local](http://127.0.0.1:3000), introduce el token obtenido por CLI y crea el administrador con una contraseña de al menos 12 caracteres. El token no es una contraseña predeterminada y desaparece tras completar la configuración. Añade una biblioteca de la raíz local y ejecuta su escaneo.

El puerto del host escucha en loopback por defecto; el worker no publica puertos. Para acceso remoto, utiliza un túnel SSH o un proxy HTTPS y configura `SOTO_PUBLIC_URL` con el origen externo exacto.

Consulta la [guía operativa](docs/installation.md) para volúmenes, respaldo/restauración, Portainer, cuotas y diagnóstico, y las [notas de 0.2](docs/releases/0.2.0-alpha.1.md). La [versión 0.1](docs/releases/0.1.0-alpha.1.md), su [verificación visual](docs/stage-1-verification.md) y su [verificación de distribución](docs/distribution-verification.md) son documentos históricos de la demostración.

## Funcionalidad y límites

- SQLite conserva bibliotecas, metadatos, perfiles, colecciones, tareas y progreso. Los perfiles son registros persistentes; el progreso y las colecciones se asocian al usuario autenticado, sin aislamiento de reproducción por perfil.
- Los medios locales se montan de solo lectura. El worker inspecciona formatos y convierte cuando se solicita; la reproducción directa depende de los códecs del navegador.
- En Ajustes, asigna una cuota de caché mayor que cero para convertir. Se permite una tarea multimedia simultánea, con HLS fMP4, H.264 y AAC. Los flujos remotos no admiten búsqueda arbitraria durante la conversión y algunos contenedores requieren entrada con búsqueda.
- P2P comienza deshabilitado y requiere activación explícita, cuota de disco y límites de transferencia. Al añadir una fuente se elige expresamente P2P o Real-Debrid; no se cambia de motor en silencio.
- Real-Debrid utiliza un token personal configurado en el servidor y cifrado en reposo. Una cuenta conectada o un título en un catálogo no implica que exista un archivo disponible.
- Los addons se añaden mediante una URL de manifiesto compatible; no se instalan proveedores predeterminados ni se llenan catálogos con ejemplos. Utiliza únicamente medios y servicios a los que tengas derecho de acceso.

La API y la interfaz usan por defecto 256 MiB y 0,5 CPU; el worker, 1 GiB y 1 CPU. Son límites configurados, no una medición de rendimiento garantizada.

## Desarrollo

La aplicación, sus primitivas y la interfaz están escritas en Rust: Axum/Tokio, SQLite, librqbit y Dioxus. El navegador ejecuta WebAssembly con enlaces JavaScript generados por Dioxus y un adaptador de reproducción; FFmpeg/FFprobe son procesos externos del worker. CSS/SVG, YAML y scripts de verificación forman parte del repositorio; no hay un servidor Node.js de aplicación.

Instala la toolchain de `rust-toolchain.toml`, Dioxus CLI 0.7.10 y FFmpeg/FFprobe. En dos terminales, con las mismas rutas y secreto interno (creado al iniciar el servidor):

```sh
dx build -p soto-web --platform web --profile wasm-release --release --locked
cargo run --release --locked -p soto-server --bin soto-server
```

```sh
cargo run --release --locked -p soto-server --bin soto-worker
```

Los valores locales predeterminados son `data`, `torrents`, `cache` y `http://127.0.0.1:3002` para el worker. Configura `SOTO_LOCAL_ROOTS='{"local":"/ruta/absoluta/a/medios"}'` en ambos procesos antes de iniciarlos. En Windows, usa la sintaxis de variables de PowerShell y un enlazador disponible para la toolchain.

Para construir los dos contenedores desde el código:

```sh
docker compose -f compose.yaml -f compose.build.yaml up -d --build --wait
```

Comprobaciones del proyecto:

```sh
cargo fmt --all --check
cargo test --locked --workspace
cargo clippy --locked -p soto-core -p soto-server --all-targets -- -D warnings
cargo clippy --locked -p soto-web --target wasm32-unknown-unknown -- -D warnings
python3 scripts/docker-smoke.py --image soto:local --worker-image soto:worker-local
```

La última prueba requiere las imágenes construidas y Docker; genera sus propios medios y comprueba configuración inicial, escaneo, rangos reales, conversión y persistencia tras reiniciar. No utiliza una cuenta externa.

El código propio se distribuye bajo [MIT](LICENSE). Las dependencias y las herramientas del sistema conservan sus licencias: [avisos de terceros](THIRD_PARTY_NOTICES.md). Consulta [CONTRIBUTING.md](CONTRIBUTING.md) y [SECURITY.md](SECURITY.md).
