//! Append-only audit log (threat model T21). Entries carry the request ID and never secrets.

use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::request_id;

pub struct Entry<'a> {
    pub workspace_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub action: &'a str,
    pub target_kind: &'a str,
    pub target_id: Option<Uuid>,
    pub ip_hash: Option<&'a str>,
    pub details: Value,
}

pub async fn record(conn: &mut PgConnection, e: Entry<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "insert into audit_logs (workspace_id, actor_user_id, action, target_kind, target_id, request_id, ip_hash, details)
         values ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(e.workspace_id)
    .bind(e.actor_user_id)
    .bind(e.action)
    .bind(e.target_kind)
    .bind(e.target_id)
    .bind(request_id::current().map(|r| r.0))
    .bind(e.ip_hash)
    .bind(e.details)
    .execute(conn)
    .await?;
    Ok(())
}
