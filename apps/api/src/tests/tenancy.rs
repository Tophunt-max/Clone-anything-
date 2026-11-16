//! Tenant isolation and RBAC (threat model T5): a user sees and changes only what their
//! workspace memberships allow, whatever IDs they send.

use axum::http::{Method, StatusCode};
use serde_json::json;

use super::*;

#[tokio::test]
async fn users_only_see_their_own_workspaces_and_projects() {
    let Some(app) = app().await else { return };
    let (a, _, a_ws) = app.android_user("alice@example.com").await;
    let (b, _, b_ws) = app.android_user("bob@example.com").await;
    let a_project = app.project(&a, &a_ws, "Alice's app").await;

    let b_list = app.get("/api/v1/workspaces", &b).await;
    let ids: Vec<&str> = b_list.body["items"].as_array().unwrap().iter().filter_map(|w| w["id"].as_str()).collect();
    assert_eq!(ids, [b_ws.as_str()]);

    // Every way Bob could reach Alice's things answers 404, the same as an ID that doesn't exist.
    let missing = uuid::Uuid::now_v7();
    for (method, path, body) in [
        (Method::GET, format!("/api/v1/workspaces/{a_ws}"), None),
        (Method::PATCH, format!("/api/v1/workspaces/{a_ws}"), Some(json!({"name": "mine now"}))),
        (Method::DELETE, format!("/api/v1/workspaces/{a_ws}"), None),
        (Method::GET, format!("/api/v1/workspaces/{a_ws}/members"), None),
        (Method::POST, format!("/api/v1/workspaces/{a_ws}/members"), Some(json!({"email": "bob@example.com", "role": "owner"}))),
        (Method::GET, format!("/api/v1/workspaces/{a_ws}/projects"), None),
        (Method::POST, format!("/api/v1/workspaces/{a_ws}/projects"), Some(json!({"name": "x", "kind": "code"}))),
        (Method::GET, format!("/api/v1/projects/{a_project}"), None),
        (Method::PATCH, format!("/api/v1/projects/{a_project}"), Some(json!({"name": "stolen"}))),
        (Method::DELETE, format!("/api/v1/projects/{a_project}"), None),
        (Method::GET, format!("/api/v1/projects/{missing}"), None),
        (Method::GET, format!("/api/v1/workspaces/{missing}"), None),
    ] {
        let r = app.call(method.clone(), &path, &b, body).await;
        assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"), "{method} {path}");
    }

    // Nothing changed for Alice.
    let p = app.get(&format!("/api/v1/projects/{a_project}"), &a).await;
    assert_eq!((p.status, p.body["name"].as_str()), (StatusCode::OK, Some("Alice's app")));
    let members: i64 = sqlx::query_scalar("select count(*) from workspace_members where workspace_id = $1::uuid").bind(&a_ws).fetch_one(&app.db).await.unwrap();
    assert_eq!(members, 1);
}

#[tokio::test]
async fn roles_decide_what_members_can_do() {
    let Some(app) = app().await else { return };
    let (owner, _, ws) = app.android_user("owner@example.com").await;
    let (bob, bob_id, _) = app.android_user("bob2@example.com").await;
    let project = app.project(&owner, &ws, "Shared").await;
    let members = format!("/api/v1/workspaces/{ws}/members");

    let r = app.post(&members, &owner, json!({"email": "BOB2@example.com", "role": "viewer"})).await;
    assert_eq!((r.status, r.body["role"].as_str()), (StatusCode::CREATED, Some("viewer")));
    assert_eq!(app.post(&members, &owner, json!({"email": "bob2@example.com", "role": "viewer"})).await.code(), "already_member");

    // Viewer: can read, can't write.
    assert_eq!(app.get(&format!("/api/v1/projects/{project}"), &bob).await.status, StatusCode::OK);
    let r = app.post(&format!("/api/v1/workspaces/{ws}/projects"), &bob, json!({"name": "x", "kind": "code"})).await;
    assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "insufficient_role"));
    let r = app.call(Method::PATCH, &format!("/api/v1/projects/{project}"), &bob, Some(json!({"name": "renamed"}))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    // Maintainer: creates and edits projects, but can't delete them or manage members.
    let r = app.call(Method::PATCH, &format!("{members}/{bob_id}"), &owner, Some(json!({"role": "maintainer"}))).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let mine = app.project(&bob, &ws, "Bob's").await;
    assert_eq!(app.call(Method::PATCH, &format!("/api/v1/projects/{project}"), &bob, Some(json!({"archived": true}))).await.status, StatusCode::OK);
    assert_eq!(app.call(Method::DELETE, &format!("/api/v1/projects/{mine}"), &bob, None).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.post(&members, &bob, json!({"email": "owner@example.com", "role": "viewer"})).await.status, StatusCode::FORBIDDEN);

    // Admin: can't hand out admin or owner, or touch the owner.
    app.call(Method::PATCH, &format!("{members}/{bob_id}"), &owner, Some(json!({"role": "admin"}))).await;
    let (carol, carol_id, _) = app.android_user("carol@example.com").await;
    assert_eq!(app.post(&members, &bob, json!({"email": "carol@example.com", "role": "admin"})).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.post(&members, &bob, json!({"email": "carol@example.com", "role": "editor"})).await.status, StatusCode::CREATED);
    let owner_id = app.get("/api/v1/me", &owner).await.body["id"].as_str().unwrap().to_string();
    assert_eq!(app.call(Method::DELETE, &format!("{members}/{owner_id}"), &bob, None).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.call(Method::DELETE, &format!("/api/v1/projects/{mine}"), &bob, None).await.status, StatusCode::NO_CONTENT);

    // Members can leave on their own.
    assert_eq!(app.call(Method::DELETE, &format!("{members}/{carol_id}"), &carol, None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get(&format!("/api/v1/projects/{project}"), &carol).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_workspace_always_keeps_an_owner() {
    let Some(app) = app().await else { return };
    let (owner, owner_id, ws) = app.android_user("solo@example.com").await;
    let members = format!("/api/v1/workspaces/{ws}/members");
    let r = app.call(Method::PATCH, &format!("{members}/{owner_id}"), &owner, Some(json!({"role": "admin"}))).await;
    assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "last_owner"));
    let r = app.call(Method::DELETE, &format!("{members}/{owner_id}"), &owner, None).await;
    assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "last_owner"));

    let (dana, dana_id, _) = app.android_user("dana@example.com").await;
    app.post(&members, &owner, json!({"email": "dana@example.com", "role": "owner"})).await;
    assert_eq!(app.call(Method::PATCH, &format!("{members}/{owner_id}"), &owner, Some(json!({"role": "viewer"}))).await.status, StatusCode::OK);
    let r = app.call(Method::DELETE, &format!("{members}/{dana_id}"), &dana, None).await;
    assert_eq!(r.code(), "last_owner", "the remaining owner can't leave either");
}

