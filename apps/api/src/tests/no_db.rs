//! Tests that need no database: they run everywhere, including the upstream gate. The pool is
//! lazy and never connects.

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::routing::get;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

use crate::auth::tokens::TokenKey;
use crate::problem::Problem;
use crate::{AppState, request_id, timeout};

fn state(request_timeout: Duration) -> AppState {
    AppState {
        db: PgPoolOptions::new().connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap(),
        token_key: TokenKey::new(vec![9; 32]).unwrap(),
        request_timeout,
        cookie_secure: true,
        trust_proxy: false,
        limits: Default::default(),
        limiter: Default::default(),
    }
}

async fn send(app: Router, req: Request<Body>) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let resp = app.oneshot(req).await.unwrap();
    let (status, headers) = (resp.status(), resp.headers().clone());
    let bytes = http_body_util::BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    (status, headers, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tokio::test]
async fn healthz_and_meta_need_no_database() {
    let app = crate::router(state(Duration::from_secs(5)), 1024);
    let (status, headers, body) = send(app.clone(), Request::get("/healthz").body(Body::empty()).unwrap()).await;
    assert_eq!((status, body["status"].as_str()), (StatusCode::OK, Some("ok")));
    assert!(uuid::Uuid::parse_str(headers.get("x-request-id").unwrap().to_str().unwrap()).is_ok());
    let (status, _, body) = send(app, Request::get("/api/v1/meta").body(Body::empty()).unwrap()).await;
    assert_eq!((status, body["api_version"].as_str()), (StatusCode::OK, Some("v1")));
}

#[tokio::test]
async fn a_valid_client_request_id_is_echoed_and_a_bad_one_replaced() {
    let app = crate::router(state(Duration::from_secs(5)), 1024);
    let id = "0190b2c4-1a2b-7c3d-8e4f-123456789abc";
    let (_, h, _) = send(app.clone(), Request::get("/healthz").header("x-request-id", id).body(Body::empty()).unwrap()).await;
    assert_eq!(h.get("x-request-id").unwrap(), id);
    let (_, h, _) = send(app, Request::get("/healthz").header("x-request-id", "abc\"; drop").body(Body::empty()).unwrap()).await;
    assert!(uuid::Uuid::parse_str(h.get("x-request-id").unwrap().to_str().unwrap()).is_ok());
}

#[tokio::test]
async fn readyz_is_unready_when_the_database_is_unreachable() {
    let mut s = state(Duration::from_secs(5));
    s.db = PgPoolOptions::new().acquire_timeout(Duration::from_millis(200)).connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap();
    let (status, _, body) = send(crate::router(s, 1024), Request::get("/readyz").body(Body::empty()).unwrap()).await;
    assert_eq!((status, body["code"].as_str()), (StatusCode::SERVICE_UNAVAILABLE, Some("not_ready")));
}

#[tokio::test]
async fn signed_in_routes_refuse_requests_without_a_session_before_touching_the_database() {
    let app = crate::router(state(Duration::from_secs(5)), 1024);
    let (status, _, body) = send(app, Request::get("/api/v1/me").body(Body::empty()).unwrap()).await;
    assert_eq!((status, body["code"].as_str()), (StatusCode::UNAUTHORIZED, Some("unauthenticated")));
}

#[tokio::test]
async fn slow_requests_time_out_as_problem_json() {
    let s = state(Duration::from_millis(20));
    let app = Router::new()
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "late"
            }),
        )
        .layer(middleware::from_fn_with_state(s.clone(), timeout))
        .layer(middleware::from_fn(request_id::middleware))
        .with_state(s);
    let (status, _, body) = send(app, Request::get("/slow").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let p: Problem = serde_json::from_value(body).unwrap();
    assert_eq!(p.code, "timed_out");
    assert!(p.request_id.is_some(), "the problem carries the request ID from the task-local");
}
