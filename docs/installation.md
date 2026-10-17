# Instalación y operación de Soto 0.2

## Alcance

Alfa funcional para Linux amd64 con Docker Engine, Compose v2 y un navegador con WebAssembly. El despliegue contiene API/interfaz y worker con FFmpeg/FFprobe. No requiere Rust ni Node.js en el host. ARM, un APK y Android TV nativo no están validados. Las comprobaciones de instalación de 0.2 siguen el pipeline de CI; las verificaciones de [0.1](releases/0.1.0-alpha.1.md) describen exclusivamente la demostración histórica.

## Instalar y configurar

```sh
git clone https://github.com/mateoschreiber/Soto.git
cd Soto
git checkout v0.2.0-alpha.1
cp .env.example .env
```

El tag y las imágenes deben estar publicados para ese procedimiento. Durante el desarrollo, conserva el checkout de la rama y construye con `compose.build.yaml`. Edita `.env` antes del arranque:

| Variable de Compose | Valor inicial | Uso |
| --- | --- | --- |
| `SOTO_IMAGE` | `ghcr.io/mateoschreiber/soto:0.2.0-alpha.1` | API e interfaz |
| `SOTO_WORKER_IMAGE` | `ghcr.io/mateoschreiber/soto:0.2.0-alpha.1-worker` | Worker con FFmpeg; misma versión que la API |
| `SOTO_LOCAL_PATH` | `./media/local` | Usa una ruta absoluta existente del host para medios reales |
| `SOTO_HOST_IP` / `SOTO_PORT` | `127.0.0.1` / `3000` | Publicación del puerto de la API |
| `SOTO_PUBLIC_URL` | vacío | Origen externo exacto, por ejemplo `https://soto.example.com` |
| `SOTO_MEMORY_LIMIT` / `SOTO_CPU_LIMIT` | `256m` / `0.50` | Presupuesto de la API |
| `SOTO_WORKER_MEMORY_LIMIT` / `SOTO_WORKER_CPU_LIMIT` | `1g` / `1.0` | Presupuesto del worker |

```sh
docker compose pull
docker compose up -d --wait
docker compose exec web /app/soto-server --print-setup-token
```

