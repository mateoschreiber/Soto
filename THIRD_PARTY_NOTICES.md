# Recursos y dependencias

La licencia MIT de Soto cubre el código y los recursos originales del proyecto. Las dependencias Rust conservan sus licencias; sus versiones y repositorios se identifican mediante `Cargo.lock` y `cargo metadata`.

`THIRD_PARTY_LICENSES.json` contiene los textos de licencia de las dependencias normales y de compilación utilizadas por el servidor Linux amd64 y la interfaz WebAssembly. Se generó con cargo-bundle-licenses 4.2.0 y se filtró con `cargo tree --edges normal,build` para esos destinos. Las licencias ausentes en los paquetes de Dioxus y Gloo se obtuvieron de sus repositorios. Para Sledgehammer y Warnings, cuyos paquetes declaran MIT pero no incluyen el archivo, se conserva el texto estándar MIT, la atribución del manifiesto y una nota explícita de procedencia. El contenedor incluye estos avisos en `/app/licenses/`. Al cambiar dependencias se debe actualizar y revisar este archivo.

La interfaz solicita imágenes de demostración a Metahub (`images.metahub.space` y su redirección `live.metahub.space`). Las portadas, fotografías, títulos y marcas pertenecen a sus respectivos titulares. No se incluyen las imágenes en Git, en la imagen Docker ni en los archivos de la versión. La licencia MIT de Soto no concede derechos sobre ellas. Las imágenes se pueden desactivar en Servidor.

Plex, Stremio y Real-Debrid se mencionan como referencias o integraciones previstas. Soto es un proyecto independiente, sin afiliación ni código copiado de esos productos. Esta versión no incorpora películas ni proveedores de reproducción.
