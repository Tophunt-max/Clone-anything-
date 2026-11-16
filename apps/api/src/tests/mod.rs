//! HTTP tests against a real PostgreSQL. Each test gets its own fresh database.
//!
//! Set `FORGE_TEST_DATABASE_URL` (a server where the user may create databases) to run the
//! database tests; without it they are skipped with a note, so `cargo test --workspace` works on
//! machines with no database. CI sets `FORGE_REQUIRE_DB_TESTS=1`, which turns a skip into a failure.

mod auth;
mod basics;
mod no_db;
mod tenancy;

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool};
use tower::ServiceExt;

use crate::AppState;
use crate::auth::tokens::TokenKey;
use crate::limits::{Limit, Limits};

pub struct TestApp {
    pub router: Router,
    pub db: PgPool,
}

fn generous() -> Limits {
    let l = Limit { max: 10_000, per: Duration::from_secs(60) };
    Limits { auth_per_ip: l, login_per_email: l, refresh_per_ip: l }
}

pub async fn app() -> Option<TestApp> {
    app_with(generous()).await
}

pub async fn app_with(limits: Limits) -> Option<TestApp> {
    let Ok(url) = std::env::var("FORGE_TEST_DATABASE_URL") else {
        assert!(std::env::var_os("FORGE_REQUIRE_DB_TESTS").is_none(), "FORGE_REQUIRE_DB_TESTS is set but FORGE_TEST_DATABASE_URL is not");
        eprintln!("skipped: set FORGE_TEST_DATABASE_URL to run database tests");
        return None;
    };
    let admin_opts: PgConnectOptions = url.parse().unwrap();
    let mut admin = admin_opts.connect().await.unwrap();
    let name = format!("forge_test_{}", uuid::Uuid::now_v7().simple());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("create database {name}"))).execute(&mut admin).await.unwrap();
    let db = PgPoolOptions::new().max_connections(5).connect_with(admin_opts.database(&name)).await.unwrap();
    crate::MIGRATOR.run(&db).await.unwrap();
    let state = AppState {
        db: db.clone(),
        token_key: TokenKey::new(vec![9; 32]).unwrap(),
        request_timeout: Duration::from_secs(20),
        cookie_secure: true,
        trust_proxy: false,
        limits,
        limiter: Default::default(),
    };
    Some(TestApp { router: crate::router(state, 64 * 1024), db })
}

/// How a request authenticates.
#[derive(Clone, Debug)]
pub enum As {
    Nobody,
    Bearer(String),
    /// A browser: the cookies it holds and the CSRF token it echoes (`None` = forgets to).
    Browser {
        cookies: String,
        csrf: Option<String>,
    },
}

pub struct Resp {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
}

impl Resp {
    pub fn code(&self) -> &str {
        self.body["code"].as_str().unwrap_or_default()
    }

    /// `name=value` pairs from every Set-Cookie header, joined like a browser's Cookie header.
    pub fn cookies(&self) -> String {
        self.headers.get_all(header::SET_COOKIE).iter().filter_map(|v| v.to_str().ok()).filter_map(|c| c.split(';').next()).collect::<Vec<_>>().join("; ")
    }
}

impl TestApp {
    pub async fn call(&self, method: Method, path: &str, who: &As, body: Option<Value>) -> Resp {
        let mut req = Request::builder().method(method).uri(path);
        match who {
            As::Nobody => {}
            As::Bearer(t) => req = req.header(header::AUTHORIZATION, format!("Bearer {t}")),
            As::Browser { cookies, csrf } => {
                req = req.header(header::COOKIE, cookies);
                if let Some(c) = csrf {
                    req = req.header("x-csrf-token", c);
                }
            }
        }
        let req = match body {
            Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
        };
        Resp { status, headers, body }
    }

    pub async fn get(&self, path: &str, who: &As) -> Resp {
        self.call(Method::GET, path, who, None).await
    }

    pub async fn post(&self, path: &str, who: &As, body: Value) -> Resp {
        self.call(Method::POST, path, who, Some(body)).await
    }

    /// Registers through the Android flow and returns (Bearer identity, user id, personal workspace id).
    pub async fn android_user(&self, email: &str) -> (As, String, String) {
        let r = self
            .post(
                "/api/v1/auth/register",
                &As::Nobody,
                json!({"email": email, "password": "a long enough passphrase", "display_name": "Tester", "client": "android"}),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        let who = As::Bearer(r.body["access_token"].as_str().unwrap().to_string());
        let user = r.body["user"]["id"].as_str().unwrap().to_string();
        let ws = self.get("/api/v1/workspaces", &who).await.body["items"][0]["id"].as_str().unwrap().to_string();
        (who, user, ws)
    }

    pub async fn project(&self, who: &As, ws: &str, name: &str) -> String {
        let r = self.post(&format!("/api/v1/workspaces/{ws}/projects"), who, json!({"name": name, "kind": "code"})).await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        r.body["id"].as_str().unwrap().to_string()
    }
}
