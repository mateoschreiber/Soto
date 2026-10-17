# Soto 0.2: instalación vacía y servicios reales

Plan actualizado para alpha.2: sustituir datos de demostración por configuración persistente y abrir directamente Inicio, sin cuentas ni autenticación; mantener bibliotecas montadas SMB/NFS, reproducción directa/conversión, Real-Debrid y protocolo HTTP Stremio con P2P Rust. Soto es un servicio de un único hogar y su puerto debe permanecer en loopback o una LAN de confianza. Conservar diseño y orígenes Local/Torrents. No modificar versiones ya publicadas ni incluir un APK.

## Entregas

1. SQLite, propietario local único, estado vacío real y migración de colecciones/progreso desde cuentas anteriores; eliminar sesiones y perfiles legados.
2. Bibliotecas autorizadas, exploración relativa, escaneo incremental/cancelable, FFprobe y detección de desconexión.
3. Reproducción autorizada con rangos, progreso, subtítulos y conversión FFmpeg en worker separado.
4. Cuenta Real-Debrid validada, secreto cifrado, sincronización, selección de archivos y resolución de fuentes.
5. Complementos HTTP Stremio explícitos, catálogo/metadatos/fuentes/subtítulos y P2P mediante librqbit sin interfaz de terceros.
6. Compose persistente y limitado, pruebas, revisión de seguridad y distribución 0.2-alpha.

## Reglas

- Código propio Rust; FFmpeg/FFprobe como procesos externos sin shell.
- Solo configuración visual compatible migra desde localStorage; los datos ficticios no se importan.
- SQLite y clave de cifrado en almacenamiento local /data; medios locales de solo lectura, Torrents separado y caché con cuota.
- Ningún proveedor instalado ni solicitud externa al arrancar una instalación vacía.
- El navegador nunca recibe secretos ni rutas absolutas del servidor.
- La disponibilidad y las acciones corresponden a operaciones reales; no se sustituye un error por una simulación.

## Verificación

Pruebas de instalación sin autenticación, persistencia/migración, seguridad de rutas y URLs, desconexión de montajes, Range/progreso, conversión/cancelación, proveedores y disco lleno. CI usa respuestas controladas y medios generados. Credenciales reales solo para pruebas optativas. Validación de navegador y contenedor antes de publicar.
