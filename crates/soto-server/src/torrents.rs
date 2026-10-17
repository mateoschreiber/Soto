use crate::app::{ApiError, AppState, new_id};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/torrents", get(list).post(create))
        .route("/torrents/{id}/{action}", post(action))
}
pub fn playable(n: &str) -> bool {
    let ext = n.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    ["mkv", "mp4", "webm", "avi", "mov", "m4v", "ts"].contains(&ext.as_str())
}
pub fn selection(v: &Value) -> Result<Vec<u32>, ApiError> {
    let a = v
        .as_array()
        .ok_or_else(|| ApiError::bad("Selección inválida"))?;
    if a.is_empty() || a.len() > 10000 {
        return Err(ApiError::bad("Selección inválida"));
    }
    let mut out = Vec::new();
    for x in a {
        let id = x
            .as_u64()
            .filter(|n| *n <= u32::MAX as u64)
            .ok_or_else(|| ApiError::bad("Archivo inválido"))? as u32;
        if out.contains(&id) {
            return Err(ApiError::bad("Archivo duplicado"));
        }
        out.push(id)
    }
    Ok(out)
}
fn validate_magnet(m: &str) -> Result<(), ApiError> {
    if m.len() > 16384 {
        return Err(ApiError::bad("Magnet demasiado largo"));
    }
    let u = url::Url::parse(m).map_err(|_| ApiError::bad("Magnet inválido"))?;
    let valid = u.scheme() == "magnet"
        && u.query_pairs().any(|(k, v)| {
            k == "xt"
                && (v.strip_prefix("urn:btih:").is_some_and(|h| {
                    (h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()))
                        || (h.len() == 32
                            && h.chars()
                                .all(|c| c.is_ascii_alphabetic() || ('2'..='7').contains(&c)))
                }) || v
                    .strip_prefix("urn:btmh:1220")
                    .is_some_and(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())))
        });
    if !valid {
        return Err(ApiError::bad("Magnet inválido"));
    }
    Ok(())
}
fn input(m: Option<&str>, b: Option<&str>) -> Result<Option<Vec<u8>>, ApiError> {
    if m.is_some() == b.is_some() {
        return Err(ApiError::bad("Indica magnet o archivo torrent"));
    }
    if let Some(m) = m {
        validate_magnet(m)?;
        return Ok(None);
    }
    let b = b.unwrap();
    if b.len() > 2_800_000 {
        return Err(ApiError::bad("Torrent demasiado grande"));
    }
    let bytes = STANDARD
        .decode(b)
        .map_err(|_| ApiError::bad("Torrent inválido"))?;
    if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
        return Err(ApiError::bad("Torrent demasiado grande"));
    }
    Ok(Some(bytes))
}
pub async fn worker(
    s: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, ApiError> {
    let mut r = s
        .client
        .request(
            method,
            format!(
                "{}/internal/torrents{path}",
                s.config.worker_url.trim_end_matches('/')
            ),
        )
        .bearer_auth(s.worker_token.as_str());
    if let Some(b) = body {
        r = r.json(&b)
    }
    crate::net::json_response(
        r.send()
            .await
            .map_err(|_| ApiError::external("Worker no disponible"))?,
    )
    .await
}
async fn record(s: &AppState, id: &str, engine: &str, remote: &str) -> Result<(), ApiError> {
    let (id, details) = (
        id.to_string(),
        json!({"engine":engine,"torrent_id":remote}).to_string(),
    );
    s.db.call(move |c| {
        c.execute(
            "UPDATE jobs SET details=?,status='running' WHERE id=?",
            params![details, id],
        )?;
        Ok(())
    })
    .await
}
pub async fn create_debrid(
    s: &AppState,
    m: Option<&str>,
    b: Option<&str>,
    files: Option<Vec<u32>>,
) -> Result<Value, ApiError> {
    let bytes = input(m, b)?;
    let id = crate::app::job(s, "torrent_debrid").await?;
    let result = async {
        let response = if let Some(bytes) = bytes {
            crate::debrid::api(
                s,
                reqwest::Method::PUT,
                "torrents/addTorrent",
                None,
                Some(bytes),
            )
            .await?
        } else {
            crate::debrid::api(
                s,
                reqwest::Method::POST,
                "torrents/addMagnet",
                Some(&[("magnet", m.unwrap().to_string())]),
                None,
            )
            .await?
        };
        let remote = response["id"]
            .as_str()
            .ok_or_else(|| ApiError::external("Torrent inválido"))?;
        record(s, &id, "debrid", remote).await?;
        if let Some(files) = files {
            let key = id.clone();
            let details =
                json!({"engine":"debrid","torrent_id":remote,"file_ids":files}).to_string();
            s.db.call(move |c| {
                c.execute(
                    "UPDATE jobs SET details=? WHERE id=?",
                    params![details, key],
                )?;
                Ok(())
            })
            .await?;
        }
        Ok::<(), ApiError>(())
    }
    .await;
    if let Err(e) = result {
        crate::app::finish_job(s, &id, "failed", Some(&e.2)).await?;
        return Err(e);
    }
    crate::app::job_value(s, &id).await
}
pub async fn create_value(s: &AppState, v: Value) -> Result<Value, ApiError> {
    let Json(value) = create(State(s.clone()), Json(v)).await?;
    Ok(value)
}
async fn create(State(s): State<AppState>, Json(v): Json<Value>) -> Result<Json<Value>, ApiError> {
    let files = if v.get("file_ids").is_some() {
        Some(selection(&v["file_ids"])?)
    } else {
        None
    };
    if v["engine"] == "debrid" {
        return Ok(Json(
            create_debrid(
                &s,
                v["magnet"].as_str(),
                v["torrent_base64"].as_str(),
                files,
            )
            .await?,
        ));
    }
    if v["engine"] != "p2p" {
        return Err(ApiError::bad("Motor inválido"));
    }
    let settings = crate::catalog::settings_value(&s).await?;
    if settings["torrent_enabled"] != true
        || settings["torrent_quota_gb"].as_u64().unwrap_or(0) == 0
    {
        return Err(ApiError::bad("P2P desactivado o sin cuota"));
    }
    input(v["magnet"].as_str(), v["torrent_base64"].as_str())?;
    let id = crate::app::job(&s, "torrent_p2p").await?;
    let body = json!({"magnet":v["magnet"],"torrent_base64":v["torrent_base64"],"file_ids":files,"quota_gb":settings["torrent_quota_gb"],"download_kib":settings["download_kib"],"upload_kib":settings["upload_kib"]});
    match worker(&s, reqwest::Method::POST, "", Some(body)).await {
        Ok(t) => {
            let remote = t["id"]
                .as_str()
                .ok_or_else(|| ApiError::external("Torrent inválido"))?;
            record(&s, &id, "p2p", remote).await?;
            persist(&s, &t).await?;
        }
        Err(e) => {
            crate::app::finish_job(&s, &id, "failed", Some(&e.2)).await?;
            return Err(e);
        }
    }
    Ok(Json(crate::app::job_value(&s, &id).await?))
}
async fn list(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    if crate::catalog::settings_value(&s).await?["torrent_enabled"] != true {
        return Ok(Json(list_persisted(&s).await?));
    }
    let v = worker(&s, reqwest::Method::GET, "", None).await?;
    s.db.call(|c| {
        c.execute("DELETE FROM torrents WHERE engine='p2p'", [])?;
        Ok(())
    })
    .await?;
    for t in v.as_array().into_iter().flatten() {
        persist(&s, t).await?;
    }
    Ok(Json(v))
}
async fn list_persisted(s: &AppState) -> Result<Value, ApiError> {
    s.db.call(|c| {
        let mut stmt =
            c.prepare("SELECT details FROM torrents WHERE engine='p2p' ORDER BY updated DESC")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!(
            rows.into_iter()
                .filter_map(|d| serde_json::from_str::<Value>(&d).ok())
                .collect::<Vec<_>>()
        ))
    })
    .await
}
async fn store_torrent(s: &AppState, engine: &str, t: &Value) -> Result<(), ApiError> {
    let id = t["id"].as_str().unwrap_or("").to_string();
    let name = t["name"].as_str().unwrap_or("").to_string();
    let status = t["status"].as_str().unwrap_or("pending").to_string();
    let progress = t["progress"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
    let details = t.to_string();
    let updated = crate::app::now();
    let engine = engine.to_string();
    s.db.call(move |c| {
        c.execute(
            "INSERT INTO torrents(id,engine,name,status,progress,details,updated) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET engine=excluded.engine,name=excluded.name,status=excluded.status,progress=excluded.progress,details=excluded.details,updated=excluded.updated",
            params![id, engine, name, status, progress, details, updated],
        )?;
        Ok(())
    })
    .await
}
async fn action(
    State(s): State<AppState>,
    Path((id, action)): Path<(String, String)>,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    let v = body.map(|b| b.0).unwrap_or(Value::Null);
    crate::debrid::safe_id(&id)?;
    if !["pause", "resume", "cancel", "select"].contains(&action.as_str()) {
        return Err(ApiError::not_found());
    }
    let body = if action == "select" {
        Some(json!({"file_ids":selection(&v["file_ids"])?}))
    } else {
        None
    };
    let result = worker(&s, reqwest::Method::POST, &format!("/{id}/{action}"), body).await?;
    if action == "cancel" {
        let remote = id.clone();
        s.db.call(move|c|{c.execute("UPDATE sources SET status='unavailable' WHERE kind='p2p' AND json_extract(details,'$.torrent_id')=?",[&remote])?;c.execute("UPDATE jobs SET status='cancelled' WHERE json_extract(details,'$.torrent_id')=? AND json_extract(details,'$.engine')='p2p'",[&remote])?;c.execute("DELETE FROM torrents WHERE engine='p2p' AND id=?",[&remote])?;Ok(())}).await?;
    }
    Ok(Json(result))
}
async fn persist(s: &AppState, t: &Value) -> Result<(), ApiError> {
    let id = t["id"]
        .as_str()
        .ok_or_else(|| ApiError::external("Torrent inválido"))?;
    let remote = id.to_string();
    s.db.call(move|c|{c.execute("UPDATE sources SET status='unavailable' WHERE kind='p2p' AND json_extract(details,'$.torrent_id')=?",[remote])?;Ok(())}).await?;
    for f in t["files"].as_array().into_iter().flatten() {
        let name = f["name"].as_str().unwrap_or("");
        if f["selected"] != true || !playable(name) {
            continue;
        }
        let Some(file) = f["id"].as_u64().filter(|i| *i <= u32::MAX as u64) else {
            continue;
        };
        let locator = format!("{id}/{file}");
        let key = format!("p2p:{locator}");
        let title = name.rsplit(['/', '\\']).next().unwrap_or(name).to_string();
        let size = f["size"].as_u64().map(|n| n.min(i64::MAX as u64) as i64);
        let details = json!({"torrent_id":id,"file_id":file}).to_string();
        s.db.call(move|c|{let mid=c.query_row("SELECT id FROM media WHERE file_key=?",[&key],|r|r.get::<_,String>(0)).optional()?.unwrap_or_else(new_id);c.execute("INSERT OR IGNORE INTO media(id,title,origin,file_key) VALUES(?,?,'torrent',?)",params![mid,title,key])?;let sid=c.query_row("SELECT id FROM sources WHERE locator=? AND kind='p2p'",[&locator],|r|r.get::<_,String>(0)).optional()?.unwrap_or_else(new_id);c.execute("INSERT INTO sources(id,media_id,origin,kind,label,status,locator,size,details) VALUES(?,?,'torrent','p2p',?,'available',?,?,?) ON CONFLICT(id) DO UPDATE SET status='available'",params![sid,mid,title,locator,size,details])?;Ok(())}).await?;
        bind_source(s, "p2p", id, file, file).await?;
    }
    let remote = id.to_string();
    let status = t["status"].as_str().unwrap_or("pending").to_string();
    let progress = t["progress"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
    s.db.call(move|c|{c.execute("UPDATE jobs SET progress=?,status=CASE WHEN ?='completed' THEN 'completed' WHEN ?='error' THEN 'failed' ELSE status END WHERE json_extract(details,'$.torrent_id')=? AND json_extract(details,'$.engine')='p2p'",params![progress,status,status,remote])?;Ok(())}).await?;
    store_torrent(s, "p2p", t).await
}

pub async fn cancel_job(s: &AppState, job_id: &str) -> Result<(), ApiError> {
    let key = job_id.to_string();
    let raw =
        s.db.call(move |c| {
            Ok(
                c.query_row("SELECT details FROM jobs WHERE id=?", [key], |r| {
                    r.get::<_, String>(0)
                })
                .optional()?,
            )
        })
        .await?
        .ok_or_else(ApiError::not_found)?;
    let d: Value =
        serde_json::from_str(&raw).map_err(|_| ApiError::internal("Trabajo inválido"))?;
    if let Some(remote) = d["torrent_id"].as_str() {
        crate::debrid::safe_id(remote)?;
        match d["engine"].as_str() {
            Some("p2p") => {
                worker(s, reqwest::Method::POST, &format!("/{remote}/cancel"), None).await?;
            }
            Some("debrid") => {
                crate::debrid::api(
                    s,
                    reqwest::Method::DELETE,
                    &format!("torrents/delete/{remote}"),
                    None,
                    None,
                )
                .await?;
            }
            _ => {}
        }
        let remote = remote.to_string();
        s.db.call(move|c|{c.execute("UPDATE sources SET status='unavailable' WHERE json_extract(details,'$.torrent_id')=?",[&remote])?;c.execute("DELETE FROM torrents WHERE engine='p2p' AND id=?",[remote])?;Ok(())}).await?;
    }
    crate::app::finish_job(s, job_id, "cancelled", None).await
}

pub async fn bind_source(
    s: &AppState,
    engine: &str,
    torrent: &str,
    file: u64,
    index: u64,
) -> Result<(), ApiError> {
    let (engine, torrent) = (engine.to_string(), torrent.to_string());
    s.db.call(move|c|{let media:Option<String>=c.query_row("SELECT json_extract(details,'$.media_id') FROM jobs WHERE json_extract(details,'$.engine')=? AND json_extract(details,'$.torrent_id')=? AND json_extract(details,'$.media_id') IS NOT NULL AND (json_extract(details,'$.file_idx') IS NULL OR json_extract(details,'$.file_idx')=?) ORDER BY created DESC LIMIT 1",params![engine,torrent,index as i64],|r|r.get(0)).optional()?;if let Some(media)=media{c.execute("UPDATE sources SET media_id=? WHERE kind=? AND json_extract(details,'$.torrent_id')=? AND json_extract(details,'$.file_id')=?",params![media,if engine=="debrid"{"debrid"}else{"p2p"},torrent,file as i64])?;}Ok(())}).await
}

pub async fn poll_worker(s: &AppState) -> Result<(), ApiError> {
    let _ = list(axum::extract::State(s.clone())).await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn magnet_validation() {
        assert!(
            validate_magnet("magnet:?xt=urn:btih:0123456789012345678901234567890123456789").is_ok()
        );
        assert!(validate_magnet("https://host/a").is_err());
        assert!(validate_magnet("magnet:?xt=urn:btih:bad").is_err());
    }
    #[test]
    fn file_validation() {
        assert!(selection(&json!([1, 2])).is_ok());
        assert!(selection(&json!([1, 1])).is_err());
        assert!(selection(&json!([4294967296u64])).is_err());
        assert!(input(None, Some("bad")).is_err());
        assert!(playable("film.MKV"));
    }
}
