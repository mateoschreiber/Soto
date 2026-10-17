# Contribuir a Soto

Abre un issue describiendo el problema y el resultado esperado antes de cambios grandes. Los informes deben incluir versión o digest de imagen, arquitectura, navegador y pasos para reproducir, sin credenciales ni datos personales.

Todo el código propio de aplicación y pruebas es Rust. CSS, SVG, YAML y los comandos de CI configuran presentación y distribución. Mantén la separación de Local/Torrents y marca los datos de ejemplo. Una pantalla no convierte una integración pendiente en funcionalidad implementada.

Usa una rama y un PR pequeño. Instala Rust y Dioxus según el README y ejecuta:

```sh
cargo fmt --all --check
cargo test --locked --workspace
cargo clippy --locked -p soto-core -p soto-server --all-targets -- -D warnings
cargo clippy --locked -p soto-web --target wasm32-unknown-unknown -- -D warnings
```

Para cambios visuales, comprueba escritorio, móvil y navegación por teclado en vista TV. Para cambios de despliegue, compila y arranca con `compose.build.yaml` y comprueba salud y logs. Describe la validación y cualquier límite en el PR.

No incluyas archivos multimedia, secretos, `.env`, bases de datos ni recursos sin derechos de redistribución. Al contribuir aceptas publicar tus aportes originales bajo la licencia MIT del repositorio.
