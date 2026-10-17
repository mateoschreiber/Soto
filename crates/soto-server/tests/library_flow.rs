use axum::{
    Json, Router,
    body::Body,
    http::{Request, StatusCode},
    routing::post,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;
async fn request(app: Router, method: &str, path: &str, body: Value) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("x-soto-request", "1")
            .header("cookie", "soto_session=test-only-session")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}
async fn value(r: axum::response::Response) -> Value {
    let status = r.status();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    assert!(
        status.is_success(),
        "{status}: {}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes).unwrap()
}
async fn wait_job(app: Router, id: &str) -> Value {
    for _ in 0..100 {
        let jobs = value(request(app.clone(), "GET", "/api/v1/jobs", Value::Null).await).await;
        let job = jobs
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["id"] == id)
            .unwrap()
            .clone();
        if ["completed", "failed", "cancelled"].contains(&job["status"].as_str().unwrap()) {
            return job;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("Job did not finish")
}
#[tokio::test]
async fn mounted_library_scan_range_progress_cancel_and_disconnect() {
    let root = std::env::temp_dir().join(format!("soto-library-{}", uuid::Uuid::new_v4()));
    let media = root.join("mounted");
    std::fs::create_dir_all(&media).unwrap();
    std::fs::write(media.join("Personal recording.mp4"), b"0123456789").unwrap();
    let probes = Arc::new(AtomicUsize::new(0));
    let p = probes.clone();
    let worker=Router::new().route("/internal/probe",post(move||{let p=p.clone();async move{p.fetch_add(1,Ordering::SeqCst);tokio::time::sleep(std::time::Duration::from_millis(180)).await;Json(json!({"format":{"duration":"10.0"},"streams":[{"index":0,"codec_type":"video","codec_name":"h264"},{"index":1,"codec_type":"audio","codec_name":"aac"}]}))}}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, worker).await.unwrap() });
    let config = soto_server::app::AppConfig {
        data: root.join("data"),
        roots: HashMap::from([("media".into(), media.clone())]),
        cache: root.join("cache"),
        torrents: root.join("torrents"),
        worker_url: format!("http://{address}"),
        public_url: None,
        allow_private_providers: false,
    };
    let state = soto_server::app::AppState::open(config).unwrap();
    state
        .db
        .call(|c| {
            c.execute(
                "INSERT INTO users VALUES('owner','owner','test-only',1)",
                [],
            )?;
            c.execute(
                "INSERT INTO sessions VALUES(?,'owner',?)",
                rusqlite::params![
                    soto_server::app::digest("test-only-session"),
                    soto_server::app::now() + 1000
                ],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let app = soto_server::application(root.join("static"), state.clone());
    let forbidden = request(
        app.clone(),
        "GET",
        "/api/v1/libraries/browse?root_id=media&path=..",
        Value::Null,
    )
    .await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    let library = value(
        request(
            app.clone(),
            "POST",
            "/api/v1/libraries",
            json!({"name":"Recordings","root_id":"media","path":"","kind":"mixed"}),
        )
        .await,
    )
    .await;
    let scan_path = format!("/api/v1/libraries/{}/scan", library["id"].as_str().unwrap());
    let job = value(request(app.clone(), "POST", &scan_path, Value::Null).await).await;
    let done = wait_job(app.clone(), job["id"].as_str().unwrap()).await;
    assert_eq!(done["status"], "completed", "{done}");
    assert_eq!(probes.load(Ordering::SeqCst), 1);
    let catalog = value(
        request(
            app.clone(),
            "GET",
            "/api/v1/catalog?origin=local",
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(catalog.as_array().unwrap().len(), 1);
    assert_eq!(catalog[0]["year"], Value::Null);
    assert!(!catalog.to_string().contains(media.to_str().unwrap()));
    let mid = catalog[0]["id"].as_str().unwrap();
    let detail = value(
        request(
            app.clone(),
            "GET",
            &format!("/api/v1/catalog/{mid}"),
            Value::Null,
        )
        .await,
    )
    .await;
    assert!(!detail.to_string().contains(media.to_str().unwrap()));
    let sid = detail["sources"][0]["id"].as_str().unwrap();
    let session = value(
        request(
            app.clone(),
            "POST",
            "/api/v1/playback",
            json!({"source_id":sid,"mode":"direct"}),
        )
        .await,
    )
    .await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(session["url"].as_str().unwrap())
                .header("cookie", "soto_session=test-only-session")
                .header("range", "bytes=2-4")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "234"
    );
    value(
        request(
            app.clone(),
            "PUT",
            &format!("/api/v1/progress/{mid}"),
            json!({"position":4.0,"duration":10.0}),
        )
        .await,
    )
    .await;
    let job = value(request(app.clone(), "POST", &scan_path, Value::Null).await).await;
    assert_eq!(
        wait_job(app.clone(), job["id"].as_str().unwrap()).await["status"],
        "completed"
    );
    assert_eq!(
        probes.load(Ordering::SeqCst),
        1,
        "unchanged files must not reprobe"
    );
    std::fs::write(media.join("New recording.mp4"), b"other file").unwrap();
    let job = value(request(app.clone(), "POST", &scan_path, Value::Null).await).await;
    for _ in 0..100 {
        if probes.load(Ordering::SeqCst) > 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    value(
        request(
            app.clone(),
            "POST",
            &format!("/api/v1/jobs/{}/cancel", job["id"].as_str().unwrap()),
            Value::Null,
        )
        .await,
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    assert_eq!(
        wait_job(app.clone(), job["id"].as_str().unwrap()).await["status"],
        "cancelled"
    );
    std::fs::rename(&media, root.join("disconnected")).unwrap();
    let job = value(request(app.clone(), "POST", &scan_path, Value::Null).await).await;
    assert_eq!(
        wait_job(app.clone(), job["id"].as_str().unwrap()).await["status"],
        "failed"
    );
    assert!(
        !value(
            request(
                app.clone(),
                "GET",
                "/api/v1/catalog?origin=local",
                Value::Null
            )
            .await
        )
        .await
        .as_array()
        .unwrap()
        .is_empty()
    );
    task.abort();
}
