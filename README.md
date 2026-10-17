# Soto

Aplicación multimedia para un homelab: biblioteca de archivos locales, descubrimiento de películas y series, fuentes de torrents y conexión a Real-Debrid, con una interfaz web y un futuro cliente instalable en Android TV.

**Estado: planificación.** Este repositorio contiene el diseño del proyecto y su configuración propuesta. Todavía no contiene una aplicación ejecutable, imágenes Docker, APK ni integraciones implementadas. Los nombres de módulos y variables de este documento forman el contrato inicial de implementación; podrán ajustarse de forma documentada antes de cada etapa.

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
| Web | Interfaz, navegación, estados y controles de reproducción | React, TypeScript y Vite |
| API | Sesiones, permisos, bibliotecas, fuentes y contratos de clientes | TypeScript y Fastify |
| Persistencia | Perfiles, metadatos, colecciones, trabajos y progreso | SQLite con migraciones |
| Trabajos | Escaneo, actualización de metadatos y tareas multimedia | Worker separado con trabajos persistentes |
| Multimedia | Inspección de formatos y conversión cuando sea necesaria | FFprobe y FFmpeg |
| Conectores | Adaptadores independientes para cada servicio | Interfaces tipadas en el servidor |
| Android TV | Aplicación instalable, mando y reproducción nativa | Kotlin y Media3; reutilización de interfaz por validar |
| Despliegue | Servicios, volúmenes y actualizaciones del homelab | Docker Compose |

Las versiones concretas y el gestor de paquetes se fijarán al iniciar la implementación y quedarán registrados en el lockfile. No se adoptarán automáticamente nuevas versiones mayores.

La estrategia de reutilización de la interfaz en Android TV se validará con una prueba de navegación y reproducción. El cliente podrá reutilizar la web mediante un contenedor y un puente de reproducción si la prueba lo permite; en caso contrario se implementará la capa de presentación nativa manteniendo los contratos de la API. Una PWA no sustituye al APK de Android TV.

### Estructura futura

```text
apps/
  web/                 Interfaz web
  api/                 API y sesiones
  worker/              Trabajos en segundo plano
  android-tv/          Cliente instalable
packages/
  contracts/           Esquemas de API y tipos compartidos
  ui/                  Componentes y tokens visuales
  connectors/          Adaptadores de catálogo y fuentes
deploy/                Docker, proxy y ejemplos de despliegue
docs/                  Decisiones, contratos y operación
tests/                 Integración y recorridos de usuario
```

Estas carpetas todavía no se han generado.

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

**Configuración prevista; aún no implementada.** No hay comandos de instalación disponibles en esta etapa. El despliegue deberá proporcionar:

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
| `SOTO_HOST` | `127.0.0.1` fuera de Docker | Dirección de escucha; dentro del contenedor podrá ser `0.0.0.0` |
| `SOTO_PORT` | `3000` | Puerto interno de la API y la web |
| `SOTO_PUBLIC_URL` | `https://soto.example.com` | URL real configurada por el administrador |
| `SOTO_DATA_DIR` | `/data` | Base de datos y configuración persistentes |
| `SOTO_LOCAL_ROOTS` | `["/media/local"]` | Lista JSON de directorios permitidos para Local |
| `SOTO_TORRENTS_DIR` | `/media/torrents` | Archivos obtenidos mediante Torrents |
| `SOTO_TRANSCODE_DIR` | `/cache/transcode` | Trabajo temporal de FFmpeg |
| `SOTO_SESSION_SECRET_FILE` | `/run/secrets/soto_session` | Archivo de secreto para sesiones |
| `SOTO_REALDEBRID_TOKEN_FILE` | `/run/secrets/realdebrid_token` | Token del proveedor, opcional y solo en el servidor |
| `SOTO_TRUST_PROXY` | `false` | Activar únicamente con la cadena de proxies definida |
| `SOTO_LOG_LEVEL` | `info` | Nivel de registro sin secretos ni enlaces privados |
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

## Pruebas y GitHub

Al incorporar código, GitHub Actions ejecutará comprobación de tipos, análisis estático, pruebas y compilación. Las credenciales personales no se necesitarán para las pruebas normales: los proveedores se probarán con respuestas controladas y habrá pruebas reales optativas para el propietario.

Recorridos prioritarios: mantener el origen al descargar, buscar en cada espacio, reproducir y reanudar, cancelar un trabajo, recuperar un reinicio, rechazar acceso no autorizado y navegar con mando. Se revisarán escritorio, móvil y TV, además de formatos y dispositivos reales para las capacidades multimedia.

La rama principal será `main`; los cambios de implementación se propondrán en ramas con revisión y comprobaciones. No habrá despliegue automático al homelab ni publicación automática de APK sin configurar ese proceso expresamente.

## Decisiones pendientes antes de implementar

- Seleccionar los proveedores de catálogo y búsqueda de fuentes y revisar sus condiciones de uso.
- Elegir el motor para torrents directos y fijar cuotas de disco y concurrencia.
- Confirmar hardware del homelab, formatos prioritarios y modelo de Android TV para las pruebas.
- Validar el contenedor web frente a una presentación nativa para TV mediante una prueba técnica.
- Elegir una licencia para el código y revisar las licencias de dependencias y recursos antes de redistribuirlos.

## Referencias técnicas

- [React: sincronización con sistemas externos](https://react.dev/reference/react/useEffect)
- [Node.js: servidor HTTP](https://nodejs.org/api/http.html)
- [Real-Debrid: documentación de la API](https://api.real-debrid.com/)
- [Stremio: contrato de catálogos](https://github.com/Stremio/stremio-addon-sdk/blob/master/docs/api/requests/defineCatalogHandler.md)

Las referencias orientan el diseño. Cada integración se contrastará con la documentación vigente al implementarla.
