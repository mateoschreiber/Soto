use crate::db::Store;
use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit,
    aead::{Aead, AeadCore, OsRng as CryptoRng},
};
use rand::{RngCore, rngs::OsRng};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub &'static str, pub String);
impl ApiError {
    pub fn bad(s: &str) -> Self {
        Self(StatusCode::BAD_REQUEST, "invalid_input", s.into())
    }
    pub fn forbidden() -> Self {
        Self(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Acceso no permitido".into(),
        )
    }
    pub fn unauthorized() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "El worker rechazó la credencial interna".into(),
        )
    }
    pub fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, "not_found", "No encontrado".into())
    }
    pub fn internal(s: &str) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, "internal", s.into())
    }
    pub fn external(s: &str) -> Self {
        Self(StatusCode::BAD_GATEWAY, "provider_error", s.into())
    }
}
impl From<rusqlite::Error> for ApiError {
    fn from(_: rusqlite::Error) -> Self {
        Self::internal("Error de almacenamiento")
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"code":self.1,"message":self.2}))).into_response()
    }
}
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub data: PathBuf,
    pub roots: HashMap<String, PathBuf>,
    pub torrents: PathBuf,
    pub cache: PathBuf,
    pub worker_url: String,
    pub public_url: Option<String>,
    pub allow_private_providers: bool,
}
impl AppConfig {
    pub fn from_env() -> Result<Self, ApiError> {
        let roots: HashMap<String, String> = serde_json::from_str(
            &std::env::var("SOTO_LOCAL_ROOTS").unwrap_or_else(|_| "{}".into()),
        )
        .map_err(|_| ApiError::bad("SOTO_LOCAL_ROOTS debe ser un objeto JSON"))?;
        Ok(Self {
            data: std::env::var("SOTO_DATA_DIR")
                .unwrap_or_else(|_| "data".into())
                .into(),
            roots: roots
                .into_iter()
                .map(|(k, v)| (k, PathBuf::from(v)))
                .collect(),
            torrents: std::env::var("SOTO_TORRENTS_DIR")
                .unwrap_or_else(|_| "torrents".into())
                .into(),
            cache: std::env::var("SOTO_CACHE_DIR")
                .unwrap_or_else(|_| "cache".into())
                .into(),
            worker_url: std::env::var("SOTO_WORKER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3002".into()),
            public_url: std::env::var("SOTO_PUBLIC_URL")
                .ok()
                .filter(|s| !s.is_empty()),
            allow_private_providers: std::env::var("SOTO_ALLOW_PRIVATE_PROVIDERS")
                .ok()
                .as_deref()
                == Some("true"),
        })
    }
}
#[derive(Clone)]
pub struct AppState {
    pub db: Store,
    pub config: Arc<AppConfig>,
    pub client: reqwest::Client,
    pub key: [u8; 32],
    pub worker_token: Arc<String>,
}
#[derive(Clone)]
pub struct LocalOwner {
    pub id: String,
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn random_secret() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    STANDARD.encode(b)
}
pub fn digest(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn private_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), ApiError> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    match options.open(path) {
        Ok(mut f) => f
            .write_all(bytes)
            .map_err(|_| ApiError::internal("No se pudo guardar el secreto")),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(ApiError::internal("No se pudo crear el archivo de secreto")),
    }
}
impl AppState {
    pub fn open(config: AppConfig) -> Result<Self, ApiError> {
        std::fs::create_dir_all(&config.data)
            .map_err(|_| ApiError::internal("El volumen /data no es escribible"))?;
        let key_path = config.data.join("master.key");
        let mut fresh = [0u8; 32];
        OsRng.fill_bytes(&mut fresh);
        private_file(&key_path, &fresh)?;
        let key: [u8; 32] = std::fs::read(key_path)
            .map_err(|_| ApiError::internal("No se puede leer la clave"))?
            .try_into()
            .map_err(|_| ApiError::internal("Clave de cifrado inválida"))?;
        let token_file = std::env::var("SOTO_WORKER_TOKEN_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| config.data.join("worker-token"));
        if let Some(parent) = token_file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| ApiError::internal("Directorio interno no disponible"))?;
        }
        private_file(&token_file, random_secret().as_bytes())?;
        let worker_token = std::fs::read_to_string(token_file)
            .map_err(|_| ApiError::internal("Falta el secreto del worker"))?;
        let db = Store::open(&config.data.join("soto.sqlite"))?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| ApiError::internal("Cliente HTTP no disponible"))?;
        Ok(Self {
            db,
            config: Arc::new(config),
            client,
            key,
            worker_token: Arc::new(worker_token),
        })
    }
    pub async fn setting(&self, key: &str) -> Result<Option<String>, ApiError> {
        let k = key.to_string();
        self.db
            .call(move |c| {
                Ok(
                    c.query_row("SELECT value FROM meta WHERE key=?", [k], |r| r.get(0))
                        .optional()?,
                )
            })
            .await
    }
    pub async fn secret(&self, name: &str) -> Result<Option<String>, ApiError> {
        let n = name.to_string();
        let value: Option<String> = self
            .db
            .call(move |c| {
                Ok(
                    c.query_row("SELECT ciphertext FROM secrets WHERE name=?", [n], |r| {
                        r.get(0)
                    })
                    .optional()?,
                )
            })
            .await?;
        value
            .map(|v| {
                let bytes = STANDARD
                    .decode(v)
                    .map_err(|_| ApiError::internal("Secreto inválido"))?;
                if bytes.len() < 12 {
                    return Err(ApiError::internal("Secreto inválido"));
                }
                let cipher = ChaCha20Poly1305::new((&self.key).into());
                let plain = cipher
                    .decrypt((&bytes[..12]).into(), &bytes[12..])
                    .map_err(|_| ApiError::internal("No se pudo descifrar"))?;
                String::from_utf8(plain).map_err(|_| ApiError::internal("Secreto inválido"))
            })
            .transpose()
    }
    pub async fn save_secret(&self, name: &str, value: &str) -> Result<(), ApiError> {
        let cipher = ChaCha20Poly1305::new((&self.key).into());
        let nonce = ChaCha20Poly1305::generate_nonce(&mut CryptoRng);
        let encrypted = cipher
            .encrypt(&nonce, value.as_bytes())
            .map_err(|_| ApiError::internal("No se pudo cifrar"))?;
        let mut bytes = nonce.to_vec();
        bytes.extend(encrypted);
        let text = STANDARD.encode(bytes);
        let name = name.to_string();
        self.db.call(move|c|{c.execute("INSERT INTO secrets(name,ciphertext) VALUES(?,?) ON CONFLICT(name) DO UPDATE SET ciphertext=excluded.ciphertext",params![name,text])?;Ok(())}).await
    }
}
pub async fn job(state: &AppState, kind: &str) -> Result<String, ApiError> {
    let id = new_id();
    let value = id.clone();
    let kind = kind.to_string();
    state
        .db
        .call(move |c| {
            c.execute(
                "INSERT INTO jobs(id,kind,created,updated) VALUES(?,?,?,?)",
                params![value, kind, now(), now()],
            )?;
            Ok(())
        })
        .await?;
    Ok(id)
}
pub async fn finish_job(
    state: &AppState,
    id: &str,
    status: &str,
    error: Option<&str>,
) -> Result<(), ApiError> {
    let (id, status, error) = (
        id.to_string(),
        status.to_string(),
        error.map(str::to_string),
    );
    state.db.call(move|c|{c.execute("UPDATE jobs SET status=?,error=?,progress=CASE WHEN ?='completed' THEN 1 ELSE progress END,updated=? WHERE id=? AND status!='cancelled'",params![status,error,status,now(),id])?;Ok(())}).await
}
pub async fn job_value(state: &AppState, id: &str) -> Result<Value, ApiError> {
    let id = id.to_string();
    state.db.call(move|c|Ok(c.query_row("SELECT id,kind,status,progress,error FROM jobs WHERE id=?",[id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"kind":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,"progress":r.get::<_,f64>(3)?,"error":r.get::<_,Option<String>>(4)?})))?)).await
}

pub async fn local_request_policy(
    State(s): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        if req
            .headers()
            .get("x-soto-request")
            .and_then(|v| v.to_str().ok())
            != Some("1")
        {
            return Err(ApiError::forbidden());
        }
        if req
            .headers()
            .get("sec-fetch-site")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == "cross-site")
        {
            return Err(ApiError::forbidden());
        }
        if let Some(origin) = req.headers().get("origin").and_then(|v| v.to_str().ok()) {
            let host = req
                .headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let allowed = s
                .config
                .public_url
                .as_deref()
                .map(|u| u.trim_end_matches('/').to_string())
                .unwrap_or_else(|| format!("http://{host}"));
            if origin != allowed {
                return Err(ApiError::forbidden());
            }
        }
    }
    // Soto is a single-owner home service: the owner is the installation, not a
    // person account. The `local` row only preserves the existing DB foreign keys.
    req.extensions_mut()
        .insert(LocalOwner { id: "local".into() });
    Ok(next.run(req).await)
}
