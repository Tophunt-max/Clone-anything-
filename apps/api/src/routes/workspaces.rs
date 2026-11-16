//! `/api/v1/workspaces*`: workspaces and their members. Every handler starts from the caller's
//! membership; a workspace the caller isn't in answers 404, never 403, so IDs can't be probed.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::AppState;
use crate::audit::{self, Entry};
use crate::auth::AuthUser;
use crate::auth::rbac::{self, Action, Role};
use crate::client::ClientMeta;
use crate::error::{ApiError, is_unique_violation};
use crate::extract::{Body, Id, Params};
use crate::page::{Page, PageQuery, page};
use crate::validate;

/// The caller's role in a workspace, or 404 when they aren't a member (or it doesn't exist).
pub async fn role_in<'e>(db: impl PgExecutor<'e>, workspace_id: Uuid, user_id: Uuid) -> Result<Role, ApiError> {
    let role: Option<String> = sqlx::query_scalar("select role from workspace_members where workspace_id = $1 and user_id = $2")
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?;
    role.as_deref().and_then(Role::parse).ok_or_else(ApiError::not_found)
}

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct WorkspaceOut {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

pub async fn list(State(state): State<AppState>, user: AuthUser, Params(q): Params<PageQuery>) -> Result<Json<Page<WorkspaceOut>>, ApiError> {
    let a = q.args()?;
    let rows = sqlx::query_as::<_, WorkspaceOut>(
        "select w.id, w.name, w.slug, m.role, w.created_at from workspaces w join workspace_members m on m.workspace_id = w.id
         where m.user_id = $1 and w.id > $2 order by w.id limit $3",
    )
    .bind(user.user_id)
    .bind(a.after)
    .bind(a.limit + 1)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(page(rows, a.limit, |w| w.id)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBody {
    name: String,
    slug: String,
}

pub async fn create(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Body(b): Body<CreateBody>,
) -> Result<(StatusCode, Json<WorkspaceOut>), ApiError> {
    let name = validate::name("name", &b.name)?;
    let slug = validate::slug(&b.slug)?;
    let mut tx = state.db.begin().await?;
    let id = Uuid::now_v7();
    let created = sqlx::query_as::<_, WorkspaceOut>(
        "insert into workspaces (id, name, slug, created_by) values ($1, $2, $3, $4) returning id, name, slug, 'owner' as role, created_at",
    )
    .bind(id)
    .bind(&name)
    .bind(&slug)
    .bind(user.user_id)
    .fetch_one(&mut *tx)
    .await;
    let ws = match created {
        Ok(w) => w,
        Err(e) if is_unique_violation(&e) => return Err(ApiError::conflict("slug_taken", "Another workspace uses this short name. Pick another.")),
        Err(e) => return Err(e.into()),
    };
    sqlx::query("insert into workspace_members (workspace_id, user_id, role) values ($1, $2, 'owner')").bind(id).bind(user.user_id).execute(&mut *tx).await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(id),
            actor_user_id: Some(user.user_id),
            action: "workspace.create",
            target_kind: "workspace",
            target_id: Some(id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"slug": slug}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(ws)))
}

pub async fn get(State(state): State<AppState>, user: AuthUser, Id(ws): Id<Uuid>) -> Result<Json<WorkspaceOut>, ApiError> {
    let role = role_in(&state.db, ws, user.user_id).await?;
    rbac::require(role, Action::ViewWorkspace)?;
    let w = sqlx::query_as::<_, WorkspaceOut>("select id, name, slug, $2 as role, created_at from workspaces where id = $1")
        .bind(ws)
        .bind(role.as_str())
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(w))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameBody {
    name: String,
}

pub async fn rename(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id(ws): Id<Uuid>,
    Body(b): Body<RenameBody>,
) -> Result<Json<WorkspaceOut>, ApiError> {
    let name = validate::name("name", &b.name)?;
    let mut tx = state.db.begin().await?;
    let role = role_in(&mut *tx, ws, user.user_id).await?;
    rbac::require(role, Action::RenameWorkspace)?;
    let w =
        sqlx::query_as::<_, WorkspaceOut>("update workspaces set name = $2, updated_at = now() where id = $1 returning id, name, slug, $3 as role, created_at")
            .bind(ws)
            .bind(&name)
            .bind(role.as_str())
            .fetch_one(&mut *tx)
            .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(ws),
            actor_user_id: Some(user.user_id),
            action: "workspace.rename",
            target_kind: "workspace",
            target_id: Some(ws),
            ip_hash: Some(&meta.ip_hash),
            details: json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(w))
}

