# Instalación y operación

## Requisitos y alcance

Docker Engine con Docker Compose v2 o posterior, Linux amd64 (Intel/AMD de 64 bits) y un navegador con WebAssembly. La imagen Docker no requiere Rust, Node.js, FFmpeg, base de datos ni volúmenes. ARM y Android TV instalable no están validados en esta versión.

`0.1.0-alpha.1` es una **demostración de interfaz**: navegación, Local/Torrents, filtros, fichas, colecciones, preferencias y modo TV web. No reproduce películas, escanea carpetas, descarga torrents ni conecta Real-Debrid. Todos los títulos y fuentes son ejemplos.

## Docker Compose

```sh
git clone https://github.com/mateoschreiber/Soto.git
cd Soto
git checkout v0.1.0-alpha.1
cp .env.example .env
docker compose pull
docker compose up -d --wait
docker compose ps
```

Abrir `http://127.0.0.1:3000` en el servidor. Desde otro equipo se puede usar un túnel `ssh -L 3000:127.0.0.1:3000 usuario@servidor` y abrir esa misma URL localmente.

La imagen versionada se descarga de GHCR sin compilar en el homelab. No se usa `latest` ni actualización automática. Para instalaciones reproducibles, establece `SOTO_IMAGE=ghcr.io/mateoschreiber/soto@sha256:...` con el digest publicado por el registro.

## Acceso desde la LAN

En `.env`, cambia `SOTO_HOST_IP` por la IPv4 LAN del servidor. Por ejemplo, `192.168.1.20` permite abrir `http://192.168.1.20:3000` desde la misma red. Es un ejemplo; usa tu propia dirección. También se permite `0.0.0.0`, que publica en todas las interfaces IPv4.

El puerto 3000 es configurable con `SOTO_PORT`. No se necesita `network_mode: host`. La red bridge y el puerto publicado permiten convivir con otros servicios. No abras el puerto en el router hacia Internet: esta demostración aún no tiene autenticación.

## Portainer

1. Elige el entorno **Docker Standalone**, abre **Stacks → Add stack** y llama al stack `soto`.
2. Usa **Repository** con `https://github.com/mateoschreiber/Soto`, referencia `refs/tags/v0.1.0-alpha.1` y ruta Compose `compose.yaml`. También puedes pegar `compose.yaml` en el editor web: es autocontenido y no necesita archivos externos.
3. Define las variables del stack: `SOTO_HOST_IP` (IP LAN para acceso remoto), `SOTO_PORT`, y opcionalmente `SOTO_MEMORY_LIMIT`, `SOTO_CPU_LIMIT` y `SOTO_IMAGE`.
4. Despliega y comprueba el estado healthy. No añadas carpetas multimedia ni credenciales: esta versión no las utiliza.

## Proxy inverso

Sirve Soto en la raíz de un dominio o subdominio; los recursos utilizan `/assets/` y la navegación utiliza fragmentos `#/...`. No se admite instalarlo bajo `/soto/` mediante una simple reescritura.

- Proxy en el host: conserva `SOTO_HOST_IP=127.0.0.1` y apunta a `http://127.0.0.1:3000`.
- Proxy en otro contenedor: su `127.0.0.1` corresponde al propio proxy. Conecta ambos servicios a una red Docker común y usa `http://web:3000`, o apunta al puerto publicado en una IP del host accesible desde esa red. La red y las etiquetas dependen del proxy que utilices.
- Termina HTTPS en el proxy y conserva CSP, Content-Type, Content-Encoding, Vary y Cache-Control. No fuerces caché persistente para el HTML ni comprimas de nuevo una respuesta que ya trae Content-Encoding.
- No requiere WebSocket en esta etapa. Para publicación fuera de una red de confianza, añade control de acceso en el proxy.

## Recursos y datos

