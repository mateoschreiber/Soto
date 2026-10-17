# Seguridad

Soto 0.2 es una alfa funcional con autenticación, almacenamiento persistente e integraciones opcionales. Las comprobaciones de CI y distribución siguen en validación; no constituyen una auditoría de seguridad. La demostración [0.1](docs/releases/0.1.0-alpha.1.md) se conserva como publicación histórica y no debe tratarse como un servidor multimedia autenticado.

Informa de vulnerabilidades mediante **Security → Report a vulnerability** en GitHub. Incluye versión o digest, impacto y reproducción mínima sin credenciales ni datos personales. Evita publicar secretos o detalles explotables en issues públicos. No hay compromiso de plazo de respuesta.

La instalación crea el administrador mediante un token obtenido por CLI; no existe una contraseña predeterminada. Las sesiones usan cookies HttpOnly y las mutaciones requieren comprobación de origen y cabecera propia. Configura el origen HTTPS exacto en `SOTO_PUBLIC_URL` al usar un proxy. Conserva loopback o acceso mediante túnel hasta configurar tu red; el worker no debe publicar puertos.

API y worker usan el UID 10001, raíz de solo lectura, capabilities eliminadas y límites independientes. Los medios locales se montan de solo lectura dentro de raíces permitidas. SMB/NFS se monta en el host. Estas medidas reducen exposición, pero no certifican aislamiento frente a todas las vulnerabilidades del sistema o de los decodificadores multimedia.

Los tokens de proveedores se cifran en SQLite con `/data/master.key`. Respalda juntos la base, la clave y los archivos auxiliares con los procesos detenidos. Protege ese respaldo: contiene los materiales necesarios para recuperar los secretos. El volumen `control` contiene otro secreto, `worker-token`, compartido únicamente entre API y worker. No expongas ninguno en Git, capturas, registros, URLs públicas o informes.

P2P es optativo y requiere cuota y límites explícitos; puede comunicar tu dirección IP a pares y trackers. Los addons y proveedores realizan conexiones externas configuradas por el administrador. No habilites destinos privados para proveedores no confiables ni asumas que una cuenta conectada garantiza disponibilidad o derechos de acceso.

Los perfiles persistidos no son una frontera adicional de autorización: progreso y colecciones se asocian al usuario autenticado. No afirmes privacidad independiente por perfil. Consulta [instalación y respaldo](docs/installation.md) antes de almacenar datos reales.
