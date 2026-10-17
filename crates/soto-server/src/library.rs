use crate::app::{ApiError, AppState, finish_job, job, job_value, new_id, now};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::{Component, Path as FsPath, PathBuf},
    sync::{Arc, OnceLock},
};

pub fn safe_path(root: &FsPath, relative: &str) -> Result<PathBuf, ApiError> {
    let rel = FsPath::new(relative);
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(ApiError::forbidden());
    }
    let canonical = root
        .canonicalize()
        .map_err(|_| ApiError::bad("La ubicación no está disponible"))?;
    let mut path = canonical.clone();
    for component in rel.components() {
        path.push(component);
        let meta =
            std::fs::symlink_metadata(&path).map_err(|_| ApiError::bad("Ubicación inaccesible"))?;
        if meta.file_type().is_symlink() {
            return Err(ApiError::forbidden());
        }
    }
    let path = path.canonicalize().map_err(|_| ApiError::not_found())?;
    if !path.starts_with(canonical) {
        return Err(ApiError::forbidden());
    }
    Ok(path)
}
fn fingerprint(path: &FsPath) -> Result<String, ApiError> {
    let m = std::fs::metadata(path).map_err(|_| ApiError::bad("Montaje desconectado"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(m.dev().to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        Ok("local".into())
    }
}
async fn roots(State(s): State<AppState>) -> Json<Value> {
    let mut values = s
        .config
        .roots
        .keys()
        .map(|id| json!({"id":id,"name":id}))
        .collect::<Vec<_>>();
    values.sort_by_key(|v| v["id"].as_str().unwrap_or("").to_string());
    Json(json!(values))
}
#[derive(Deserialize)]
struct Browse {
    root_id: String,
    #[serde(default)]
    path: String,
}
async fn browse(
    State(s): State<AppState>,
    Query(i): Query<Browse>,
) -> Result<Json<Value>, ApiError> {
    let root = s
        .config
        .roots
        .get(&i.root_id)
        .ok_or_else(ApiError::forbidden)?
        .clone();
    let relative = i.path;
    tokio::task::spawn_blocking(move||{let path=safe_path(&root,&relative)?;let mut dirs=Vec::new();for entry in std::fs::read_dir(path).map_err(|_|ApiError::bad("No se puede leer la carpeta"))?{let e=entry.map_err(|_|ApiError::bad("Error leyendo carpeta"))?;let ft=e.file_type().map_err(|_|ApiError::bad("Error leyendo carpeta"))?;if ft.is_dir()&&!ft.is_symlink(){let name=e.file_name().to_string_lossy().to_string();dirs.push(json!({"name":name,"path":FsPath::new(&relative).join(&name).to_string_lossy().replace('\\',"/")}));}
if dirs.len()>1000{return Err(ApiError::bad("Demasiadas carpetas; elige una raíz más específica"))}}dirs.sort_by_key(|v|v["name"].as_str().unwrap_or("").to_string());Ok(Json(json!(dirs)))}).await.map_err(|_|ApiError::internal("Exploración interrumpida"))?
}
fn library_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(
        json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"root_id":r.get::<_,String>(2)?,"path":r.get::<_,String>(3)?,"kind":r.get::<_,String>(4)?,"status":r.get::<_,String>(5)?,"last_scan":r.get::<_,Option<i64>>(6)?,"error":r.get::<_,Option<String>>(7)?}),
    )
}
async fn list(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    s.db.call(|c| {
        let mut q = c.prepare(
            "SELECT id,name,root_id,path,kind,status,last_scan,error FROM libraries ORDER BY name",
        )?;
        Ok(Json(json!(
            q.query_map([], library_row)?
                .collect::<Result<Vec<_>, _>>()?
        )))
    })
    .await
}
#[derive(Deserialize)]
struct Add {
    name: String,
    root_id: String,
    #[serde(default)]
    path: String,
    kind: String,
}
async fn add(State(s): State<AppState>, Json(i): Json<Add>) -> Result<Json<Value>, ApiError> {
    if i.name.trim().is_empty()
        || i.name.len() > 160
        || !["movie", "series", "mixed"].contains(&i.kind.as_str())
    {
        return Err(ApiError::bad("Biblioteca inválida"));
    }
    let root = s
        .config
        .roots
        .get(&i.root_id)
        .ok_or_else(ApiError::forbidden)?;
    let path = safe_path(root, &i.path)?;
    if !path.is_dir() {
        return Err(ApiError::bad("Selecciona una carpeta"));
    }
    let device = fingerprint(&path)?;
    std::fs::read_dir(&path).map_err(|_| ApiError::bad("Carpeta sin permiso de lectura"))?;
    let id = new_id();
    let copy = id.clone();
    s.db.call(move |c| {
        let tx = c.transaction()?;
        tx.execute(
            "INSERT INTO libraries(id,name,root_id,path,kind) VALUES(?,?,?,?,?)",
            params![copy, i.name.trim(), i.root_id, i.path, i.kind],
        )?;
        tx.execute(
            "INSERT INTO meta VALUES(?,?)",
            params![format!("rootdev:{copy}"), device],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await?;
    Ok(Json(json!({"id":id})))
}
async fn delete(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    s.db.call(move |c| {
        c.execute("DELETE FROM libraries WHERE id=?", [id])?;
        Ok(Json(json!({"ok":true})))
    })
    .await
}
pub fn filename_metadata(path: &FsPath, kind: &str) -> (String, String, Option<u16>) {
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace(['.', '_'], " ");
    let parts = stem.split_whitespace().collect::<Vec<_>>();
    let year = parts.iter().find_map(|p| {
        p.trim_matches(['(', ')', '[', ']'])
            .parse::<u16>()
            .ok()
            .filter(|y| (1900..=2100).contains(y))
    });
    let title = if let Some(year) = year {
        stem.split(&year.to_string())
            .next()
            .unwrap_or(&stem)
            .trim_matches([' ', '(', '[', '-'])
            .to_string()
    } else {
        stem.trim().to_string()
    };
    let episode = parts.iter().any(|p| {
        let p = p.to_ascii_lowercase();
        p.starts_with('s') && p.contains('e') && p.chars().any(|c| c.is_ascii_digit())
    });
    (
        if title.is_empty() { stem } else { title },
        if kind == "series" || episode {
            "series"
        } else {
            "movie"
        }
        .into(),
        year,
    )
}
pub async fn probe(path: &FsPath) -> Result<Value, ApiError> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(25),
        tokio::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_format",
                "-show_streams",
                "-of",
                "json",
            ])
            .arg(path)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| ApiError::bad("FFprobe agotó el tiempo de espera"))?
    .map_err(|_| ApiError::bad("FFprobe no está disponible en el servidor"))?;
    if !output.status.success() || output.stdout.len() > 2 * 1024 * 1024 {
        return Err(ApiError::bad("Archivo multimedia no reconocido"));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|_| ApiError::bad("Metadatos multimedia inválidos"))
}
fn walk(path: &FsPath, files: &mut Vec<PathBuf>) -> Result<(), ApiError> {
    for entry in std::fs::read_dir(path)
        .map_err(|_| ApiError::bad("Biblioteca desconectada o sin acceso"))?
    {
        let e = entry.map_err(|_| ApiError::bad("Lectura de biblioteca interrumpida"))?;
        let ft = e
            .file_type()
            .map_err(|_| ApiError::bad("Lectura de biblioteca interrumpida"))?;
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            walk(&e.path(), files)?
        } else if ft.is_file()
            && e.path()
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| {
                    ["mp4", "mkv", "webm", "m4v", "avi", "mov", "ts", "m2ts"]
                        .contains(&s.to_ascii_lowercase().as_str())
                })
        {
            files.push(e.path());
        }
        if files.len() > 100000 {
            return Err(ApiError::bad(
                "Biblioteca demasiado grande para un único escaneo",
            ));
        }
    }
    Ok(())
}
async fn scan_run(s: &AppState, library: &str, jid: &str) -> Result<(), ApiError> {
    static LOCK: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let _permit = LOCK
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(1)))
        .acquire()
        .await
        .map_err(|_| ApiError::internal("Escáner detenido"))?;
    let id = library.to_string();
    let (root_id, relative, kind): (String, String, String) =
        s.db.call(move |c| {
            Ok(c.query_row(
                "SELECT root_id,path,kind FROM libraries WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .await?;
    let root = s
        .config
        .roots
        .get(&root_id)
        .ok_or_else(|| ApiError::bad("Raíz ya no configurada"))?;
    let folder = safe_path(root, &relative)?;
    let expected = s.setting(&format!("rootdev:{library}")).await?;
    if expected.as_deref() != Some(&fingerprint(&folder)?) {
        return Err(ApiError::bad(
            "El montaje cambió o está desconectado; vuelve a validar la biblioteca",
        ));
    }
    let folder_copy = folder.clone();
    let files = tokio::task::spawn_blocking(move || {
        let mut f = Vec::new();
        walk(&folder_copy, &mut f)?;
        Ok::<_, ApiError>(f)
    })
    .await
    .map_err(|_| ApiError::internal("Escaneo interrumpido"))??;
    let generation = new_id();
    let total = files.len().max(1);
    for (index, path) in files.into_iter().enumerate() {
        let status = job_value(s, jid).await?;
        if status["status"] == "cancelled" {
            return Err(ApiError::bad("Escaneo cancelado"));
        }
        let safe = safe_path(
            root,
            &path
                .strip_prefix(
                    root.canonicalize()
                        .map_err(|_| ApiError::bad("Montaje desconectado"))?,
                )
                .map_err(|_| ApiError::forbidden())?
                .to_string_lossy(),
        )?;
        let metadata = std::fs::metadata(&safe)
            .map_err(|_| ApiError::bad("Archivo no disponible durante escaneo"))?;
        let mark = format!(
            "{}:{}",
            metadata.len(),
            metadata
                .modified()
                .ok()
                .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|v| v.as_secs())
                .unwrap_or(0)
        );
        let file_key = format!(
            "{library}:{}",
            safe.strip_prefix(&folder)
                .unwrap_or(&safe)
                .to_string_lossy()
        );
        let find = file_key.clone();
        let existing:Option<(String,String)>=s.db.call(move|c|Ok(c.query_row("SELECT m.id,s.details FROM media m JOIN sources s ON s.media_id=m.id WHERE m.file_key=?",[find],|r|Ok((r.get(0)?,r.get(1)?))).optional()?)).await?;
        let unchanged = existing.as_ref().is_some_and(|(_, d)| {
            serde_json::from_str::<Value>(d)
                .ok()
                .is_some_and(|v| v["fingerprint"] == mark)
        });
        let (title, media_kind, year) = filename_metadata(&safe, &kind);
        let info = if unchanged {
            None
        } else {
            Some(
                crate::net::json_response(
                    s.client
                        .post(format!("{}/internal/probe", s.config.worker_url))
                        .bearer_auth(s.worker_token.as_str())
                        .json(&json!({"path":safe.to_string_lossy()}))
                        .send()
                        .await
                        .map_err(|_| ApiError::external("Worker de metadatos no disponible"))?,
                )
                .await?,
            )
        };
        let media_id = existing.map(|v| v.0).unwrap_or_else(new_id);
        let sid = new_id();
        let duration = info
            .as_ref()
            .and_then(|v| v["format"]["duration"].as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .map(|v| v as i64);
        let streams = info.as_ref().and_then(|v| v["streams"].as_array());
        let video = streams
            .and_then(|v| v.iter().find(|s| s["codec_type"] == "video"))
            .cloned()
            .unwrap_or(Value::Null);
        let audio = streams
            .and_then(|v| v.iter().find(|s| s["codec_type"] == "audio"))
            .cloned()
            .unwrap_or(Value::Null);
        let details=json!({"fingerprint":mark,"streams":info.as_ref().map(|v|v["streams"].clone()).unwrap_or(Value::Null)}).to_string();
        let label = safe
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let locator = safe.to_string_lossy().to_string();
        let (lib, scan_marker, jid) = (library.to_string(), generation.clone(), jid.to_string());
        let size =
            i64::try_from(metadata.len()).map_err(|_| ApiError::bad("Archivo demasiado grande"))?;
        s.db.call(move|c|{let tx=c.transaction()?;let status:String=tx.query_row("SELECT status FROM jobs WHERE id=?",[&jid],|r|r.get(0))?;if status=="cancelled"{return Err(ApiError::bad("Escaneo cancelado"))}
if unchanged{tx.execute("UPDATE media SET scan_generation=? WHERE id=?",params![scan_marker,media_id])?;}else{tx.execute("INSERT INTO media(id,title,kind,year,duration_seconds,origin,library_id,file_key,scan_generation) VALUES(?,?,?,?,?,'local',?,?,?) ON CONFLICT(file_key) DO UPDATE SET title=excluded.title,kind=excluded.kind,year=excluded.year,duration_seconds=excluded.duration_seconds,scan_generation=excluded.scan_generation",params![media_id,title,media_kind,year,duration,lib,file_key,scan_marker])?;tx.execute("DELETE FROM sources WHERE media_id=?",[&media_id])?;tx.execute("INSERT INTO sources(id,media_id,origin,kind,label,locator,size,video_codec,audio_codec,details) VALUES(?,?,'local','local',?,?,?,?,?,?)",params![sid,media_id,label,locator,size,video["codec_name"].as_str(),audio["codec_name"].as_str(),details])?;}tx.execute("UPDATE jobs SET status='running',progress=?,updated=? WHERE id=?",params![(index+1) as f64/total as f64,now(),jid])?;tx.commit()?;Ok(())}).await?;
    }
    // Only a fully successful traversal on the original mount may remove stale metadata.
    if expected.as_deref() != Some(&fingerprint(&folder)?) {
        return Err(ApiError::bad("Montaje desconectado durante escaneo"));
    }
    let (lib, scan_marker) = (library.to_string(), generation);
    let job_id = jid.to_string();
    s.db.call(move|c|{let tx=c.transaction()?;let status:String=tx.query_row("SELECT status FROM jobs WHERE id=?",[job_id],|r|r.get(0))?;if status=="cancelled"{return Err(ApiError::bad("Escaneo cancelado"))}tx.execute("DELETE FROM media WHERE library_id=? AND (scan_generation IS NULL OR scan_generation!=?)",params![lib,scan_marker])?;tx.execute("UPDATE libraries SET status='available',error=NULL,last_scan=? WHERE id=?",params![now(),lib])?;tx.commit()?;Ok(())}).await
}
async fn scan(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    let exists = id.clone();
    s.db.call(move |c| {
        let count: i64 =
            c.query_row("SELECT count(*) FROM libraries WHERE id=?", [exists], |r| {
                r.get(0)
            })?;
        if count == 0 {
            return Err(ApiError::not_found());
        }
        Ok(())
    })
    .await?;
    let jid = job(&s, "scan").await?;
    let (job_id, details) = (jid.clone(), json!({"library_id":id}).to_string());
    s.db.call(move |c| {
        c.execute(
            "UPDATE jobs SET details=? WHERE id=?",
            params![details, job_id],
        )?;
        Ok(())
    })
    .await?;
    let output = job_value(&s, &jid).await?;
    tokio::spawn(async move {
        let result = scan_run(&s, &id, &jid).await;
        match result {
            Ok(()) => {
                let _ = finish_job(&s, &jid, "completed", None).await;
            }
            Err(e) => {
                let cancelled = job_value(&s, &jid)
                    .await
                    .ok()
                    .is_some_and(|j| j["status"] == "cancelled");
                if !cancelled {
                    let _ = finish_job(&s, &jid, "failed", Some(&e.2)).await;
                    let _ =
                        s.db.call(move |c| {
                            c.execute(
                                "UPDATE libraries SET status='disconnected',error=? WHERE id=?",
                                params![e.2, id],
                            )?;
                            Ok(())
                        })
                        .await;
                }
            }
        }
    });
    Ok(Json(output))
}
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/roots", get(roots))
        .route("/libraries/browse", get(browse))
        .route("/libraries", get(list).post(add))
        .route("/libraries/{id}", axum::routing::delete(delete))
        .route("/libraries/{id}/scan", post(scan))
}

pub async fn recover(s: &AppState) -> Result<(), ApiError> {
    let pending: Vec<(String, String)> =
        s.db.call(|c| {
            let mut q = c.prepare(
                "SELECT id,details FROM jobs WHERE kind='scan' AND status IN ('queued','running')",
            )?;
            Ok(q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    for (id, details) in pending {
        if let Some(library) = serde_json::from_str::<Value>(&details)
            .ok()
            .and_then(|v| v["library_id"].as_str().map(str::to_string))
        {
            let state = s.clone();
            tokio::spawn(async move {
                let result = scan_run(&state, &library, &id).await;
                let _ = finish_job(
                    &state,
                    &id,
                    if result.is_ok() {
                        "completed"
                    } else {
                        "failed"
                    },
                    result.err().as_ref().map(|e| e.2.as_str()),
                )
                .await;
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filenames_do_not_invent_missing_metadata() {
        let (name, kind, year) = filename_metadata(FsPath::new("Some.Movie.2024.mkv"), "mixed");
        assert_eq!(
            (name, kind, year),
            ("Some Movie".into(), "movie".into(), Some(2024))
        );
        assert_eq!(
            filename_metadata(FsPath::new("Home video.mp4"), "mixed").2,
            None
        );
    }
    #[test]
    fn rejects_parent_and_absolute_paths() {
        let root = std::env::temp_dir();
        assert!(safe_path(&root, "../secret").is_err());
        assert!(safe_path(&root, "/etc/passwd").is_err());
    }
}
