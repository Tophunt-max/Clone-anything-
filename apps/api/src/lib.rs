//! `forge-api`: the control plane of the hybrid platform (docs/platform/01-architecture.md).
//!
//! Phase 1 scope: the HTTP skeleton every later endpoint builds on: request IDs, RFC 9457
//! problem responses, body size limit, per-request timeout, liveness and metadata endpoints.
//! Authentication, PostgreSQL and the worker protocol arrive in Phase 2 and later.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod config;
pub mod problem;
pub mod request_id;

use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::Serialize;

pub use config::Config;
use problem::Problem;
use request_id::RequestId;

/// Product name shown by `/api/v1/meta`. One place to change it.
pub const PRODUCT_NAME: &str = "Tophunt Forge";
/// The REST API version served under `/api/v1`.
pub const API_VERSION: &str = "v1";

#[derive(Clone, Debug)]
pub struct AppState {
    pub request_timeout: Duration,
}

impl AppState {
    pub fn new(config: &Config) -> Self {
        AppState { request_timeout: config.request_timeout }
    }
}

/// The whole HTTP surface. Layers run outside-in: request ID first, so every response and every
/// error (including timeouts and 404s) carries the ID.
pub fn router(state: AppState, body_limit: usize) -> Router {
    let api = Router::new().route("/meta", get(meta));
    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1", api)
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(state.clone(), timeout))
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(middleware::from_fn(request_id::middleware))
        .with_state(state)
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

/// Liveness: the process is up and serving. Readiness (`/readyz`, database and workers) comes
/// with Phase 2, when there is something to be ready for.
async fn healthz() -> Json<Health> {
    Json(Health { status: "ok" })
}

#[derive(Serialize, Debug, PartialEq, serde::Deserialize)]
pub struct Meta {
    pub name: String,
    pub version: String,
    pub api_version: String,
    /// Features this server has turned on. Empty until they exist; clients hide what isn't listed.
    pub features: Vec<String>,
}

async fn meta() -> Json<Meta> {
    Json(Meta { name: PRODUCT_NAME.to_string(), version: env!("CARGO_PKG_VERSION").to_string(), api_version: API_VERSION.to_string(), features: Vec::new() })
}

async fn not_found(Extension(id): Extension<RequestId>) -> Response {
    Problem::new(StatusCode::NOT_FOUND, "not_found", "Not found", "There is nothing at this address.").with_request_id(&id).into_response()
}

async fn timeout(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let id = req.extensions().get::<RequestId>().cloned();
    match tokio::time::timeout(state.request_timeout, next.run(req)).await {
        Ok(resp) => resp,
        Err(_) => {
            let p = Problem::new(StatusCode::SERVICE_UNAVAILABLE, "timed_out", "Request timed out", "The server took too long to answer. Try again.");
            match id {
                Some(id) => p.with_request_id(&id).into_response(),
                None => p.into_response(),
            }
        }
    }
}

#[cfg(test)]
mod tests;
