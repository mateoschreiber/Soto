# Recursos, herramientas y dependencias

La licencia [MIT](LICENSE) de Soto cubre el código y los recursos originales del proyecto. Las dependencias Rust, herramientas y paquetes del sistema conservan sus propias licencias; su inclusión no las convierte en MIT.

`Cargo.lock` identifica las versiones Rust resueltas. `THIRD_PARTY_LICENSES.json` recoge los avisos y textos de las dependencias incluidas en los destinos publicados; debe regenerarse y revisarse cuando cambien dependencias, destinos o características. El historial del inventario conserva notas de procedencia para paquetes cuyos archivos publicados no incluían su licencia. El servidor y el worker incluyen esos avisos en `/app/licenses/`. El motor Rust librqbit 9.0.1 declara Apache-2.0 y debe figurar con sus dependencias en el inventario correspondiente.

El worker instala **FFmpeg y FFprobe como ejecutables externos de Debian**. La compilación del paquete FFmpeg con componentes GPL conserva las condiciones GPL aplicables a ese paquete y a sus bibliotecas; la licencia MIT del código propio de Soto no lo relicencia. Otros paquetes Debian conservan igualmente sus licencias. Consulta los avisos y copyright de los paquetes en `/usr/share/doc/`, la configuración de `ffmpeg -version` y las fuentes de la versión exacta distribuida por Debian al redistribuir una imagen modificada. El inventario JSON de Cargo no sustituye los avisos de los paquetes del sistema.

Las imágenes, metadatos, marcas y medios obtenidos de bibliotecas, addons o proveedores pertenecen a sus respectivos titulares. Soto no incluye películas, portadas comerciales ni proveedores predeterminados en la instalación funcional. Los patrones y tonos generados por las pruebas no proceden de una película ni requieren una cuenta externa. Las referencias y verificaciones de la demostración 0.1 son históricas; no otorgan derechos de redistribución sobre imágenes remotas que aquella solicitaba.

Plex, Stremio y Real-Debrid se mencionan para describir referencias o interoperabilidad. Soto es independiente, sin afiliación ni derechos sobre esas marcas. Configurar una integración no concede derechos de acceso, reproducción o distribución de su contenido.
