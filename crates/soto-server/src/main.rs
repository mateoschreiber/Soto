#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = soto_server::Config::from_env()?;
    if std::env::args().any(|arg| arg == "--healthcheck") {
        return soto_server::healthcheck(config.bind).map_err(Into::into);
    }
    let state =
        soto_server::app::AppState::open(soto_server::app::AppConfig::from_env().map_err(|e| e.2)?)
            .map_err(|e| e.2)?;
    if !config.static_dir.join("index.html").is_file() {
        return Err("Falta la interfaz compilada. Ejecuta dx build -p soto-web --platform web --release o configura SOTO_STATIC_DIR.".into());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "soto_server=info,tower_http=warn".into()),
        )
        .init();
    soto_server::library::recover(&state)
        .await
        .map_err(|e| e.2)?;
    let polling = state.clone();
    tokio::spawn(async move {
        loop {
            let _ = soto_server::debrid::poll_pending(&polling).await;
            let _ = soto_server::torrents::poll_worker(&polling).await;
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(address=%config.bind,"Soto: configuración y servicios multimedia");
    axum::serve(listener, soto_server::application(config.static_dir, state))
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
