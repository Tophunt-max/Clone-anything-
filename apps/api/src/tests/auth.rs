use std::time::Duration;

use axum::http::{Method, StatusCode, header};
use serde_json::json;

use super::*;
use crate::limits::{Limit, Limits};

const PW: &str = "a long enough passphrase";

fn register_body(email: &str, client: &str) -> serde_json::Value {
    json!({"email": email, "password": PW, "display_name": "Ada", "client": client})
}

#[tokio::test]
async fn web_register_uses_httponly_cookies_and_never_returns_tokens() {
    let Some(app) = app().await else { return };
    let r = app.post("/api/v1/auth/register", &As::Nobody, register_body("Ada@Example.com", "web")).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["user"]["email"], "ada@example.com");
    assert!(r.body.get("access_token").is_none() && r.body.get("refresh_token").is_none());
    let csrf = r.body["csrf_token"].as_str().unwrap().to_string();

    let set: Vec<String> = r.headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_string()).collect();
    let find = |name: &str| set.iter().find(|c| c.starts_with(&format!("{name}="))).cloned().unwrap();
    for name in ["forge_at", "forge_rt"] {
        let c = find(name);
        assert!(c.contains("HttpOnly") && c.contains("Secure") && c.contains("SameSite=Strict"), "{c}");
    }
    assert!(find("forge_rt").contains("Path=/api/v1/auth"), "the refresh cookie only goes to the auth endpoints");
    assert!(!find("forge_csrf").contains("HttpOnly"), "the page must be able to read the CSRF cookie");

    // Reads work with the cookie alone.
    let browser = As::Browser { cookies: r.cookies(), csrf: None };
    let me = app.get("/api/v1/me", &browser).await;
    assert_eq!((me.status, me.body["display_name"].as_str()), (StatusCode::OK, Some("Ada")));

    // Writes need the CSRF header: a cross-site form post has the cookies but not the header.
    let forged = app.post("/api/v1/workspaces", &browser, json!({"name": "X", "slug": "xxx"})).await;
    assert_eq!((forged.status, forged.code()), (StatusCode::FORBIDDEN, "csrf_failed"));
    let wrong = As::Browser { cookies: r.cookies(), csrf: Some("wrong".into()) };
    assert_eq!(app.post("/api/v1/workspaces", &wrong, json!({"name": "X", "slug": "xxx"})).await.status, StatusCode::FORBIDDEN);
    let good = As::Browser { cookies: r.cookies(), csrf: Some(csrf) };
    assert_eq!(app.post("/api/v1/workspaces", &good, json!({"name": "X", "slug": "xxx"})).await.status, StatusCode::CREATED);
}

