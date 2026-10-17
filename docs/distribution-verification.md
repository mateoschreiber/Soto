# Verificación de la distribución pública

17 de octubre de 2026. Versión `v0.1.0-alpha.1`, commit `f82215521a83ec46c46c53a6f85166496fee37a0`.

## Publicación

- Repositorio público, acceso anónimo HTTP 200 y licencia identificada como MIT por GitHub.
- Reporte privado de vulnerabilidades habilitado.
- [Revisión previa: Rust y Docker](https://github.com/mateoschreiber/Soto/actions/runs/38999550888): aprobada.
- [Validación de main](https://github.com/mateoschreiber/Soto/actions/runs/38999721169): aprobada.
- [Publicación de la versión](https://github.com/mateoschreiber/Soto/actions/runs/38999726374): aprobada. Comprueba la imagen antes de subirla: salud, UID, HTTP, descarga WASM con gzip válido y cabecera Vary.
- GHCR muestra el paquete público. La imagen se descargó en el homelab con un directorio de configuración Docker vacío, sin credenciales de registro.
- Los recursos de GitHub Release se descargaron sin autenticación y sus SHA-256 coinciden con `SHA256SUMS`. El archivo portátil contiene el mismo nombre y tag utilizados por Compose.

Imagen fijable por digest:

```text
ghcr.io/mateoschreiber/soto@sha256:079b40233012ec19f367d96d490eadfb4fb62d285ab0bb21ad3bb3273a57acbe
```

## Pruebas y optimización

Las 14 pruebas Rust pasan, junto con rustfmt, Clippy de host/WASM y cargo-audit sin vulnerabilidades ni advertencias. Se añadieron pruebas de negociación gzip/identity, Vary, caché por huella y rutas inválidas. Dos pruebas fallaron antes de corregir caché y Vary.

Se eliminó la compresión dinámica: el build prepara gzip y el runtime entrega el archivo adecuado. La compilación Docker conserva la caché de Cargo y limpia la salida web anterior para no acumular bundles obsoletos. El contenedor incluye los avisos de licencia de las dependencias utilizadas.

## Prueba temporal en el homelab

Linux x86_64 Intel, Docker Engine 29.8.2 y Compose 5.6.0, con 2 CPU y unos 4 GB de RAM. Se usó un contenedor temporal aislado, un puerto aleatorio publicado únicamente en loopback y ningún volumen.

- Salud HTTP y binaria correctas; HTML servido.
- Usuario/grupo 10001 y raíz de solo lectura confirmados.
- Límite de memoria 134.217.728 bytes y CPU 0,5 confirmados.
- Archivo de licencias presente en la imagen.
- Muestra puntual en reposo: **816 KiB de memoria**, **0,00 % CPU**, 2 procesos. No es un benchmark de concurrencia ni multimedia.
- Cierre y eliminación del contenedor temporal confirmados. Los servicios existentes no se modificaron; no queda una instalación permanente de Soto.

## Tamaños observados

- Imagen según `docker image inspect` del runner de publicación: **83.761.331 bytes**.
- Archivo portátil comprimido: **34.971.440 bytes**.
- SHA-256 del archivo portátil: `ccf5d1d251754323663227d5aaac9fbfdcfb8453f45c9b649928cbd706d8a71f`.
- El daemon del homelab informa 125.434.089 bytes para la imagen; el tamaño indicado por distintos almacenes Docker no se usa aquí como una comparación de ahorro entre versiones.

## Límites

La distribución sigue siendo una demostración: sin autenticación, reproducción, escaneo, descargas, proveedores conectados ni APK. ARM y un dispositivo Android TV real no se han validado. La instalación permanente y la elección de IP/dominio/proxy quedan a cargo del administrador según la guía estándar.
