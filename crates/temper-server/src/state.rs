//! Process configuration, shared state and the device identity extractor.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::error::AppError;
use crate::store::Db;

#[derive(Debug, Clone)]
pub struct Config {
    pub addr: SocketAddr,
    pub db_path: PathBuf,
    pub static_dir: PathBuf,
    pub cors: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let addr = std::env::var("TEMPER_ADDR")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 8787)));
        let db_path = std::env::var("TEMPER_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data/temper.db"));
        let static_dir = std::env::var("TEMPER_STATIC")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("web"));
        let cors = matches!(
            std::env::var("TEMPER_CORS").as_deref(),
            Ok("1") | Ok("true")
        );
        Self { addr, db_path, static_dir, cors }
    }
}

pub struct AppState {
    pub db: Db,
    pub config: Config,
    pub started: Instant,
    pub native_ok: bool,
    /// Serialised once at boot; the catalog is immutable.
    pub catalog_json: String,
}

impl AppState {
    pub fn new(config: Config) -> Result<Arc<Self>, AppError> {
        let db = Db::open(&config.db_path).map_err(|err| {
            tracing::error!(error = %err, path = %config.db_path.display(), "cannot open database");
            AppError::internal("无法打开数据库文件")
        })?;
        let native_ok = match temper_core::selftest() {
            Ok(()) => true,
            Err(err) => {
                tracing::error!(error = %err, "native core self-test failed");
                false
            }
        };
        Ok(Arc::new(Self {
            db,
            config,
            started: Instant::now(),
            native_ok,
            catalog_json: serde_json::to_string(&temper_core::catalog())
                .unwrap_or_else(|_| "{}".into()),
        }))
    }

    pub fn uptime_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }
}

pub type SharedState = Arc<AppState>;

/// Opaque per-browser identity. No passwords, no e-mail; the client generates
/// a random token once and keeps it in localStorage.
#[derive(Debug, Clone)]
pub struct Device(pub String);

impl Device {
    fn validate(raw: &str) -> bool {
        (6..=64).contains(&raw.len())
            && raw
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }
}

impl FromRequestParts<SharedState> for Device {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get("x-temper-device")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if !Self::validate(raw) {
            return Err(AppError::bad_request(
                "缺少或非法的 X-Temper-Device 头（6–64 位字母、数字、- 或 _）",
            ));
        }
        Ok(Device(raw.to_string()))
    }
}