Abre [Soto local](http://127.0.0.1:3000) e introduce el token del comando. Crea el administrador y una contraseña de al menos 12 caracteres. El token es secreto y de un solo uso para esa instalación; no se publica una contraseña predeterminada. Después se elimina `/data/bootstrap-token`; el comando de impresión deja de servir para el inicio de sesión normal.

Añade una biblioteca seleccionando la raíz y una carpeta relativa, y ejecuta el escaneo. Un catálogo recién instalado estará vacío hasta importar archivos o configurar proveedores; no contiene títulos de demostración.

## Medios locales y recursos de red

Monta SMB/NFS en el **host** antes de arrancar Docker, por ejemplo en `/mnt/medios`, y establece `SOTO_LOCAL_PATH=/mnt/medios`. Las credenciales del montaje pertenecen al host; no se introducen en Soto ni en una URL de biblioteca. El contenedor recibe esa carpeta en `/media/local:ro`, tanto en la API como en el worker, con raíz lógica `local`.

El usuario 10001 debe poder recorrer directorios y leer los archivos. Comprueba permisos y disponibilidad del montaje antes de escanear. Una carpeta vacía en lugar del montaje puede producir un catálogo incompleto; no basta con que exista el directorio del host. Las rutas del catálogo son relativas a una raíz configurada, sin acceso libre al filesystem.

Para varias raíces, adapta los volúmenes y el mismo mapa JSON `SOTO_LOCAL_ROOTS` en ambos servicios. No basta con declarar una variable en `.env` si Compose no la referencia. No montes `/`, el socket Docker ni directorios de credenciales dentro de los servicios.

## Datos, cuotas y fuentes

| Volumen | Contenido | Respaldo |
| --- | --- | --- |
| `data` → `/data` | `soto.sqlite`, posibles archivos WAL/SHM, `master.key` y estado de configuración | Obligatorio, consistente y conjunto |
| `control` → `/run/soto` | `worker-token`, secreto entre API y worker | Conservar de forma protegida |
| `torrents` → `/torrents` | Descargas, sesión P2P y límites persistidos en `.session` | Según necesidad; imprescindible para reanudar esas descargas |
| `cache` → `/cache` | Salidas temporales de conversiones | Regenerable; no es biblioteca ni respaldo |

`master.key` permite descifrar secretos de proveedores guardados en SQLite. Perderla impide recuperar esos secretos; copiar solo la base de datos no es una restauración completa. La clave y la base juntas también permiten acceder a las credenciales cifradas: protege el respaldo como un secreto. `worker-token` es independiente de esa clave; el worker lo monta de solo lectura y no recibe la base de datos.

En Ajustes, asigna una cuota de caché mayor que cero antes de convertir. La conversión usa una sola tarea multimedia simultánea y produce HLS fMP4 con H.264/AAC. La reproducción directa depende del contenedor y los códecs del navegador. Las conversiones de flujo remoto no admiten búsqueda arbitraria; algunos MP4 requieren una entrada con búsqueda y pueden fallar con un error visible.

P2P comienza deshabilitado. Actívalo expresamente con cuota de Torrents mayor que cero y límites de descarga/subida. Los límites en KiB/s con valor cero significan sin límite de esa dirección. La cuota considera archivos seleccionados y espacio existente; no sustituye un límite de filesystem ni una alerta del host. Reserva espacio para SQLite, logs y otros servicios. Deshabilitar P2P pausa las descargas activas. Cancelar no elimina archivos personales del host.

Al añadir o resolver una fuente, elige el motor **P2P** o **Real-Debrid** explícitamente. Real-Debrid requiere el token personal del propietario; se configura mediante la interfaz y se almacena cifrado en el servidor. Una cuenta conectada, una entrada de catálogo o un torrent añadido no garantizan disponibilidad. Puede ser necesario seleccionar archivos y esperar su preparación; el estado debe consultarse antes de reproducir.

Añade solo los manifiestos de addons que quieras utilizar. No se instalan addons predeterminados ni se habilita un proveedor implícito. El acceso externo normal exige HTTPS y destinos públicos; no actives opciones de destinos privados para proveedores ajenos. Usa contenido y servicios autorizados para tu instalación.

Los perfiles se guardan en SQLite. En esta alfa, progreso y colecciones pertenecen al **usuario autenticado**, no a cada perfil; elegir o crear un perfil no establece una frontera adicional de privacidad ni un historial de reproducción independiente.

## Red y Portainer

El worker no publica puertos. La API escucha en loopback del host por defecto. Para un túnel:

```sh
ssh -L 3000:127.0.0.1:3000 usuario@servidor
```

Para un proxy HTTPS, sirve Soto en la raíz de un dominio y configura `SOTO_PUBLIC_URL` con ese origen exacto, sin una ruta como `/soto/`. Conserva las cabeceras de seguridad, cookies y `Content-Type`, `Content-Encoding`, `Vary` y `Cache-Control`. No vuelvas a comprimir respuestas gzip ni caches el HTML de forma persistente. Con proxy en el host, apunta a loopback; con proxy en Docker, usa una red común y `http://web:3000`. No publiques el puerto del worker ni lo abras en el router.

En Portainer Standalone, crea un stack desde este repositorio y la referencia `refs/tags/v0.2.0-alpha.1`, con `compose.yaml`. Define las dos imágenes de la misma versión, una ruta absoluta `SOTO_LOCAL_PATH` y las variables de red/recursos necesarias. Conserva los cuatro volúmenes y obtén el token desde la consola del servicio web con `/app/soto-server --print-setup-token`. No pegues el token ni credenciales de proveedores en el stack.

## Respaldo consistente

Detén ambos procesos antes de copiar SQLite para que no haya escrituras ni cambios de secretos durante la copia. Respalda **todo** `/data`, incluidos `master.key` y cualquier WAL/SHM existente, y el volumen `control`. Los siguientes comandos, desde el directorio de Compose, crean archivos sin compresión mediante herramientas de la imagen:

```sh
mkdir -p backups
chmod 700 backups
docker compose stop worker web
docker compose run --rm --no-deps --user 0:0 --entrypoint tar -v "$PWD/backups:/backup" web -C /data -cf /backup/soto-data.tar .
docker compose run --rm --no-deps --user 0:0 --entrypoint tar -v "$PWD/backups:/backup" web -C /run/soto -cf /backup/soto-control.tar .
cp .env compose.yaml backups/
chmod 600 backups/*
docker compose up -d --wait
```

Archiva también `torrents` con el worker detenido si necesitas conservar descargas y su sesión; los medios locales se respaldan con el procedimiento del almacenamiento del host. No es necesario respaldar `cache`. Lleva una copia cifrada fuera del host y prueba una restauración separada antes de confiar en el respaldo.

## Restaurar y actualizar

Restaura sobre volúmenes **nuevos y vacíos**, con las mismas imágenes y configuración del respaldo. No combines una base antigua con una clave nueva ni extraigas un respaldo encima de un SQLite en uso. Por ejemplo, usando un nombre de proyecto nuevo y el mismo directorio de Compose:

```sh
docker compose -p soto-restored create
docker compose -p soto-restored run --rm --no-deps --user 0:0 --entrypoint tar -v "$PWD/backups:/backup:ro" web -C /data -xf /backup/soto-data.tar
docker compose -p soto-restored run --rm --no-deps --user 0:0 --entrypoint tar -v "$PWD/backups:/backup:ro" web -C /run/soto -xf /backup/soto-control.tar
docker compose -p soto-restored up -d --wait
```

El proyecto de ejemplo crea volúmenes distintos; detén la instalación original o usa otro puerto antes de arrancar la copia. Conserva UID/GID 10001 para que los servicios puedan escribir sus volúmenes. Restaura también `torrents` si se respaldó. Verifica acceso, bibliotecas, progreso y credenciales; esta guía no certifica una restauración ya ensayada en tu entorno.

Antes de actualizar, crea el respaldo consistente y lee las notas. Fija `SOTO_IMAGE` y `SOTO_WORKER_IMAGE` a la misma versión o a sus respectivos digests y ejecuta `docker compose pull` y `docker compose up -d --wait`. Cambiar solo el tag no garantiza revertir cambios de esquema: para volver atrás usa las imágenes anteriores **y su respaldo compatible**. Nunca uses `docker compose down -v` como actualización rutinaria.

## Instalación portátil y compilación

Cuando la versión esté publicada, descarga juntos `soto-0.2.0-alpha.1-linux-amd64.tar.gz`, `soto-compose.yaml` y `SHA256SUMS` de la versión de GitHub. El archivo contiene las dos imágenes:

```sh
sha256sum -c SHA256SUMS
gzip -dc soto-0.2.0-alpha.1-linux-amd64.tar.gz | docker load
SOTO_LOCAL_PATH=/ruta/absoluta/a/medios docker compose -f soto-compose.yaml up -d --pull never --wait
docker compose -f soto-compose.yaml exec web /app/soto-server --print-setup-token
```

Para construir ambas imágenes desde el checkout:

```sh
docker compose -f compose.yaml -f compose.build.yaml up -d --build --wait
```

La compilación necesita más recursos y conectividad que el runtime. No exige instalar una toolchain Rust en el homelab cuando se usan imágenes precompiladas.

## Diagnóstico

```sh
docker compose ps
docker compose logs --tail=100 web worker
docker compose exec -T web /app/soto-server --healthcheck
docker compose exec -T worker /app/soto-worker --healthcheck
docker stats --no-stream
```

Los healthchecks comprueban disponibilidad del proceso/API interna; no certifican acceso al NAS ni disponibilidad de proveedores externos. No compartas registros que incluyan URLs firmadas, credenciales o rutas personales.

| Síntoma | Revisar |
| --- | --- |
| No abre desde otro equipo | Loopback, túnel/proxy, IP publicada y firewall |
| Configuración inicial rechazada | Token obtenido de esa instalación, contraseña mínima y ausencia de un administrador previo |
| El proxy permite leer pero rechaza cambios | Origen externo exacto en `SOTO_PUBLIC_URL` y cookies HTTPS |
| Biblioteca vacía o desconectada | Montaje del host, permisos de UID 10001 y estado real del escaneo |
| Fallo de conversión | Worker healthy, cuota de caché, formato admitido, recursos y error de la tarea |
| No inicia P2P | Activación explícita, cuota válida, límites, salida de red y estado de la tarea |
| Real-Debrid conectado sin reproducción | Archivos seleccionados, preparación real, token vigente y disponibilidad del proveedor |
| Se pierden credenciales al restaurar | Par original `soto.sqlite` + `master.key` y volumen control coherente |
| Interfaz antigua | Recarga y política de caché del HTML en el proxy |
