//! Request IDs: accept a client's `X-Request-Id` when it is a UUID, otherwise make one (UUIDv7).
//! The ID is put in the request's extensions, on the response header and in the log span.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use uuid::Uuid;

pub const HEADER: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestId(pub Uuid);

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The client's ID if it is a well-formed UUID; anything else (missing, too long, not a UUID) is
/// replaced, so a header can't inject text into logs.
pub fn from_header(value: Option<&HeaderValue>) -> RequestId {
    value
        .and_then(|v| v.to_str().ok())
        .filter(|s| s.len() <= 36)
        .and_then(|s| Uuid::parse_str(s).ok())
        .map(RequestId)
        .unwrap_or_else(|| RequestId(Uuid::now_v7()))
}

pub async fn middleware(mut req: Request, next: Next) -> Response {
    let id = from_header(req.headers().get(&HEADER));
    req.extensions_mut().insert(id);
    let span = tracing::info_span!("request", request_id = %id, method = %req.method(), path = %req.uri().path());
    let mut resp = next.run(req).instrument(span.clone()).await;
    span.in_scope(|| tracing::info!(status = resp.status().as_u16(), "done"));
    if let Ok(v) = HeaderValue::from_str(&id.to_string()) {
        resp.headers_mut().insert(HEADER, v);
    }
    resp
}
