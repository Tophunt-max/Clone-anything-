use axum::http::{Method, StatusCode, header};
use serde_json::json;

use super::*;
use crate::problem;

#[tokio::test]
async fn health_ready_meta_and_security_headers() {
    let Some(app) = app().await else { return };
    let r = app.get("/healthz", &As::Nobody).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(uuid::Uuid::parse_str(r.headers.get("x-request-id").unwrap().to_str().unwrap()).is_ok());
    for (h, v) in [("x-content-type-options", "nosniff"), ("x-frame-options", "DENY"), ("cache-control", "no-store")] {
        assert_eq!(r.headers.get(h).unwrap(), v);
    }
    let r = app.get("/readyz", &As::Nobody).await;
    assert_eq!((r.status, r.body["status"].as_str()), (StatusCode::OK, Some("ready")));
    let r = app.get("/api/v1/meta", &As::Nobody).await;
    assert_eq!(r.body["name"], crate::PRODUCT_NAME);
    assert_eq!(r.body["features"], json!(["auth", "workspaces", "projects"]));
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let Some(app) = app().await else { return };
    crate::MIGRATOR.run(&app.db).await.unwrap();
    let tables: Vec<String> =
        sqlx::query_scalar("select tablename::text from pg_tables where schemaname = 'public' order by 1").fetch_all(&app.db).await.unwrap();
    for t in ["audit_logs", "devices", "login_codes", "projects", "refresh_tokens", "sessions", "users", "workspace_members", "workspaces"] {
        assert!(tables.iter().any(|x| x == t), "{t} missing: {tables:?}");
    }
}

#[tokio::test]
async fn readyz_says_not_ready_when_a_migration_is_missing() {
    let Some(app) = app().await else { return };
    sqlx::query("delete from _sqlx_migrations").execute(&app.db).await.unwrap();
    let r = app.get("/readyz", &As::Nobody).await;
    assert_eq!((r.status, r.code()), (StatusCode::SERVICE_UNAVAILABLE, "not_ready"));
}

#[tokio::test]
async fn errors_are_problem_json_with_the_request_id() {
    let Some(app) = app().await else { return };
    let r = app.get("/api/v1/nope", &As::Nobody).await;
    assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"));
    assert_eq!(r.headers.get(header::CONTENT_TYPE).unwrap(), problem::CONTENT_TYPE);
    assert_eq!(r.body["request_id"].as_str(), r.headers.get("x-request-id").and_then(|v| v.to_str().ok()));

    // Unknown fields, wrong types and a missing content type are problems too, not plain text.
    let r = app.post("/api/v1/auth/login", &As::Nobody, json!({"email": "a@b.co", "password": "x", "admin": true})).await;
    assert_eq!((r.status, r.code()), (StatusCode::UNPROCESSABLE_ENTITY, "bad_input"));
    let mut req = axum::http::Request::post("/api/v1/auth/login").body(axum::body::Body::from("{}")).unwrap();
    req.headers_mut().remove(header::CONTENT_TYPE);
    let resp = tower::ServiceExt::oneshot(app.router.clone(), req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    // A malformed ID in a path is a 404, like any ID the caller can't see.
    let (me, _, _) = app.android_user("ids@example.com").await;
    let r = app.get("/api/v1/projects/not-a-uuid", &me).await;
    assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"));
    let r = app.get("/api/v1/workspaces?limit=0", &me).await;
    assert_eq!((r.status, r.body["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("limit")));
    let r = app.get("/api/v1/workspaces?bogus=1", &me).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn oversized_bodies_are_refused() {
    let Some(app) = app().await else { return };
    let big = "x".repeat(2 * 1024 * 1024);
    let r = app.call(Method::POST, "/api/v1/auth/login", &As::Nobody, Some(json!({"email": "a@b.co", "password": big}))).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);
}
