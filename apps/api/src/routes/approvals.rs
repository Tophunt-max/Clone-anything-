//! `/api/v1/workspaces/{ws}/approvals` and decisions: a maintainer approves a tool that runs
//! code or changes a device before it runs (threat model T11, T20).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::audit::{self, Entry};
use crate::auth::AuthUser;
use crate::auth::rbac::{self, Action};
use crate::client::ClientMeta;
use crate::error::ApiError;
use crate::extract::{Id, Params};
use crate::page::{Page, PageQuery, page};
use crate::routes::workspaces::role_in;

#[derive(Serialize, sqlx::FromRow)]
pub struct ApprovalOut {
    pub id: Uuid,
    pub project_id: Uuid,
    pub subject_kind: String,
    pub tool: Option<String>,
    pub summary: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn list(State(state): State<AppState>, user: AuthUser, Id(ws): Id<Uuid>, Params(q): Params<PageQuery>) -> Result<Json<Page<ApprovalOut>>, ApiError> {
    let role = role_in(&state.db, ws, user.user_id).await?;
    rbac::require(role, Action::ViewWorkspace)?;
    let a = q.args()?;
    let rows = sqlx::query_as::<_, ApprovalOut>(
        "select id, project_id, subject_kind, tool, summary, status, created_at, expires_at from approvals
         where workspace_id = $1 and status = 'pending' and expires_at > now() and id > $2 order by id limit $3",
    )
    .bind(ws)
    .bind(a.after)
    .bind(a.limit + 1)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(page(rows, a.limit, |r| r.id)))
}

async fn decide(state: &AppState, meta: &ClientMeta, user: Uuid, id: Uuid, approve: bool) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await?;
    // The approver must be a maintainer+ of the approval's workspace.
    let row =
        sqlx::query_as::<_, (Uuid, String)>("select workspace_id, status from approvals where id = $1 for update").bind(id).fetch_optional(&mut *tx).await?;
    let Some((workspace_id, status)) = row else { return Err(ApiError::not_found()) };
    let role = role_in(&mut *tx, workspace_id, user).await?;
    rbac::require(role, Action::ApproveSensitive)?;
    if status != "pending" {
        return Err(ApiError::conflict("already_decided", "This request was already decided or has expired."));
    }
    let new = if approve { "approved" } else { "rejected" };
    sqlx::query("update approvals set status = $2, decided_by = $3, decided_at = now() where id = $1 and status = 'pending' and expires_at > now()")
        .bind(id)
        .bind(new)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(workspace_id),
            actor_user_id: Some(user),
            action: if approve { "approval.approve" } else { "approval.reject" },
            target_kind: "approval",
            target_id: Some(id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn approve(State(state): State<AppState>, meta: ClientMeta, user: AuthUser, Id(id): Id<Uuid>) -> Result<StatusCode, ApiError> {
    decide(&state, &meta, user.user_id, id, true).await
}

pub async fn reject(State(state): State<AppState>, meta: ClientMeta, user: AuthUser, Id(id): Id<Uuid>) -> Result<StatusCode, ApiError> {
    decide(&state, &meta, user.user_id, id, false).await
}
