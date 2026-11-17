//! `/api/v1/auth/*`: register, login, refresh, logout.
//!
//! `client: "web"` (default) answers with HttpOnly cookies and a CSRF token; the body never holds
//! a session token. `client: "android"` answers with the tokens in the body for the app's
//! encrypted store, and the app sends `Authorization: Bearer`.

use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::AppState;
use crate::audit::{self, Entry};
use crate::auth::tokens::{self, ACCESS_TTL_SECS, Claims, SESSION_TTL_DAYS};
use crate::auth::{AuthUser, check_csrf, cookies, password};
use crate::client::ClientMeta;
use crate::error::{ApiError, is_unique_violation};
use crate::extract::Body;
use crate::validate;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClientKind {
    #[default]
    Web,
    Android,
}

impl ClientKind {
    fn as_str(self) -> &'static str {
        match self {
            ClientKind::Web => "web",
            ClientKind::Android => "android",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterBody {
    email: String,
    password: String,
    display_name: String,
    #[serde(default)]
    client: ClientKind,
    device_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginBody {
    email: String,
    password: String,
    #[serde(default)]
    client: ClientKind,
    device_name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RefreshBody {
    /// Android only; browsers send the refresh cookie.
    refresh_token: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
pub struct UserOut {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SessionOut {
    pub user: UserOut,
    pub session_id: Uuid,
    pub access_expires_at: DateTime<Utc>,
    /// Web: echo this in `X-CSRF-Token` on state-changing requests (it is also in a cookie).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
    /// Android only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// Android only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

struct Issued {
    session_id: Uuid,
    access: String,
    access_exp: DateTime<Utc>,
    refresh: String,
}

fn issue_access(state: &AppState, user_id: Uuid, session_id: Uuid) -> Result<(String, DateTime<Utc>), ApiError> {
    let exp = Utc::now() + Duration::seconds(ACCESS_TTL_SECS);
    let token = tokens::issue_access(&state.token_key, &Claims { sub: user_id, sid: session_id, exp: exp.timestamp() }).map_err(ApiError::internal)?;
    Ok((token, exp))
}

async fn start_session(conn: &mut PgConnection, state: &AppState, user_id: Uuid, client: ClientKind, device_name: Option<&str>) -> Result<Issued, ApiError> {
    let device_id = Uuid::now_v7();
    let default_name = match client {
        ClientKind::Web => "Web browser",
        ClientKind::Android => "Android device",
    };
    let name = match device_name {
        Some(n) => validate::name("device_name", n)?,
        None => default_name.to_string(),
    };
    sqlx::query("insert into devices (id, user_id, kind, name) values ($1, $2, $3, $4)")
        .bind(device_id)
        .bind(user_id)
        .bind(client.as_str())
        .bind(&name)
        .execute(&mut *conn)
        .await?;
    let session_id = Uuid::now_v7();
    let expires = Utc::now() + Duration::days(SESSION_TTL_DAYS);
    sqlx::query("insert into sessions (id, user_id, device_id, expires_at) values ($1, $2, $3, $4)")
        .bind(session_id)
        .bind(user_id)
        .bind(device_id)
        .bind(expires)
        .execute(&mut *conn)
        .await?;
    let (refresh, hash) = tokens::new_refresh().map_err(ApiError::internal)?;
    sqlx::query("insert into refresh_tokens (token_hash, session_id) values ($1, $2)").bind(hash).bind(session_id).execute(&mut *conn).await?;
    let (access, access_exp) = issue_access(state, user_id, session_id)?;
    Ok(Issued { session_id, access, access_exp, refresh })
}

fn session_response(state: &AppState, status: StatusCode, client: ClientKind, user: UserOut, issued: Issued) -> Result<Response, ApiError> {
    let mut out =
        SessionOut { user, session_id: issued.session_id, access_expires_at: issued.access_exp, csrf_token: None, access_token: None, refresh_token: None };
    let mut headers = HeaderMap::new();
    match client {
        ClientKind::Android => {
            out.access_token = Some(issued.access);
            out.refresh_token = Some(issued.refresh);
        }
        ClientKind::Web => {
            out.csrf_token = Some(web_cookies(state, &issued, &mut headers)?);
        }
    }
    Ok((status, headers, Json(out)).into_response())
}

/// Sets the three browser cookies for a new session; returns the CSRF token.
fn web_cookies(state: &AppState, issued: &Issued, headers: &mut HeaderMap) -> Result<String, ApiError> {
    let csrf = tokens::new_csrf().map_err(ApiError::internal)?;
    let secure = state.cookie_secure;
    let session_secs = SESSION_TTL_DAYS * 86_400;
    for c in [
        cookies::set(cookies::ACCESS, &issued.access, cookies::ACCESS_PATH, ACCESS_TTL_SECS, true, secure),
        cookies::set(cookies::REFRESH, &issued.refresh, cookies::REFRESH_PATH, session_secs, true, secure),
        cookies::set(cookies::CSRF, &csrf, cookies::CSRF_PATH, session_secs, false, secure),
    ]
    .into_iter()
    .flatten()
    {
        headers.append(SET_COOKIE, c);
    }
    Ok(csrf)
}

async fn limit(state: &AppState, key: String, l: crate::limits::Limit) -> Result<(), ApiError> {
    state.limiter.hit(&key, l).await.map_err(|retry_after_secs| ApiError::RateLimited { retry_after_secs })
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f).await.map_err(ApiError::internal)
}

pub async fn register(State(state): State<AppState>, meta: ClientMeta, Body(body): Body<RegisterBody>) -> Result<Response, ApiError> {
    limit(&state, format!("auth:ip:{}", meta.ip_hash), state.limits.auth_per_ip).await?;
    let email = validate::email(&body.email)?;
    let display_name = validate::name("display_name", &body.display_name)?;
    if let Some(p) = password::policy_problem(&body.password) {
        return Err(ApiError::invalid("password", &p));
    }
    let pw = body.password;
    let hash = blocking(move || password::hash(&pw)).await?.map_err(ApiError::internal)?;

    let mut tx = state.db.begin().await?;
    let user_id = Uuid::now_v7();
    let inserted = sqlx::query_as::<_, UserOut>(
        "insert into users (id, email, display_name, password_hash) values ($1, $2, $3, $4) returning id, email, display_name, created_at",
    )
    .bind(user_id)
    .bind(&email)
    .bind(&display_name)
    .bind(&hash)
    .fetch_one(&mut *tx)
    .await;
    let user = match inserted {
        Ok(u) => u,
        Err(e) if is_unique_violation(&e) => return Err(ApiError::conflict("email_taken", "An account already uses this email. Sign in instead.")),
        Err(e) => return Err(e.into()),
    };
    // Everyone starts with a personal workspace they own.
    let ws_id = Uuid::now_v7();
    let simple = ws_id.simple().to_string();
    let slug = format!("personal-{}", simple.get(20..).unwrap_or(&simple));
    sqlx::query("insert into workspaces (id, name, slug, created_by) values ($1, $2, $3, $4)")
        .bind(ws_id)
        .bind(format!("{display_name}'s workspace"))
        .bind(&slug)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("insert into workspace_members (workspace_id, user_id, role) values ($1, $2, 'owner')").bind(ws_id).bind(user_id).execute(&mut *tx).await?;
    let issued = start_session(&mut tx, &state, user_id, body.client, body.device_name.as_deref()).await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: None,
            actor_user_id: Some(user_id),
            action: "user.register",
            target_kind: "user",
            target_id: Some(user_id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"client": body.client.as_str()}),
        },
    )
    .await?;
    tx.commit().await?;
    session_response(&state, StatusCode::CREATED, body.client, user, issued)
}

#[derive(sqlx::FromRow)]
struct LoginRow {
    id: Uuid,
    email: String,
    display_name: String,
    created_at: DateTime<Utc>,
    password_hash: String,
    status: String,
}

pub async fn login(State(state): State<AppState>, meta: ClientMeta, Body(body): Body<LoginBody>) -> Result<Response, ApiError> {
    limit(&state, format!("auth:ip:{}", meta.ip_hash), state.limits.auth_per_ip).await?;
    let email = validate::email(&body.email)?;
    limit(&state, format!("login:email:{}", tokens::keyed_hash(&state.token_key, &email)), state.limits.login_per_email).await?;
    if body.password.chars().count() > password::MAX_LEN {
        return Err(ApiError::unauthorized("invalid_credentials", "The email or password is wrong."));
    }
    let row = sqlx::query_as::<_, LoginRow>("select id, email, display_name, created_at, password_hash, status from users where lower(email) = $1")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?;
    let stored = row.as_ref().map(|r| r.password_hash.clone()).unwrap_or_else(|| password::dummy_hash().to_string());
    let pw = body.password;
    let ok = blocking(move || password::verify(&pw, &stored)).await?;
    let row = match row {
        Some(r) if ok => r,
        other => {
            let mut conn = state.db.acquire().await?;
            audit::record(
                &mut conn,
                Entry {
                    workspace_id: None,
                    actor_user_id: other.map(|r| r.id),
                    action: "auth.login_failed",
                    target_kind: "user",
                    target_id: None,
                    ip_hash: Some(&meta.ip_hash),
                    details: json!({}),
                },
            )
            .await?;
            return Err(ApiError::unauthorized("invalid_credentials", "The email or password is wrong."));
        }
    };
    if row.status != "active" {
        return Err(ApiError::forbidden("account_locked", "This account is locked. Contact the workspace administrator."));
    }
    let mut tx = state.db.begin().await?;
    let issued = start_session(&mut tx, &state, row.id, body.client, body.device_name.as_deref()).await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: None,
            actor_user_id: Some(row.id),
            action: "auth.login",
            target_kind: "session",
            target_id: Some(issued.session_id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"client": body.client.as_str()}),
        },
    )
    .await?;
    tx.commit().await?;
    let user = UserOut { id: row.id, email: row.email, display_name: row.display_name, created_at: row.created_at };
    session_response(&state, StatusCode::OK, body.client, user, issued)
}

