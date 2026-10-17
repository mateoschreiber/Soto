use crate::app::{ApiError, AppState, LocalOwner, new_id, now};
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, Request, State},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use tower::ServiceExt;
#[derive(Deserialize)]
struct Start {
    source_id: String,
    #[serde(default = "auto")]
    mode: String,
    audio_stream: Option<u32>,
}
fn auto() -> String {
    "auto".into()
}
async fn start(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
    Json(i): Json<Start>,
) -> Result<Json<Value>, ApiError> {
    if !["auto", "direct", "convert"].contains(&i.mode.as_str()) {
        return Err(ApiError::bad("Modo de reproducción inválido"));
    }
    let refreshed = crate::debrid::refresh_source(&s, &i.source_id).await?;
    let sid = i.source_id.clone();
    let (media_id,kind,locator,codec,audio,status):(String,String,String,Option<String>,Option<String>,String)=s.db.call(move|c|c.query_row("SELECT media_id,kind,locator,video_codec,audio_codec,status FROM sources WHERE id=?",[sid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?.ok_or_else(ApiError::not_found)).await?;
    let locator = refreshed.unwrap_or(locator);
    if status != "available" {
        return Err(ApiError::bad("La fuente todavía no está disponible"));
    }
    let position_id = media_id.clone();
    let uid = u.id.clone();
    let position: f64 =
        s.db.call(move |c| {
            Ok(c.query_row(
                "SELECT position FROM progress WHERE user_id=? AND media_id=?",
                params![uid, position_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0.))
        })
        .await?;
    let duration_id = media_id.clone();
    let duration: Option<f64> =
        s.db.call(move |c| {
            Ok(c.query_row(
                "SELECT duration_seconds FROM media WHERE id=?",
                [duration_id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten()
            .map(|v| v as f64))
        })
        .await?;
    let compatible = codec
        .as_deref()
        .is_some_and(|v| ["h264", "vp8", "vp9", "av1"].contains(&v))
        && audio
            .as_deref()
            .is_none_or(|a| ["aac", "mp3", "opus", "vorbis"].contains(&a));
    let mp4 = std::path::Path::new(&locator)
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| ["mp4", "m4v", "webm"].contains(&s));
    if let Some(index) = i.audio_stream {
        if kind != "local" {
            return Err(ApiError::bad(
                "La selección de audio requiere una fuente local",
            ));
        }
        let sid = i.source_id.clone();
        let details: String =
            s.db.call(move |c| {
                Ok(
                    c.query_row("SELECT details FROM sources WHERE id=?", [sid], |r| {
                        r.get(0)
                    })?,
                )
            })
            .await?;
        let details = serde_json::from_str::<Value>(&details).unwrap_or(Value::Null);
        if !details["streams"].as_array().is_some_and(|streams| {
            streams.iter().any(|t| {
                t["codec_type"] == "audio" && t["index"].as_u64() == Some(u64::from(index))
            })
        }) {
            return Err(ApiError::bad("Pista de audio no disponible"));
        }
    }
    let convert = i.audio_stream.is_some()
        || i.mode == "convert"
        || (i.mode == "auto" && kind == "local" && !(compatible && mp4));
    let id = new_id();
    let mut worker_id = None;
    if convert {
        let settings = crate::catalog::settings_value(&s).await?;
        if settings["cache_quota_gb"].as_u64().unwrap_or(0) == 0 {
            return Err(ApiError::bad(
                "Configura la cuota de caché para convertir este formato",
            ));
        }
        let response=s.client.post(format!("{}/internal/jobs",s.config.worker_url)).bearer_auth(s.worker_token.as_str()).json(&json!({"id":id,"kind":"transcode","source_kind":kind,"locator":locator,"cache_quota_gb":settings["cache_quota_gb"],"audio_stream":i.audio_stream,"position":if kind=="local"{position}else{0.0}})).send().await.map_err(|_|ApiError::external("Worker no disponible"))?;
        if !response.status().is_success() {
            return Err(ApiError::external("No se pudo iniciar la conversión"));
        }
        worker_id = Some(id.clone());
    }
    let timeline_offset = if convert && kind == "local" {
        position
    } else {
        0.
    };
    let copy = id.clone();
    let mode = if convert { "convert" } else { "direct" };
    let mode_copy = mode.to_string();
    let persisted_source = i.source_id.clone();
    s.db.call(move |c| {
        c.execute(
            "INSERT INTO playback VALUES(?,?,?,?,?,?)",
            params![copy, u.id, persisted_source, mode_copy, worker_id, now()],
        )?;
        Ok(())
    })
    .await?;
    Ok(Json(
        json!({"id":id,"url":if convert{format!("/api/v1/playback/{id}/hls/index.m3u8")}else{format!("/api/v1/playback/{id}/stream")},"mode":mode,"position":if convert{0.0}else{position},"timeline_offset":timeline_offset,"duration":duration,"subtitles":subtitle_tracks(&s,&id,&i.source_id,&media_id,timeline_offset).await?,"audio_streams":[]}),
    ))
}
pub async fn source_for_session(
    s: &AppState,
    user: &LocalOwner,
    id: &str,
) -> Result<(String, String, String), ApiError> {
    let (id, user) = (id.to_string(), user.id.clone());
    s.db.call(move|c|c.query_row("SELECT s.kind,s.locator,p.mode FROM playback p JOIN sources s ON s.id=p.source_id WHERE p.id=? AND p.user_id=? AND s.status='available'",params![id,user],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or_else(ApiError::not_found)).await
}
pub async fn proxy_response(response: reqwest::Response) -> Result<Response, ApiError> {
    let mut out = Response::builder().status(response.status().as_u16());
    for name in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
    ] {
        if let Some(value) = response.headers().get(name) {
            out = out.header(name, value);
        }
    }
    out.body(axum::body::Body::from_stream(response.bytes_stream()))
        .map_err(|_| ApiError::internal("Respuesta multimedia inválida"))
}
async fn stream(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
    Path(id): Path<String>,
    req: Request,
) -> Result<Response, ApiError> {
    let (kind, mut locator, _) = source_for_session(&s, &u, &id).await?;
    if kind == "debrid" {
        let copy = id.clone();
        let source: String =
            s.db.call(move |c| {
                Ok(
                    c.query_row("SELECT source_id FROM playback WHERE id=?", [copy], |r| {
                        r.get(0)
                    })?,
                )
            })
            .await?;
        locator = crate::debrid::refresh_source(&s, &source)
            .await?
            .ok_or_else(ApiError::not_found)?;
    }
    if kind == "local" {
        let path = std::path::PathBuf::from(&locator);
        let mut valid = false;
        for root in s.config.roots.values() {
            if let Ok(canonical) = root.canonicalize()
                && let Ok(relative) = path.strip_prefix(&canonical)
                && crate::library::safe_path(root, &relative.to_string_lossy()).is_ok()
            {
                valid = true;
                break;
            }
        }
        if !valid {
            return Err(ApiError::forbidden());
        }
        return tower_http::services::ServeFile::new(path)
            .oneshot(req)
            .await
            .map(IntoResponse::into_response)
            .map_err(|_| ApiError::internal("No se puede leer el archivo"));
    }
    let url = if kind == "p2p" {
        format!(
            "{}/internal/torrents/{}/stream",
            s.config.worker_url, locator
        )
    } else {
        locator
    };
    let mut request = if kind == "p2p" {
        s.client.get(&url)
    } else {
        let (client, url) =
            crate::net::external_client(&url, s.config.allow_private_providers).await?;
        client.get(url)
    }
    .timeout(std::time::Duration::from_secs(8 * 3600));
    if kind == "p2p" {
        request = request.bearer_auth(s.worker_token.as_str());
    } else {
        crate::net::validate_url(&url, s.config.allow_private_providers).await?;
    }
    if let Some(range) = req.headers().get("range") {
        request = request.header("range", range);
    }
    let response = request
        .send()
        .await
        .map_err(|_| ApiError::external("La fuente no responde"))?;
    proxy_response(response).await
}
async fn hls(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
    Path((id, file)): Path<(String, String)>,
    req: Request,
) -> Result<Response, ApiError> {
    source_for_session(&s, &u, &id).await?;
    if file.contains(['/', '\\'])
        || file.contains("..")
        || !(file.ends_with(".m3u8")
            || (file.ends_with(".m4s") || file == "init.mp4")
            || file.ends_with(".vtt"))
    {
        return Err(ApiError::forbidden());
    }
    let mut request = s
        .client
        .get(format!(
            "{}/internal/jobs/{id}/files/{file}",
            s.config.worker_url
        ))
        .bearer_auth(s.worker_token.as_str());
    if let Some(v) = req.headers().get("range") {
        request = request.header("range", v);
    }
    proxy_response(
        request
            .send()
            .await
            .map_err(|_| ApiError::external("Conversión no disponible"))?,
    )
    .await
}
async fn stop(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let (copy, uid) = (id.clone(), u.id.clone());
    let mode: String =
        s.db.call(move |c| {
            c.query_row(
                "SELECT mode FROM playback WHERE id=? AND user_id=?",
                params![copy, uid],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(ApiError::not_found)
        })
        .await?;
    if mode == "convert" {
        let _ = s
            .client
            .delete(format!("{}/internal/jobs/{id}", s.config.worker_url))
            .bearer_auth(s.worker_token.as_str())
            .send()
            .await;
    }
    s.db.call(move |c| {
        c.execute(
            "DELETE FROM playback WHERE id=? AND user_id=?",
            params![id, u.id],
        )?;
        Ok(())
    })
    .await?;
    Ok(Json(json!({"ok":true})))
}
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/playback", post(start))
        .route("/playback/{id}", axum::routing::delete(stop))
        .route("/playback/{id}/stream", get(stream).head(stream))
        .route("/playback/{id}/hls/{file}", get(hls))
        .route("/playback/{id}/subtitles/{index}", get(local_subtitle))
}

async fn subtitle_tracks(
    s: &AppState,
    id: &str,
    source: &str,
    media: &str,
    offset: f64,
) -> Result<Vec<Value>, ApiError> {
    let source = source.to_string();
    let details: String =
        s.db.call(move |c| {
            Ok(c.query_row(
                "SELECT details FROM sources WHERE id=? AND kind='local'",
                [source],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default())
        })
        .await?;
    let details = serde_json::from_str::<Value>(&details).unwrap_or(Value::Null);
    let mut tracks:Vec<Value>=details["streams"].as_array().into_iter().flatten().filter(|t|t["codec_type"]=="subtitle"&&["subrip","ass","ssa","webvtt","mov_text","text"].contains(&t["codec_name"].as_str().unwrap_or(""))).filter_map(|t|t["index"].as_u64().map(|idx|json!({"id":idx.to_string(),"label":t["tags"]["language"].as_str().unwrap_or("Subtítulo local"),"url":format!("/api/v1/playback/{id}/subtitles/{idx}?offset={offset}")}))).collect();
    tracks.extend(
        crate::providers::subtitles(s, media)
            .await
            .unwrap_or_default(),
    );
    Ok(tracks)
}
async fn local_subtitle(
    State(s): State<AppState>,
    Extension(u): Extension<LocalOwner>,
    Path((id, index)): Path<(String, u32)>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let (kind, path, _) = source_for_session(&s, &u, &id).await?;
    if kind != "local" {
        return Err(ApiError::not_found());
    }
    let offset = query
        .get("offset")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(0.0);
    let response = s
        .client
        .post(format!("{}/internal/subtitles", s.config.worker_url))
        .bearer_auth(s.worker_token.as_str())
        .json(&json!({"path":path,"stream_index":index,"offset":offset}))
        .send()
        .await
        .map_err(|_| ApiError::external("Subtítulo no disponible"))?;
    proxy_response(response).await
}
