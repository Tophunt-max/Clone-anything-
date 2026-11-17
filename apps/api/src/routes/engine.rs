//! `/api/v1/projects/{id}/tool-calls` and evidence: running engine tools with policy and
//! approval, and reading the evidence they record (screens 16–20).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::AppState;
use crate::audit::{self, Entry};
use crate::auth::AuthUser;
use crate::auth::rbac::{self, Action};
use crate::client::ClientMeta;
use crate::engine::{Policy, deadline_for};
use crate::error::ApiError;
use crate::extract::{Body, Id, Params};
use crate::page::{Page, PageQuery, page};
use crate::problem::Problem;
use crate::routes::projects::ProjectAccess;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallBody {
    tool: String,
    #[serde(default)]
    params: Value,
    /// An approval that authorizes a consent-gated tool (from a prior 409).
    #[serde(default)]
    approval_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct CallResult {
    pub status: &'static str,
    pub tool: String,
    pub policy: String,
    pub output: Value,
    pub evidence_id: Option<String>,
    pub duration_ms: u64,
}

pub async fn call(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id(project_id): Id<Uuid>,
    Body(body): Body<CallBody>,
) -> Result<Json<CallResult>, ApiError> {
    let access = ProjectAccess::load(&state, project_id, user.user_id).await?;
    let catalog = state.engine.catalog().await.map_err(super::tools::worker_problem)?;
    let info = catalog.get(&body.tool).ok_or_else(|| ApiError::invalid("tool", "No such tool."))?;

    // Permission by policy.
    let (needs_approval, min_action) = match info.policy {
        Policy::Unavailable => {
            return Err(ApiError::Problem(Box::new(Problem::new(
                StatusCode::FAILED_DEPENDENCY,
                "tool_unavailable",
                "Not available",
                "This tool can't run on the server (it needs a device or desktop OS).",
            ))));
        }
        Policy::OperatorOnly => return Err(ApiError::forbidden("operator_only", "This tool is for the server operator, not projects.")),
        Policy::Viewer => (false, Action::RunReadOnlyTool),
        Policy::Editor | Policy::EditorWorker => (false, Action::RunTool),
        Policy::ApprovalGate => (true, Action::RunTool),
    };
    rbac::require(access.role, min_action)?;

    // Consent-gated tools need an approved, unused approval for this project and tool.
    let mut approval_used: Option<Uuid> = None;
    if needs_approval {
        match body.approval_id {
            None => return Err(pending_approval(&state, &meta, &access, user.user_id, &body.tool).await?),
            Some(id) => {
                approval_used = Some(claim_approval(&state, project_id, id, &body.tool).await?);
            }
        }
    }

    // Deep mode reads inside a target; it is a per-project setting an admin turned on.
    let worker =
        state.engine.worker_for(&access.workspace_id.to_string(), &project_id.to_string(), access.deep_mode).await.map_err(super::tools::worker_problem)?;
    let call_id = Uuid::now_v7().to_string();
    let consent = needs_approval; // only granted when an approval authorized it
    let result = worker.call(&call_id, &body.tool, body.params.clone(), consent, deadline_for(info), |_, _| {}).await;

    let redacted = crate::engine::redact::params(&body.params);
    match result {
        Ok(ok) => {
            if let Some(id) = &ok.evidence_id {
                index_evidence(&state, &access, &body.tool, id, &ok.output).await;
            }
            record(
                &state,
                &access,
                user.user_id,
                &body.tool,
                &info.policy,
                approval_used,
                "succeeded",
                None,
                ok.evidence_id.as_deref(),
                Some(ok.duration_ms),
                &redacted,
            )
            .await;
            Ok(Json(CallResult {
                status: "succeeded",
                tool: body.tool,
                policy: info.policy.as_str().to_string(),
                output: ok.output,
                evidence_id: ok.evidence_id,
                duration_ms: ok.duration_ms,
            }))
        }
        Err(e) => {
            let code = if let crate::engine::WorkerError::Call { code, .. } = &e { Some(code.clone()) } else { None };
            record(&state, &access, user.user_id, &body.tool, &info.policy, approval_used, "failed", code.as_deref(), None, None, &redacted).await;
            Err(super::tools::worker_problem(e))
        }
    }
}

async fn pending_approval(state: &AppState, meta: &ClientMeta, access: &ProjectAccess, user: Uuid, tool: &str) -> Result<ApiError, ApiError> {
    let id = Uuid::now_v7();
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "insert into approvals (id, workspace_id, project_id, subject_kind, tool, summary, requested_by, expires_at)
         values ($1, $2, $3, 'tool_call', $4, $5, $6, now() + interval '1 hour')",
    )
    .bind(id)
    .bind(access.workspace_id)
    .bind(access.project_id)
    .bind(tool)
    .bind(format!("Run {tool}"))
    .bind(user)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(access.workspace_id),
            actor_user_id: Some(user),
            action: "approval.request",
            target_kind: "tool",
            target_id: Some(id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"tool": tool}),
        },
    )
    .await?;
    tx.commit().await?;
    let mut p = Problem::new(
        StatusCode::CONFLICT,
        "approval_required",
        "Approval needed",
        "This tool runs code or changes a device. A maintainer must approve it, then try again.",
    );
    p.field = Some(id.to_string());
    Ok(ApiError::Problem(Box::new(p)))
}

