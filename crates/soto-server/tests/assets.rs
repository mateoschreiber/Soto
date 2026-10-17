use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use tower::ServiceExt;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "soto-http-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), "<title>Soto</title>").unwrap();
        std::fs::write(dir.join("assets/app-dxh123abc.js"), "hello").unwrap();
        std::fs::write(dir.join("assets/settings.json"), "{}").unwrap();
        // gzip("hello"), generated once; no compressor is needed by the runtime or test.
        std::fs::write(
            dir.join("assets/app-dxh123abc.js.gz"),
            [
                31, 139, 8, 0, 0, 0, 0, 0, 2, 3, 203, 72, 205, 201, 201, 7, 0, 134, 166, 16, 54, 5,
                0, 0, 0,
            ],
        )
        .unwrap();
        Self(dir)
    }
    async fn get(&self, path: &str, encoding: &str) -> axum::response::Response {
        soto_server::router(self.0.clone())
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("accept-encoding", encoding)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn only_fingerprinted_successful_assets_are_immutable() {
    let files = Fixture::new();
    for (path, expected) in [
        ("/", "no-store"),
        ("/assets/settings.json", "no-store"),
        ("/assets/missing-dxh123abc.js", "no-store"),
        (
            "/assets/app-dxh123abc.js",
            "public, max-age=31536000, immutable",
        ),
    ] {
        assert_eq!(
            files.get(path, "identity").await.headers()["cache-control"],
            expected,
            "{path}"
        );
    }
}

#[tokio::test]
async fn compressed_and_identity_clients_receive_correct_variants() {
    let files = Fixture::new();
    let zipped = files.get("/assets/app-dxh123abc.js", "gzip").await;
    assert_eq!(zipped.status(), StatusCode::OK);
    assert_eq!(zipped.headers()["content-encoding"], "gzip");
    assert_eq!(zipped.headers()["vary"], "accept-encoding");
    let bytes = zipped.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..2], &[31, 139]);
    let plain = files.get("/assets/app-dxh123abc.js", "identity").await;
    assert!(plain.headers().get("content-encoding").is_none());
    assert_eq!(plain.headers()["vary"], "accept-encoding");
    assert_eq!(
        plain.into_body().collect().await.unwrap().to_bytes(),
        "hello"
    );
}

#[tokio::test]
async fn traversal_and_missing_files_are_not_spa_successes() {
    let files = Fixture::new();
    for path in ["/assets/%2e%2e/%2e%2e/private", "/missing.wasm"] {
        let response = files.get(path, "gzip").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}
