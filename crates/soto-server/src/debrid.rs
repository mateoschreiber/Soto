use crate::app::{ApiError, AppState, new_id};
use axum::{
    Json, Router,
    extract::{Path, State},
    middleware,
    routing::{get, post},
};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/debrid", get(status))
        .route("/debrid/torrents", get(list))
        .merge(
            Router::new()
                .route("/debrid", post(connect).delete(disconnect))
                .route("/debrid/sync", post(sync))
                .route("/debrid/magnet", post(magnet))
                .route("/debrid/torrents/{id}/select", post(select))
                .route_layer(middleware::from_fn(crate::app::administrator)),
        )
}
pub async fn api(
    s: &AppState,
    method: reqwest::Method,
    path: &str,
    form: Option<&[(&str, String)]>,
    body: Option<Vec<u8>>,
) -> Result<Value, ApiError> {
    let token = s
        .secret("realdebrid")
        .await?
        .ok_or_else(|| ApiError::bad("Conecta RealDebrid"))?;
    request(&token, method, path, form, body).await
}
async fn request(
    token: &str,
    method: reqwest::Method,
    path: &str,
    form: Option<&[(&str, String)]>,
    body: Option<Vec<u8>>,
) -> Result<Value, ApiError> {
    let (c, u) = crate::net::external_client(
        &format!("https://api.real-debrid.com/rest/1.0/{path}"),
        false,
    )
    .await?;
    let mut r = c.request(method, u).bearer_auth(token);
    if let Some(f) = form {
        r = r.form(f)
    }
    if let Some(b) = body {
        r = r.body(b)
    }
    let response = r
        .send()
        .await
        .map_err(|_| ApiError::external("RealDebrid no disponible"))?;
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(json!({"ok":true}));
    }
    crate::net::json_response(response).await
}
async fn status(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    if s.secret("realdebrid").await?.is_none() {
        return Ok(Json(json!({"connected":false,"status":"disconnected"})));
    }
    match api(&s, reqwest::Method::GET, "user", None, None).await {
        Ok(v) => Ok(Json(
            json!({"connected":true,"username":v["username"],"premium_until":v["expiration"],"status":v["type"]}),
        )),
        Err(e) => Ok(Json(json!({"connected":true,"status":"error","error":e.2}))),
    }
}
async fn connect(State(s): State<AppState>, Json(i): Json<Value>) -> Result<Json<Value>, ApiError> {
    let t = i["token"]
        .as_str()
        .filter(|t| !t.is_empty() && t.len() < 2048)
        .ok_or_else(|| ApiError::bad("Token inválido"))?;
    request(t, reqwest::Method::GET, "user", None, None).await?;
    s.save_secret("realdebrid", t).await?;
    status(State(s)).await
}
async fn disconnect(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    s.db.call(|c| {
        c.execute("DELETE FROM secrets WHERE name='realdebrid'", [])?;
        c.execute(
            "UPDATE sources SET status='unavailable' WHERE kind='debrid'",
            [],
        )?;
        Ok(Json(json!({"ok":true})))
    })
    .await
}
pub fn safe_id(id: &str) -> Result<(), ApiError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ApiError::bad("ID inválido"));
    }
    Ok(())
}
pub async fn torrent(s: &AppState, id: &str) -> Result<Value, ApiError> {
    safe_id(id)?;
    api(
        s,
        reqwest::Method::GET,
        &format!("torrents/info/{id}"),
        None,
        None,
    )
    .await
}
pub async fn list_values(s: &AppState) -> Result<Vec<Value>, ApiError> {
    let items = api(s, reqwest::Method::GET, "torrents?limit=5000", None, None).await?;
    let mut out = Vec::new();
    for item in items
        .as_array()
        .ok_or_else(|| ApiError::external("Respuesta RealDebrid inválida"))?
        .iter()
        .take(5000)
    {
        let id = item["id"]
            .as_str()
            .ok_or_else(|| ApiError::external("Torrent inválido"))?;
        let info = torrent(s, id).await?;
        persist(s, &info).await?;
        out.push(dto(&info));
    }
    Ok(out)
}
pub fn dto(v: &Value) -> Value {
    json!({"id":v["id"],"name":v["filename"],"status":v["status"],"progress":v["progress"].as_f64().unwrap_or(0.0)/100.0,"files":v["files"].as_array().into_iter().flatten().map(|f|json!({"id":f["id"],"name":f["path"],"size":f["bytes"],"selected":f["selected"].as_u64()==Some(1)})).collect::<Vec<_>>()})
}
async fn list(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(list_values(&s).await?)))
}
async fn sync(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    let id = crate::app::job(&s, "debrid_sync").await?;
    let result = list_values(&s).await;
    crate::app::finish_job(
        &s,
        &id,
        if result.is_ok() {
            "completed"
        } else {
            "failed"
        },
        result.as_ref().err().map(|e| e.2.as_str()),
    )
    .await?;
    result?;
    Ok(Json(crate::app::job_value(&s, &id).await?))
}
async fn magnet(State(s): State<AppState>, Json(v): Json<Value>) -> Result<Json<Value>, ApiError> {
    crate::torrents::create_debrid(&s, v["magnet"].as_str(), None, None)
        .await
        .map(Json)
}
async fn select(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let ids = crate::torrents::selection(&v["file_ids"])?;
    select_files(&s, &id, &ids).await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn select_files(s: &AppState, id: &str, ids: &[u32]) -> Result<(), ApiError> {
    let info = torrent(s, id).await?;
    let files = info["files"]
        .as_array()
        .ok_or_else(|| ApiError::bad("Metadatos pendientes"))?;
    if ids.is_empty()
        || ids
            .iter()
            .any(|id| !files.iter().any(|f| f["id"].as_u64() == Some(*id as u64)))
    {
        return Err(ApiError::bad("Selección inválida"));
    }
    api(
        s,
        reqwest::Method::POST,
        &format!("torrents/selectFiles/{id}"),
        Some(&[(
            "files",
            ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
        )]),
        None,
    )
    .await?;
    Ok(())
}
pub async fn persist(s: &AppState, v: &Value) -> Result<(), ApiError> {
    let id = v["id"]
        .as_str()
        .ok_or_else(|| ApiError::external("Torrent inválido"))?;
    let remote = id.to_string();
    s.db.call(move|c|{c.execute("UPDATE sources SET status='unavailable' WHERE kind='debrid' AND json_extract(details,'$.torrent_id')=?",[remote])?;Ok(())}).await?;
    let all_files = v["files"].as_array().cloned().unwrap_or_default();
    let selected = v["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["selected"].as_u64() == Some(1))
        .collect::<Vec<_>>();
    for (index, f) in selected.iter().enumerate() {
        let name = f["path"].as_str().unwrap_or("");
        if !crate::torrents::playable(name) {
            continue;
        }
        let file = f["id"].as_u64().unwrap_or(0);
        let file_index = all_files
            .iter()
            .position(|item| item["id"] == f["id"])
            .unwrap_or(index) as u64;
        let link = v["links"][index].as_str().map(str::to_string);
        let available = v["status"] == "downloaded" && link.is_some();
        let details = json!({"torrent_id":id,"file_id":file,"link":link});
        let key = format!("rd:{id}:{file}");
        let title = name.rsplit('/').next().unwrap_or(name).to_string();
        let size = f["bytes"].as_u64().map(|x| x.min(i64::MAX as u64) as i64);
        let status = if available { "available" } else { "pending" };
        let remote = id.to_string();
        s.db.call(move|c|{let mid=c.query_row("SELECT id FROM media WHERE file_key=?",[&key],|r|r.get::<_,String>(0)).optional()?.unwrap_or_else(new_id);c.execute("INSERT OR IGNORE INTO media(id,title,origin,file_key) VALUES(?,?,'torrent',?)",params![mid,title,key])?;let source=c.query_row("SELECT id FROM sources WHERE kind='debrid' AND json_extract(details,'$.torrent_id')=? AND json_extract(details,'$.file_id')=?",params![remote,file as i64],|r|r.get::<_,String>(0)).optional()?.unwrap_or_else(new_id);c.execute("INSERT INTO sources(id,media_id,origin,kind,label,status,locator,size,details) VALUES(?,?,'torrent','debrid',?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,details=excluded.details",params![source,mid,title,status,"",size,details.to_string()])?;Ok(())}).await?;
        crate::torrents::bind_source(s, "debrid", id, file, file_index).await?;
    }
    Ok(())
}
pub async fn refresh_source(s: &AppState, id: &str) -> Result<Option<String>, ApiError> {
    let key = id.to_string();
    let raw =
        s.db.call(move |c| {
            Ok(c.query_row(
                "SELECT details FROM sources WHERE id=? AND kind='debrid'",
                [key],
                |r| r.get::<_, String>(0),
            )
            .optional()?)
        })
        .await?;
    let Some(raw) = raw else { return Ok(None) };
    let mut d: Value =
        serde_json::from_str(&raw).map_err(|_| ApiError::internal("Fuente inválida"))?;
    let torrent = torrent(
        s,
        d["torrent_id"]
            .as_str()
            .ok_or_else(|| ApiError::internal("Fuente inválida"))?,
    )
    .await?;
    persist(s, &torrent).await?;
    let files = torrent["files"]
        .as_array()
        .ok_or_else(|| ApiError::external("Archivos pendientes"))?;
    let index = files
        .iter()
        .filter(|f| f["selected"].as_u64() == Some(1))
        .position(|f| f["id"] == d["file_id"])
        .ok_or_else(|| ApiError::external("Archivo no seleccionado"))?;
    d["link"] = torrent["links"][index].clone();
    let link = d["link"]
        .as_str()
        .ok_or_else(|| ApiError::external("Descarga pendiente"))?;
    let v = api(
        s,
        reqwest::Method::POST,
        "unrestrict/link",
        Some(&[("link", link.to_string())]),
        None,
    )
    .await?;
    let url = v["download"]
        .as_str()
        .ok_or_else(|| ApiError::external("Enlace no disponible"))?;
    crate::net::validate_url(url, false).await?;
    Ok(Some(url.to_string()))
}

pub async fn poll_pending(s: &AppState) -> Result<(), ApiError> {
    if s.secret("realdebrid").await?.is_none() {
        return Ok(());
    }
    let jobs=s.db.call(|c|{let mut q=c.prepare("SELECT id,details FROM jobs WHERE kind='torrent_debrid' AND status IN ('queued','running')")?;Ok(q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()?)}).await?;
    for (job, raw) in jobs {
        let d: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        let Some(id) = d["torrent_id"].as_str() else {
            continue;
        };
        match torrent(s, id).await {
            Ok(t) => {
                if t["status"] == "waiting_files_selection" {
                    if let Some(ids) = d.get("file_ids") {
                        let ids = crate::torrents::selection(ids)?;
                        select_files(s, id, &ids).await?;
                    } else if let Some(index) = d["file_idx"].as_u64() {
                        let file = t["files"][index as usize]["id"]
                            .as_u64()
                            .filter(|v| *v <= u32::MAX as u64)
                            .ok_or_else(|| ApiError::bad("Archivo del proveedor no encontrado"))?;
                        select_files(s, id, &[file as u32]).await?;
                    }
                }
                persist(s, &t).await?;
                let status = t["status"].as_str().unwrap_or("");
                if status == "downloaded" {
                    crate::app::finish_job(s, &job, "completed", None).await?;
                } else if ["error", "magnet_error", "virus", "dead"].contains(&status) {
                    crate::app::finish_job(
                        s,
                        &job,
                        "failed",
                        Some("RealDebrid no pudo descargar el torrent"),
                    )
                    .await?;
                } else {
                    let progress = t["progress"].as_f64().unwrap_or(0.0) / 100.0;
                    let job = job.clone();
                    s.db.call(move |c| {
                        c.execute(
                            "UPDATE jobs SET progress=? WHERE id=?",
                            params![progress.clamp(0.0, 1.0), job],
                        )?;
                        Ok(())
                    })
                    .await?;
                }
            }
            Err(e) => {
                let key = job.clone();
                s.db.call(move |c| {
                    c.execute("UPDATE jobs SET error=? WHERE id=?", params![e.2, key])?;
                    Ok(())
                })
                .await?;
            }
        }
    }
    Ok(())
}