#[tokio::test]
async fn only_owners_delete_workspaces() {
    let Some(app) = app().await else { return };
    let (owner, _, ws) = app.android_user("del@example.com").await;
    let (eve, eve_id, _) = app.android_user("eve@example.com").await;
    let members = format!("/api/v1/workspaces/{ws}/members");
    app.post(&members, &owner, json!({"email": "eve@example.com", "role": "admin"})).await;
    let project = app.project(&owner, &ws, "Doomed").await;
    assert_eq!(app.call(Method::DELETE, &format!("/api/v1/workspaces/{ws}"), &eve, None).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.call(Method::DELETE, &format!("/api/v1/workspaces/{ws}"), &owner, None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get(&format!("/api/v1/projects/{project}"), &owner).await.status, StatusCode::NOT_FOUND);
    let left: i64 = sqlx::query_scalar("select count(*) from workspace_members where user_id = $1::uuid").bind(&eve_id).fetch_one(&app.db).await.unwrap();
    assert_eq!(left, 1, "eve keeps only her personal workspace");
}

#[tokio::test]
async fn lists_are_paged_and_archived_projects_hidden() {
    let Some(app) = app().await else { return };
    let (a, _, ws) = app.android_user("pages@example.com").await;
    let ids: Vec<String> = futures_ids(&app, &a, &ws, 3).await;
    let base = format!("/api/v1/workspaces/{ws}/projects");
    let p1 = app.get(&format!("{base}?limit=2"), &a).await;
    assert_eq!(p1.body["items"].as_array().unwrap().len(), 2);
    let cursor = p1.body["next_cursor"].as_str().unwrap().to_string();
    let p2 = app.get(&format!("{base}?limit=2&cursor={cursor}"), &a).await;
    assert_eq!(p2.body["items"].as_array().unwrap().len(), 1);
    assert!(p2.body["next_cursor"].is_null());
    let seen: Vec<String> =
        p1.body["items"].as_array().unwrap().iter().chain(p2.body["items"].as_array().unwrap()).map(|p| p["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(seen, ids, "in creation order, nothing repeated or lost");

    app.call(Method::PATCH, &format!("/api/v1/projects/{}", ids[0]), &a, Some(json!({"archived": true}))).await;
    assert_eq!(app.get(&base, &a).await.body["items"].as_array().unwrap().len(), 2);
    assert_eq!(app.get(&format!("{base}?include_archived=true"), &a).await.body["items"].as_array().unwrap().len(), 3);
}

async fn futures_ids(app: &TestApp, who: &As, ws: &str, n: usize) -> Vec<String> {
    let mut ids = Vec::new();
    for i in 0..n {
        ids.push(app.project(who, ws, &format!("P{i}")).await);
    }
    ids
}

#[tokio::test]
async fn workspace_slugs_are_unique_and_changes_are_audited() {
    let Some(app) = app().await else { return };
    let (a, _, _) = app.android_user("slug@example.com").await;
    let r = app.post("/api/v1/workspaces", &a, json!({"name": "Team", "slug": "team-one"})).await;
    assert_eq!((r.status, r.body["role"].as_str()), (StatusCode::CREATED, Some("owner")));
    let (b, _, _) = app.android_user("slug2@example.com").await;
    let r = app.post("/api/v1/workspaces", &b, json!({"name": "Team", "slug": "team-one"})).await;
    assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "slug_taken"));
    let ws = app.get("/api/v1/workspaces", &a).await.body["items"][1]["id"].as_str().unwrap().to_string();
    app.project(&a, &ws, "Audited").await;
    let actions: Vec<String> =
        sqlx::query_scalar("select action from audit_logs where workspace_id = $1::uuid order by id").bind(&ws).fetch_all(&app.db).await.unwrap();
    assert_eq!(actions, ["workspace.create", "project.create"]);
}
