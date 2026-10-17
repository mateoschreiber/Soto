# Seguridad

Solo se mantiene la demostración `0.1.0-alpha.1`. No contiene autenticación ni funciones multimedia reales: úsala en una red de confianza, un túnel SSH o detrás de controles de acceso propios. No montes archivos personales ni introduzcas credenciales de proveedores en esta etapa.

Para informar de una vulnerabilidad, usa **Security → Report a vulnerability** en GitHub. Incluye versión, impacto y reproducción mínima; evita publicar secretos o detalles explotables en issues públicos. No hay un compromiso de plazo de respuesta.

La distribución usa usuario sin privilegios, raíz de solo lectura, capabilities eliminadas, healthcheck y límites de recursos. El proxy debe conservar las cabeceras de seguridad de Soto. Estas medidas no sustituyen a la autenticación pendiente.
