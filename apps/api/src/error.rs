//! The error type of every handler. Each variant becomes a problem+json response; internal
//! errors are logged with the request ID and never shown to the client.

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::problem::Problem;

#[derive(Debug)]
pub enum ApiError {
    Problem(Box<Problem>),
    RateLimited { retry_after_secs: u64 },
    Internal(String),
}

impl ApiError {
    pub fn invalid(field: &str, detail: &str) -> Self {
        ApiError::Problem(Box::new(Problem::new(StatusCode::UNPROCESSABLE_ENTITY, "bad_input", "Invalid input", detail).with_field(field)))
    }

    pub fn unauthenticated() -> Self {
        ApiError::Problem(Box::new(Problem::new(StatusCode::UNAUTHORIZED, "unauthenticated", "Sign in required", "Sign in again to continue.")))
    }

    pub fn unauthorized(code: &str, detail: &str) -> Self {
        ApiError::Problem(Box::new(Problem::new(StatusCode::UNAUTHORIZED, code, "Not signed in", detail)))
    }

    pub fn forbidden(code: &str, detail: &str) -> Self {
        ApiError::Problem(Box::new(Problem::new(StatusCode::FORBIDDEN, code, "Not allowed", detail)))
    }

    /// Also used for resources the caller may not see, so their existence isn't revealed.
    pub fn not_found() -> Self {
        ApiError::Problem(Box::new(Problem::new(StatusCode::NOT_FOUND, "not_found", "Not found", "There is nothing here, or you don't have access to it.")))
    }

    pub fn conflict(code: &str, detail: &str) -> Self {
        ApiError::Problem(Box::new(Problem::new(StatusCode::CONFLICT, code, "Conflict", detail)))
    }

    pub fn internal(e: impl std::fmt::Display) -> Self {
        ApiError::Internal(e.to_string())
    }

    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::Problem(p) => StatusCode::from_u16(p.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            ApiError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(format!("database: {e}"))
    }
}

/// True for a PostgreSQL unique-constraint violation (SQLSTATE 23505).
pub fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(d) if d.code().as_deref() == Some("23505"))
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Problem(p) => p.into_response(),
            ApiError::RateLimited { retry_after_secs } => {
                let mut resp =
                    Problem::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many requests", "Wait a moment, then try again.").into_response();
                if let Ok(v) = HeaderValue::from_str(&retry_after_secs.max(1).to_string()) {
                    resp.headers_mut().insert(header::RETRY_AFTER, v);
                }
                resp
            }
            ApiError::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                Problem::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "Server error",
                    "Something went wrong on the server. Quote the request ID if you report it.",
                )
                .into_response()
            }
        }
    }
}