#[tokio::test]
async fn registration_validates_and_refuses_duplicates() {
    let Some(app) = app().await else { return };
    for (field, body) in [
        ("email", json!({"email": "nope", "password": PW, "display_name": "A"})),
        ("password", json!({"email": "a@example.com", "password": "short", "display_name": "A"})),
        ("display_name", json!({"email": "a@example.com", "password": PW, "display_name": " "})),
    ] {
        let r = app.post("/api/v1/auth/register", &As::Nobody, body).await;
        assert_eq!((r.status, r.body["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some(field)));
    }
    assert_eq!(app.post("/api/v1/auth/register", &As::Nobody, register_body("dup@example.com", "web")).await.status, StatusCode::CREATED);
    let r = app.post("/api/v1/auth/register", &As::Nobody, register_body("DUP@example.com", "web")).await;
    assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "email_taken"));
}

#[tokio::test]
async fn passwords_and_refresh_tokens_are_stored_only_as_hashes() {
    let Some(app) = app().await else { return };
    app.android_user("hash@example.com").await;
    let stored: String = sqlx::query_scalar("select password_hash from users").fetch_one(&app.db).await.unwrap();
    assert!(stored.starts_with("$argon2id$") && !stored.contains(PW));
    let tokens: Vec<Vec<u8>> = sqlx::query_scalar("select token_hash from refresh_tokens").fetch_all(&app.db).await.unwrap();
    assert!(!tokens.is_empty() && tokens.iter().all(|t| t.len() == 32), "refresh tokens are kept only as SHA-256");
}

#[tokio::test]
async fn login_fails_the_same_way_for_wrong_passwords_and_unknown_emails() {
    let Some(app) = app().await else { return };
    app.android_user("login@example.com").await;
    let ok = app.post("/api/v1/auth/login", &As::Nobody, json!({"email": "LOGIN@example.com", "password": PW, "client": "android"})).await;
    assert_eq!(ok.status, StatusCode::OK);
    assert!(ok.body["access_token"].is_string());
    let wrong = app.post("/api/v1/auth/login", &As::Nobody, json!({"email": "login@example.com", "password": "not the passphrase"})).await;
    let unknown = app.post("/api/v1/auth/login", &As::Nobody, json!({"email": "nobody@example.com", "password": "not the passphrase"})).await;
    assert_eq!((wrong.status, wrong.code()), (StatusCode::UNAUTHORIZED, "invalid_credentials"));
    assert_eq!((unknown.status, unknown.code()), (wrong.status, wrong.code()));
    assert_eq!(unknown.body["detail"], wrong.body["detail"]);
    let failures: i64 = sqlx::query_scalar("select count(*) from audit_logs where action = 'auth.login_failed'").fetch_one(&app.db).await.unwrap();
    assert_eq!(failures, 2);
}

#[tokio::test]
async fn locked_accounts_cannot_sign_in_or_use_old_tokens() {
    let Some(app) = app().await else { return };
    let (who, _, _) = app.android_user("locked@example.com").await;
    sqlx::query("update users set status = 'locked'").execute(&app.db).await.unwrap();
    let r = app.post("/api/v1/auth/login", &As::Nobody, json!({"email": "locked@example.com", "password": PW})).await;
    assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "account_locked"));
    assert_eq!(app.get("/api/v1/me", &who).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_rotates_and_a_reused_token_ends_the_session() {
    let Some(app) = app().await else { return };
    let r = app.post("/api/v1/auth/register", &As::Nobody, register_body("rot@example.com", "android")).await;
    let first = r.body["refresh_token"].as_str().unwrap().to_string();

    let r2 = app.post("/api/v1/auth/refresh", &As::Nobody, json!({"refresh_token": first})).await;
    assert_eq!(r2.status, StatusCode::OK, "{:?}", r2.body);
    let second = r2.body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(first, second);
    let live = As::Bearer(r2.body["access_token"].as_str().unwrap().to_string());
    assert_eq!(app.get("/api/v1/me", &live).await.status, StatusCode::OK);

    // Someone replays the first token: the session ends for everyone holding it.
    let replay = app.post("/api/v1/auth/refresh", &As::Nobody, json!({"refresh_token": first})).await;
    assert_eq!((replay.status, replay.code()), (StatusCode::UNAUTHORIZED, "refresh_reused"));
    assert_eq!(app.get("/api/v1/me", &live).await.status, StatusCode::UNAUTHORIZED, "the access token dies with its session");
    let after = app.post("/api/v1/auth/refresh", &As::Nobody, json!({"refresh_token": second})).await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED, "the newest token is dead too");
    let reason: String = sqlx::query_scalar("select revoke_reason from sessions").fetch_one(&app.db).await.unwrap();
    assert_eq!(reason, "refresh_reuse");

    for junk in ["", "fr1.nope", "something else"] {
        let r = app.post("/api/v1/auth/refresh", &As::Nobody, json!({"refresh_token": junk})).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{junk:?}");
    }
}

#[tokio::test]
async fn a_browser_refresh_uses_the_cookie_and_needs_csrf() {
    let Some(app) = app().await else { return };
    let r = app.post("/api/v1/auth/register", &As::Nobody, register_body("webrot@example.com", "web")).await;
    let csrf = r.body["csrf_token"].as_str().unwrap().to_string();
    let no_csrf = app.call(Method::POST, "/api/v1/auth/refresh", &As::Browser { cookies: r.cookies(), csrf: None }, None).await;
    assert_eq!((no_csrf.status, no_csrf.code()), (StatusCode::FORBIDDEN, "csrf_failed"));
    let ok = app.call(Method::POST, "/api/v1/auth/refresh", &As::Browser { cookies: r.cookies(), csrf: Some(csrf) }, None).await;
    assert_eq!(ok.status, StatusCode::OK, "{:?}", ok.body);
    assert!(ok.body.get("refresh_token").is_none());
    assert!(ok.cookies().contains("forge_rt="), "a new refresh cookie is set");
}