#[derive(sqlx::FromRow)]
struct TokenRow {
    session_id: Uuid,
    used: bool,
    /// Used within the grace window: most likely the same client retrying a refresh whose answer
    /// it never received (a tab closed or navigated mid-request).
    recently_used: bool,
}

/// How long a just-used refresh token may be presented again without ending the session.
pub const REUSE_GRACE_SECS: f64 = 30.0;

pub async fn refresh(State(state): State<AppState>, meta: ClientMeta, headers: HeaderMap, body: axum::body::Bytes) -> Result<Response, ApiError> {
    limit(&state, format!("refresh:ip:{}", meta.ip_hash), state.limits.refresh_per_ip).await?;
    // Browsers send no body (the token is in a cookie); the app sends `{"refresh_token": …}`.
    let from_body = if body.iter().all(u8::is_ascii_whitespace) {
        None
    } else {
        serde_json::from_slice::<RefreshBody>(&body)
            .map_err(|_| ApiError::invalid("refresh_token", "Send {\"refresh_token\": \"…\"} or no body."))?
            .refresh_token
    };
    let (token, client) = match from_body {
        Some(t) => (t, ClientKind::Android),
        None => {
            // A cookie refresh changes state, so it needs the CSRF header like any other.
            let t = cookies::get(&headers, cookies::REFRESH).ok_or_else(ApiError::unauthenticated)?.to_string();
            check_csrf(&headers)?;
            (t, ClientKind::Web)
        }
    };
    let invalid = || ApiError::unauthorized("invalid_refresh", "Your session has ended. Sign in again.");
    if !tokens::looks_like_refresh(&token) {
        return Err(invalid());
    }
    let hash = tokens::hash_refresh(&token);
    let mut tx = state.db.begin().await?;
    // Lock the token row: of two refreshes racing with the same token, exactly one wins.
    let row = sqlx::query_as::<_, TokenRow>(
        "select session_id, used_at is not null as used, coalesce(used_at > now() - make_interval(secs => $2), false) as recently_used
         from refresh_tokens where token_hash = $1 for update",
    )
    .bind(&hash)
    .bind(REUSE_GRACE_SECS)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        return Err(invalid());
    };
    if row.used && !row.recently_used {
        // A used token came back: it was copied. End the whole session, for the thief and the owner.
        sqlx::query("update sessions set revoked_at = now(), revoke_reason = 'refresh_reuse' where id = $1 and revoked_at is null")
            .bind(row.session_id)
            .execute(&mut *tx)
            .await?;
        let user: Option<Uuid> = sqlx::query_scalar("select user_id from sessions where id = $1").bind(row.session_id).fetch_optional(&mut *tx).await?;
        audit::record(
            &mut tx,
            Entry {
                workspace_id: None,
                actor_user_id: user,
                action: "auth.refresh_reuse",
                target_kind: "session",
                target_id: Some(row.session_id),
                ip_hash: Some(&meta.ip_hash),
                details: json!({}),
            },
        )
        .await?;
        tx.commit().await?;
        return Err(ApiError::unauthorized("refresh_reused", "This sign-in was used from somewhere else, so it was ended for safety. Sign in again."));
    }
    let user = sqlx::query_as::<_, UserOut>(
        "select u.id, u.email, u.display_name, u.created_at from sessions s join users u on u.id = s.user_id
         where s.id = $1 and s.revoked_at is null and s.expires_at > now() and u.status = 'active'",
    )
    .bind(row.session_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(invalid)?;
    if row.used {
        // Grace retry: the token handed out last time (whose answer was lost) is superseded.
        // Marking it used long ago means whoever presents it later ends the session.
        sqlx::query("update refresh_tokens set used_at = now() - interval '1 day' where session_id = $1 and used_at is null")
            .bind(row.session_id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("update refresh_tokens set used_at = now() where token_hash = $1").bind(&hash).execute(&mut *tx).await?;
    }
    let (new_refresh, new_hash) = tokens::new_refresh().map_err(ApiError::internal)?;
    sqlx::query("insert into refresh_tokens (token_hash, session_id) values ($1, $2)").bind(new_hash).bind(row.session_id).execute(&mut *tx).await?;
    sqlx::query("update sessions set last_refreshed_at = now() where id = $1").bind(row.session_id).execute(&mut *tx).await?;
    sqlx::query("update devices set last_seen_at = now() where id = (select device_id from sessions where id = $1)")
        .bind(row.session_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let (access, access_exp) = issue_access(&state, user.id, row.session_id)?;
    session_response(&state, StatusCode::OK, client, user, Issued { session_id: row.session_id, access, access_exp, refresh: new_refresh })
}

pub async fn logout(State(state): State<AppState>, meta: ClientMeta, user: AuthUser) -> Result<Response, ApiError> {
    let mut tx = state.db.begin().await?;
    sqlx::query("update sessions set revoked_at = now(), revoke_reason = 'logout' where id = $1 and revoked_at is null")
        .bind(user.session_id)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: None,
            actor_user_id: Some(user.user_id),
            action: "auth.logout",
            target_kind: "session",
            target_id: Some(user.session_id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::NO_CONTENT, clear_cookies(&state)).into_response())
}

pub fn clear_cookies(state: &AppState) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let secure = state.cookie_secure;
    for c in [
        cookies::clear(cookies::ACCESS, cookies::ACCESS_PATH, true, secure),
        cookies::clear(cookies::REFRESH, cookies::REFRESH_PATH, true, secure),
        cookies::clear(cookies::CSRF, cookies::CSRF_PATH, false, secure),
    ]
    .into_iter()
    .flatten()
    {
        headers.append(SET_COOKIE, c);
    }
    headers
}

