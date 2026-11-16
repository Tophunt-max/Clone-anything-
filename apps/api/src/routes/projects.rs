//! Projects. A project is reached by its own ID (`/api/v1/projects/{id}`), but every query joins
//! through the caller's workspace membership, so an ID from another tenant answers 404.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::workspaces::role_in;
use crate::AppState;
use crate::audit::{self, Entry};
use crate::auth::AuthUser;
use crate::auth::rbac::{self, Action, Role};
use crate::client::ClientMeta;
use crate::error::ApiError;
use crate::extract::{Body, Id, Params};
use crate::page::{Page, PageQuery, page};
use crate::validate;

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct ProjectOut {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub kind: String,
    pub deep_mode: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Code,
    Analysis,
    Rebuild,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Code => "code",
            Kind::Analysis => "analysis",
            Kind::Rebuild => "rebuild",
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    limit: Option<i64>,
    cursor: Option<String>,
    #[serde(default)]
    include_archived: bool,
}

pub async fn list(State(state): State<AppState>, user: AuthUser, Id(ws): Id<Uuid>, Params(q): Params<ListQuery>) -> Result<Json<Page<ProjectOut>>, ApiError> {
    let a = PageQuery { limit: q.limit, cursor: q.cursor }.args()?;
    let role = role_in(&state.db, ws, user.user_id).await?;
    rbac::require(role, Action::ViewProject)?;
    let rows = sqlx::query_as::<_, ProjectOut>(
        "select p.id, p.workspace_id, p.name, p.kind, p.deep_mode, p.created_at, p.updated_at, p.archived_at from projects p
         where p.workspace_id = $1 and p.id > $2 and ($4 or p.archived_at is null) order by p.id limit $3",
    )
    .bind(ws)
    .bind(a.after)
    .bind(a.limit + 1)
    .bind(q.include_archived)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(page(rows, a.limit, |p| p.id)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBody {
    name: String,
    kind: Kind,
}

pub async fn create(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id(ws): Id<Uuid>,
    Body(b): Body<CreateBody>,
) -> Result<(StatusCode, Json<ProjectOut>), ApiError> {
    let name = validate::name("name", &b.name)?;
    let mut tx = state.db.begin().await?;
    let role = role_in(&mut *tx, ws, user.user_id).await?;
    rbac::require(role, Action::CreateProject)?;
    let id = Uuid::now_v7();
    let p = sqlx::query_as::<_, ProjectOut>(
        "insert into projects as p (id, workspace_id, name, kind, created_by) values ($1, $2, $3, $4, $5)
         returning p.id, p.workspace_id, p.name, p.kind, p.deep_mode, p.created_at, p.updated_at, p.archived_at",
    )
    .bind(id)
    .bind(ws)
    .bind(&name)
    .bind(b.kind.as_str())
    .bind(user.user_id)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(ws),
            actor_user_id: Some(user.user_id),
            action: "project.create",
            target_kind: "project",
            target_id: Some(id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"kind": b.kind.as_str()}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(p)))
}

/// The project and the caller's role in its workspace; 404 if either is missing.
async fn visible(conn: &mut sqlx::PgConnection, project: Uuid, user: Uuid, lock: bool) -> Result<(ProjectOut, Role), ApiError> {
    let sql = if lock {
        "select p.id, p.workspace_id, p.name, p.kind, p.deep_mode, p.created_at, p.updated_at, p.archived_at, m.role
         from projects p join workspace_members m on m.workspace_id = p.workspace_id and m.user_id = $2 where p.id = $1 for update of p"
    } else {
        "select p.id, p.workspace_id, p.name, p.kind, p.deep_mode, p.created_at, p.updated_at, p.archived_at, m.role
         from projects p join workspace_members m on m.workspace_id = p.workspace_id and m.user_id = $2 where p.id = $1"
    };
    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        p: ProjectOut,
        role: String,
    }
    let row = sqlx::query_as::<_, Row>(sql).bind(project).bind(user).fetch_optional(conn).await?.ok_or_else(ApiError::not_found)?;
    let role = Role::parse(&row.role).ok_or_else(ApiError::not_found)?;
    Ok((row.p, role))
}

pub async fn get(State(state): State<AppState>, user: AuthUser, Id(id): Id<Uuid>) -> Result<Json<ProjectOut>, ApiError> {
    let mut conn = state.db.acquire().await?;
    let (p, role) = visible(&mut conn, id, user.user_id, false).await?;
    rbac::require(role, Action::ViewProject)?;
    Ok(Json(p))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateBody {
    name: Option<String>,
    archived: Option<bool>,
}

pub async fn update(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id(id): Id<Uuid>,
    Body(b): Body<UpdateBody>,
) -> Result<Json<ProjectOut>, ApiError> {
    let name = b.name.as_deref().map(|n| validate::name("name", n)).transpose()?;
    let mut tx = state.db.begin().await?;
    let (p, role) = visible(&mut tx, id, user.user_id, true).await?;
    rbac::require(role, Action::EditProject)?;
    let updated = sqlx::query_as::<_, ProjectOut>(
        "update projects as p set name = coalesce($2, p.name),
            archived_at = case when $3::boolean is null then p.archived_at when $3 then coalesce(p.archived_at, now()) else null end,
            updated_at = now()
         where p.id = $1 returning p.id, p.workspace_id, p.name, p.kind, p.deep_mode, p.created_at, p.updated_at, p.archived_at",
    )
    .bind(id)
    .bind(name)
    .bind(b.archived)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(p.workspace_id),
            actor_user_id: Some(user.user_id),
            action: "project.update",
            target_kind: "project",
            target_id: Some(id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"archived": b.archived}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(updated))
}

pub async fn delete(State(state): State<AppState>, meta: ClientMeta, user: AuthUser, Id(id): Id<Uuid>) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await?;
    let (p, role) = visible(&mut tx, id, user.user_id, true).await?;
    rbac::require(role, Action::DeleteProject)?;
    sqlx::query("delete from projects where id = $1").bind(id).execute(&mut *tx).await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(p.workspace_id),
            actor_user_id: Some(user.user_id),
            action: "project.delete",
            target_kind: "project",
            target_id: Some(id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
