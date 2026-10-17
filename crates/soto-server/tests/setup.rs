use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf};
use tower::ServiceExt;
fn config(data: PathBuf) -> soto_server::app::AppConfig {
    soto_server::app::AppConfig {
        data,
        roots: HashMap::new(),
        torrents: "unused".into(),
        cache: "unused".into(),
        worker_url: "http://127.0.0.1:9".into(),
        public_url: None,
        allow_private_providers: false,
    }
}
async fn send(
    app: axum::Router,
    method: &str,
    path: &str,
    body: Value,
    cookie: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("x-soto-request", "1")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn fresh_install_is_empty_and_setup_is_one_time_and_persistent() {
    let dir = std::env::temp_dir().join(format!("soto-setup-{}", uuid::Uuid::new_v4()));
    let s = soto_server::app::AppState::open(config(dir.clone())).unwrap();
    let app = soto_server::application("missing".into(), s.clone());
    let initial = send(app.clone(), "GET", "/api/v1/setup", Value::Null, "").await;
    let body: Value =
        serde_json::from_slice(&initial.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["configured"], false);
    assert_eq!(
        send(app.clone(), "GET", "/api/v1/catalog", Value::Null, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let setup = json!({"setup_token":std::fs::read_to_string(dir.join("bootstrap-token")).unwrap(),"server_name":"My home","username":"owner","password":"correct horse battery staple"});
    let result = send(app.clone(), "POST", "/api/v1/setup", setup.clone(), "").await;
    assert_eq!(result.status(), StatusCode::OK);
    let cookie = result.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(
        send(app.clone(), "POST", "/api/v1/setup", setup, "")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    for path in [
        "/api/v1/catalog?origin=local",
        "/api/v1/catalog?origin=torrent",
        "/api/v1/profiles",
        "/api/v1/collections",
    ] {
        let response = send(app.clone(), "GET", path, Value::Null, &cookie).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let data: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(data, json!([]), "{path}");
    }
    let s2 = soto_server::app::AppState::open(config(dir.clone())).unwrap();
    let app2 = soto_server::application("missing".into(), s2);
    let response = send(app2, "GET", "/api/v1/session", Value::Null, &cookie).await;
    assert_eq!(response.status(), StatusCode::OK);
    s.save_secret("debrid", "private-test-token").await.unwrap();
    assert_eq!(
        s.secret("debrid").await.unwrap().as_deref(),
        Some("private-test-token")
    );
    let dbbytes = std::fs::read(dir.join("soto.sqlite-wal")).unwrap();
    assert!(!dbbytes.windows(18).any(|w| w == b"private-test-token"));
    drop(app);
    drop(s);
    let _ = std::fs::remove_dir_all(dir);
}