#[tokio::test]
async fn logout_and_session_revocation_take_effect_at_once() {
    let Some(app) = app().await else { return };
    let (phone, _, _) = app.android_user("multi@example.com").await;
    let laptop =
        app.post("/api/v1/auth/login", &As::Nobody, json!({"email": "multi@example.com", "password": PW, "client": "android", "device_name": "Laptop"})).await;
    let laptop = As::Bearer(laptop.body["access_token"].as_str().unwrap().to_string());

    let list = app.get("/api/v1/me/sessions", &phone).await;
    let sessions = list.body.as_array().unwrap().clone();
    assert_eq!(sessions.len(), 2);
    let other = sessions.iter().find(|s| s["current"] == false).unwrap();
    assert_eq!(other["device_name"], "Laptop");

    let r = app.call(Method::DELETE, &format!("/api/v1/me/sessions/{}", other["id"].as_str().unwrap()), &phone, None).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/v1/me", &laptop).await.status, StatusCode::UNAUTHORIZED);

    let r = app.call(Method::POST, "/api/v1/auth/logout", &phone, None).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(r.headers.get_all(header::SET_COOKIE).iter().any(|c| c.to_str().unwrap().contains("Max-Age=0")));
    assert_eq!(app.get("/api/v1/me", &phone).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sessions_of_other_users_cannot_be_revoked() {
    let Some(app) = app().await else { return };
    let (a, _, _) = app.android_user("sa@example.com").await;
    let (b, _, _) = app.android_user("sb@example.com").await;
    let b_session = app.get("/api/v1/me/sessions", &b).await.body[0]["id"].as_str().unwrap().to_string();
    let r = app.call(Method::DELETE, &format!("/api/v1/me/sessions/{b_session}"), &a, None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get("/api/v1/me", &b).await.status, StatusCode::OK);
}

#[tokio::test]
async fn forged_or_missing_tokens_are_refused() {
    let Some(app) = app().await else { return };
    let (who, _, _) = app.android_user("forge@example.com").await;
    let As::Bearer(t) = &who else { panic!("android users sign in with Bearer tokens") };
    let other_key = crate::auth::tokens::TokenKey::new(vec![1; 32]).unwrap();
    let claims = crate::auth::tokens::verify_access(&crate::auth::tokens::TokenKey::new(vec![9; 32]).unwrap(), t, 0).unwrap();
    let forged = crate::auth::tokens::issue_access(&other_key, &claims).unwrap();
    for bad in [As::Nobody, As::Bearer(forged), As::Bearer("garbage".into())] {
        let r = app.get("/api/v1/me", &bad).await;
        assert_eq!((r.status, r.code()), (StatusCode::UNAUTHORIZED, "unauthenticated"), "{bad:?}");
    }
}

#[tokio::test]
async fn login_attempts_are_rate_limited() {
    let tight = Limit { max: 3, per: Duration::from_secs(60) };
    let roomy = Limit { max: 1000, per: Duration::from_secs(60) };
    let Some(app) = app_with(Limits { auth_per_ip: roomy, login_per_email: tight, refresh_per_ip: roomy }).await else { return };
    let body = json!({"email": "limited@example.com", "password": "wrong password!!"});
    for _ in 0..3 {
        assert_eq!(app.post("/api/v1/auth/login", &As::Nobody, body.clone()).await.status, StatusCode::UNAUTHORIZED);
    }
    let r = app.post("/api/v1/auth/login", &As::Nobody, body).await;
    assert_eq!((r.status, r.code()), (StatusCode::TOO_MANY_REQUESTS, "rate_limited"));
    assert!(r.headers.get(header::RETRY_AFTER).is_some());
}

#[tokio::test]
async fn auth_events_are_audited_with_request_ids() {
    let Some(app) = app().await else { return };
    app.android_user("audit@example.com").await;
    app.post("/api/v1/auth/login", &As::Nobody, json!({"email": "audit@example.com", "password": PW})).await;
    let rows: Vec<(String, Option<uuid::Uuid>, Option<String>)> =
        sqlx::query_as("select action, request_id, ip_hash from audit_logs order by id").fetch_all(&app.db).await.unwrap();
    let actions: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(actions, ["user.register", "auth.login"]);
    assert!(rows.iter().all(|r| r.1.is_some() && r.2.as_deref().is_some_and(|h| h.len() == 32)));
    let details: Vec<serde_json::Value> = sqlx::query_scalar("select details from audit_logs").fetch_all(&app.db).await.unwrap();
    assert!(details.iter().all(|d| !d.to_string().contains(PW)), "no secrets in the audit log");
}
