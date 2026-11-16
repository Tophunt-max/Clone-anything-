//! The Android app's WebView sign-in: a single-use code from the native session, exchanged by a
//! form POST for browser cookies and a redirect.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use tower::ServiceExt;

use super::*;
use crate::routes::auth::safe_next;

impl TestApp {
    async fn code(&self, who: &As) -> String {
        let r = self.post("/api/v1/auth/login-code", who, json!({})).await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
        assert_eq!(r.body["expires_in_secs"], 60);
        r.body["code"].as_str().unwrap().to_string()
    }

    async fn exchange(&self, form: &str, fetch_site: Option<&str>) -> Resp {
        let mut req = Request::post("/api/v1/auth/exchange").header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        if let Some(s) = fetch_site {
            req = req.header("sec-fetch-site", s);
        }
        let resp = self.router.clone().oneshot(req.body(Body::from(form.to_string())).unwrap()).await.unwrap();
        let (status, headers) = (resp.status(), resp.headers().clone());
        let bytes = http_body_util::BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
        Resp { status, headers, body: serde_json::from_slice(&bytes).unwrap_or_default() }
    }
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

#[tokio::test]
async fn a_code_opens_a_browser_session_once_and_redirects_inside_the_site() {
    let Some(app) = app().await else { return };
    let (phone, _, _) = app.android_user("webview@example.com").await;
    let code = app.code(&phone).await;
    let r = app.exchange(&format!("code={}&next={}", enc(&code), enc("/p/123?tab=files")), Some("none")).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.body);
    assert_eq!(r.headers.get(header::LOCATION).unwrap(), "/p/123?tab=files");
    let cookies = r.cookies();
    assert!(cookies.contains("forge_at=") && cookies.contains("forge_rt=") && cookies.contains("forge_csrf="));
    let set: Vec<String> = r.headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_string()).collect();
    assert!(set.iter().filter(|c| c.starts_with("forge_at=") || c.starts_with("forge_rt=")).all(|c| c.contains("HttpOnly")));

    // The cookies work like a browser's.
    let browser = As::Browser { cookies, csrf: None };
    assert_eq!(app.get("/api/v1/me", &browser).await.status, StatusCode::OK);
    let sessions = app.get("/api/v1/me/sessions", &phone).await;
    assert!(sessions.body.as_array().unwrap().iter().any(|s| s["device_name"] == "Android app (web view)"));

    // Used once.
    let again = app.exchange(&format!("code={}", enc(&code)), None).await;
    assert_eq!((again.status, again.code()), (StatusCode::UNAUTHORIZED, "invalid_code"));
    let stored: Vec<Vec<u8>> = sqlx::query_scalar("select code_hash from login_codes").fetch_all(&app.db).await.unwrap();
    assert!(stored.iter().all(|h| h.len() == 32 && h.as_slice() != code.as_bytes()), "only hashes are stored");
}

#[tokio::test]
async fn expired_garbage_cross_site_and_revoked_codes_are_refused() {
    let Some(app) = app().await else { return };
    let (phone, _, _) = app.android_user("refuse@example.com").await;

    let cross = app.code(&phone).await;
    let r = app.exchange(&format!("code={}", enc(&cross)), Some("cross-site")).await;
    assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "cross_site"));

    for junk in ["code=", "code=flc1.nope", "code=fr1.abc", "nothing=1"] {
        let r = app.exchange(junk, None).await;
        assert!(matches!(r.status, StatusCode::UNAUTHORIZED | StatusCode::UNPROCESSABLE_ENTITY), "{junk}: {}", r.status);
        assert!(r.headers.get(header::SET_COOKIE).is_none());
    }

    let old = app.code(&phone).await;
    sqlx::query("update login_codes set expires_at = now() - interval '1 second'").execute(&app.db).await.unwrap();
    assert_eq!(app.exchange(&format!("code={}", enc(&old)), None).await.code(), "invalid_code");

    // A code dies with the session it came from.
    let orphan = app.code(&phone).await;
    app.call(axum::http::Method::POST, "/api/v1/auth/logout", &phone, None).await;
    assert_eq!(app.exchange(&format!("code={}", enc(&orphan)), None).await.code(), "invalid_code");
}

#[tokio::test]
async fn codes_need_a_signed_in_caller() {
    let Some(app) = app().await else { return };
    let r = app.post("/api/v1/auth/login-code", &As::Nobody, json!({})).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[test]
fn next_stays_on_this_site() {
    assert_eq!(safe_next(Some("/p/1")), "/p/1");
    for bad in [None, Some(""), Some("https://evil.example"), Some("//evil.example"), Some("/\\evil.example"), Some("/a\r\nb")] {
        assert_eq!(safe_next(bad), "/", "{bad:?}");
    }
}
