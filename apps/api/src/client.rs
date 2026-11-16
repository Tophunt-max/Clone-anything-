//! Who is calling, for rate limits and the audit log: a keyed hash of the client address (never
//! the address itself) and the User-Agent.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::header::USER_AGENT;
use axum::http::request::Parts;

use crate::AppState;
use crate::auth::tokens::keyed_hash;
use crate::error::ApiError;

#[derive(Debug, Clone)]
pub struct ClientMeta {
    pub ip_hash: String,
    pub user_agent: String,
}

impl FromRequestParts<AppState> for ClientMeta {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let direct = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip().to_string());
        let forwarded = state
            .trust_proxy
            .then(|| parts.headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|v| v.rsplit(',').next()).map(|s| s.trim().to_string()))
            .flatten()
            .filter(|s| s.parse::<std::net::IpAddr>().is_ok());
        let ip = forwarded.or(direct).unwrap_or_else(|| "unknown".into());
        let user_agent = parts.headers.get(USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or_default().chars().take(200).collect();
        Ok(ClientMeta { ip_hash: keyed_hash(&state.token_key, &ip), user_agent })
    }
}