pub const LOGIN_CODE_TTL_SECS: i64 = 60;

#[derive(Serialize, Deserialize, Debug)]
pub struct LoginCodeOut {
    pub code: String,
    pub expires_in_secs: i64,
}

/// A signed-in app asks for a single-use code to open its WebView signed in, without putting a
/// token in a URL or in page storage. Valid 60 seconds, once.
pub async fn login_code(State(state): State<AppState>, meta: ClientMeta, user: AuthUser) -> Result<(StatusCode, Json<LoginCodeOut>), ApiError> {
    limit(&state, format!("login_code:user:{}", user.user_id), state.limits.refresh_per_ip).await?;
    let (code, hash) = tokens::new_login_code().map_err(ApiError::internal)?;
    let mut tx = state.db.begin().await?;
    sqlx::query("insert into login_codes (code_hash, user_id, source_session_id, expires_at) values ($1, $2, $3, now() + make_interval(secs => $4))")
        .bind(hash)
        .bind(user.user_id)
        .bind(user.session_id)
        .bind(LOGIN_CODE_TTL_SECS as f64)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: None,
            actor_user_id: Some(user.user_id),
            action: "auth.login_code",
            target_kind: "session",
            target_id: Some(user.session_id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(LoginCodeOut { code, expires_in_secs: LOGIN_CODE_TTL_SECS })))
}

