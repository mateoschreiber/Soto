use crate::app::{ApiError, AppState, LocalOwner, now};
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    routing::{get, post, put},
};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
pub fn media_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(
        json!({"id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"year":r.get::<_,Option<u16>>(3)?,"description":r.get::<_,Option<String>>(4)?,"rating":r.get::<_,Option<String>>(5)?,"duration_seconds":r.get::<_,Option<i64>>(6)?,"poster":r.get::<_,Option<String>>(7)?,"origin":r.get::<_,String>(8)?,"source_count":r.get::<_,u32>(9)?}),
    )
}
pub const MEDIA_SELECT: &str = "SELECT m.id,m.title,m.kind,m.year,m.description,m.rating,m.duration_seconds,m.poster,m.origin,(SELECT count(*) FROM sources s WHERE s.media_id=m.id AND s.status='available') FROM media m";
pub fn source_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(
        json!({"id":r.get::<_,String>(0)?,"media_id":r.get::<_,String>(1)?,"origin":r.get::<_,String>(2)?,"kind":r.get::<_,String>(3)?,"label":r.get::<_,String>(4)?,"status":r.get::<_,String>(5)?,"quality":r.get::<_,Option<String>>(6)?,"size":r.get::<_,Option<i64>>(7)?,"video_codec":r.get::<_,Option<String>>(8)?,"audio_codec":r.get::<_,Option<String>>(9)?,"audio_streams":audio_tracks(&r.get::<_,String>(10)?)}),
    )
}
pub const SOURCE_SELECT: &str = "SELECT id,media_id,origin,kind,label,status,quality,size,video_codec,audio_codec,details FROM sources";
#[derive(Deserialize)]
struct CatalogQuery {
    origin: Option<String>,
    q: Option<String>,
    kind: Option<String>,
}
async fn catalog(
    State(s): State<AppState>,
    Query(q): Query<CatalogQuery>,
) -> Result<Json<Value>, ApiError> {
    let origin = q.origin.unwrap_or_else(|| "local".into());
    if !["local", "torrent"].contains(&origin.as_str()) {
        return Err(ApiError::bad("Origen inválido"));
    }
    let search = q.q.unwrap_or_default();
    if search.len() > 500 {
        return Err(ApiError::bad("Búsqueda demasiado larga"));
    }
    let kind = q.kind.unwrap_or_else(|| "all".into());
    if origin == "torrent" {
        crate::providers::discover(&s, &search).await?;
    }
    let values=s.db.call(move|c|{let mut stmt=c.prepare(&format!("{MEDIA_SELECT} WHERE m.origin=? AND (m.file_key IS NULL OR m.file_key NOT LIKE 'episode:%') AND (?='all' OR m.kind=?) AND lower(m.title) LIKE '%' || lower(?) || '%' ORDER BY m.title LIMIT 500"))?;Ok(stmt.query_map(params![origin,kind,kind,search],media_row)?.collect::<Result<Vec<_>,_>>()?)}).await?;
    Ok(Json(json!(values)))
}
async fn detail(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    s.db.call(move |c| {
        let media = c
            .query_row(&format!("{MEDIA_SELECT} WHERE m.id=?"), [&id], media_row)
            .optional()?
            .ok_or_else(ApiError::not_found)?;
        let mut stmt = c.prepare(&format!("{SOURCE_SELECT} WHERE media_id=?"))?;
        let sources = stmt
            .query_map([&id], source_row)?
            .collect::<Result<Vec<_>, _>>()?;
        let mut stmt = c.prepare(&format!(
            "{MEDIA_SELECT} WHERE m.file_key LIKE ? ORDER BY m.title"
        ))?;
        let episodes = stmt
            .query_map([format!("episode:{id}:%")], media_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Json(
            json!({"media":media,"sources":sources,"episodes":episodes}),
        ))
    })
    .await
}
async fn collections(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
) -> Result<Json<Value>, ApiError> {
    s.db.call(move|c|{let mut stmt=c.prepare(&format!("{MEDIA_SELECT} JOIN collections cl ON cl.media_id=m.id WHERE cl.user_id=? ORDER BY m.title"))?;Ok(Json(json!(stmt.query_map([u.id],media_row)?.collect::<Result<Vec<_>,_>>()?)))}).await
}
#[derive(Deserialize)]
struct Save {
    media_id: String,
}
async fn save(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
    Json(i): Json<Save>,
) -> Result<Json<Value>, ApiError> {
    s.db.call(move |c| {
        c.execute(
            "INSERT OR IGNORE INTO collections VALUES(?,?)",
            params![u.id, i.media_id],
        )?;
        Ok(Json(json!({"ok":true})))
    })
    .await
}
async fn unsave(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    s.db.call(move |c| {
        c.execute(
            "DELETE FROM collections WHERE user_id=? AND media_id=?",
            params![u.id, id],
        )?;
        Ok(Json(json!({"ok":true})))
    })
    .await
}
async fn progress(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
) -> Result<Json<Value>, ApiError> {
    s.db.call(move|c|{let mut stmt=c.prepare("SELECT media_id,position,duration FROM progress WHERE user_id=?")?;Ok(Json(json!(stmt.query_map([u.id],|r|Ok(json!({"media_id":r.get::<_,String>(0)?,"position":r.get::<_,f64>(1)?,"duration":r.get::<_,f64>(2)?})))?.collect::<Result<Vec<_>,_>>()?)))}).await
}
#[derive(Deserialize)]
struct Progress {
    position: f64,
    duration: f64,
}
async fn set_progress(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
    Path(id): Path<String>,
    Json(i): Json<Progress>,
) -> Result<Json<Value>, ApiError> {
    if !i.position.is_finite()
        || !i.duration.is_finite()
        || i.position < 0.
        || i.duration < 0.
        || i.position > i.duration + 10.
    {
        return Err(ApiError::bad("Progreso inválido"));
    }
    s.db.call(move|c|{c.execute("INSERT INTO progress VALUES(?,?,?,?) ON CONFLICT(user_id,media_id) DO UPDATE SET position=excluded.position,duration=excluded.duration",params![u.id,id,i.position,i.duration])?;Ok(Json(json!({"ok":true})))}).await
}
pub async fn settings_value(s: &AppState) -> Result<Value, ApiError> {
    let value=s.setting("settings").await?.and_then(|v|serde_json::from_str::<Value>(&v).ok()).unwrap_or_else(||json!({"torrent_enabled":false,"torrent_quota_gb":0,"cache_quota_gb":0,"download_kib":0,"upload_kib":1024}));
    let mut value = value;
    value["server_name"] = json!(
        s.setting("server_name")
            .await?
            .unwrap_or_else(|| "Soto".into())
    );
    Ok(value)
}
async fn settings(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(settings_value(&s).await?))
}
async fn update_settings(
    State(s): State<AppState>,
    Json(i): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let name = i["server_name"]
        .as_str()
        .unwrap_or("Soto")
        .trim()
        .to_string();
    if name.is_empty() || name.len() > 160 {
        return Err(ApiError::bad("Nombre inválido"));
    }
    for key in [
        "torrent_quota_gb",
        "cache_quota_gb",
        "download_kib",
        "upload_kib",
    ] {
        if i[key].as_u64().is_none_or(|v| v > 1_000_000) {
            return Err(ApiError::bad("Cuotas inválidas"));
        }
    }
    if i["torrent_enabled"].as_bool() == Some(true) && i["torrent_quota_gb"].as_u64() == Some(0) {
        return Err(ApiError::bad("Configura una cuota antes de activar P2P"));
    }
    let limits = json!({"enabled":i["torrent_enabled"].as_bool().unwrap_or(false),"quota_gb":i["torrent_quota_gb"],"download_kib":i["download_kib"],"upload_kib":i["upload_kib"]});
    let previous = settings_value(&s).await?;
    if previous["torrent_enabled"] == true || limits["enabled"] == true {
        let response = s
            .client
            .post(format!("{}/internal/limits", s.config.worker_url))
            .bearer_auth(s.worker_token.as_str())
            .json(&limits)
            .send()
            .await
            .map_err(|_| {
                ApiError::external("Worker no disponible; configuración P2P sin cambios")
            })?;
        if !response.status().is_success() {
            return Err(ApiError::external(
                "El worker rechazó los límites; configuración sin cambios",
            ));
        }
    }
    let data = i.to_string();
    s.db.call(move |c| {
        let tx = c.transaction()?;
        tx.execute("INSERT OR REPLACE INTO meta VALUES('settings',?)", [data])?;
        tx.execute(
            "INSERT OR REPLACE INTO meta VALUES('server_name',?)",
            [name],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await?;
    Ok(Json(settings_value(&s).await?))
}
async fn jobs(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    s.db.call(|c|{let mut stmt=c.prepare("SELECT id,kind,status,progress,error FROM jobs ORDER BY created DESC LIMIT 200")?;Ok(Json(json!(stmt.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"kind":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,"progress":r.get::<_,f64>(3)?,"error":r.get::<_,Option<String>>(4)?})))?.collect::<Result<Vec<_>,_>>()?)))}).await
}
async fn cancel(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let kind = crate::app::job_value(&s, &id).await?["kind"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if kind.starts_with("torrent_") {
        crate::torrents::cancel_job(&s, &id).await?;
    }
    s.db.call(move|c|{c.execute("UPDATE jobs SET status='cancelled',updated=? WHERE id=? AND status IN ('queued','running')",params![now(),id])?;Ok(Json(json!({"ok":true})))}).await
}
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/catalog", get(catalog))
        .route("/catalog/{id}", get(detail))
        .route("/catalog/{id}/poster", get(poster))
        .route("/collections", get(collections).post(save))
        .route("/collections/{id}", axum::routing::delete(unsave))
        .route("/progress", get(progress))
        .route("/progress/{id}", put(set_progress))
        .route("/settings", get(settings).put(update_settings))
        .route("/jobs", get(jobs))
        .route("/jobs/{id}/cancel", post(cancel))
}

