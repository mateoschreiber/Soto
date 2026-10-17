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

async fn send(app: axum::Router, method: &str, path: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("x-soto-request", "1")
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn fresh_install_opens_without_users_and_legacy_accounts_are_collapsed() {
    let dir = std::env::temp_dir().join(format!("soto-no-auth-{}", uuid::Uuid::new_v4()));
    let state = soto_server::app::AppState::open(config(dir.clone())).unwrap();
    let app = soto_server::application("missing".into(), state.clone());

    let catalog = send(app.clone(), "GET", "/api/v1/catalog?origin=local").await;
    assert_eq!(catalog.status(), StatusCode::OK);
    assert_eq!(json(catalog).await, json!([]));
    let settings = send(app.clone(), "GET", "/api/v1/settings").await;
    assert_eq!(settings.status(), StatusCode::OK);
    assert!(settings.headers().get("set-cookie").is_none());
    assert_eq!(
        send(app.clone(), "GET", "/api/v1/setup").await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(app.clone(), "GET", "/api/v1/session").await.status(),
        StatusCode::NOT_FOUND
    );

    state
        .db
        .call(|c| {
            c.execute("CREATE TABLE profiles(id TEXT PRIMARY KEY,name TEXT NOT NULL)", [])?;
            c.execute("INSERT INTO profiles VALUES('old-profile','Old profile')", [])?;
            c.execute(
                "CREATE TABLE sessions(token_hash TEXT PRIMARY KEY,user_id TEXT NOT NULL REFERENCES users(id),expires INTEGER NOT NULL)",
                [],
            )?;
            c.execute(
                "INSERT INTO users(id,username,password_hash,admin) VALUES('old','old-user','obsolete',1)",
                [],
            )?;
            c.execute("INSERT INTO sessions VALUES('old-token-hash','old',9999999999)", [])?;
            c.execute(
                "INSERT INTO media(id,title,origin) VALUES('saved','Saved movie','local')",
                [],
            )?;
            c.execute(
                "INSERT INTO collections(user_id,media_id) VALUES('old','saved')",
                [],
            )?;
            c.execute(
                "INSERT INTO progress(user_id,media_id,position,duration) VALUES('old','saved',60,120)",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    drop(app);
    drop(state);

    let reopened = soto_server::app::AppState::open(config(dir.clone())).unwrap();
    let users = reopened
        .db
        .call(|c| {
            Ok(c.query_row("SELECT group_concat(id) FROM users", [], |r| {
                r.get::<_, String>(0)
            })?)
        })
        .await
        .unwrap();
    assert_eq!(users, "local");
    let legacy_schema = reopened
        .db
        .call(|c| {
            Ok(c.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('sessions','profiles')",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(legacy_schema, 0);
    let library = soto_server::application("missing".into(), reopened.clone());
    let collections = send(library.clone(), "GET", "/api/v1/collections").await;
    assert_eq!(collections.status(), StatusCode::OK);
    assert_eq!(json(collections).await[0]["id"], "saved");
    let progress = send(library, "GET", "/api/v1/progress").await;
    assert_eq!(progress.status(), StatusCode::OK);
    assert_eq!(json(progress).await[0]["position"].as_f64(), Some(60.0));

    drop(reopened);
    let _ = std::fs::remove_dir_all(dir);
}
