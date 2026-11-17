//! Uploading a target and analyzing it through the engine (the Binary Analyzer flow).

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use tower::ServiceExt;

use super::*;

impl TestApp {
    async fn upload(&self, who: &As, project: &str, name: &str, bytes: &[u8]) -> Resp {
        let As::Bearer(token) = who else { panic!("upload needs a bearer identity") };
        let req = Request::post(format!("/api/v1/projects/{project}/targets?name={name}"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(bytes.to_vec()))
            .unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let b = http_body_util::BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
        Resp { status, headers, body: serde_json::from_slice(&b).unwrap_or(serde_json::Value::Null) }
    }
}

#[tokio::test]
async fn upload_a_target_then_triage_it_and_see_the_evidence() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (who, _, ws) = app.android_user("analyze@example.com").await;
    let project =
        app.post(&format!("/api/v1/workspaces/{ws}/projects"), &who, json!({"name": "A", "kind": "analysis"})).await.body["id"].as_str().unwrap().to_string();
    sqlx::query("update projects set deep_mode = true where id = $1::uuid").bind(&project).execute(&app.db).await.unwrap();

    // A real ELF: read the test binary itself so the native parser has something to chew on.
    let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let up = app.upload(&who, &project, "sample.bin", &bytes[..bytes.len().min(2 * 1024 * 1024)]).await;
    assert_eq!(up.status, StatusCode::CREATED, "{:?}", up.body);
    let path = up.body["path"].as_str().unwrap().to_string();
    assert!(path.contains("/targets/") && path.ends_with("sample.bin") && up.body["sha256"].as_str().unwrap().len() == 64);

    let run = app.post(&format!("/api/v1/projects/{project}/tool-calls"), &who, json!({"tool": "core_target_triage", "params": {"path": path}})).await;
    assert_eq!(run.status, StatusCode::OK, "{:?}", run.body);
    assert!(run.body["evidence_id"].as_str().unwrap().starts_with("ev_"));
    assert!(
        app.get(&format!("/api/v1/projects/{project}/evidence"), &who).await.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["evidence_id"] == run.body["evidence_id"])
    );
}

#[tokio::test]
async fn uploads_are_bounded_and_the_filename_is_made_safe() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (who, _, ws) = app.android_user("upload2@example.com").await;
    let project =
        app.post(&format!("/api/v1/workspaces/{ws}/projects"), &who, json!({"name": "A", "kind": "analysis"})).await.body["id"].as_str().unwrap().to_string();

    // A path-traversal name is reduced to a plain base name under targets/.
    let up = app.upload(&who, &project, "..%2F..%2Fetc%2Fpasswd", b"data").await;
    // (the query value is percent-decoded by axum, giving "../../etc/passwd")
    assert_eq!(up.status, StatusCode::CREATED, "{:?}", up.body);
    let path = up.body["path"].as_str().unwrap();
    assert!(path.contains("/targets/") && !path.contains("/..") && path.ends_with("passwd"), "{path}");

    assert_eq!(app.upload(&who, &project, "empty.bin", b"").await.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn viewers_cannot_upload() {
    if !have_worker() {
        return;
    }
    let Some(app) = app().await else { return };
    let (owner, _, ws) = app.android_user("up-owner@example.com").await;
    let project =
        app.post(&format!("/api/v1/workspaces/{ws}/projects"), &owner, json!({"name": "A", "kind": "analysis"})).await.body["id"].as_str().unwrap().to_string();
    let (viewer, _, _) = app.android_user("up-viewer@example.com").await;
    app.post(&format!("/api/v1/workspaces/{ws}/members"), &owner, json!({"email": "up-viewer@example.com", "role": "viewer"})).await;
    assert_eq!(app.upload(&viewer, &project, "x.bin", b"data").await.status, StatusCode::FORBIDDEN);
}