async fn claim_approval(state: &AppState, project_id: Uuid, id: Uuid, tool: &str) -> Result<Uuid, ApiError> {
    // Approved, for this project and tool, not expired, not yet used: mark it used and run once.
    let done = sqlx::query_scalar::<_, Uuid>(
        "update approvals set status = 'used' where id = $1 and project_id = $2 and tool = $3 and status = 'approved' and expires_at > now() returning id",
    )
    .bind(id)
    .bind(project_id)
    .bind(tool)
    .fetch_optional(&state.db)
    .await?;
    done.ok_or_else(|| {
        ApiError::Problem(Box::new(Problem::new(
            StatusCode::CONFLICT,
            "approval_invalid",
            "Approval not usable",
            "This approval is missing, not yet granted, already used or expired. Request it again.",
        )))
    })
}

#[allow(clippy::too_many_arguments)]
async fn record(
    state: &AppState,
    access: &ProjectAccess,
    user: Uuid,
    tool: &str,
    policy: &Policy,
    approval: Option<Uuid>,
    status: &str,
    error_code: Option<&str>,
    evidence_id: Option<&str>,
    duration_ms: Option<u64>,
    params_redacted: &Value,
) {
    let r = sqlx::query(
        "insert into tool_executions (id, workspace_id, project_id, user_id, tool, params_redacted, policy, approval_id, status, error_code, evidence_id, duration_ms)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(Uuid::now_v7())
    .bind(access.workspace_id)
    .bind(access.project_id)
    .bind(user)
    .bind(tool)
    .bind(params_redacted)
    .bind(policy.as_str())
    .bind(approval)
    .bind(status)
    .bind(error_code)
    .bind(evidence_id)
    .bind(duration_ms.map(|d| d as i32))
    .execute(&state.db)
    .await;
    if let Err(e) = r {
        tracing::error!(error = %e, "recording a tool execution failed");
    }
}

/// Stores the evidence the tool recorded in the searchable index. The body is the full record
/// the worker returned.
async fn index_evidence(state: &AppState, access: &ProjectAccess, tool: &str, evidence_id: &str, body: &Value) {
    let operation = body.get("operation").and_then(Value::as_str).unwrap_or("").to_string();
    let subject = body.get("subject").cloned().unwrap_or_else(|| json!({}));
    let confidence = body.get("confidence").and_then(Value::as_str).map(str::to_string);
    let summary = body.get("summary").and_then(Value::as_str).unwrap_or("").to_string();
    let r = sqlx::query(
        "insert into evidence_records (workspace_id, project_id, evidence_id, operation, subject, confidence, summary, tool, body)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9) on conflict (workspace_id, evidence_id) do nothing",
    )
    .bind(access.workspace_id)
    .bind(access.project_id)
    .bind(evidence_id)
    .bind(&operation)
    .bind(&subject)
    .bind(&confidence)
    .bind(&summary)
    .bind(tool)
    .bind(body)
    .execute(&state.db)
    .await;
    if let Err(e) = r {
        tracing::error!(error = %e, "indexing evidence failed");
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeepModeBody {
    enabled: bool,
    /// Why reading inside targets is authorized here (recorded in the audit log).
    #[serde(default)]
    authorization: Option<String>,
}

/// `POST /projects/{id}/deep-mode`: turn reading inside targets on or off. Deep mode is sensitive
/// (threat model T20), so it needs the maintainer role and records who enabled it and why.
pub async fn set_deep_mode(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id(project_id): Id<Uuid>,
    Body(body): Body<DeepModeBody>,
) -> Result<Json<super::projects::ProjectOut>, ApiError> {
    let access = ProjectAccess::load(&state, project_id, user.user_id).await?;
    rbac::require(access.role, Action::ApproveSensitive)?;
    if body.enabled && body.authorization.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(ApiError::invalid("authorization", "Say why you are authorized to read inside targets in this project."));
    }
    let mut tx = state.db.begin().await?;
    let updated = sqlx::query_as::<_, super::projects::ProjectOut>(
        "update projects as p set deep_mode = $2, updated_at = now() where id = $1
         returning p.id, p.workspace_id, p.name, p.kind, p.deep_mode, p.created_at, p.updated_at, p.archived_at",
    )
    .bind(project_id)
    .bind(body.enabled)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        Entry {
            workspace_id: Some(access.workspace_id),
            actor_user_id: Some(user.user_id),
            action: if body.enabled { "project.deep_mode_on" } else { "project.deep_mode_off" },
            target_kind: "project",
            target_id: Some(project_id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"authorization": body.authorization}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(updated))
}

// ---- reads ----

#[derive(Serialize, sqlx::FromRow)]
pub struct ExecutionOut {
    pub id: Uuid,
    pub tool: String,
    pub policy: String,
    pub status: String,
    pub error_code: Option<String>,
    pub evidence_id: Option<String>,
    pub duration_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
}

pub async fn executions(
    State(state): State<AppState>,
    user: AuthUser,
    Id(project_id): Id<Uuid>,
    Params(q): Params<PageQuery>,
) -> Result<Json<Page<ExecutionOut>>, ApiError> {
    let access = ProjectAccess::load(&state, project_id, user.user_id).await?;
    rbac::require(access.role, Action::ViewProject)?;
    let a = q.args()?;
    let rows = sqlx::query_as::<_, ExecutionOut>(
        "select id, tool, policy, status, error_code, evidence_id, duration_ms, created_at from tool_executions
         where project_id = $1 and id > $2 order by id limit $3",
    )
    .bind(project_id)
    .bind(a.after)
    .bind(a.limit + 1)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(page(rows, a.limit, |r| r.id)))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct EvidenceRow {
    pub evidence_id: String,
    pub operation: String,
    pub confidence: Option<String>,
    pub summary: String,
    pub tool: Option<String>,
    pub subject: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EvidenceQuery {
    limit: Option<i64>,
    cursor: Option<String>,
    operation: Option<String>,
}

pub async fn evidence_list(
    State(state): State<AppState>,
    user: AuthUser,
    Id(project_id): Id<Uuid>,
    Params(q): Params<EvidenceQuery>,
) -> Result<Json<Page<EvidenceRow>>, ApiError> {
    let access = ProjectAccess::load(&state, project_id, user.user_id).await?;
    rbac::require(access.role, Action::ViewProject)?;
    let limit = q.limit.unwrap_or(crate::page::DEFAULT_LIMIT);
    if !(1..=crate::page::MAX_LIMIT).contains(&limit) {
        return Err(ApiError::invalid("limit", "limit must be between 1 and 200."));
    }
    // The cursor is the last evidence_id (a content hash, so a stable total order).
    let after = match q.cursor.as_deref() {
        None | Some("") => String::new(),
        Some(c) if c.len() <= 67 && c.starts_with("ev_") => c.to_string(),
        Some(_) => return Err(ApiError::invalid("cursor", "cursor is not one this server gave out.")),
    };
    let rows = sqlx::query_as::<_, EvidenceRow>(
        "select evidence_id, operation, confidence, summary, tool, subject, created_at from evidence_records
         where project_id = $1 and ($4::text is null or operation = $4) and evidence_id > $3 order by evidence_id limit $2",
    )
    .bind(project_id)
    .bind(limit + 1)
    .bind(&after)
    .bind(&q.operation)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(page_text(rows, limit, |r| r.evidence_id.clone())))
}

pub async fn evidence_get(State(state): State<AppState>, user: AuthUser, Id((project_id, evidence_id)): Id<(Uuid, String)>) -> Result<Json<Value>, ApiError> {
    let access = ProjectAccess::load(&state, project_id, user.user_id).await?;
    rbac::require(access.role, Action::ViewProject)?;
    let body = sqlx::query_scalar::<_, Value>("select body from evidence_records where project_id = $1 and evidence_id = $2")
        .bind(project_id)
        .bind(&evidence_id)
        .fetch_optional(&state.db)
        .await?;
    body.map(Json).ok_or_else(ApiError::not_found)
}

// ---- target upload ----

#[derive(Serialize)]
pub struct TargetOut {
    /// A path inside the project the engine tools can open.
    pub path: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

/// Largest file a tenant may upload to analyze. Real isolation and quotas arrive in Phase 8.
pub const MAX_TARGET_BYTES: usize = 64 * 1024 * 1024;

/// `POST /projects/{id}/targets?name=<filename>`: stores an uploaded file in the project's
/// `targets/` folder and returns a path to analyze. The body is the raw file bytes.
pub async fn upload_target(
    State(state): State<AppState>,
    meta: ClientMeta,
    user: AuthUser,
    Id(project_id): Id<Uuid>,
    Params(q): Params<UploadQuery>,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<TargetOut>), ApiError> {
    use sha2::{Digest, Sha256};
    let access = ProjectAccess::load(&state, project_id, user.user_id).await?;
    rbac::require(access.role, Action::RunTool)?;
    if body.is_empty() {
        return Err(ApiError::invalid("file", "The file is empty."));
    }
    let name = safe_filename(q.name.as_deref().unwrap_or("upload.bin"));
    let root = &state.engine.launch().workspace_root;
    if !crate::engine::worker::is_dir_safe(root, &access.workspace_id.to_string(), &project_id.to_string()) {
        return Err(ApiError::internal("bad workspace root"));
    }
    // Make sure the engine has initialised the project (its `.cany` layout) before we add a
    // `targets/` folder, otherwise the engine would refuse a non-empty, uninitialised folder.
    state.engine.worker_for(&access.workspace_id.to_string(), &project_id.to_string(), access.deep_mode).await.map_err(super::tools::worker_problem)?;
    let dir = root.join(access.workspace_id.to_string()).join(project_id.to_string()).join("targets");
    tokio::fs::create_dir_all(&dir).await.map_err(ApiError::internal)?;
    let unique = format!("{}-{}", Uuid::now_v7().simple(), name);
    let full = dir.join(&unique);
    tokio::fs::write(&full, &body).await.map_err(ApiError::internal)?;
    let sha256 = hex::encode(Sha256::digest(&body));
    audit::record(
        &mut *state.db.acquire().await?,
        Entry {
            workspace_id: Some(access.workspace_id),
            actor_user_id: Some(user.user_id),
            action: "target.upload",
            target_kind: "project",
            target_id: Some(project_id),
            ip_hash: Some(&meta.ip_hash),
            details: json!({"name": name, "size": body.len()}),
        },
    )
    .await
    .ok();
    // The absolute path inside the project's sandbox that engine tools open. The client treats it
    // as an opaque token to pass back as a tool's `path`.
    let path = full.to_string_lossy().into_owned();
    Ok((StatusCode::CREATED, Json(TargetOut { path, name, size: body.len() as u64, sha256 })))
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UploadQuery {
    name: Option<String>,
}

/// Keeps only the base name's safe characters, so an upload can't escape `targets/`.
fn safe_filename(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("upload.bin");
    let cleaned: String = base.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')).take(80).collect();
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() { "upload.bin".to_string() } else { trimmed.to_string() }
}

fn page_text<T>(mut rows: Vec<T>, limit: i64, id: impl Fn(&T) -> String) -> Page<T> {
    let limit = usize::try_from(limit).unwrap_or(0);
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = if more { rows.last().map(&id) } else { None };
    Page { items: rows, next_cursor }
}
