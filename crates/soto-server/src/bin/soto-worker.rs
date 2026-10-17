#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--healthcheck") {
        return soto_server::worker::healthcheck()
            .await
            .map_err(|e| e.2.into());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "soto_server=info,librqbit=error".into()),
        )
        .init();
    soto_server::worker::run().await.map_err(|e| e.2.into())
}
