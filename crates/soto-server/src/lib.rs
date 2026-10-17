pub mod app;
pub mod catalog;
pub mod db;
pub mod debrid;
pub mod library;
pub mod net;
pub mod playback;
pub mod providers;
pub mod torrents;
pub mod worker;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{net::SocketAddr, path::PathBuf};
use tower_http::{services::ServeDir, trace::TraceLayer};

#[derive(Debug)]
pub struct Config {
    pub bind: SocketAddr,
    pub static_dir: PathBuf,
}
impl Config {
    pub fn from_values(bind: Option<&str>, static_dir: Option<&str>) -> Result<Self, String> {
        let bind = bind
            .unwrap_or("127.0.0.1:3000")
            .parse()
            .map_err(|_| "SOTO_BIND debe ser una dirección IP:puerto válida".to_string())?;
        let static_dir = static_dir
            .unwrap_or("target/dx/soto-web/release/web/public")
            .into();
        Ok(Self { bind, static_dir })
    }
    pub fn from_env() -> Result<Self, String> {
        Self::from_values(
            std::env::var("SOTO_BIND").ok().as_deref(),
            std::env::var("SOTO_STATIC_DIR").ok().as_deref(),
        )
    }
}

/// Hash only trusted build-generated inline scripts; never allow arbitrary inline application code.
fn csp_for_index(index: &str) -> HeaderValue {
    let mut hashes = String::new();
    let mut rest = index;
    while let Some((_, after)) = rest.split_once("<script") {
        let Some((attributes, body)) = after.split_once('>') else {
            break;
        };
        let Some((content, next)) = body.split_once("</script>") else {
            break;
        };
        rest = next;
        if !attributes.contains("src=") && !content.trim().is_empty() {
            hashes.push_str(&format!(
                " 'sha256-{}'",
                STANDARD.encode(Sha256::digest(content.as_bytes()))
            ));
        }
    }
    HeaderValue::from_str(&format!("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'{hashes}; style-src 'self' 'unsafe-inline'; img-src 'self' https: data:; media-src 'self' blob:; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'" )).expect("static policy and base64 are valid header values")
}

async fn security_headers(State(csp): State<HeaderValue>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let asset = path.starts_with("/assets/");
    let fingerprinted = asset
        && path.rsplit_once("-dxh").is_some_and(|(_, suffix)| {
            suffix.split_once('.').is_some_and(|(hash, _)| {
                !hash.is_empty() && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        });
    let mut response = next.run(req).await;
    let immutable = fingerprinted
        && (response.status().is_success()
            || response.status() == axum::http::StatusCode::NOT_MODIFIED);
    let headers = response.headers_mut();
    if asset {
        headers.insert(header::VARY, HeaderValue::from_static("accept-encoding"));
    }
    headers.insert(header::CONTENT_SECURITY_POLICY, csp);
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if immutable {
            "public, max-age=31536000, immutable"
        } else {
            "no-store"
        }),
    );
    response
}

pub fn router(static_dir: PathBuf) -> Router {
    let index = std::fs::read_to_string(static_dir.join("index.html")).unwrap_or_default();
    Router::new().route("/api/health",get(||async{Json(serde_json::json!({"status":"ok","stage":2,"version":env!("CARGO_PKG_VERSION")}))}))
        .fallback_service(ServeDir::new(static_dir).precompressed_gzip())
        .layer(middleware::from_fn_with_state(csp_for_index(&index),security_headers))
        .layer(TraceLayer::new_for_http())
}

pub fn application(static_dir: PathBuf, state: app::AppState) -> Router {
    let protected = app::session_routes()
        .merge(catalog::routes())
        .merge(library::routes())
        .merge(playback::routes())
        .merge(providers::routes())
        .merge(debrid::routes())
        .merge(torrents::routes())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            app::authorize,
        ));
    let readiness = state.clone();
    let api = app::public_routes().merge(protected).with_state(state);
    router(static_dir)
        .nest("/api/v1", api)
        .route(
            "/api/ready",
            get(move || {
                let s = readiness.clone();
                async move {
                    match s
                        .db
                        .call(|c| {
                            c.query_row("SELECT 1", [], |_| Ok(()))?;
                            Ok(())
                        })
                        .await
                    {
                        Ok(()) => (
                            axum::http::StatusCode::OK,
                            Json(serde_json::json!({"status":"ready"})),
                        ),
                        Err(_) => (
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            Json(serde_json::json!({"status":"unavailable"})),
                        ),
                    }
                }
            }),
        )
        .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024))
}

pub fn healthcheck(bind: SocketAddr) -> std::io::Result<()> {
    use std::io::{Read, Write};
    let address = SocketAddr::new(
        if bind.ip().is_unspecified() {
            if bind.is_ipv6() {
                std::net::Ipv6Addr::LOCALHOST.into()
            } else {
                std::net::Ipv4Addr::LOCALHOST.into()
            }
        } else {
            bind.ip()
        },
        bind.port(),
    );
    let mut stream =
        std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(3))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut buffer = [0; 512];
    let size = stream.read(&mut buffer)?;
    if buffer[..size].starts_with(b"HTTP/1.1 200") {
        Ok(())
    } else {
        Err(std::io::Error::other("health endpoint unavailable"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn inline_script_hashes_are_content_specific() {
        let p = super::csp_for_index(
            "<script type=\"module\">bootstrap()</script><script src=\"/assets/app.js\"></script>",
        );
        let p = p.to_str().unwrap();
        assert!(p.contains("sha256-"));
        assert!(!p.contains("script-src 'unsafe-inline'"));
        assert_ne!(
            p,
            super::csp_for_index("<script>different()</script>")
                .to_str()
                .unwrap()
        );
    }
}
