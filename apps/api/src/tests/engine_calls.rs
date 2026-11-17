//! Running real engine tools through the HTTP API: policy, approval, evidence and tenant
//! isolation. Skipped unless the `forge-worker` binary is present (built in the same target dir).

use axum::http::{Method, StatusCode};
use serde_json::json;

use super::*;

/// An Android-registered owner plus their personal workspace and a fresh project.
async fn project(app: &TestApp, email: &str, kind: &str) -> (As, String, String) {
    let (who, _, ws) = app.android_user(email).await;
    let r = app.post(&format!("/api/v1/workspaces/{ws}/projects"), &who, json!({"name": "Analysis", "kind": kind})).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    (who, ws, r.body["id"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn the_catalogue_lists_tools_with_their_policy() {
    if !have_worker() {
        eprintln!("skipped: forge-worker not built");
        return;
    }
    let Some(app) = app().await else { return };
    let (who, _, _) = app.android_user("cat@example.com").await;
    let r = app.get("/api/v1/tools", &who).await;
    assert_eq!(r.status, StatusCode::OK);
    let tools = r.body.as_array().unwrap();
    assert!(tools.len() >= 300);
    let by = |id: &str| tools.iter().find(|t| t["id"] == id).cloned().unwrap();
    assert_eq!(by("core_session_show")["policy"], "viewer");
    assert_eq!(by("build_command_run")["policy"], "approval_gate");
    assert_eq!(by("engines_install")["policy"], "operator_only");
    assert_eq!(app.get("/api/v1/tools/core_target_triage", &who).await.body["id"], "core_target_triage");
    assert_eq!(app.get("/api/v1/tools/no_such_tool", &who).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_read_only_tool_runs_and_an_unknown_tool_is_rejected() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (who, _, project) = project(&app, "ro@example.com", "analysis").await;
    let r = app.post(&format!("/api/v1/projects/{project}/tool-calls"), &who, json!({"tool": "core_session_show"})).await;
    assert_eq!((r.status, r.body["status"].as_str()), (StatusCode::OK, Some("succeeded")), "{:?}", r.body);
    assert_eq!(r.body["policy"], "viewer");

    let bad = app.post(&format!("/api/v1/projects/{project}/tool-calls"), &who, json!({"tool": "no_such_tool"})).await;
    assert_eq!((bad.status, bad.body["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("tool")));
}

#[tokio::test]
async fn triage_records_evidence_that_the_evidence_api_returns() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (who, _, project) = project(&app, "ev@example.com", "analysis").await;
    // Turn on Deep mode (triage reads inside the file); the owner may.
    sqlx::query("update projects set deep_mode = true where id = $1::uuid").bind(&project).execute(&app.db).await.unwrap();

    // A real file on disk, analyzed by the real engine.
    let file = std::env::temp_dir().join(format!("forge-target-{}.sh", uuid::Uuid::now_v7().simple()));
    std::fs::write(&file, b"#!/bin/sh\necho hello world\n").unwrap();
    let r = app
        .post(&format!("/api/v1/projects/{project}/tool-calls"), &who, json!({"tool": "core_target_triage", "params": {"path": file.to_str().unwrap()}}))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let ev = r.body["evidence_id"].as_str().expect("evidence id");
    assert!(ev.starts_with("ev_"));

    // The evidence index lists it and returns the full record.
    let list = app.get(&format!("/api/v1/projects/{project}/evidence"), &who).await;
    assert!(list.body["items"].as_array().unwrap().iter().any(|e| e["evidence_id"] == ev));
    let full = app.get(&format!("/api/v1/projects/{project}/evidence/{ev}"), &who).await;
    assert_eq!((full.status, full.body["id"].as_str()), (StatusCode::OK, Some(ev)));
    assert!(full.body["confidence"].is_string());

    // It is recorded as a tool execution.
    let execs = app.get(&format!("/api/v1/projects/{project}/tool-calls"), &who).await;
    assert!(execs.body["items"].as_array().unwrap().iter().any(|e| e["tool"] == "core_target_triage" && e["status"] == "succeeded" && e["evidence_id"] == ev));
    let _ = std::fs::remove_file(&file);
}

#[tokio::test]
async fn a_code_running_tool_needs_an_approval_then_runs() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (who, ws, project) = project(&app, "approve@example.com", "code").await;

    // First attempt: 409 with an approval id.
    let r = app.post(&format!("/api/v1/projects/{project}/tool-calls"), &who, json!({"tool": "build_command_run", "params": {"command": "echo hi"}})).await;
    assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "approval_required"));
    let approval = r.body["field"].as_str().unwrap().to_string();

    // It shows up as pending for the workspace, and is approved by the owner (a maintainer+).
    let pending = app.get(&format!("/api/v1/workspaces/{ws}/approvals"), &who).await;
    assert!(pending.body["items"].as_array().unwrap().iter().any(|a| a["id"].as_str() == Some(approval.as_str()) && a["tool"] == "build_command_run"));
    assert_eq!(app.call(Method::POST, &format!("/api/v1/approvals/{approval}/approve"), &who, None).await.status, StatusCode::NO_CONTENT);

    // With the approval, the call passes the gate (it then needs a build workspace, which is a
    // different, expected error — the point is consent was granted, not refused).
    let run = app
        .post(
            &format!("/api/v1/projects/{project}/tool-calls"),
            &who,
            json!({"tool": "build_command_run", "params": {"command": "echo hi"}, "approval_id": approval}),
        )
        .await;
    assert_ne!(run.code(), "consent_required", "{:?}", run.body);
    assert_ne!(run.code(), "approval_required");

    // The approval is single-use.
    let reuse = app
        .post(
            &format!("/api/v1/projects/{project}/tool-calls"),
            &who,
            json!({"tool": "build_command_run", "params": {"command": "echo hi"}, "approval_id": approval}),
        )
        .await;
    assert_eq!((reuse.status, reuse.code()), (StatusCode::CONFLICT, "approval_invalid"));
}

#[tokio::test]
async fn editors_run_tools_but_viewers_and_non_members_cannot() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (owner, ws, project) = project(&app, "owner-eng@example.com", "analysis").await;
    let (viewer, viewer_id, _) = app.android_user("viewer-eng@example.com").await;
    let (stranger, _, _) = app.android_user("stranger-eng@example.com").await;
    app.post(&format!("/api/v1/workspaces/{ws}/members"), &owner, json!({"email": "viewer-eng@example.com", "role": "viewer"})).await;

    let path = format!("/api/v1/projects/{project}/tool-calls");
    // A viewer may run a read-only tool but not an editor tool.
    assert_eq!(app.post(&path, &viewer, json!({"tool": "core_session_show"})).await.status, StatusCode::OK);
    let editor_tool = app.post(&path, &viewer, json!({"tool": "core_target_open", "params": {"path": "/bin/true"}})).await;
    assert_eq!((editor_tool.status, editor_tool.code()), (StatusCode::FORBIDDEN, "insufficient_role"));
    // A viewer can't approve.
    let r = app.post(&path, &owner, json!({"tool": "build_command_run", "params": {"command": "echo hi"}})).await;
    let approval = r.body["field"].as_str().unwrap();
    assert_eq!(app.call(Method::POST, &format!("/api/v1/approvals/{approval}/approve"), &viewer, None).await.status, StatusCode::FORBIDDEN);

    // A non-member sees nothing.
    assert_eq!(app.post(&path, &stranger, json!({"tool": "core_session_show"})).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get(&format!("/api/v1/projects/{project}/evidence"), &stranger).await.status, StatusCode::NOT_FOUND);
    let _ = viewer_id;
}

#[tokio::test]
async fn deep_mode_needs_an_authorization_and_the_maintainer_role() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (owner, ws, project) = project(&app, "deep@example.com", "analysis").await;

    // Enabling without a stated authorization is refused.
    let bad = app.post(&format!("/api/v1/projects/{project}/deep-mode"), &owner, json!({"enabled": true})).await;
    assert_eq!((bad.status, bad.body["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("authorization")));
    let ok = app.post(&format!("/api/v1/projects/{project}/deep-mode"), &owner, json!({"enabled": true, "authorization": "we own this"})).await;
    assert_eq!((ok.status, ok.body["deep_mode"].as_bool()), (StatusCode::OK, Some(true)));
    // Audited with the stated reason.
    let actions: Vec<String> = sqlx::query_scalar("select action from audit_logs where target_id = $1::uuid and action like 'project.deep%'")
        .bind(&project)
        .fetch_all(&app.db)
        .await
        .unwrap();
    assert_eq!(actions, ["project.deep_mode_on"]);

    // A viewer cannot change it.
    let (viewer, _, _) = app.android_user("deep-viewer@example.com").await;
    app.post(&format!("/api/v1/workspaces/{ws}/members"), &owner, json!({"email": "deep-viewer@example.com", "role": "editor"})).await;
    let forbidden = app.post(&format!("/api/v1/projects/{project}/deep-mode"), &viewer, json!({"enabled": false})).await;
    assert_eq!((forbidden.status, forbidden.code()), (StatusCode::FORBIDDEN, "insufficient_role"));
}

#[tokio::test]
async fn operator_only_and_device_tools_are_refused() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (who, _, project) = project(&app, "refuse-eng@example.com", "analysis").await;
    let path = format!("/api/v1/projects/{project}/tool-calls");
    let op = app.post(&path, &who, json!({"tool": "engines_install", "params": {"id": "chromium"}})).await;
    assert_eq!((op.status, op.code()), (StatusCode::FORBIDDEN, "operator_only"));
    let dev = app.post(&path, &who, json!({"tool": "adb_devices_find"})).await;
    assert_eq!((dev.status, dev.code()), (StatusCode::FAILED_DEPENDENCY, "tool_unavailable"));
}

#[tokio::test]
async fn the_worker_pool_shuts_down_cleanly() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (who, _, project) = project(&app, "shutdown-eng@example.com", "analysis").await;
    assert_eq!(app.post(&format!("/api/v1/projects/{project}/tool-calls"), &who, json!({"tool": "core_session_show"})).await.status, StatusCode::OK);
    // A worker is now warm for the project; stopping the pool stops its child process.
    app.state.engine.shutdown_all().await;
}
