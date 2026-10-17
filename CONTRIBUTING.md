# Contribuir a Soto

Abre un issue con el problema y el resultado esperado antes de cambios grandes. Incluye versión/digest, arquitectura, navegador y reproducción mínima sin secretos ni medios personales. Usa una rama y un PR con alcance revisable.

La aplicación, los contratos y la interfaz son Rust. Dioxus genera WebAssembly y enlaces JavaScript; el adaptador del navegador, CSS/SVG, YAML y scripts de comprobación acompañan esa implementación. El worker llama a FFmpeg/FFprobe como herramientas externas. No añadas un servidor Node.js ni una aplicación paralela sin una decisión explícita.

Mantén Local y Torrents separados, la selección explícita de motor P2P/Real-Debrid y la ausencia de datos o proveedores predeterminados simulados. Una pantalla o un estado conectado no demuestran disponibilidad. Los perfiles persistidos no aíslan el progreso: actualmente pertenece al usuario autenticado.

Instala la toolchain y Dioxus según el [README](README.md), y FFmpeg/FFprobe para pruebas multimedia. Ejecuta:

```sh
cargo fmt --all --check
cargo test --locked --workspace
cargo clippy --locked -p soto-core -p soto-server --all-targets -- -D warnings
cargo clippy --locked -p soto-web --target wasm32-unknown-unknown -- -D warnings
```

Para despliegue, construye las dos imágenes y ejecuta la prueba con medios sintéticos:

```sh
docker compose -f compose.yaml -f compose.build.yaml build
python3 scripts/docker-smoke.py --image soto:local --worker-image soto:worker-local
```

La prueba requiere Docker y comprueba instalación limpia, autorización, escaneo por worker, rangos reales, conversión HLS fMP4 y persistencia después de reiniciar. La prueba Rust multimedia marcada como ignorada se ejecuta explícitamente en un entorno con FFmpeg:

```sh
cargo test --locked -p soto-server actual_ffmpeg_conversion_produces_fmp4_hls --lib -- --ignored
```

Para cambios visuales, comprueba escritorio, móvil y navegación por teclado. Para cambios de datos o secretos, documenta respaldo/restauración y compatibilidad de esquema. Describe evidencia y límites del PR; no marques como verificados Docker, una cuenta Real-Debrid, proveedores reales o un APK que no se hayan probado.

No incluyas multimedia protegida, secretos, archivos `.env` reales, bases de datos ni recursos sin derechos de redistribución. Genera medios de prueba sintéticos o usa recursos con licencia documentada. Actualiza los avisos e inventario de licencias al cambiar dependencias o paquetes del sistema. Tus aportes originales se publican bajo MIT; eso no cambia las licencias de terceros.
