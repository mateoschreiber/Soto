use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn health_and_headers_do_not_imply_real_services() {
    let app = soto_server::router("./missing-static".into());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["x-frame-options"], "DENY");
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("wasm-unsafe-eval")
    );
    assert!(
        !response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("'unsafe-eval'")
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["stage"], 1);
    assert_eq!(body["media_services_connected"], false);
}

#[tokio::test]
async fn unknown_api_is_not_a_fake_success() {
    let app = soto_server::router("./missing-static".into());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/local")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn configuration_defaults_to_loopback_and_rejects_invalid_bind() {
    let config = soto_server::Config::from_values(None, None).unwrap();
    assert!(config.bind.ip().is_loopback());
    assert!(soto_server::Config::from_values(Some("not-a-bind"), None).is_err());
}
