//! `/api/v1/tools`: the engine tool catalogue with its hosted-mode policy (screen 20).

use axum::Json;
use axum::extract::State;
use serde_json::Value;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::extract::Id;

pub async fn list(State(state): State<AppState>, _user: AuthUser) -> Result<Json<Vec<Value>>, ApiError> {
    let json = state.engine.catalog_json().await.map_err(worker_problem)?;
    Ok(Json((*json).clone()))
}

pub async fn get(State(state): State<AppState>, _user: AuthUser, Id(id): Id<String>) -> Result<Json<Value>, ApiError> {
    let json = state.engine.catalog_json().await.map_err(worker_problem)?;
    json.iter().find(|t| t.get("id").and_then(Value::as_str) == Some(id.as_str())).cloned().map(Json).ok_or_else(ApiError::not_found)
}

/// A worker/engine failure that isn't a tool error: report it as a 424 the client can retry.
pub fn worker_problem(e: crate::engine::WorkerError) -> ApiError {
    use crate::engine::WorkerError;
    match e {
        WorkerError::Timeout => ApiError::Problem(Box::new(crate::problem::Problem::new(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            "engine_timeout",
            "Engine timed out",
            "The engine took too long. Try again.",
        ))),
        WorkerError::Call { code, message, remediation } => {
            let mut p = crate::problem::Problem::new(axum::http::StatusCode::FAILED_DEPENDENCY, tool_http_code(&code), "Tool error", &message);
            p.field = remediation;
            ApiError::Problem(Box::new(p))
        }
        WorkerError::Spawn(m) | WorkerError::Protocol(m) => ApiError::internal(format!("engine worker: {m}")),
    }
}

/// Maps an engine error code to the API error code clients decide on.
fn tool_http_code(code: &str) -> &'static str {
    match code {
        "consent_required" => "consent_required",
        "not_found" => "tool_not_found",
        "bad_input" => "tool_bad_input",
        "feature_missing" | "engine_missing" | "not_supported" => "tool_unavailable",
        "timed_out" => "tool_timed_out",
        "cancelled" => "tool_cancelled",
        _ => "tool_failed",
    }
}
