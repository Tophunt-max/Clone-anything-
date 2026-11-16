//! Authentication (docs/platform/01-architecture.md §4): Argon2id passwords, short-lived signed
//! access tokens, rotating refresh tokens with reuse detection, cookie sessions with CSRF
//! protection for browsers and Bearer tokens for the Android app.

pub mod cookies;
pub mod password;
pub mod rbac;
pub mod tokens;

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::AppState;
use crate::error::ApiError;

pub const CSRF_HEADER: &str = "x-csrf-token";

/// The signed-in user of a request. Extracting it checks the access token's signature and expiry,
/// the CSRF header for cookie sessions on state-changing requests, and that the session is still
/// live in the database (so a revoked session stops working at once, not after 10 minutes).
#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let bearer = parts.headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
        let (token, via_cookie) = match bearer {
            Some(t) => (t.trim().to_string(), false),
            None => match cookies::get(&parts.headers, cookies::ACCESS) {
                Some(t) => (t.to_string(), true),
                None => return Err(ApiError::unauthenticated()),
            },
        };
        if via_cookie && !is_safe_method(&parts.method) {
            check_csrf(&parts.headers)?;
        }
        let claims = tokens::verify_access(&state.token_key, &token, chrono::Utc::now().timestamp()).ok_or_else(ApiError::unauthenticated)?;
        let live: Option<bool> = sqlx::query_scalar(
            "select true from sessions s join users u on u.id = s.user_id
             where s.id = $1 and s.user_id = $2 and s.revoked_at is null and s.expires_at > now() and u.status = 'active'",
        )
        .bind(claims.sid)
        .bind(claims.sub)
        .fetch_optional(&state.db)
        .await?;
        match live {
            Some(_) => Ok(AuthUser { user_id: claims.sub, session_id: claims.sid }),
            None => Err(ApiError::unauthenticated()),
        }
    }
}

pub fn is_safe_method(m: &Method) -> bool {
    matches!(*m, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Double-submit check: the `X-CSRF-Token` header must equal the `forge_csrf` cookie. A page on
/// another site can't read the cookie, so it can't send the header.
pub fn check_csrf(headers: &HeaderMap) -> Result<(), ApiError> {
    let cookie = cookies::get(headers, cookies::CSRF);
    let header = headers.get(CSRF_HEADER).and_then(|v| v.to_str().ok());
    match (cookie, header) {
        (Some(c), Some(h)) if !c.is_empty() && bool::from(c.as_bytes().ct_eq(h.as_bytes())) => Ok(()),
        _ => Err(ApiError::forbidden("csrf_failed", "The request is missing its security token. Reload the page and try again.")),
    }
}