Valores predeterminados: **128 MiB**, **0,5 CPU**, **64 procesos**, raíz de solo lectura, usuario **10001**, capabilities eliminadas, logs limitados a dos archivos de 10 MB y 15 segundos para cierre ordenado. Se pueden adaptar memoria y CPU desde `.env`. Los límites son para la demostración estática; no dimensionan los futuros trabajos multimedia.

Las preferencias y colecciones viven en `localStorage` del navegador, no en el contenedor ni en una cuenta. Cambiar el dominio/puerto crea otro origen de almacenamiento. Borrar los datos del navegador elimina esas preferencias. Los perfiles de ejemplo no aíslan colecciones por usuario. Respalda `.env` y cualquier configuración de proxy; esta etapa no tiene una base de datos que respaldar.

Las imágenes se solicitan directamente desde el navegador a Metahub. Puedes desactivarlas en **Servidor**; se muestran fondos propios. La aplicación puede arrancar aunque ese proveedor no esté disponible.

## Actualizar y volver atrás

Lee las notas de versión y conserva el tag o digest anterior. En `.env`, cambia `SOTO_IMAGE` por la versión deseada:

```sh
docker compose pull
docker compose up -d --wait
docker compose logs --tail=50 web
```

Para volver atrás, restaura el tag/digest anterior y repite esos comandos. Portainer permite editar la variable de imagen y actualizar el stack con la descarga de imagen activada. Nunca uses `docker compose down -v` como procedimiento rutinario de actualización.

## Instalación portátil sin registro

La [versión de GitHub](https://github.com/mateoschreiber/Soto/releases/tag/v0.1.0-alpha.1) adjunta `soto-0.1.0-alpha.1-linux-amd64.tar.gz`, `soto-compose.yaml` y `SHA256SUMS`. Descárgalos juntos y ejecuta:

```sh
sha256sum -c SHA256SUMS
gzip -dc soto-0.1.0-alpha.1-linux-amd64.tar.gz | docker load
docker compose -f soto-compose.yaml up -d --pull never --wait
```

La imagen cargada tiene el mismo nombre y tag usados por Compose. Este método permite instalar sin una cuenta de GHCR o transferir el archivo a un servidor sin acceso al registro. Las imágenes remotas siguen necesitando conectividad del navegador.

## Compilar desde el código

```sh
docker compose -f compose.yaml -f compose.build.yaml up -d --build --wait
```

La compilación necesita más RAM, CPU, disco y conexión que el runtime. Para un homelab pequeño, usa la distribución precompilada. BuildKit conserva la caché de Cargo durante compilaciones sucesivas; la imagen de ejecución contiene únicamente los artefactos finales.

## Diagnóstico

```sh
docker compose ps
docker compose logs --tail=100 web
docker compose exec -T web /app/soto-server --healthcheck
docker stats --no-stream
```

| Síntoma | Comprobación |
| --- | --- |
| No abre desde otro equipo | IP publicada: `127.0.0.1` solo acepta conexiones del host; revisar `SOTO_HOST_IP`, puerto y firewall |
| Puerto ocupado | Cambiar `SOTO_PORT`, sin cambiar el puerto interno 3000 |
| Error al descargar GHCR | Confirmar nombre/tag; usar el archivo portátil de GitHub Release |
| `exec format error` | Confirmar host Linux amd64; ARM no está publicado como compatible |
| Portadas ausentes | Verificar acceso del navegador a Metahub o desactivar imágenes remotas |
| Interfaz antigua tras actualizar | Recargar la página; el proxy no debe cachear el HTML |
| OOM/reinicio del contenedor | Consultar logs y eventos Docker antes de aumentar `SOTO_MEMORY_LIMIT` |
| Botones de reproducción muestran un aviso | Es el comportamiento previsto de la demostración; la integración no está implementada |

Fuentes oficiales: [Compose](https://docs.docker.com/reference/compose-file/services/), [Portainer Stacks](https://docs.portainer.io/user/docker/stacks/add) y [GHCR](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry).