pub async fn delete(State(state): State<AppState>, meta: ClientMeta, user: AuthUser, Id(ws): Id<Uuid>) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await?;
    let role = role_in(&mut *tx, ws, user.user_id).await?;
    rbac::require(role, Action::DeleteWorkspace)?;
    sqlx::query("delete from workspaces where id = $1").bind(ws).execute(&mut *tx).await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(ws),
            actor_user_id: Some(user.user_id),
            action: "workspace.delete",
            target_kind: "workspace",
            target_id: Some(ws),
            ip_hash: Some(&meta.ip_hash),
            details: json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct MemberOut {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

pub async fn members(State(state): State<AppState>, user: AuthUser, Id(ws): Id<Uuid>, Params(q): Params<PageQuery>) -> Result<Json<Page<MemberOut>>, ApiError> {
    let a = q.args()?;
    let role = role_in(&state.db, ws, user.user_id).await?;
    rbac::require(role, Action::ViewWorkspace)?;
    let rows = sqlx::query_as::<_, MemberOut>(
        "select u.id as user_id, u.email, u.display_name, m.role, m.created_at from workspace_members m join users u on u.id = m.user_id
         where m.workspace_id = $1 and u.id > $2 order by u.id limit $3",
    )
    .bind(ws)
    .bind(a.after)
    .bind(a.limit + 1)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(page(rows, a.limit, |m| m.user_id)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddMemberBody {
    email: String,
    role: Role,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleBody {
    role: Role,
}

fn cannot_change() -> ApiError {
    ApiError::forbidden("insufficient_role", "You can only give or change roles below your own.")
}

async fn owners_left<'e>(db: impl PgExecutor<'e>, ws: Uuid) -> Result<i64, ApiError> {
    Ok(sqlx::query_scalar("select count(*) from workspace_members where workspace_id = $1 and role = 'owner'").bind(ws).fetch_one(db).await?)
}

pub async fn add_member(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id(ws): Id<Uuid>,
    Body(b): Body<AddMemberBody>,
) -> Result<(StatusCode, Json<MemberOut>), ApiError> {
    let email = validate::email(&b.email)?;
    let mut tx = state.db.begin().await?;
    let actor = role_in(&mut *tx, ws, user.user_id).await?;
    rbac::require(actor, Action::ManageMembers)?;
    if !rbac::may_change_member(actor, None, Some(b.role)) {
        return Err(cannot_change());
    }
    let target: Option<Uuid> =
        sqlx::query_scalar("select id from users where lower(email) = $1 and status = 'active'").bind(&email).fetch_optional(&mut *tx).await?;
    let target = target.ok_or_else(|| ApiError::invalid("email", "No active account uses this email. Ask them to register first."))?;
    let inserted = sqlx::query("insert into workspace_members (workspace_id, user_id, role) values ($1, $2, $3)")
        .bind(ws)
        .bind(target)
        .bind(b.role.as_str())
        .execute(&mut *tx)
        .await;
    match inserted {
        Ok(_) => {}
        Err(e) if is_unique_violation(&e) => return Err(ApiError::conflict("already_member", "This person is already a member.")),
        Err(e) => return Err(e.into()),
    }
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(ws),
            actor_user_id: Some(user.user_id),
            action: "member.add",
            target_kind: "user",
            target_id: Some(target),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"role": b.role.as_str()}),
        },
    )
    .await?;
    let m = member(&mut tx, ws, target).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(m)))
}

async fn member(tx: &mut sqlx::PgConnection, ws: Uuid, user: Uuid) -> Result<MemberOut, ApiError> {
    sqlx::query_as::<_, MemberOut>("select u.id as user_id, u.email, u.display_name, m.role, m.created_at from workspace_members m join users u on u.id = m.user_id where m.workspace_id = $1 and m.user_id = $2")
        .bind(ws)
        .bind(user)
        .fetch_optional(tx)
        .await?
        .ok_or_else(ApiError::not_found)
}

pub async fn change_member(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id((ws, target)): Id<(Uuid, Uuid)>,
    Body(b): Body<RoleBody>,
) -> Result<Json<MemberOut>, ApiError> {
    let mut tx = state.db.begin().await?;
    // Lock this workspace's member rows so two owners can't demote each other at the same time.
    sqlx::query("select 1 from workspace_members where workspace_id = $1 for update").bind(ws).fetch_all(&mut *tx).await?;
    let actor = role_in(&mut *tx, ws, user.user_id).await?;
    rbac::require(actor, Action::ManageMembers)?;
    let current = role_in(&mut *tx, ws, target).await?;
    if !rbac::may_change_member(actor, Some(current), Some(b.role)) {
        return Err(cannot_change());
    }
    if current == Role::Owner && b.role != Role::Owner && owners_left(&mut *tx, ws).await? <= 1 {
        return Err(ApiError::conflict("last_owner", "A workspace needs at least one owner. Make someone else an owner first."));
    }
    sqlx::query("update workspace_members set role = $3 where workspace_id = $1 and user_id = $2")
        .bind(ws)
        .bind(target)
        .bind(b.role.as_str())
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(ws),
            actor_user_id: Some(user.user_id),
            action: "member.role",
            target_kind: "user",
            target_id: Some(target),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"from": current.as_str(), "to": b.role.as_str()}),
        },
    )
    .await?;
    let m = member(&mut tx, ws, target).await?;
    tx.commit().await?;
    Ok(Json(m))
}

pub async fn remove_member(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id((ws, target)): Id<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await?;
    sqlx::query("select 1 from workspace_members where workspace_id = $1 for update").bind(ws).fetch_all(&mut *tx).await?;
    let actor = role_in(&mut *tx, ws, user.user_id).await?;
    let current = role_in(&mut *tx, ws, target).await?;
    let leaving = target == user.user_id;
    if !leaving {
        rbac::require(actor, Action::ManageMembers)?;
        if !rbac::may_change_member(actor, Some(current), None) {
            return Err(cannot_change());
        }
    }
    if current == Role::Owner && owners_left(&mut *tx, ws).await? <= 1 {
        return Err(ApiError::conflict("last_owner", "A workspace needs at least one owner. Make someone else an owner first."));
    }
    sqlx::query("delete from workspace_members where workspace_id = $1 and user_id = $2").bind(ws).bind(target).execute(&mut *tx).await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(ws),
            actor_user_id: Some(user.user_id),
            action: if leaving { "member.leave" } else { "member.remove" },
            target_kind: "user",
            target_id: Some(target),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"role": current.as_str()}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
