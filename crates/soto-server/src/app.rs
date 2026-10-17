use crate::db::Store;
use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHash, SaltString},
};
use axum::{
    Json, Router,
    extract::{Extension, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit,
    aead::{Aead, AeadCore, OsRng as CryptoRng},
};
use rand::{RngCore, rngs::OsRng};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

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
            "Inicia sesión".into(),
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
    pub attempts: Arc<Mutex<HashMap<String, (u32, i64)>>>,
}
#[derive(Clone, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub admin: bool,
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
        if db.initial_setup_required()? {
            private_file(
                &config.data.join("bootstrap-token"),
                random_secret().as_bytes(),
            )?;
        }
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
            attempts: Arc::new(Mutex::new(HashMap::new())),
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

pub async fn authorize(
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
    let token = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|c| {
            c.split(';')
                .map(str::trim)
                .find_map(|v| v.strip_prefix("soto_session="))
        })
        .ok_or_else(ApiError::unauthorized)?;
    let hash = digest(token);
    let user=s.db.call(move|c|Ok(c.query_row("SELECT u.id,u.username,u.admin FROM users u JOIN sessions s ON s.user_id=u.id WHERE s.token_hash=? AND s.expires>?",params![hash,now()],|r|Ok(User{id:r.get(0)?,username:r.get(1)?,admin:r.get::<_,i64>(2)?==1})).optional()?)).await?.ok_or_else(ApiError::unauthorized)?;
    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}
pub async fn administrator(
    Extension(user): Extension<User>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !user.admin {
        return Err(ApiError::forbidden());
    }
    Ok(next.run(req).await)
}
fn cookie(s: &AppState, token: &str, clear: bool) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "soto_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        if clear { 0 } else { 604800 },
        if s.config
            .public_url
            .as_deref()
            .is_some_and(|u| u.starts_with("https://"))
        {
            "; Secure"
        } else {
            ""
        }
    ))
    .expect("generated session cookie")
}
async fn start_session(s: &AppState, user: String) -> Result<Response, ApiError> {
    let token = random_secret();
    let hash = digest(&token);
    s.db.call(move |c| {
        c.execute("DELETE FROM sessions WHERE expires<?", [now()])?;
        c.execute(
            "INSERT INTO sessions VALUES(?,?,?)",
            params![hash, user, now() + 604800],
        )?;
        Ok(())
    })
    .await?;
    let mut response = Json(json!({"ok":true})).into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie(s, &token, false));
    Ok(response)
}
#[derive(Deserialize)]
struct Setup {
    setup_token: String,
    server_name: String,
    username: String,
    password: String,
}
fn validate_name(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 80 && !s.chars().any(char::is_control)
}
async fn setup_status(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    let count: i64 =
        s.db.call(|c| Ok(c.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?))
            .await?;
    Ok(Json(
        json!({"configured":count>0,"server_name":s.setting("server_name").await?.unwrap_or_else(||"Soto".into())}),
    ))
}
async fn setup(
    State(s): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<Setup>,
) -> Result<Response, ApiError> {
    if headers.get("x-soto-request").and_then(|v| v.to_str().ok()) != Some("1") {
        return Err(ApiError::forbidden());
    }
    if !validate_name(&input.server_name)
        || !validate_name(&input.username)
        || input.password.len() < 12
        || input.password.len() > 1024
    {
        return Err(ApiError::bad(
            "Nombre válido y contraseña de al menos 12 caracteres requeridos",
        ));
    }
    let expected = std::fs::read_to_string(s.config.data.join("bootstrap-token"))
        .map_err(|_| ApiError::forbidden())?;
    if digest(input.setup_token.trim()) != digest(expected.trim()) {
        return Err(ApiError::forbidden());
    }
    let hashed = tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(
                input.password.as_bytes(),
                &SaltString::generate(&mut CryptoRng),
            )
            .map(|p| p.to_string())
            .map_err(|_| ApiError::internal("No se pudo guardar la contraseña"))
    })
    .await
    .map_err(|_| ApiError::internal("Registro interrumpido"))??;
    let id = new_id();
    let uid = id.clone();
    s.db.call(move |c| {
        let tx = c.transaction()?;
        let count: i64 = tx.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        if count > 0 {
            return Err(ApiError::forbidden());
        }
        tx.execute(
            "INSERT INTO users VALUES(?,?,?,1)",
            params![uid, input.username.trim(), hashed],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO meta VALUES('server_name',?)",
            [input.server_name.trim()],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await?;
    let _ = std::fs::remove_file(s.config.data.join("bootstrap-token"));
    start_session(&s, id).await
}
#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}
async fn login(
    State(s): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response, ApiError> {
    if headers.get("x-soto-request").and_then(|v| v.to_str().ok()) != Some("1") {
        return Err(ApiError::forbidden());
    }
    if input.username.len() > 160 || input.password.len() > 1024 {
        return Err(ApiError::unauthorized());
    }
    {
        let mut attempts = s
            .attempts
            .lock()
            .map_err(|_| ApiError::internal("Acceso ocupado"))?;
        attempts.retain(|_, v| now() - v.1 < 60);
        if attempts.len() > 1000 {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit",
                "Espera antes de volver a intentar".into(),
            ));
        }
        let entry = attempts
            .entry(input.username.trim().to_string())
            .or_insert((0, now()));
        entry.0 += 1;
        if entry.0 > 10 {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit",
                "Espera un minuto".into(),
            ));
        }
    }
    let user: Option<(String, String)> =
        s.db.call(move |c| {
            Ok(c.query_row(
                "SELECT id,password_hash FROM users WHERE username=?",
                [input.username.trim()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
        })
        .await?;
    let (id, hash) = user.ok_or_else(ApiError::unauthorized)?;
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|h| {
            Argon2::default()
                .verify_password(input.password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false);
    if !valid {
        return Err(ApiError::unauthorized());
    }
    start_session(&s, id).await
}
async fn session(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"id":u.id,"username":u.username,"admin":u.admin,"server_name":s.setting("server_name").await?.unwrap_or_else(||"Soto".into())}),
    ))
}
async fn logout(State(s): State<AppState>, req: Request) -> Result<Response, ApiError> {
    if let Some(t) = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|c| {
            c.split(';')
                .map(str::trim)
                .find_map(|v| v.strip_prefix("soto_session="))
        })
    {
        let hash = digest(t);
        s.db.call(move |c| {
            c.execute("DELETE FROM sessions WHERE token_hash=?", [hash])?;
            Ok(())
        })
        .await?
    }
    let mut r = Json(json!({"ok":true})).into_response();
    r.headers_mut()
        .insert(header::SET_COOKIE, cookie(&s, "", true));
    Ok(r)
}
pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/setup", get(setup_status).post(setup))
        .route("/session", post(login))
}
pub fn session_routes() -> Router<AppState> {
    Router::new().route("/session", get(session).delete(logout))
}
