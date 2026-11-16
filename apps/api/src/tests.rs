use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::*;

fn app() -> Router {
    router(AppState { request_timeout: Duration::from_secs(5) }, 1024)
}

async fn body_json(resp: Response) -> serde_json::Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn healthz_is_ok_and_has_a_request_id() {
    let resp = app().oneshot(Request::get("/healthz").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let id = resp.headers().get("x-request-id").unwrap().to_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&id).is_ok(), "{id}");
    assert_eq!(body_json(resp).await, serde_json::json!({"status": "ok"}));
}

#[tokio::test]
async fn meta_reports_name_version_and_api_version() {
    let resp = app().oneshot(Request::get("/api/v1/meta").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let meta: Meta = serde_json::from_value(body_json(resp).await).unwrap();
    assert_eq!(meta.name, PRODUCT_NAME);
    assert_eq!(meta.api_version, "v1");
    assert_eq!(meta.version, env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn a_valid_client_request_id_is_echoed() {
    let id = "0190b2c4-1a2b-7c3d-8e4f-123456789abc";
    let resp = app().oneshot(Request::get("/healthz").header("x-request-id", id).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.headers().get("x-request-id").unwrap(), id);
}

#[tokio::test]
async fn a_malformed_request_id_is_replaced() {
    let resp = app().oneshot(Request::get("/healthz").header("x-request-id", "abc\"; drop").body(Body::empty()).unwrap()).await.unwrap();
    let got = resp.headers().get("x-request-id").unwrap().to_str().unwrap();
    assert_ne!(got, "abc\"; drop");
    assert!(uuid::Uuid::parse_str(got).is_ok());
}

#[tokio::test]
async fn unknown_paths_are_problem_json_404_with_the_request_id() {
    let resp = app().oneshot(Request::get("/api/v1/nope").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(resp.headers().get(header::CONTENT_TYPE).unwrap(), problem::CONTENT_TYPE);
    let header_id = resp.headers().get("x-request-id").unwrap().to_str().unwrap().to_string();
    let p: Problem = serde_json::from_value(body_json(resp).await).unwrap();
    assert_eq!(p.code, "not_found");
    assert_eq!(p.status, 404);
    assert_eq!(p.request_id.as_deref(), Some(header_id.as_str()));
}

#[tokio::test]
async fn bodies_over_the_limit_are_refused() {
    let app = Router::new()
        .route("/echo", axum::routing::post(|b: String| async move { b }))
        .layer(DefaultBodyLimit::max(16))
        .layer(axum::middleware::from_fn(request_id::middleware));
    let resp = app.oneshot(Request::post("/echo").body(Body::from(vec![b'a'; 64])).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn slow_requests_time_out_as_problem_json() {
    let state = AppState { request_timeout: Duration::from_millis(20) };
    let app = Router::new()
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "late"
            }),
        )
        .layer(middleware::from_fn_with_state(state.clone(), timeout))
        .layer(middleware::from_fn(request_id::middleware))
        .with_state(state);
    let resp = app.oneshot(Request::get("/slow").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let p: Problem = serde_json::from_value(body_json(resp).await).unwrap();
    assert_eq!(p.code, "timed_out");
    assert!(p.request_id.is_some());
}

#[test]
fn config_defaults_and_validation() {
    let c = Config::from_lookup(|_| None).unwrap();
    assert_eq!(c.addr.to_string(), "127.0.0.1:8080");
    assert_eq!(c.request_timeout, Duration::from_secs(30));
    assert_eq!(c.body_limit, 1024 * 1024);
    assert!(Config::from_lookup(|k| (k == "FORGE_API_ADDR").then(|| "nope".to_string())).is_err());
    assert!(Config::from_lookup(|k| (k == "FORGE_API_REQUEST_TIMEOUT_SECS").then(|| "0".to_string())).is_err());
    assert!(Config::from_lookup(|k| (k == "FORGE_API_BODY_LIMIT_BYTES").then(|| "-1".to_string())).is_err());
}
