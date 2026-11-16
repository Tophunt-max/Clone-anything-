//! `forge-api`: the control plane of the hybrid platform (docs/platform/01-architecture.md).
//!
//! Phase 2: PostgreSQL, accounts, sessions (cookies for browsers, Bearer for Android), refresh
//! token rotation, RBAC, workspaces, members and projects, audit log, rate limits.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod audit;
pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod extract;
pub mod limits;
pub mod page;
pub mod problem;
pub mod request_id;
pub mod routes;
pub mod validate;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

pub use config::Config;
use problem::Problem;

/// Product name shown by `/api/v1/meta`. One place to change it.
pub const PRODUCT_NAME: &str = "Tophunt Forge";
/// The REST API version served under `/api/v1`.
pub const API_VERSION: &str = "v1";

/// The schema migrations, embedded in the binary.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub token_key: auth::tokens::TokenKey,
    pub request_timeout: Duration,
    pub cookie_secure: bool,
    pub trust_proxy: bool,
    pub limits: limits::Limits,
    pub limiter: Arc<limits::RateLimiter>,
}

impl AppState {
    pub fn new(config: &Config, db: PgPool) -> Self {
        AppState {
            db,
            token_key: config.token_key.clone(),
            request_timeout: config.request_timeout,
            cookie_secure: config.cookie_secure,
            trust_proxy: config.trust_proxy,
            limits: limits::Limits::default(),
            limiter: Arc::new(limits::RateLimiter::default()),
        }
    }
}

/// The whole HTTP surface. Layers run outside-in: request ID first, so every response and every
/// error (including timeouts and 404s) carries the ID.
pub fn router(state: AppState, body_limit: usize) -> Router {
    use routes::{auth as a, me, projects as p, workspaces as w};
    let api = Router::new()
        .route("/meta", get(meta))
        .route("/auth/register", post(a::register))
        .route("/auth/login", post(a::login))
        .route("/auth/refresh", post(a::refresh))
        .route("/auth/logout", post(a::logout))
        .route("/me", get(me::me))
        .route("/me/sessions", get(me::sessions))
        .route("/me/sessions/{id}", axum::routing::delete(me::revoke_session))
        .route("/workspaces", get(w::list).post(w::create))
        .route("/workspaces/{ws}", get(w::get).patch(w::rename).delete(w::delete))
        .route("/workspaces/{ws}/members", get(w::members).post(w::add_member))
        .route("/workspaces/{ws}/members/{user}", axum::routing::patch(w::change_member).delete(w::remove_member))
        .route("/workspaces/{ws}/projects", get(p::list).post(p::create))
        .route("/projects/{id}", get(p::get).patch(p::update).delete(p::delete));
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .nest("/api/v1", api)
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(state.clone(), timeout))
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(middleware::from_fn(security_headers))
        .layer(middleware::from_fn(request_id::middleware))
        .with_state(state)
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Health {
    pub status: String,
}

/// Liveness: the process is up and serving.
async fn healthz() -> Json<Health> {
    Json(Health { status: "ok".into() })
}

/// Readiness: the database answers and every migration this binary knows is applied.
async fn readyz(State(state): State<AppState>) -> Response {
    let unready = |detail: &str| Problem::new(StatusCode::SERVICE_UNAVAILABLE, "not_ready", "Not ready", detail).into_response();
    let applied: Result<i64, _> = sqlx::query_scalar("select count(*) from _sqlx_migrations where success").fetch_one(&state.db).await;
    match applied {
        Err(_) => unready("The database can't be reached or isn't migrated."),
        Ok(n) if usize::try_from(n).ok() != Some(MIGRATOR.iter().count()) => unready("Database migrations are pending."),
        Ok(_) => Json(Health { status: "ready".into() }).into_response(),
    }
}

#[derive(Serialize, Debug, PartialEq, Deserialize)]
pub struct Meta {
    pub name: String,
    pub version: String,
    pub api_version: String,
    /// Features this server has turned on; clients hide what isn't listed.
    pub features: Vec<String>,
}

async fn meta() -> Json<Meta> {
    let features = ["auth", "workspaces", "projects"].map(String::from).to_vec();
    Json(Meta { name: PRODUCT_NAME.to_string(), version: env!("CARGO_PKG_VERSION").to_string(), api_version: API_VERSION.to_string(), features })
}

async fn not_found() -> Response {
    Problem::new(StatusCode::NOT_FOUND, "not_found", "Not found", "There is nothing at this address.").into_response()
}

async fn timeout(State(state): State<AppState>, req: Request, next: Next) -> Response {
    match tokio::time::timeout(state.request_timeout, next.run(req)).await {
        Ok(resp) => resp,
        Err(_) => {
            Problem::new(StatusCode::SERVICE_UNAVAILABLE, "timed_out", "Request timed out", "The server took too long to answer. Try again.").into_response()
        }
    }
}

/// Headers for every response. The API serves JSON only, so the CSP forbids everything.
async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    for (k, v) in [
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::CONTENT_SECURITY_POLICY, "default-src 'none'; frame-ancestors 'none'"),
        (header::CACHE_CONTROL, "no-store"),
    ] {
        h.insert(k, HeaderValue::from_static(v));
    }
    resp
}

#[cfg(test)]
mod tests;
