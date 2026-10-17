use crate::app::{ApiError, AppConfig};
use axum::{
    Json, Router,
    extract::{Path, Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use librqbit::api::TorrentIdOrHash;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Api, Session, SessionOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU32,
    path::{Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
#[derive(Clone)]
pub struct Worker {
    config: Arc<AppConfig>,
    token: Arc<String>,
    engine: Arc<tokio::sync::Mutex<Option<Api>>>,
    jobs: Arc<Mutex<HashMap<String, Work>>>,
    slot: Arc<tokio::sync::Semaphore>,
    limits: Arc<Mutex<Limits>>,
    mutations: Arc<tokio::sync::Mutex<()>>,
}
#[derive(Clone, Copy, Deserialize, Serialize)]
struct Limits {
    quota_gb: u64,
    download_kib: u32,
    upload_kib: u32,
    enabled: bool,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            quota_gb: 100,
            download_kib: 0,
            upload_kib: 0,
            enabled: false,
        }
    }
}
fn current_limits(s: &Worker) -> Result<Limits, ApiError> {
    s.limits
        .lock()
        .map(|v| *v)
        .map_err(|_| ApiError::internal("Worker ocupado"))
}
fn rate_config(l: Limits) -> librqbit::limits::LimitsConfig {
    librqbit::limits::LimitsConfig {
        download_bps: NonZeroU32::new(l.download_kib.saturating_mul(1024)),
        upload_bps: NonZeroU32::new(l.upload_kib.saturating_mul(1024)),
    }
}
async fn limits(
    State(s): State<Worker>,
    Json(value): Json<Limits>,
) -> Result<Json<Value>, ApiError> {
    if (value.enabled && value.quota_gb == 0) || value.quota_gb > 100000 {
        return Err(ApiError::bad("Cuota inválida"));
    }
    let _mutation = s.mutations.lock().await;
    let folder = s.config.torrents.join(".session");
    std::fs::create_dir_all(&folder)
        .map_err(|_| ApiError::bad("No se puede guardar la configuración"))?;
    let bytes =
        serde_json::to_vec(&value).map_err(|_| ApiError::internal("Configuración inválida"))?;
    std::fs::write(folder.join("worker-limits.json"), bytes)
        .map_err(|_| ApiError::bad("No se puede guardar la configuración"))?;
    *s.limits
        .lock()
        .map_err(|_| ApiError::internal("Worker ocupado"))? = value;
    if let Some(api) = s.engine.lock().await.as_ref() {
        let rates = rate_config(value);
        api.session()
            .ratelimits
            .set_download_bps(rates.download_bps);
        api.session().ratelimits.set_upload_bps(rates.upload_bps);
        if !value.enabled || quota_usage(&s, api, None)? > value.quota_gb * 1024 * 1024 * 1024 {
            for torrent in api.api_torrent_list().torrents {
                if let Some(id) = torrent.id {
                    let _ = api.api_torrent_action_pause(TorrentIdOrHash::Id(id)).await;
                }
            }
        }
    }
    Ok(Json(json!({"ok":true})))
}
fn quota_usage(
    s: &Worker,
    api: &Api,
    replacement: Option<(usize, &HashSet<usize>)>,
) -> Result<u64, ApiError> {
    let mut total = folder_size(&s.config.torrents)?;
    let mut counted = HashSet::new();
    for torrent in api.api_torrent_list().torrents {
        let Some(id) = torrent.id else { continue };
        let details = api
            .api_torrent_details(TorrentIdOrHash::Id(id))
            .map_err(|_| ApiError::not_found())?;
        for (index, file) in details.files.unwrap_or_default().iter().enumerate() {
            let selected = replacement
                .filter(|(tid, _)| *tid == id)
                .map(|(_, ids)| ids.contains(&index))
                .unwrap_or(file.included);
            if selected {
                let path = PathBuf::from(&details.output_folder).join(&file.name);
                total = total.saturating_add(file.length);
                // Physical bytes already counted above must not consume the reservation twice.
                if counted.insert(path.clone()) {
                    let present = safe_local(s, &path.to_string_lossy())
                        .ok()
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map(|m| m.len())
                        .unwrap_or(0);
                    total = total.saturating_sub(present.min(file.length));
                }
            }
        }
    }
    Ok(total)
}
fn check_quota(
    s: &Worker,
    api: &Api,
    replacement: Option<(usize, &HashSet<usize>)>,
) -> Result<(), ApiError> {
    let l = current_limits(s)?;
    if !l.enabled {
        return Err(ApiError::bad("P2P deshabilitado"));
    }
    if quota_usage(s, api, replacement)? > l.quota_gb * 1024 * 1024 * 1024 {
        return Err(ApiError::bad("El torrent supera la cuota configurada"));
    }
    Ok(())
}
struct Work {
    child: Option<tokio::process::Child>,
    status: String,
    error: Option<String>,
    input_cancel: tokio_util::sync::CancellationToken,
    last_access: std::time::Instant,
}
fn reap(mut child: tokio::process::Child, folder: PathBuf) {
    let _ = child.start_kill();
    tokio::spawn(async move {
        let _ = child.wait().await;
        if owned_cache(&folder) {
            let _ = tokio::fs::remove_dir_all(folder).await;
        }
    });
}
const CONVERSION_IDLE_TTL: std::time::Duration = std::time::Duration::from_secs(600);
fn touch(s: &Worker, id: &str) {
    if let Ok(mut lock) = s.jobs.lock()
        && let Some(work) = lock.get_mut(id)
    {
        work.last_access = std::time::Instant::now();
    }
}
async fn reap_idle_jobs(s: &Worker) {
    let mut idle: Vec<(String, Option<tokio::process::Child>)> = Vec::new();
    {
        let Ok(mut lock) = s.jobs.lock() else {
            return;
        };
        let mut to_remove = Vec::new();
        for (id, work) in lock.iter_mut() {
            if work.last_access.elapsed() > CONVERSION_IDLE_TTL {
                work.input_cancel.cancel();
                work.status = "cancelled".into();
                to_remove.push((id.clone(), work.child.take()));
            }
        }
        for (id, child) in to_remove {
            lock.remove(&id);
            idle.push((id, child));
        }
    }
    for (id, child) in idle {
        if let Some(mut child) = child {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        let folder = s.config.cache.join(id);
        if owned_cache(&folder) {
            let _ = tokio::fs::remove_dir_all(folder).await;
        }
    }
}
async fn auth(State(s): State<Worker>, req: Request, next: Next) -> Result<Response, ApiError> {
    let supplied = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if supplied.is_empty() || crate::app::digest(supplied) != crate::app::digest(&s.token) {
        return Err(ApiError::unauthorized());
    }
    Ok(next.run(req).await)
}
async fn engine(s: &Worker) -> Result<Api, ApiError> {
    let limits = current_limits(s)?;
    if !limits.enabled {
        return Err(ApiError::bad("P2P deshabilitado"));
    }
    let mut lock = s.engine.lock().await;
    if let Some(api) = lock.as_ref() {
        return Ok(api.clone());
    }
    std::fs::create_dir_all(&s.config.torrents)
        .map_err(|_| ApiError::bad("El volumen Torrents no es escribible"))?;
    let options = SessionOptions {
        persistence: Some(librqbit::SessionPersistenceConfig::Json {
            folder: Some(s.config.torrents.join(".session")),
        }),
        dht: Some(librqbit::DhtSessionConfig {
            persistence: None,
            ..Default::default()
        }),
        fastresume: true,
        ratelimits: rate_config(limits),
        concurrent_init_limit: Some(1),
        runtime_worker_threads: Some(2),
        peer_limit: Some(32),
        disable_local_service_discovery: true,
        ..Default::default()
    };
    let session = Session::new_with_opts(s.config.torrents.clone(), options)
        .await
        .map_err(|_| ApiError::external("No se pudo iniciar el motor P2P"))?;
    let api = Api::new(session, None);
    *lock = Some(api.clone());
    Ok(api)
}
fn safe_local(s: &Worker, value: &str) -> Result<PathBuf, ApiError> {
    let path = PathBuf::from(value)
        .canonicalize()
        .map_err(|_| ApiError::forbidden())?;
    for root in s
        .config
        .roots
        .values()
        .chain(std::iter::once(&s.config.torrents))
    {
        if let Ok(base) = root.canonicalize()
            && let Ok(relative) = path.strip_prefix(&base)
            && let Ok(path) = crate::library::safe_path(root, &relative.to_string_lossy())
        {
            return Ok(path);
        }
    }
    Err(ApiError::forbidden())
}
#[derive(Deserialize)]
struct Probe {
    path: String,
}
async fn probe(State(s): State<Worker>, Json(i): Json<Probe>) -> Result<Json<Value>, ApiError> {
    let path = safe_local(&s, &i.path)?;
    let _permit = s
        .slot
        .acquire()
        .await
        .map_err(|_| ApiError::internal("Worker detenido"))?;
    Ok(Json(probe_file(&path).await?))
}
const MEDIA_FORMATS: &str = "mov,matroska,webm,avi,mpeg,mpegts,mp3,flac,ogg,wav,aac,srt,webvtt";
async fn bounded_output(mut command: tokio::process::Command) -> Result<Vec<u8>, ApiError> {
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| ApiError::bad("Herramienta multimedia no disponible"))?;
    let output = child
        .stdout
        .take()
        .ok_or_else(|| ApiError::internal("Salida no disponible"))?;
    let operation = async {
        let mut bytes = Vec::new();
        output
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| ApiError::bad("Salida interrumpida"))?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(ApiError::bad("Salida demasiado grande"));
        }
        let status = child
            .wait()
            .await
            .map_err(|_| ApiError::bad("Proceso interrumpido"))?;
        if !status.success() {
            return Err(ApiError::bad("Formato multimedia no admitido"));
        }
        Ok(bytes)
    };
    match tokio::time::timeout(std::time::Duration::from_secs(25), operation).await {
        Ok(Ok(bytes)) => Ok(bytes),
        result => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            match result {
                Ok(Err(e)) => Err(e),
                _ => Err(ApiError::bad("Tiempo multimedia agotado")),
            }
        }
    }
}
async fn probe_file(path: &FsPath) -> Result<Value, ApiError> {
    let mut command = tokio::process::Command::new("ffprobe");
    command
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            MEDIA_FORMATS,
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(path);
    serde_json::from_slice(&bounded_output(command).await?)
        .map_err(|_| ApiError::bad("Metadatos inválidos"))
}
#[derive(Deserialize)]
struct Subtitle {
    path: String,
    stream_index: u32,
    #[serde(default)]
    offset: f64,
}
fn vtt_seconds(s: &str) -> Option<f64> {
    let mut it = s.split(':');
    let a = it.next()?.trim().parse::<f64>().ok()?;
    let b = it.next()?.trim().parse::<f64>().ok()?;
    match it.next() {
        Some(c) => {
            let c = c.trim().parse::<f64>().ok()?;
            Some(a * 3600.0 + b * 60.0 + c)
        }
        None => Some(a * 60.0 + b),
    }
}
fn vtt_time(secs: f64) -> String {
    let total_ms = (secs.max(0.0) * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}
fn shift_webvtt(input: &[u8], offset: f64) -> Vec<u8> {
    if offset <= 0.0 {
        return input.to_vec();
    }
    let text = String::from_utf8_lossy(input);
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        if let Some((start, rest)) = line.split_once("-->") {
            let start = start.trim();
            let mut tokens = rest.split_whitespace();
            let end = tokens.next().unwrap_or("");
            let settings = tokens.collect::<Vec<_>>().join(" ");
            if let (Some(a), Some(b)) = (vtt_seconds(start), vtt_seconds(end)) {
                out.push_str(&format!(
                    "{} --> {}",
                    vtt_time(a - offset),
                    vtt_time(b - offset)
                ));
                if !settings.is_empty() {
                    out.push(' ');
                    out.push_str(&settings);
                }
                out.push('\n');
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out.into_bytes()
}
async fn subtitles(State(s): State<Worker>, Json(i): Json<Subtitle>) -> Result<Response, ApiError> {
    let path = safe_local(&s, &i.path)?;
    let _permit = s
        .slot
        .try_acquire()
        .map_err(|_| ApiError::bad("Tarea multimedia en curso"))?;
    let info = probe_file(&path).await?;
    let stream = info["streams"]
        .as_array()
        .and_then(|streams| {
            streams
                .iter()
                .find(|stream| stream["index"].as_u64() == Some(i.stream_index as u64))
        })
        .ok_or_else(|| ApiError::bad("Subtítulo inválido"))?;
    if stream["codec_type"] != "subtitle"
        || !["subrip", "webvtt", "ass", "ssa", "mov_text", "text"]
            .contains(&stream["codec_name"].as_str().unwrap_or(""))
    {
        return Err(ApiError::bad("Subtítulo de texto requerido"));
    }
    let mut command = tokio::process::Command::new("ffmpeg");
    command
        .args([
            "-nostdin",
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            MEDIA_FORMATS,
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            &format!("0:{}", i.stream_index),
            "-c:s",
            "webvtt",
            "-f",
            "webvtt",
            "pipe:1",
        ]);
    let output = bounded_output(command).await?;
    let output = if i.offset > 0.0 {
        shift_webvtt(&output, i.offset)
    } else {
        output
    };
    Ok((
        [(axum::http::header::CONTENT_TYPE, "text/vtt; charset=utf-8")],
        output,
    )
        .into_response())
}
#[derive(Deserialize)]
struct Convert {
    id: String,
    kind: String,
    source_kind: String,
    locator: String,
    cache_quota_gb: u64,
    #[serde(default)]
    position: f64,
    #[serde(default)]
    audio_stream: Option<u32>,
}
fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() < 100 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn folder_size(path: &FsPath) -> Result<u64, ApiError> {
    let mut size = 0u64;
    if !path.exists() {
        return Ok(0);
    }
    for e in
        std::fs::read_dir(path).map_err(|_| ApiError::bad("No se puede leer el almacenamiento"))?
    {
        let e = e.map_err(|_| ApiError::bad("No se puede leer el almacenamiento"))?;
        let m = e
            .file_type()
            .map_err(|_| ApiError::bad("No se puede leer el almacenamiento"))?;
        if m.is_symlink() {
            continue;
        }
        size = size.saturating_add(if m.is_dir() {
            folder_size(&e.path())?
        } else {
            e.metadata()
                .map_err(|_| ApiError::bad("No se puede leer el almacenamiento"))?
                .len()
        });
    }
    Ok(size)
}
fn owned_cache(folder: &FsPath) -> bool {
    std::fs::symlink_metadata(folder).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        && std::fs::read(folder.join(".soto-worker")).is_ok_and(|b| b == b"soto-worker-v1")
}
async fn convert(State(s): State<Worker>, Json(i): Json<Convert>) -> Result<Json<Value>, ApiError> {
    if !valid_id(&i.id)
        || i.kind != "transcode"
        || i.cache_quota_gb == 0
        || i.cache_quota_gb > 100000
    {
        return Err(ApiError::bad("Conversión inválida"));
    }
    if let Some(work) = s
        .jobs
        .lock()
        .map_err(|_| ApiError::internal("Worker ocupado"))?
        .get(&i.id)
    {
        return Ok(Json(
            json!({"id":i.id,"status":work.status,"error":work.error}),
        ));
    }
    if !i.position.is_finite() || i.position < 0.0 || i.position > 604800.0 {
        return Err(ApiError::bad("Posición inválida"));
    }
    let mut remote = None;
    let locator = if i.source_kind == "local" {
        safe_local(&s, &i.locator)?.to_string_lossy().to_string()
    } else {
        if i.position > 0.0 {
            return Err(ApiError::bad(
                "Esta fuente no admite búsqueda durante la conversión",
            ));
        }
        let response = if i.source_kind == "p2p" {
            let (tid, file) = i
                .locator
                .split_once('/')
                .ok_or_else(|| ApiError::bad("Archivo P2P inválido"))?;
            let tid: usize = tid.parse().map_err(|_| ApiError::bad("Torrent inválido"))?;
            let file: usize = file
                .parse()
                .map_err(|_| ApiError::bad("Archivo inválido"))?;
            let bind =
                std::env::var("SOTO_WORKER_BIND").unwrap_or_else(|_| "127.0.0.1:3002".into());
            let port = bind
                .rsplit(':')
                .next()
                .unwrap_or("3002")
                .parse::<u16>()
                .map_err(|_| ApiError::bad("Puerto inválido"))?;
            reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(std::time::Duration::from_secs(10))
                .read_timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|_| ApiError::internal("Cliente no disponible"))?
                .get(format!(
                    "http://127.0.0.1:{port}/internal/torrents/{tid}/{file}/stream"
                ))
                .bearer_auth(s.token.as_str())
                .send()
                .await
        } else if ["http", "debrid"].contains(&i.source_kind.as_str()) {
            let (client, url) =
                crate::net::external_client(&i.locator, s.config.allow_private_providers).await?;
            client.get(url).send().await
        } else {
            return Err(ApiError::bad("Fuente inválida"));
        }
        .map_err(|_| ApiError::external("Fuente no disponible"))?;
        if !response.status().is_success() {
            return Err(ApiError::external("Fuente no disponible"));
        }
        if response
            .content_length()
            .is_some_and(|n| n > 500 * 1024 * 1024 * 1024)
        {
            return Err(ApiError::bad("Fuente demasiado grande"));
        }
        remote = Some(response);
        "pipe:0".to_owned()
    };
    std::fs::create_dir_all(&s.config.cache)
        .map_err(|_| ApiError::bad("La caché no es escribible"))?;
    let quota = i.cache_quota_gb.saturating_mul(1024 * 1024 * 1024);
    if folder_size(&s.config.cache)? >= quota {
        return Err(ApiError::bad("Caché llena"));
    }
    let permit = s
        .slot
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("Hay una tarea multimedia en curso; espera o cancélala"))?;
    let folder = s.config.cache.join(&i.id);
    std::fs::create_dir(&folder)
        .map_err(|_| ApiError::bad("La caché ya existe o no es escribible"))?;
    std::fs::write(folder.join(".soto-worker"), b"soto-worker-v1")
        .map_err(|_| ApiError::bad("No se pudo crear la caché"))?;
    let mut command = tokio::process::Command::new("ffmpeg");
    command.args([
        "-protocol_whitelist",
        if remote.is_some() {
            "pipe"
        } else {
            "file,pipe"
        },
        "-format_whitelist",
        "mov,matroska,webm,avi,mpeg,mpegts,mp3,flac,ogg,wav,aac",
    ]);
    let audio_map = i
        .audio_stream
        .map(|v| format!("0:{v}"))
        .unwrap_or_else(|| "0:a:0?".into());
    if i.position > 0.0 {
        command.args(["-ss", &i.position.to_string()]);
    }
    command
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-threads",
            "1",
            "-i",
        ])
        .arg(locator)
        .args([
            "-map",
            "0:v:0?",
            "-map",
            &audio_map,
            "-c:v",
            "libx264",
            "-profile:v",
            "baseline",
            "-level:v",
            "3.1",
            "-pix_fmt",
            "yuv420p",
            "-vf",
            "scale=w=1280:h=720:force_original_aspect_ratio=decrease:force_divisible_by=2",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-threads",
            "1",
            "-c:a",
            "aac",
            "-ac",
            "2",
            "-f",
            "hls",
            "-hls_segment_type",
            "fmp4",
            "-hls_fmp4_init_filename",
            "init.mp4",
            "-hls_time",
            "4",
            "-hls_playlist_type",
            "event",
            "-hls_segment_filename",
        ])
        .arg(folder.join("segment-%06d.m4s"))
        .arg(folder.join("index.m3u8"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    if remote.is_some() {
        command.stdin(std::process::Stdio::piped());
    }
    let mut child = command
        .spawn()
        .map_err(|_| ApiError::bad("FFmpeg no está disponible"))?;
    let input_cancel = tokio_util::sync::CancellationToken::new();
    let failed_input = Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Some(response) = remote {
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| ApiError::internal("Entrada multimedia no disponible"))?;
        let cancel = input_cancel.clone();
        let failed = failed_input.clone();
        tokio::spawn(async move {
            let transfer = async {
                let mut bytes = 0u64;
                let mut stream = response.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(|_| ())?;
                    bytes = bytes.saturating_add(chunk.len() as u64);
                    if bytes > 500 * 1024 * 1024 * 1024 {
                        return Err(());
                    }
                    input.write_all(&chunk).await.map_err(|_| ())?;
                }
                input.shutdown().await.map_err(|_| ())
            };
            tokio::select! {
                _ = cancel.cancelled() => {},
                result = tokio::time::timeout(std::time::Duration::from_secs(21600), transfer) => {
                    if !matches!(result, Ok(Ok(()))) { failed.store(true, std::sync::atomic::Ordering::Release); }
                }
            }
        });
    }
    s.jobs
        .lock()
        .map_err(|_| ApiError::internal("Worker ocupado"))?
        .insert(
            i.id.clone(),
            Work {
                child: Some(child),
                status: "running".into(),
                error: None,
                input_cancel,
                last_access: std::time::Instant::now(),
            },
        );
    let state = s.clone();
    let id = i.id.clone();
    tokio::spawn(async move {
        let _permit = permit;
        let started = std::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let usage = folder_size(&state.config.cache).unwrap_or(quota);
            let mut lock = match state.jobs.lock() {
                Ok(l) => l,
                Err(_) => break,
            };
            let Some(work) = lock.get_mut(&id) else { break };
            if work.status != "running" {
                break;
            }
            if usage > quota
                || failed_input.load(std::sync::atomic::Ordering::Acquire)
                || started.elapsed().as_secs() > 21600
            {
                work.input_cancel.cancel();
                if let Some(child) = work.child.take() {
                    reap(child, folder.clone());
                }
                work.status = "failed".into();
                work.error = Some(
                    if usage > quota {
                        "Cuota de caché agotada"
                    } else {
                        "Fuente interrumpida o tiempo de conversión agotado"
                    }
                    .into(),
                );
                break;
            }
            if let Some(child) = work.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        work.input_cancel.cancel();
                        work.status = if status.success() {
                            "completed"
                        } else {
                            "failed"
                        }
                        .into();
                        if !status.success() {
                            work.error = Some("No se pudo convertir el formato".into());
                            let _ = std::fs::remove_dir_all(&folder);
                        }
                        work.child = None;
                        break;
                    }
                    Err(_) => {
                        work.input_cancel.cancel();
                        if let Some(child) = work.child.take() {
                            reap(child, folder.clone());
                        }
                        work.status = "failed".into();
                        work.error = Some("Proceso multimedia interrumpido".into());
                        break;
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }
    });
    Ok(Json(json!({"id":i.id,"status":"running"})))
}
async fn work_status(
    State(s): State<Worker>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    touch(&s, &id);
    let lock = s
        .jobs
        .lock()
        .map_err(|_| ApiError::internal("Worker ocupado"))?;
    let work = lock.get(&id).ok_or_else(ApiError::not_found)?;
    Ok(Json(
        json!({"id":id,"status":work.status,"progress":if work.status=="completed"{1.0}else{0.0},"error":work.error}),
    ))
}
async fn stop_work(
    State(s): State<Worker>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    if !valid_id(&id) {
        return Err(ApiError::forbidden());
    }
    let child = {
        let mut lock = s
            .jobs
            .lock()
            .map_err(|_| ApiError::internal("Worker ocupado"))?;
        if let Some(work) = lock.get_mut(&id) {
            work.input_cancel.cancel();
            work.status = "cancelled".into();
            work.child.take()
        } else {
            None
        }
    };
    if let Some(mut child) = child {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    let folder = s.config.cache.join(id);
    if owned_cache(&folder) {
        let _ = tokio::fs::remove_dir_all(folder).await;
    }
    Ok(Json(json!({"ok":true})))
}
async fn work_file(
    State(s): State<Worker>,
    Path((id, file)): Path<(String, String)>,
    req: Request,
) -> Result<Response, ApiError> {
    if !valid_id(&id)
        || file.contains(['/', '\\'])
        || file.contains("..")
        || !((file.ends_with(".m4s") || file == "init.mp4")
            || file.ends_with(".m3u8")
            || file.ends_with(".vtt"))
    {
        return Err(ApiError::forbidden());
    }
    touch(&s, &id);
    let folder = s.config.cache.join(id);
    if std::fs::symlink_metadata(&folder).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(ApiError::forbidden());
    }
    let path = folder.join(file);
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(ApiError::forbidden());
    }
    for _ in 0..60 {
        if path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    use tower::ServiceExt;
    tower_http::services::ServeFile::new(path)
        .oneshot(req)
        .await
        .map(IntoResponse::into_response)
        .map_err(|_| ApiError::internal("Segmento no disponible"))
}
fn torrent_value(api: &Api, id: usize) -> Result<Value, ApiError> {
    let tid = TorrentIdOrHash::Id(id);
    let details = api
        .api_torrent_details(tid)
        .map_err(|_| ApiError::not_found())?;
    let stats = api.api_stats_v1(tid).map_err(|_| ApiError::not_found())?;
    let state = serde_json::to_value(&stats).unwrap_or(Value::Null);
    let files = details
        .files
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(id, f)| json!({"id":id,"name":f.name,"size":f.length,"selected":f.included}))
        .collect::<Vec<_>>();
    Ok(
        json!({"id":id.to_string(),"name":details.name.unwrap_or(details.info_hash),"status":if stats.finished{"completed"}else{state["state"].as_str().unwrap_or("pending")},"progress":if stats.total_bytes>0{stats.progress_bytes as f64/stats.total_bytes as f64}else{0.0},"files":files}),
    )
}
async fn torrents(State(s): State<Worker>) -> Result<Json<Value>, ApiError> {
    if s.engine.lock().await.is_none()
        && current_limits(&s)?.enabled
        && s.config.torrents.join(".session").exists()
    {
        engine(&s).await?;
    }
    let lock = s.engine.lock().await;
    let Some(api) = lock.as_ref() else {
        return Ok(Json(json!([])));
    };
    let values = api
        .api_torrent_list()
        .torrents
        .into_iter()
        .filter_map(|t| t.id.and_then(|id| torrent_value(api, id).ok()))
        .collect::<Vec<_>>();
    Ok(Json(json!(values)))
}
#[derive(Deserialize)]
struct Add {
    magnet: Option<String>,
    torrent_base64: Option<String>,
    file_ids: Option<Vec<usize>>,
    quota_gb: u64,
    #[serde(default)]
    #[serde(rename = "download_kib")]
    _download_kib: u32,
    #[serde(default)]
    #[serde(rename = "upload_kib")]
    _upload_kib: u32,
}
async fn add_torrent(State(s): State<Worker>, Json(i): Json<Add>) -> Result<Json<Value>, ApiError> {
    if i.quota_gb == 0 || i.quota_gb > 100000 {
        return Err(ApiError::bad("Cuota Torrents inválida"));
    }
    let _mutation = s.mutations.lock().await;
    let api = engine(&s).await?;
    let add = if let Some(magnet) = i.magnet {
        if !magnet.starts_with("magnet:?") || magnet.len() > 16384 {
            return Err(ApiError::bad("Magnet inválido"));
        }
        AddTorrent::from_url(magnet)
    } else {
        let bytes = STANDARD
            .decode(
                i.torrent_base64
                    .ok_or_else(|| ApiError::bad("Falta el torrent"))?,
            )
            .map_err(|_| ApiError::bad("Archivo torrent inválido"))?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(ApiError::bad("Torrent demasiado grande"));
        }
        AddTorrent::from_bytes(bytes)
    };
    let options = AddTorrentOptions {
        paused: true,
        only_files: i.file_ids.clone(),
        overwrite: false,
        peer_limit: Some(32),
        ratelimits: Default::default(),
        ..Default::default()
    };
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(25),
        api.session().add_torrent(add, Some(options)),
    )
    .await
    .map_err(|_| ApiError::external("El torrent no respondió a tiempo"))?
    .map_err(|_| ApiError::bad("No se pudo abrir el torrent"))?;
    let (id, handle, new) = match response {
        AddTorrentResponse::Added(id, h) => (id, h, true),
        AddTorrentResponse::AlreadyManaged(id, h) => (id, h, false),
        _ => return Err(ApiError::bad("Torrent sin archivos")),
    };
    let details = api
        .api_torrent_details(TorrentIdOrHash::Id(id))
        .map_err(|_| ApiError::not_found())?;
    if details.files.as_ref().is_none_or(|files| {
        files.is_empty()
            || files.iter().all(|file| !file.included)
            || i.file_ids
                .as_ref()
                .is_some_and(|ids| ids.is_empty() || ids.iter().any(|index| *index >= files.len()))
    }) {
        if new {
            let _ = api.session().delete(TorrentIdOrHash::Id(id), false).await;
        }
        return Err(ApiError::bad("Selección inválida"));
    }
    if let Err(error) = check_quota(&s, &api, None) {
        if new {
            let _ = api.session().delete(TorrentIdOrHash::Id(id), false).await;
        }
        return Err(error);
    }
    api.session()
        .unpause(&handle)
        .await
        .map_err(|_| ApiError::bad("No se pudo iniciar el torrent"))?;
    Ok(Json(torrent_value(&api, id)?))
}
async fn torrent(State(s): State<Worker>, Path(id): Path<usize>) -> Result<Json<Value>, ApiError> {
    Ok(Json(torrent_value(&engine(&s).await?, id)?))
}
async fn action(
    State(s): State<Worker>,
    Path((id, action)): Path<(usize, String)>,
) -> Result<Json<Value>, ApiError> {
    let api = engine(&s).await?;
    let tid = TorrentIdOrHash::Id(id);
    match action.as_str() {
        "pause" => {
            api.api_torrent_action_pause(tid)
                .await
                .map_err(|_| ApiError::bad("No se pudo pausar"))?;
        }
        "resume" => {
            let _mutation = s.mutations.lock().await;
            check_quota(&s, &api, None)?;
            api.api_torrent_action_start(tid)
                .await
                .map_err(|_| ApiError::bad("No se pudo reanudar"))?;
        }
        "cancel" => {
            let _ = api.api_torrent_action_pause(tid).await;
            api.api_torrent_action_forget(tid)
                .await
                .map_err(|_| ApiError::bad("No se pudo cancelar"))?;
        }
        _ => return Err(ApiError::not_found()),
    }
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
struct Select {
    file_ids: HashSet<usize>,
}
async fn select(
    State(s): State<Worker>,
    Path(id): Path<usize>,
    Json(i): Json<Select>,
) -> Result<Json<Value>, ApiError> {
    let api = engine(&s).await?;
    let details = api
        .api_torrent_details(TorrentIdOrHash::Id(id))
        .map_err(|_| ApiError::not_found())?;
    let count = details.files.map(|f| f.len()).unwrap_or(0);
    if i.file_ids.is_empty() || i.file_ids.iter().any(|id| *id >= count) {
        return Err(ApiError::bad("Selección inválida"));
    }
    let _mutation = s.mutations.lock().await;
    check_quota(&s, &api, Some((id, &i.file_ids)))?;
    api.api_torrent_action_update_only_files(TorrentIdOrHash::Id(id), &i.file_ids)
        .await
        .map_err(|_| ApiError::bad("No se pudo seleccionar archivos"))?;
    Ok(Json(torrent_value(&api, id)?))
}
pub fn range(value: Option<&str>, length: u64) -> Result<(u64, u64, bool), ApiError> {
    if length == 0 {
        return Err(ApiError::bad("Archivo vacío"));
    }
    let Some(value) = value else {
        return Ok((0, length - 1, false));
    };
    let value = value
        .strip_prefix("bytes=")
        .ok_or_else(|| ApiError::bad("Rango inválido"))?;
    let (a, b) = value
        .split_once('-')
        .ok_or_else(|| ApiError::bad("Rango inválido"))?;
    if value.contains(',') {
        return Err(ApiError::bad("Rango múltiple no admitido"));
    }
    if a.is_empty() && b.parse::<u64>().ok() == Some(0) {
        return Err(ApiError::bad("Rango inválido"));
    }
    let start = if a.is_empty() {
        length.saturating_sub(
            b.parse::<u64>()
                .map_err(|_| ApiError::bad("Rango inválido"))?,
        )
    } else {
        a.parse().map_err(|_| ApiError::bad("Rango inválido"))?
    };
    let end = if a.is_empty() || b.is_empty() {
        length - 1
    } else {
        b.parse::<u64>()
            .map_err(|_| ApiError::bad("Rango inválido"))?
            .min(length - 1)
    };
    if start > end || start >= length {
        return Err(ApiError(
            axum::http::StatusCode::RANGE_NOT_SATISFIABLE,
            "invalid_range",
            "Rango fuera del archivo".into(),
        ));
    }
    Ok((start, end, true))
}
async fn torrent_stream(
    State(s): State<Worker>,
    Path((id, file)): Path<(usize, usize)>,
    req: Request,
) -> Result<Response, ApiError> {
    let api = engine(&s).await?;
    let mut stream = api
        .api_stream(TorrentIdOrHash::Id(id), file)
        .await
        .map_err(|_| ApiError::not_found())?;
    let length = stream.len();
    let (start, end, partial) = range(
        req.headers().get("range").and_then(|v| v.to_str().ok()),
        length,
    )?;
    stream
        .seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|_| ApiError::bad("No se pudo buscar en el torrent"))?;
    let details = api
        .api_torrent_details(TorrentIdOrHash::Id(id))
        .map_err(|_| ApiError::not_found())?;
    let name = details
        .files
        .and_then(|v| v.get(file).map(|f| f.name.clone()))
        .unwrap_or_default();
    let content_type = if name.ends_with(".mp4") {
        "video/mp4"
    } else if name.ends_with(".webm") {
        "video/webm"
    } else {
        "video/x-matroska"
    };
    let mut response = Response::builder()
        .status(if partial { 206 } else { 200 })
        .header("content-type", content_type)
        .header("accept-ranges", "bytes")
        .header("content-length", end - start + 1);
    if partial {
        response = response.header("content-range", format!("bytes {start}-{end}/{length}"));
    }
    let body = if req.method() == axum::http::Method::HEAD {
        axum::body::Body::empty()
    } else {
        axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(
            stream.take(end - start + 1),
        ))
    };
    response
        .body(body)
        .map_err(|_| ApiError::internal("Stream inválido"))
}
pub async fn run() -> Result<(), ApiError> {
    let config = AppConfig::from_env()?;
    if let Ok(entries) = std::fs::read_dir(&config.cache) {
        for entry in entries.flatten() {
            if valid_id(&entry.file_name().to_string_lossy()) && owned_cache(&entry.path()) {
                std::fs::remove_dir_all(entry.path())
                    .map_err(|_| ApiError::bad("No se pudo limpiar la caché del worker"))?;
            }
        }
    }
    let token_file = std::env::var("SOTO_WORKER_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config.data.join("worker-token"));
    let token = std::fs::read_to_string(token_file)
        .map_err(|_| ApiError::internal("No se puede leer el secreto interno"))?;
    let stored_limits = std::fs::read(config.torrents.join(".session/worker-limits.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let s = Worker {
        config: Arc::new(config),
        token: Arc::new(token),
        engine: Arc::new(tokio::sync::Mutex::new(None)),
        jobs: Arc::new(Mutex::new(HashMap::new())),
        slot: Arc::new(tokio::sync::Semaphore::new(1)),
        limits: Arc::new(Mutex::new(stored_limits)),
        mutations: Arc::new(tokio::sync::Mutex::new(())),
    };
    let app = Router::new()
        .route(
            "/internal/health",
            get(|| async { Json(json!({"status":"ok"})) }),
        )
        .route("/internal/limits", post(limits))
        .route("/internal/probe", post(probe))
        .route("/internal/subtitles", post(subtitles))
        .route("/internal/jobs", post(convert))
        .route("/internal/jobs/{id}", get(work_status).delete(stop_work))
        .route("/internal/jobs/{id}/files/{file}", get(work_file))
        .route("/internal/torrents", get(torrents).post(add_torrent))
        .route("/internal/torrents/{id}", get(torrent))
        .route("/internal/torrents/{id}/select", post(select))
        .route("/internal/torrents/{id}/{action}", post(action))
        .route(
            "/internal/torrents/{id}/{file}/stream",
            get(torrent_stream).head(torrent_stream),
        )
        .route_layer(middleware::from_fn_with_state(s.clone(), auth))
        .with_state(s.clone())
        .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024));
    let address = std::env::var("SOTO_WORKER_BIND").unwrap_or_else(|_| "127.0.0.1:3002".into());
    let reaper = s.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            reap_idle_jobs(&reaper).await;
        }
    });
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|_| ApiError::internal("Puerto del worker no disponible"))?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            #[cfg(unix)]
            {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("SIGTERM handler");
                tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
            }
            #[cfg(not(unix))]
            let _ = tokio::signal::ctrl_c().await;
            let children = s
                .jobs
                .lock()
                .map(|mut jobs| {
                    jobs.values_mut()
                        .filter_map(|j| {
                            j.input_cancel.cancel();
                            j.child.take()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for mut child in children {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
            if let Some(api) = s.engine.lock().await.as_ref() {
                api.session().stop().await;
            }
        })
        .await
        .map_err(|_| ApiError::internal("Worker detenido"))
}
pub async fn healthcheck() -> Result<(), ApiError> {
    let config = AppConfig::from_env()?;
    let path = std::env::var("SOTO_WORKER_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config.data.join("worker-token"));
    let token =
        std::fs::read_to_string(path).map_err(|_| ApiError::internal("Secreto no disponible"))?;
    let bind = std::env::var("SOTO_WORKER_BIND").unwrap_or_else(|_| "127.0.0.1:3002".into());
    let port: u16 = bind
        .rsplit(':')
        .next()
        .unwrap_or("3002")
        .parse()
        .map_err(|_| ApiError::bad("Puerto inválido"))?;
    let response = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|_| ApiError::internal("Cliente no disponible"))?
        .get(format!("http://127.0.0.1:{port}/internal/health"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| ApiError::external("Worker no disponible"))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn test_worker() -> Worker {
        let base = std::env::temp_dir().join(format!("soto-worker-test-{}", uuid::Uuid::new_v4()));
        Worker {
            config: Arc::new(AppConfig {
                data: base.join("data"),
                torrents: base.join("torrents"),
                cache: base.join("cache"),
                roots: HashMap::new(),
                worker_url: "http://127.0.0.1:3002".into(),
                public_url: None,
                allow_private_providers: false,
            }),
            token: Arc::new("test-token".into()),
            engine: Arc::new(tokio::sync::Mutex::new(None)),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            slot: Arc::new(tokio::sync::Semaphore::new(1)),
            limits: Arc::new(Mutex::new(Limits::default())),
            mutations: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
    #[test]
    fn ranges_support_seek_suffix_and_reject_overflow() {
        assert_eq!(range(Some("bytes=2-4"), 10).unwrap(), (2, 4, true));
        assert_eq!(range(Some("bytes=-3"), 10).unwrap(), (7, 9, true));
        assert!(range(Some("bytes=10-"), 10).is_err());
        assert!(range(Some("bytes=1-2,4-6"), 10).is_err());
        assert_eq!(range(Some("bytes=4-"), 10).unwrap(), (4, 9, true));
        assert_eq!(range(Some("bytes=-99"), 10).unwrap(), (0, 9, true));
        assert!(range(Some("bytes=-0"), 10).is_err());
        assert!(range(Some("bytes=18446744073709551616-"), 10).is_err());
        assert!(range(Some("bytes=-18446744073709551616"), 10).is_err());
    }
    #[test]
    fn job_paths_cannot_escape_cache() {
        assert!(!valid_id("../secret"));
        assert!(!valid_id("/absolute"));
        assert!(valid_id("a2b-345"));
    }
    #[test]
    fn webvtt_timestamps_shift_by_offset_and_clamp_at_zero() {
        let input = b"WEBVTT\n\n00:00.000 --> 00:05.000 align:middle\nHola\n\n01:00.000 --> 01:05.000\nAdios\n";
        let shifted = shift_webvtt(input, 10.0);
        let text = String::from_utf8(shifted).unwrap();
        assert!(
            text.contains("00:00:00.000 --> 00:00:00.000 align:middle"),
            "{text}"
        );
        assert!(text.contains("00:00:50.000 --> 00:00:55.000"), "{text}");
        assert!(text.contains("Hola"));
        assert!(text.contains("Adios"));
        assert!(text.starts_with("WEBVTT"));
        assert_eq!(shift_webvtt(input, 0.0), input);
    }
    #[tokio::test]
    async fn disabled_requests_never_boot_engine_or_create_storage() {
        let worker = test_worker();
        assert!(engine(&worker).await.is_err());
        assert_eq!(torrents(State(worker.clone())).await.unwrap().0, json!([]));
        assert!(worker.engine.lock().await.is_none());
        assert!(!worker.config.torrents.exists());
    }
    #[tokio::test]
    async fn authorization_requires_exact_bearer_secret() {
        use tower::ServiceExt;
        let worker = test_worker();
        let router = Router::new()
            .route("/health", get(|| async { "ok" }))
            .route_layer(middleware::from_fn_with_state(worker, auth));
        for (header, status) in [
            (None, 401),
            (Some("Bearer incorrect"), 401),
            (Some("Bearer test-token"), 200),
            (Some("Basic test-token"), 401),
        ] {
            let mut request = Request::builder().uri("/health");
            if let Some(header) = header {
                request = request.header("authorization", header);
            }
            assert_eq!(
                router
                    .clone()
                    .oneshot(request.body(axum::body::Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status()
                    .as_u16(),
                status
            );
        }
    }
    #[tokio::test]
    async fn limits_persist_without_booting_network_and_cache_ownership_is_required() {
        let worker = test_worker();
        let value = Limits {
            quota_gb: 3,
            download_kib: 64,
            upload_kib: 16,
            enabled: false,
        };
        let _ = limits(State(worker.clone()), Json(value)).await.unwrap();
        let stored: Limits = serde_json::from_slice(
            &std::fs::read(worker.config.torrents.join(".session/worker-limits.json")).unwrap(),
        )
        .unwrap();
        assert!(!stored.enabled);
        assert_eq!(stored.quota_gb, 3);
        assert_eq!(rate_config(stored).download_bps.unwrap().get(), 65536);
        assert!(worker.engine.lock().await.is_none());
        let personal = worker.config.cache.join("personal");
        std::fs::create_dir_all(&personal).unwrap();
        std::fs::write(personal.join("keep"), b"personal").unwrap();
        let _ = stop_work(State(worker.clone()), Path("personal".into()))
            .await
            .unwrap();
        assert!(personal.join("keep").exists());
        assert!(!owned_cache(&personal));
        std::fs::remove_dir_all(worker.config.torrents.parent().unwrap()).unwrap();
    }
    #[tokio::test]
    async fn limits_accept_zero_quota_when_disabled_and_reject_when_enabled() {
        let worker = test_worker();
        let disabled = Limits {
            quota_gb: 0,
            download_kib: 0,
            upload_kib: 0,
            enabled: false,
        };
        let _ = limits(State(worker.clone()), Json(disabled)).await.unwrap();
        assert!(!current_limits(&worker).unwrap().enabled);
        assert_eq!(current_limits(&worker).unwrap().quota_gb, 0);
        let enabled = Limits {
            quota_gb: 0,
            download_kib: 0,
            upload_kib: 0,
            enabled: true,
        };
        assert!(limits(State(worker.clone()), Json(enabled)).await.is_err());
        std::fs::remove_dir_all(worker.config.torrents.parent().unwrap()).unwrap();
    }
    #[tokio::test]
    async fn idle_conversions_are_reaped_after_ttl_but_active_ones_survive() {
        let worker = test_worker();
        worker.jobs.lock().unwrap().insert(
            "idle-job".into(),
            Work {
                child: None,
                status: "completed".into(),
                error: None,
                input_cancel: tokio_util::sync::CancellationToken::new(),
                last_access: std::time::Instant::now()
                    - CONVERSION_IDLE_TTL
                    - std::time::Duration::from_secs(1),
            },
        );
        worker.jobs.lock().unwrap().insert(
            "active-job".into(),
            Work {
                child: None,
                status: "running".into(),
                error: None,
                input_cancel: tokio_util::sync::CancellationToken::new(),
                last_access: std::time::Instant::now(),
            },
        );
        reap_idle_jobs(&worker).await;
        let jobs = worker.jobs.lock().unwrap();
        assert!(jobs.get("idle-job").is_none());
        assert!(jobs.get("active-job").is_some());
    }
    #[tokio::test]
    #[ignore = "requires ffmpeg with libx264; run explicitly in runtime image"]
    async fn actual_ffmpeg_conversion_produces_fmp4_hls() {
        let worker = test_worker();
        std::fs::create_dir_all(&worker.config.torrents).unwrap();
        let input = worker.config.torrents.join("generated.mp4");
        let status = tokio::process::Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=160x120:rate=24",
                "-t",
                "1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&input)
            .status()
            .await
            .expect("FFmpeg required");
        assert!(status.success());
        let _ = convert(
            State(worker.clone()),
            Json(Convert {
                id: "generated-job".into(),
                kind: "transcode".into(),
                source_kind: "local".into(),
                locator: input.to_string_lossy().into(),
                cache_quota_gb: 1,
                position: 0.0,
                audio_stream: None,
            }),
        )
        .await
        .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                let status = work_status(State(worker.clone()), Path("generated-job".into()))
                    .await
                    .unwrap()
                    .0;
                if status["status"] != "running" {
                    assert_eq!(status["status"], "completed", "{status}");
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();
        let folder = worker.config.cache.join("generated-job");
        assert!(folder.join("init.mp4").exists());
        assert!(folder.join("segment-000000.m4s").exists());
        assert!(
            std::fs::read_to_string(folder.join("index.m3u8"))
                .unwrap()
                .contains("#EXT-X-MAP:URI=\"init.mp4\"")
        );
        let _ = stop_work(State(worker.clone()), Path("generated-job".into()))
            .await
            .unwrap();
        assert!(!folder.exists());
        std::fs::remove_dir_all(worker.config.torrents.parent().unwrap()).unwrap();
    }
    #[tokio::test]
    async fn library_reservations_aggregate_and_selection_replaces_only_one_torrent() {
        let worker = test_worker();
        std::fs::create_dir_all(&worker.config.torrents).unwrap();
        let session = Session::new_with_opts(
            worker.config.torrents.clone(),
            SessionOptions {
                dht: None,
                listen: None,
                disable_trackers: true,
                disable_local_service_discovery: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let api = Api::new(session.clone(), None);
        let mut ids = Vec::new();
        for name in ["one", "two"] {
            let mut bytes =
                format!("d4:infod6:lengthi1024e4:name3:{name}12:piece lengthi16384e6:pieces20:")
                    .into_bytes();
            bytes.extend_from_slice(&[0; 20]);
            bytes.extend_from_slice(b"ee");
            let response = session
                .add_torrent(
                    AddTorrent::from_bytes(bytes),
                    Some(AddTorrentOptions {
                        paused: true,
                        ..Default::default()
                    }),
                )
                .await
                .unwrap();
            let AddTorrentResponse::Added(id, _) = response else {
                panic!("expected newly managed torrent")
            };
            ids.push(id);
        }
        assert_eq!(quota_usage(&worker, &api, None).unwrap(), 2048);
        assert_eq!(
            quota_usage(&worker, &api, Some((ids[0], &HashSet::new()))).unwrap(),
            1024
        );
        std::fs::write(worker.config.torrents.join("one"), [0; 512]).unwrap();
        std::fs::write(worker.config.torrents.join("personal"), [0; 17]).unwrap();
        assert_eq!(quota_usage(&worker, &api, None).unwrap(), 2065);
        session.stop().await;
        std::fs::remove_dir_all(worker.config.torrents.parent().unwrap()).unwrap();
    }
    #[tokio::test]
    async fn controlled_local_library_transfer_has_real_progress_and_can_be_cancelled() {
        let worker = test_worker();
        let base = worker.config.torrents.parent().unwrap();
        let seed_dir = base.join("seed");
        std::fs::create_dir_all(&seed_dir).unwrap();
        let content = (0..262144).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        let input = seed_dir.join("controlled.bin");
        std::fs::write(&input, &content).unwrap();
        let options = || SessionOptions {
            dht: None,
            disable_trackers: true,
            disable_local_service_discovery: true,
            listen: Some(librqbit::ListenerOptions {
                listen_addr: "127.0.0.1:0".parse().unwrap(),
                enable_upnp_port_forwarding: false,
                ..Default::default()
            }),
            ..Default::default()
        };
        let seed = Session::new_with_opts(seed_dir, options()).await.unwrap();
        let (torrent, seed_handle) = seed
            .create_and_serve_torrent(&input, Default::default())
            .await
            .unwrap();
        let download = Session::new_with_opts(worker.config.torrents.clone(), options())
            .await
            .unwrap();
        let response = download
            .add_torrent(
                AddTorrent::from_bytes(torrent.as_bytes().unwrap()),
                Some(AddTorrentOptions {
                    initial_peers: Some(vec![seed.listen_addr().unwrap()]),
                    ..Default::default()
                }),
            )
            .await
            .unwrap();
        let AddTorrentResponse::Added(id, handle) = response else {
            panic!("new download expected")
        };
        let api = Api::new(download.clone(), None);
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                let stats = api.api_stats_v1(TorrentIdOrHash::Id(id)).unwrap();
                if stats.finished {
                    assert_eq!(stats.progress_bytes, content.len() as u64);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("controlled peer transfer must complete");
        assert_eq!(
            std::fs::read(worker.config.torrents.join("controlled.bin")).unwrap(),
            content
        );
        download.pause(&handle).await.unwrap();
        download
            .delete(TorrentIdOrHash::Id(id), false)
            .await
            .unwrap();
        assert!(download.get(TorrentIdOrHash::Id(id)).is_none());
        assert!(worker.config.torrents.join("controlled.bin").exists());
        seed.pause(&seed_handle).await.unwrap();
        download.stop().await;
        seed.stop().await;
        std::fs::remove_dir_all(base).unwrap();
    }
}