async fn poster(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    use axum::response::IntoResponse;
    use futures_util::StreamExt;
    let location: String =
        s.db.call(move |c| {
            c.query_row(
                "SELECT poster FROM media WHERE id=? AND poster IS NOT NULL",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(ApiError::not_found)
        })
        .await?;
    let (client, url) =
        crate::net::external_client(&location, s.config.allow_private_providers).await?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| ApiError::external("Portada no disponible"))?;
    if !response.status().is_success() {
        return Err(ApiError::not_found());
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .to_string();
    if !["image/jpeg", "image/png", "image/webp", "image/avif"].contains(&content_type.as_str()) {
        return Err(ApiError::bad("Formato de portada no permitido"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ApiError::external("Portada interrumpida"))?;
        if bytes.len() + chunk.len() > 5 * 1024 * 1024 {
            return Err(ApiError::bad("Portada demasiado grande"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(([(axum::http::header::CONTENT_TYPE, content_type)], bytes).into_response())
}

fn audio_tracks(details: &str) -> Vec<Value> {
    let v = serde_json::from_str::<Value>(details).unwrap_or(Value::Null);
    v["streams"].as_array().into_iter().flatten().filter(|t|t["codec_type"]=="audio").filter_map(|t|t["index"].as_u64().map(|idx|json!({"id":idx.to_string(),"label":format!("{} · {}",t["tags"]["language"].as_str().unwrap_or("Audio"),t["codec_name"].as_str().unwrap_or("")),"url":""}))).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_tracks_only_expose_real_audio_streams() {
        let details = json!({"streams":[
            {"index":0,"codec_type":"video","codec_name":"h264"},
            {"index":1,"codec_type":"audio","codec_name":"aac","tags":{"language":"spa"}},
            {"index":2,"codec_type":"audio","codec_name":"ac3","tags":{"language":"eng"}},
            {"index":3,"codec_type":"subtitle","codec_name":"subrip"}
        ]})
        .to_string();
        let tracks = audio_tracks(&details);
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0]["id"], "1");
        assert!(tracks[0]["label"].as_str().unwrap().contains("aac"));
        assert_eq!(tracks[1]["id"], "2");
        assert!(tracks[1]["label"].as_str().unwrap().contains("eng"));
        assert!(audio_tracks("not json").is_empty());
    }
}
