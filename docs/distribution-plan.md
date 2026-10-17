# Distribución de la demostración

Objetivo: instalar Soto en Docker Intel/AMD sin compilar en el homelab, con configuración estándar para Compose y Portainer. El usuario autorizó repositorio público y licencia MIT. No se amplía el alcance multimedia de la etapa 1.

1. Optimizar entrega HTTP: gzip preparado durante el build, sin compresión por petición; caché inmutable únicamente para recursos con huella; variar por Accept-Encoding. Probar respuestas comprimidas, actualizaciones y rutas inválidas.
2. Separar Compose de instalación y compilación. Parametrizar IP, puerto, imagen, memoria, CPU y logs. Conservar usuario sin privilegios, raíz de solo lectura y red bridge.
3. Publicar una imagen Linux amd64 versionada y comprobada, con caché de compilación en GitHub Actions. No prometer ARM sin probarlo.
4. Documentar instalación, Portainer, red, actualizaciones, reversión, persistencia y solución de problemas. Añadir licencia, guía de contribución, seguridad y notas de la demostración.
5. Revisar archivos e historial antes de hacer público. Ejecutar pruebas, auditoría y CI; publicar la versión solo después de comprobar el contenedor. Verificar descarga anónima.

La configuración pública no contiene IP, dominio, rutas privadas, credenciales ni inventario del homelab. No se modifican sus servicios existentes.