#[derive(Deserialize)]
pub struct ExchangeForm {
    code: String,
    next: Option<String>,
}

#[derive(sqlx::FromRow)]
struct CodeRow {
    user_id: Uuid,
    usable: bool,
}

/// Same-site paths only, so a crafted `next` can't send the WebView to another site.
pub fn safe_next(next: Option<&str>) -> &str {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.starts_with("/\\") && n.len() <= 512 && !n.chars().any(char::is_control) => n,
        _ => "/",
    }
}

/// The WebView's first navigation: a form POST with the code. Answers with the browser session
/// cookies and a redirect into the app. A code is used once; the session it came from must
/// still be live.
pub async fn exchange(
    State(state): State<AppState>,
    meta: ClientMeta,
    headers: HeaderMap,
    form: Result<axum::Form<ExchangeForm>, axum::extract::rejection::FormRejection>,
) -> Result<Response, ApiError> {
    // A page on another site could post a code of its own (login CSRF). Browsers and WebViews
    // mark such posts `Sec-Fetch-Site: cross-site`; the app's own navigation is `none`.
    if headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("cross-site") {
        return Err(ApiError::forbidden("cross_site", "This sign-in link must be opened by the app."));
    }
    let axum::Form(form) = form.map_err(|_| ApiError::invalid("code", "Send the code as a form field."))?;
    let invalid = || ApiError::unauthorized("invalid_code", "This sign-in link has expired or was already used. Open the project again from the app.");
    if !tokens::looks_like_login_code(&form.code) {
        return Err(invalid());
    }
    let hash = tokens::hash_refresh(&form.code);
    let mut tx = state.db.begin().await?;
    let row = sqlx::query_as::<_, CodeRow>(
        "select c.user_id, (c.used_at is null and c.expires_at > now() and s.revoked_at is null and s.expires_at > now() and u.status = 'active') as usable
         from login_codes c join sessions s on s.id = c.source_session_id join users u on u.id = c.user_id
         where c.code_hash = $1 for update of c",
    )
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else { return Err(invalid()) };
    // Mark it used even when it is no longer usable, so a code is never tried twice.
    sqlx::query("update login_codes set used_at = coalesce(used_at, now()) where code_hash = $1").bind(&hash).execute(&mut *tx).await?;
    if !row.usable {
        tx.commit().await?;
        return Err(invalid());
    }
    let issued = start_session(&mut tx, &state, row.user_id, ClientKind::Android, Some("Android app (web view)")).await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: None,
            actor_user_id: Some(row.user_id),
            action: "auth.exchange",
            target_kind: "session",
            target_id: Some(issued.session_id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    let mut out = HeaderMap::new();
    web_cookies(&state, &issued, &mut out)?;
    if let Ok(v) = axum::http::HeaderValue::from_str(safe_next(form.next.as_deref())) {
        out.insert(axum::http::header::LOCATION, v);
    }
    Ok((StatusCode::SEE_OTHER, out).into_response())
}
