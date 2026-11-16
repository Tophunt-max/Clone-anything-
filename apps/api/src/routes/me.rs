//! `/api/v1/me*`: the signed-in user and their sessions.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::auth::UserOut;
use crate::AppState;
use crate::audit::{self, Entry};
use crate::auth::AuthUser;
use crate::client::ClientMeta;
use crate::error::ApiError;
use crate::extract::Id;

pub async fn me(State(state): State<AppState>, user: AuthUser) -> Result<Json<UserOut>, ApiError> {
    let u = sqlx::query_as::<_, UserOut>("select id, email, display_name, created_at from users where id = $1")
        .bind(user.user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(ApiError::unauthenticated)?;
    Ok(Json(u))
}

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub device_kind: String,
    pub device_name: String,
    pub created_at: DateTime<Utc>,
    pub last_refreshed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[sqlx(default)]
    pub current: bool,
}

pub async fn sessions(State(state): State<AppState>, user: AuthUser) -> Result<Json<Vec<SessionRow>>, ApiError> {
    let rows = sqlx::query_as::<_, SessionRow>(
        "select s.id, d.kind as device_kind, d.name as device_name, s.created_at, s.last_refreshed_at, s.expires_at, s.id = $2 as current
         from sessions s join devices d on d.id = s.device_id
         where s.user_id = $1 and s.revoked_at is null and s.expires_at > now()
         order by s.last_refreshed_at desc limit 200",
    )
    .bind(user.user_id)
    .bind(user.session_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub async fn revoke_session(State(state): State<AppState>, meta: ClientMeta, user: AuthUser, Id(id): Id<Uuid>) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await?;
    let done = sqlx::query("update sessions set revoked_at = now(), revoke_reason = 'revoked_by_user' where id = $1 and user_id = $2 and revoked_at is null")
        .bind(id)
        .bind(user.user_id)
        .execute(&mut *tx)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::not_found());
    }
    audit::record(
        &mut tx,
        Entry {
            workspace_id: None,
            actor_user_id: Some(user.user_id),
            action: "session.revoke",
            target_kind: "session",
            target_id: Some(id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
