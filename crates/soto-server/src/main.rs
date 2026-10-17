#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = soto_server::Config::from_env()?;
    if std::env::args().any(|arg| arg == "--healthcheck") {
        return soto_server::healthcheck(config.bind).map_err(Into::into);
    }
    if !config.static_dir.join("index.html").is_file() {
        return Err("Falta la interfaz compilada. Ejecuta dx build -p soto-web --platform web --release o configura SOTO_STATIC_DIR.".into());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "soto_server=info,tower_http=warn".into()),
        )
        .init();
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(address=%config.bind,"Soto etapa 1: interfaz de demostración");
    axum::serve(listener, soto_server::router(config.static_dir))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}
async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {()=ctrl_c=>{},()=terminate=>{}}
}
