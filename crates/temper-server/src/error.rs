//! One error shape for every API failure, plus JSON extractor that uses it.

use axum::extract::rejection::JsonRejection;
use axum::extract::FromRequest;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
    pub retryable: bool,
}

impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into(), details: None, retryable: false }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub fn validation(field: &str, message: impl Into<String>) -> Self {
        let mut err = Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
            message,
        );
        err.details = Some(json!({ "field": field }));
        err
    }

    pub fn not_found(what: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{what} not found"),
        )
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        let mut err = Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            message,
        );
        err.retryable = true;
        err
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            message,
        )
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn from_db(err: rusqlite::Error) -> Self {
        match &err {
            rusqlite::Error::SqliteFailure(code, _)
                if code.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Self::conflict("该记录已存在或被引用")
            }
            _ => {
                tracing::error!(error = %err, "database error");
                Self::internal("数据库暂时不可用")
            }
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorPayload<'a>,
}

#[derive(Serialize)]
struct ErrorPayload<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a Value>,
    retryable: bool,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(code = self.code, message = %self.message, "request failed");
        } else {
            tracing::debug!(code = self.code, message = %self.message, "request rejected");
        }
        let body = ErrorBody {
            error: ErrorPayload {
                code: self.code,
                message: &self.message,
                details: self.details.as_ref(),
                retryable: self.retryable,
            },
        };
        (
            self.status,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            Json(body),
        )
            .into_response()
    }
}

/// `Json` with our error envelope instead of axum's plain-text rejection.
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(
        req: axum::http::Request<axum::body::Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(ApiJson(value)),
            Err(rejection) => Err(map_json_rejection(rejection)),
        }
    }
}

fn map_json_rejection(rejection: JsonRejection) -> AppError {
    let status = rejection.status();
    let message = match status {
        StatusCode::UNSUPPORTED_MEDIA_TYPE => "请求需要 Content-Type: application/json",
        StatusCode::PAYLOAD_TOO_LARGE => "请求体过大",
        _ => "请求体不是合法的 JSON，或字段类型不匹配",
    };
    let mut err = AppError::new(status, "invalid_json", message);
    err.details = Some(json!({ "reason": rejection.body_text() }));
    err
}
