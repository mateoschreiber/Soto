use crate::app::{ApiError, AppState, new_id};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sources/{id}/prepare", post(prepare))
        .route("/subtitles/{id}", get(subtitle))
        .route("/catalog/{id}/subtitles", get(subtitles_route))
        .route("/providers", get(list).post(install))
        .route("/providers/{id}", axum::routing::delete(remove))
        .route("/providers/{id}/refresh", post(refresh))
        .route("/catalog/{id}/resolve", post(resolve))
}
async fn fetch(s: &AppState, url: &str) -> Result<Value, ApiError> {
    let (c, u) = crate::net::external_client(url, s.config.allow_private_providers).await?;
    crate::net::json_response(
        c.get(u)
            .send()
            .await
            .map_err(|_| ApiError::external("Proveedor no disponible"))?,
    )
    .await
}
fn manifest(v: &Value) -> Result<(), ApiError> {
    if v["id"].as_str().is_none_or(str::is_empty)
        || v["name"].as_str().is_none_or(str::is_empty)
        || !v["resources"].is_array()
        || !v["types"].is_array()
    {
        return Err(ApiError::bad("Manifest inválido"));
    }
    Ok(())
}
async fn list(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    s.db.call(|c|{let mut q=c.prepare("SELECT id,name,url,status,error FROM providers")?;Ok(Json(json!(q.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"url":r.get::<_,String>(2)?,"status":r.get::<_,String>(3)?,"error":r.get::<_,Option<String>>(4)?})))?.collect::<Result<Vec<_>,_>>()?)))}).await
}
#[derive(Deserialize)]
struct Install {
    manifest_url: String,
}
async fn install(
    State(s): State<AppState>,
    Json(i): Json<Install>,
) -> Result<Json<Value>, ApiError> {
    let v = fetch(&s, &i.manifest_url).await?;
    manifest(&v)?;
    let id = new_id();
    s.save_secret(&format!("provider:{id}"), &i.manifest_url)
        .await?;
    let origin = url::Url::parse(&i.manifest_url)
        .map_err(|_| ApiError::bad("URL inválida"))?
        .origin()
        .ascii_serialization();
    let key = id.clone();
    s.db.call(move |c| {
        c.execute(
            "INSERT INTO providers(id,name,url,manifest) VALUES(?,?,?,?)",
            params![key, v["name"].as_str().unwrap(), origin, v.to_string()],
        )?;
        Ok(())
    })
    .await?;
    Ok(Json(json!({"id":id})))
}
async fn remove(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let provider = id.clone();
    let source_ids: Vec<String> = s
        .db
        .call(move |c| {
            let mut q = c.prepare(
                "SELECT sources.id FROM sources WHERE (sources.media_id IN (SELECT id FROM media WHERE provider_id=?1) OR json_extract(sources.details,'$.provider_id')=?1) AND sources.kind IN ('http','magnet')",
            )?;
            Ok(q.query_map([&provider], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    let mut conversions = Vec::new();
    for sid in &source_ids {
        let key = sid.clone();
        let rows: Vec<String> =
            s.db.call(move |c| {
                let mut q =
                    c.prepare("SELECT id FROM playback WHERE source_id=? AND mode='convert'")?;
                Ok(q.query_map([key], |r| r.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?)
            })
            .await?;
        conversions.extend(rows);
    }
    for playback_id in &conversions {
        let _ = s
            .client
            .delete(format!(
                "{}/internal/jobs/{playback_id}",
                s.config.worker_url
            ))
            .bearer_auth(s.worker_token.as_str())
            .send()
            .await;
    }
    if !conversions.is_empty() {
        s.db.call(move |c| {
            for playback_id in &conversions {
                c.execute("DELETE FROM playback WHERE id=?", [playback_id])?;
            }
            Ok(())
        })
        .await?;
    }
    let key = id.clone();
    s.db.call(move |c| {
        c.execute("UPDATE sources SET status='unavailable' WHERE (media_id IN (SELECT id FROM media WHERE provider_id=?1) OR json_extract(details,'$.provider_id')=?1) AND kind IN ('http','magnet')",[&key])?;
        c.execute("DELETE FROM media WHERE provider_id=? AND NOT EXISTS(SELECT 1 FROM sources WHERE sources.media_id=media.id AND sources.status='available' AND sources.kind IN ('p2p','debrid'))",[&key])?;
        c.execute("DELETE FROM providers WHERE id=?", [&key])?;
        c.execute(
            "DELETE FROM secrets WHERE name=?",
            [format!("provider:{key}")],
        )?;
        Ok(Json(json!({"ok":true})))
    })
    .await
}
async fn refresh(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let url = s
        .secret(&format!("provider:{id}"))
        .await?
        .ok_or_else(ApiError::not_found)?;
    let v = fetch(&s, &url).await?;
    manifest(&v)?;
    s.db.call(move |c| {
        c.execute(
            "UPDATE providers SET name=?,manifest=?,status='available',error=NULL WHERE id=?",
            params![v["name"].as_str().unwrap(), v.to_string(), id],
        )?;
        Ok(Json(json!({"ok":true})))
    })
    .await
}
async fn installed(s: &AppState) -> Result<Vec<(String, Value)>, ApiError> {
    s.db.call(|c| {
        let mut q = c.prepare("SELECT id,manifest FROM providers WHERE status='available'")?;
        Ok(q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                serde_json::from_str::<Value>(&r.get::<_, String>(1)?).unwrap_or(Value::Null),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?)
    })
    .await
}
fn compatible(m: &Value, resource: &str, kind: &str, id: &str) -> bool {
    m["types"]
        .as_array()
        .is_some_and(|a| a.iter().any(|t| t == kind))
        && m["resources"].as_array().is_some_and(|a| {
            a.iter().any(|r| {
                if r.as_str() == Some(resource) {
                    if resource == "catalog" {
                        return true;
                    }
                    return m["idPrefixes"].as_array().is_none_or(|p| {
                        p.iter()
                            .any(|p| p.as_str().is_some_and(|p| id.starts_with(p)))
                    });
                }
                r["name"] == resource
                    && r["types"]
                        .as_array()
                        .is_none_or(|a| a.iter().any(|t| t == kind))
                    && (resource == "catalog"
                        || r["idPrefixes"].as_array().is_none_or(|a| {
                            a.iter()
                                .any(|p| p.as_str().is_some_and(|p| id.starts_with(p)))
                        }))
            })
        })
}
async fn resource_url(s: &AppState, p: &str, segments: &[&str]) -> Result<String, ApiError> {
    let location = s
        .secret(&format!("provider:{p}"))
        .await?
        .ok_or_else(ApiError::not_found)?;
    let mut u = url::Url::parse(&location).map_err(|_| ApiError::bad("URL inválida"))?;
    {
        let mut path = u
            .path_segments_mut()
            .map_err(|_| ApiError::bad("URL inválida"))?;
        path.pop();
        for segment in segments {
            path.push(segment);
        }
    }
    Ok(u.to_string())
}
pub async fn discover(s: &AppState, q: &str) -> Result<(), ApiError> {
    for (p, m) in installed(s).await? {
        for cat in m["catalogs"].as_array().into_iter().flatten().take(30) {
            let Some(kind) = cat["type"]
                .as_str()
                .filter(|k| ["movie", "series"].contains(k))
            else {
                continue;
            };
            let Some(cid) = cat["id"].as_str() else {
                continue;
            };
            if !compatible(&m, "catalog", kind, cid) {
                continue;
            }
            let extra = cat["extra"].as_array();
            if !q.is_empty() && !extra.is_some_and(|a| a.iter().any(|x| x["name"] == "search")) {
                continue;
            }
            let last = if q.is_empty() {
                format!("{cid}.json")
            } else {
                format!("search={q}.json")
            };
            let segments = if q.is_empty() {
                vec!["catalog", kind, last.as_str()]
            } else {
                vec!["catalog", kind, cid, last.as_str()]
            };
            let v = fetch(s, &resource_url(s, &p, &segments).await?).await?;
            for meta in v["metas"].as_array().into_iter().flatten().take(500) {
                let (Some(external), Some(title)) = (meta["id"].as_str(), meta["name"].as_str())
                else {
                    continue;
                };
                let (external, title, kind, p, description, poster) = (
                    external.to_string(),
                    title.to_string(),
                    kind.to_string(),
                    p.clone(),
                    meta["description"].as_str().map(str::to_string),
                    meta["poster"].as_str().map(str::to_string),
                );
                let key = format!("addon:{p}:{external}");
                s.db.call(move|c|{c.execute("INSERT INTO media(id,title,kind,origin,external_id,provider_id,file_key,description,poster) VALUES(?,?,?,'torrent',?,?,?,?,?) ON CONFLICT(file_key) DO UPDATE SET title=excluded.title,description=excluded.description,poster=excluded.poster",params![new_id(),title,kind,external,p,key,description,poster])?;Ok(())}).await?;
            }
        }
    }
    Ok(())
}
async fn resolve(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let key = id.clone();
    let (kind, external): (String, String) =
        s.db.call(move |c| {
            Ok(c.query_row(
                "SELECT kind,external_id FROM media WHERE id=?",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        })
        .await?;
    for (p, m) in installed(&s).await? {
        if compatible(&m, "meta", &kind, &external) {
            let meta = fetch(
                &s,
                &resource_url(&s, &p, &["meta", &kind, &format!("{external}.json")]).await?,
            )
            .await?;
            if kind == "series" {
                let parent = id.clone();
                let provider = p.clone();
                let videos = meta["meta"]["videos"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                s.db.call(move|c|{
                    let tx=c.transaction()?;
                    for video in videos.into_iter().take(10000){
                        let Some(external)=video["id"].as_str().filter(|v|v.len()<500) else{continue};
                        let title=video["title"].as_str().filter(|v|!v.is_empty()).map(str::to_string).unwrap_or_else(||external.to_string());
                        let title=match (video["season"].as_u64(),video["episode"].as_u64()){(Some(a),Some(b))=>format!("S{a:02}E{b:02} · {title}"),_=>title};
                        tx.execute("INSERT INTO media(id,title,kind,description,origin,external_id,provider_id,file_key) VALUES(?,?,'series',?,'torrent',?,?,?) ON CONFLICT(file_key) DO UPDATE SET title=excluded.title,description=excluded.description",params![new_id(),title,video["overview"].as_str(),external,provider,format!("episode:{parent}:{external}")])?;
                    }
                    tx.commit()?;Ok(())
                }).await?;
            }
            let (key, description, poster) = (
                id.clone(),
                meta["meta"]["description"].as_str().map(str::to_string),
                meta["meta"]["poster"].as_str().map(str::to_string),
            );
            s.db.call(move|c|{c.execute("UPDATE media SET description=coalesce(?,description),poster=coalesce(?,poster) WHERE id=? AND origin='torrent'",params![description,poster,key])?;Ok(())}).await?;
        }
        if !compatible(&m, "stream", &kind, &external) {
            continue;
        }
        let url = resource_url(&s, &p, &["stream", &kind, &format!("{external}.json")]).await?;
        let v = fetch(&s, &url).await?;
        for stream in v["streams"].as_array().into_iter().flatten().take(100) {
            if let Some(hash) = stream["infoHash"]
                .as_str()
                .filter(|h| h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()))
            {
                let (mid, locator, label, details) = (
                    id.clone(),
                    format!("magnet:?xt=urn:btih:{hash}"),
                    stream["name"].as_str().unwrap_or("Torrent").to_string(),
                    json!({"file_idx":stream["fileIdx"],"provider_id":p}).to_string(),
                );
                s.db.call(move|c|{let count:i64=c.query_row("SELECT count(*) FROM sources WHERE media_id=? AND locator=?",params![mid,locator],|r|r.get(0))?;if count==0{c.execute("INSERT INTO sources(id,media_id,origin,kind,label,status,locator,details) VALUES(?,?,'torrent','magnet',?,'pending',?,?)",params![new_id(),mid,label,locator,details])?;}Ok(())}).await?;
                continue;
            }
            let Some(locator) = stream["url"].as_str() else {
                continue;
            };
            crate::net::validate_url(locator, s.config.allow_private_providers).await?;
            let (mid, locator, label, p) = (
                id.clone(),
                locator.to_string(),
                stream["name"].as_str().unwrap_or("HTTP").to_string(),
                p.clone(),
            );
            s.db.call(move|c|{let exists:i64=c.query_row("SELECT count(*) FROM sources WHERE media_id=? AND locator=?",params![mid,locator],|r|r.get(0))?;if exists==0{c.execute("INSERT INTO sources(id,media_id,origin,kind,label,locator,details) VALUES(?,?,'torrent','http',?,?,?)",params![new_id(),mid,label,locator,json!({"provider_id":p}).to_string()])?;}Ok(())}).await?;
        }
    }
    s.db.call(move |c| {
        let mut q = c.prepare(&format!(
            "{} WHERE media_id=?",
            crate::catalog::SOURCE_SELECT
        ))?;
        Ok(Json(json!(
            q.query_map([id], crate::catalog::source_row)?
                .collect::<Result<Vec<_>, _>>()?
        )))
    })
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_restrictions() {
        let m = json!({"types":["movie"],"resources":[{"name":"stream","types":["movie"],"idPrefixes":["tt"]}]});
        assert!(compatible(&m, "stream", "movie", "tt123"));
        assert!(!compatible(&m, "stream", "movie", "other"));
        assert!(!compatible(&m, "stream", "series", "tt123"));
    }
    #[test]
    fn rejects_invalid_manifest() {
        assert!(manifest(&json!({"id":"x"})).is_err());
    }
}

#[cfg(test)]
mod controlled_http_tests {
    use super::*;
    #[tokio::test]
    async fn addon_install_discover_resolve_keeps_secrets_private() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let stream_url = format!("{origin}/media.mp4");
        let manifest = json!({"id":"controlled","name":"Controlled","version":"1.0.0","types":["movie"],"resources":["catalog","stream"],"idPrefixes":["tt"],"catalogs":[{"id":"movies","type":"movie"}]});
        let app = Router::new()
            .route(
                "/secret-token/manifest.json",
                get(move || {
                    let m = manifest.clone();
                    async move { Json(m) }
                }),
            )
            .route(
                "/secret-token/catalog/movie/movies.json",
                get(|| async {
                    Json(json!({"metas":[{"id":"tt001","name":"Controlled film","type":"movie"}]}))
                }),
            )
            .route(
                "/secret-token/stream/movie/tt001.json",
                get(move || {
                    let u = stream_url.clone();
                    async move { Json(json!({"streams":[{"name":"Controlled stream","url":u}]})) }
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let data = std::env::temp_dir().join(format!("soto-addon-test-{}", new_id()));
        let s = AppState::open(crate::app::AppConfig {
            data: data.clone(),
            roots: Default::default(),
            torrents: data.join("torrents"),
            cache: data.join("cache"),
            worker_url: origin.clone(),
            public_url: None,
            allow_private_providers: true,
        })
        .unwrap();
        assert!(installed(&s).await.unwrap().is_empty());
        discover(&s, "").await.unwrap();
        let Json(created) = install(
            State(s.clone()),
            Json(Install {
                manifest_url: format!("{origin}/secret-token/manifest.json"),
            }),
        )
        .await
        .unwrap();
        let Json(public) = list(State(s.clone())).await.unwrap();
        assert!(!public.to_string().contains("secret-token"));
        assert_eq!(public[0]["url"], origin);
        assert!(
            s.secret(&format!("provider:{}", created["id"].as_str().unwrap()))
                .await
                .unwrap()
                .unwrap()
                .contains("secret-token")
        );
        discover(&s, "").await.unwrap();
        let id =
            s.db.call(|c| {
                Ok(
                    c.query_row("SELECT id FROM media WHERE external_id='tt001'", [], |r| {
                        r.get::<_, String>(0)
                    })?,
                )
            })
            .await
            .unwrap();
        let Json(sources) = resolve(State(s.clone()), Path(id.clone())).await.unwrap();
        assert_eq!(sources[0]["origin"], "torrent");
        assert_eq!(sources[0]["kind"], "http");
        assert!(sources[0].get("locator").is_none());
        let Json(removed) = remove(
            State(s.clone()),
            Path(created["id"].as_str().unwrap().to_string()),
        )
        .await
        .unwrap();
        assert_eq!(removed["ok"], true);
        let remaining_id = id.clone();
        let remaining: usize = s
            .db
            .call(move |c| {
                Ok(c.query_row(
                    "SELECT count(*) FROM sources WHERE media_id=? AND kind='http' AND status='available'",
                    [remaining_id],
                    |r| r.get::<_, i64>(0),
                )? as usize)
            })
            .await
            .unwrap();
        assert_eq!(remaining, 0, "uninstall must invalidate http sources");
        let media_count: usize =
            s.db.call(move |c| {
                Ok(
                    c.query_row("SELECT count(*) FROM media WHERE id=?", [id], |r| {
                        r.get::<_, i64>(0)
                    })? as usize,
                )
            })
            .await
            .unwrap();
        assert_eq!(media_count, 0, "provider-only media must be removed");
        server.abort();
    }
    #[tokio::test]
    async fn resolving_series_inserts_real_episodes_without_inventing_any() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let manifest = json!({"id":"series","name":"Series","version":"1.0.0","types":["series"],"resources":[{"name":"catalog","types":["series"]},{"name":"meta","types":["series"],"idPrefixes":["tt"]},{"name":"stream","types":["series"],"idPrefixes":["tt"]}],"idPrefixes":["tt"],"catalogs":[{"id":"series","type":"series"}]});
        let app = Router::new()
            .route(
                "/series-token/manifest.json",
                get(move || {
                    let m = manifest.clone();
                    async move { Json(m) }
                }),
            )
            .route(
                "/series-token/catalog/series/series.json",
                get(|| async {
                    Json(json!({"metas":[{"id":"tt100","name":"A show","type":"series"}]}))
                }),
            )
            .route(
                "/series-token/meta/series/tt100.json",
                get(|| async {
                    Json(json!({"meta":{"id":"tt100","type":"series","name":"A show","videos":[
                        {"id":"tt100:1:1","title":"Pilot","season":1,"episode":1,"overview":"First"},
                        {"id":"tt100:1:2","title":"Second","season":1,"episode":2}
                    ]}}))
                }),
            )
            .route(
                "/series-token/stream/series/tt100.json",
                get(|| async { Json(json!({"streams":[]})) }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let data = std::env::temp_dir().join(format!("soto-series-test-{}", new_id()));
        let s = AppState::open(crate::app::AppConfig {
            data: data.clone(),
            roots: Default::default(),
            torrents: data.join("torrents"),
            cache: data.join("cache"),
            worker_url: origin.clone(),
            public_url: None,
            allow_private_providers: true,
        })
        .unwrap();
        let Json(_created) = install(
            State(s.clone()),
            Json(Install {
                manifest_url: format!("{origin}/series-token/manifest.json"),
            }),
        )
        .await
        .unwrap();
        discover(&s, "").await.unwrap();
        let parent =
            s.db.call(|c| {
                Ok(
                    c.query_row("SELECT id FROM media WHERE external_id='tt100'", [], |r| {
                        r.get::<_, String>(0)
                    })?,
                )
            })
            .await
            .unwrap();
        let _ = resolve(State(s.clone()), Path(parent.clone()))
            .await
            .unwrap();
        let episodes = s
            .db
            .call(move |c| {
                let mut stmt = c.prepare(
                    "SELECT title,file_key FROM media WHERE file_key LIKE 'episode:%' ORDER BY file_key",
                )?;
                Ok(stmt
                    .query_map([], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?)
            })
            .await
            .unwrap();
        assert_eq!(episodes.len(), 2, "{episodes:?}");
        assert_eq!(episodes[0].0, "S01E01 · Pilot");
        assert_eq!(episodes[0].1, format!("episode:{parent}:tt100:1:1"));
        assert_eq!(episodes[1].1, format!("episode:{parent}:tt100:1:2"));
        let catalog_rows: usize = s
            .db
            .call(|c| {
                Ok(c.query_row(
                    "SELECT count(*) FROM media WHERE file_key IS NULL OR file_key NOT LIKE 'episode:%'",
                    [],
                    |r| r.get::<_, i64>(0),
                )? as usize)
            })
            .await
            .unwrap();
        assert_eq!(catalog_rows, 1, "catalog must exclude episode children");
        server.abort();
    }
}

pub async fn subtitles(s: &AppState, media_id: &str) -> Result<Vec<Value>, ApiError> {
    let key = media_id.to_string();
    let row =
        s.db.call(move |c| {
            use rusqlite::OptionalExtension;
            Ok(c.query_row(
                "SELECT kind,external_id FROM media WHERE id=? AND external_id IS NOT NULL",
                [key],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?)
        })
        .await?;
    let Some((kind, external)) = row else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for (p, m) in installed(s).await? {
        if !compatible(&m, "subtitles", &kind, &external) {
            continue;
        }
        let v = fetch(
            s,
            &resource_url(s, &p, &["subtitles", &kind, &format!("{external}.json")]).await?,
        )
        .await?;
        for sub in v["subtitles"].as_array().into_iter().flatten().take(100) {
            let Some(url) = sub["url"].as_str() else {
                continue;
            };
            crate::net::validate_url(url, s.config.allow_private_providers).await?;
            let id = crate::app::digest(&format!("{p}:{}:{}", media_id, sub["id"]));
            s.save_secret(&format!("subtitle:{id}"), url).await?;
            out.push(json!({"id":id,"label":sub["lang"].as_str().unwrap_or("Subtítulo"),"url":format!("/api/v1/subtitles/{id}")}));
        }
    }
    Ok(out)
}
async fn subtitles_route(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(subtitles(&s, &id).await?)))
}
async fn subtitle(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    use axum::response::IntoResponse;
    use futures_util::StreamExt;
    if id.len() != 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::not_found());
    }
    let location = s
        .secret(&format!("subtitle:{id}"))
        .await?
        .ok_or_else(ApiError::not_found)?;
    let (c, u) = crate::net::external_client(&location, s.config.allow_private_providers).await?;
    let response = c
        .get(u)
        .send()
        .await
        .map_err(|_| ApiError::external("Subtítulo no disponible"))?;
    if !response.status().is_success() {
        return Err(ApiError::external("Subtítulo no disponible"));
    }
    let mut stream = response.bytes_stream();
    let mut data = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ApiError::external("Subtítulo interrumpido"))?;
        if data.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err(ApiError::external("Subtítulo demasiado grande"));
        }
        data.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(data)
        .map_err(|_| ApiError::external("Subtítulo sin codificación UTF-8"))?;
    let data = if text.trim_start_matches('\u{feff}').starts_with("WEBVTT") {
        text
    } else {
        format!(
            "WEBVTT\n\n{}",
            text.lines()
                .map(|line| if line.contains(" --> ") {
                    line.replace(',', ".")
                } else {
                    line.to_string()
                })
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/vtt; charset=utf-8"),
            (axum::http::header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        data,
    )
        .into_response())
}

async fn prepare(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    use rusqlite::OptionalExtension;
    let row =
        s.db.call(move |c| {
            Ok(c.query_row(
                "SELECT media_id,locator,details FROM sources WHERE id=? AND kind='magnet' AND status IN ('pending','available') AND EXISTS(SELECT 1 FROM providers WHERE providers.id=json_extract(sources.details,'$.provider_id'))",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?)
        })
        .await?
        .ok_or_else(ApiError::not_found)?;
    let details: Value = serde_json::from_str(&row.2).unwrap_or(Value::Null);
    let file_idx = details["file_idx"]
        .as_u64()
        .filter(|x| *x <= u32::MAX as u64);
    let mut request = json!({"engine":v["engine"],"magnet":row.1});
    if v["engine"] == "p2p"
        && let Some(idx) = file_idx
    {
        request["file_ids"] = json!([idx]);
    }
    let job = crate::torrents::create_value(&s, request).await?;
    let key = job["id"].as_str().unwrap().to_string();
    let mid = row.0;
    s.db.call(move |c| {
        let raw: String =
            c.query_row("SELECT details FROM jobs WHERE id=?", [&key], |r| r.get(0))?;
        let mut d: Value = serde_json::from_str(&raw).unwrap_or(json!({}));
        d["media_id"] = json!(mid);
        d["file_idx"] = json!(file_idx);
        c.execute(
            "UPDATE jobs SET details=? WHERE id=?",
            params![d.to_string(), key],
        )?;
        Ok(())
    })
    .await?;
    Ok(Json(job))
}
