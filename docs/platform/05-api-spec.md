# API specification (v1)

Status: §1–4 **implemented in Phase 2** (see the table in §3a); the rest is proposed. The OpenAPI 3.1 document will be generated from the Axum handlers
with `utoipa` in Phase 2 and published at `/api/v1/openapi.json`. This page fixes the conventions
and the endpoint list.

## 1. Conventions

- Base path `/api/v1`. Breaking changes go to `/api/v2`.
- JSON only (`application/json`), UTF-8. Request body limit 1 MiB by default; uploads use a separate
  endpoint with a per-workspace limit.
- **Request IDs:** the API accepts `X-Request-Id` (if a valid UUID) or generates one, and returns it
  on every response and in every error body and log line.
- **Errors:** RFC 9457 `application/problem+json`:
  ```json
  {"type":"https://docs/errors/consent_required","title":"Approval required","status":409,
   "code":"consent_required","detail":"…","request_id":"…","approval_id":"…"}
  ```
  Engine `ToolError` codes map to HTTP: `bad_input` → 422, `not_found` → 404,
  `consent_required` → 409 (with an `approval_id`), `feature_missing`/`engine_missing` → 424,
  `timed_out` → 504, `cancelled` → 499 (logged), `internal` → 500.
- **Pagination:** cursor-based. `?limit=50&cursor=…`; response `{ "items": [...], "next_cursor": "…" | null }`.
  Maximum limit 200.
- **Validation:** request bodies are typed with serde and `deny_unknown_fields`; field rules
  (lengths, formats) are checked with `validator`. Tool params are passed to the engine, which
  validates them itself.
- **Rate limits:** per IP for auth endpoints (e.g. 10/min on login), per session for the rest
  (token bucket, tower-governor). Headers: `RateLimit-Limit`, `RateLimit-Remaining`, `Retry-After`.
- **Auth:** cookie session (browser and WebView) with `X-CSRF-Token`, or `Authorization: Bearer`
  (native Android). See [01-architecture.md §4](01-architecture.md#4-authentication-model).
- **IDs** in paths are always re-authorized; a resource the caller cannot see returns 404, not 403.

## 2. Health and meta
| Method | Path | Notes |
|---|---|---|
| GET | `/healthz` | Liveness, no auth |
| GET | `/readyz` | DB reachable, migrations current, worker supervisor reachable |
| GET | `/api/v1/meta` | Version, features enabled, auth methods |
| GET | `/api/v1/openapi.json` | Generated spec |

## 3. Auth, sessions, devices
| Method | Path |
|---|---|
| POST | `/auth/register` · `/auth/login` · `/auth/logout` · `/auth/refresh` |
| POST | `/auth/mfa/totp/setup` · `/auth/mfa/totp/verify` · `/auth/mfa/totp/disable` |
| POST | `/auth/login-code` (native: get a single-use WebView code) · `/auth/exchange` (WebView: code → cookies) |
| GET | `/me` · `/me/sessions` · `/me/devices` |
| DELETE | `/me/sessions/{id}` · `/me/devices/{id}` |

## 3a. Implemented in Phase 2

| Method | Path | Who | Notes |
|---|---|---|---|
| GET | `/healthz` | anyone | liveness |
| GET | `/readyz` | anyone | 503 `not_ready` if the DB is unreachable or a migration is pending |
| POST | `/api/v1/auth/register` | anyone | `{email, password, display_name, client?: "web"\|"android", device_name?}` → 201. Creates a personal workspace (owner) |
| POST | `/api/v1/auth/login` | anyone | same `client` rule; 401 `invalid_credentials` for a wrong password and an unknown email alike |
| POST | `/api/v1/auth/refresh` | refresh token | web: cookie + `X-CSRF-Token`, no body; android: `{refresh_token}`. Rotates; a token reused more than 30 s after its use revokes the session (401 `refresh_reused`); within 30 s it rotates again (lost-answer retry) |
| POST | `/api/v1/auth/logout` | signed in | revokes this session, clears cookies, 204 |
| GET | `/api/v1/me` | signed in | |
| GET | `/api/v1/me/sessions` | signed in | live sessions with device and `current` |
| DELETE | `/api/v1/me/sessions/{id}` | signed in | own sessions only |
| GET, POST | `/api/v1/workspaces` | signed in | list (paged) the caller's workspaces; create `{name, slug}` |
| GET, PATCH, DELETE | `/api/v1/workspaces/{ws}` | viewer / admin / owner | |
| GET, POST | `/api/v1/workspaces/{ws}/members` | viewer / admin | add `{email, role}` |
| PATCH, DELETE | `/api/v1/workspaces/{ws}/members/{user}` | admin (self for leaving) | admins act only below their own role; the last owner can't be demoted or leave (409 `last_owner`) |
| GET, POST | `/api/v1/workspaces/{ws}/projects` | viewer / maintainer | `?include_archived=true`; create `{name, kind: code\|analysis\|rebuild}` |
| GET, PATCH, DELETE | `/api/v1/projects/{id}` | viewer / maintainer / admin | PATCH `{name?, archived?}` |

Session delivery: web responses set `forge_at` (HttpOnly, `Path=/api`, 10 min), `forge_rt`
(HttpOnly, `Path=/api/v1/auth`, 30 days) and `forge_csrf` (readable, sent back as
`X-CSRF-Token` on every non-GET request), all `SameSite=Strict; Secure`. Android responses carry
`access_token` and `refresh_token` in the body. Error codes added: `unauthenticated`,
`invalid_credentials`, `invalid_refresh`, `refresh_reused`, `account_locked`, `csrf_failed`,
`insufficient_role`, `email_taken`, `slug_taken`, `already_member`, `last_owner`,
`rate_limited` (429 with `Retry-After`), `unsupported_media_type`, `too_large`.

Not yet: OpenAPI generation, MFA, `/auth/login-code` + `/auth/exchange` (WebView, Phase 4),
`/me/devices`, project members, Deep mode endpoint (Phase 5).

## 4. Workspaces, projects, members
| Method | Path |
|---|---|
| GET, POST | `/workspaces` |
| GET, PATCH, DELETE | `/workspaces/{ws}` |
| GET, POST, PATCH, DELETE | `/workspaces/{ws}/members[/{user}]` |
| GET, POST | `/workspaces/{ws}/projects` |
| GET, PATCH, DELETE | `/projects/{p}` |
| GET, POST, PATCH, DELETE | `/projects/{p}/members[/{user}]` |
| POST | `/projects/{p}/deep-mode` (body: `enabled`, `authorization` statement; maintainer+) |

## 5. Files and editor
| Method | Path | Notes |
|---|---|---|
| GET | `/projects/{p}/files?path=&depth=` | Tree listing |
| GET | `/projects/{p}/files/content?path=` | Returns content and `etag` |
| PUT | `/projects/{p}/files/content?path=` | Requires `If-Match: <etag>`; 412 on conflict |
| POST | `/projects/{p}/files/move` · `/files/delete` · `/files/upload` | |
| GET | `/projects/{p}/search?q=&glob=` | |

## 6. Engine tools and evidence
| Method | Path | Notes |
|---|---|---|
| GET | `/tools` | Catalogue: id, title, summary, toolset, effects, requires, platform policy |
| GET | `/tools/{id}` | Includes the JSON Schema |
| POST | `/projects/{p}/tool-calls` | `{tool, params}` → 200 with result (sync tools), 202 with `job_id` (job tools), or 409 with `approval_id` |
| GET | `/projects/{p}/tool-calls[/{id}]` | History |
| POST | `/projects/{p}/tool-calls/{id}/cancel` | |
| GET | `/projects/{p}/evidence?operation=&q=` | |
| GET | `/projects/{p}/evidence/{ev_id}` | Full evidence record |
| GET | `/projects/{p}/targets` · POST `/projects/{p}/targets` (upload a file to analyze) | |

## 7. Approvals
| Method | Path |
|---|---|
| GET | `/workspaces/{ws}/approvals?status=pending` |
| POST | `/approvals/{id}/approve` · `/approvals/{id}/reject` |

## 8. AI and agents
| Method | Path |
|---|---|
| GET, POST, PATCH, DELETE | `/workspaces/{ws}/providers[/{id}]` (API keys write-only; responses show only the last 4 characters) |
| POST | `/workspaces/{ws}/providers/{id}/test` |
| GET, POST | `/projects/{p}/conversations` · `/conversations/{id}/messages` |
| GET, POST, PATCH | `/workspaces/{ws}/agents[/{id}]` |
| POST | `/projects/{p}/plans` (goal → draft plan) · `/plans/{id}/approve` · `/plans/{id}/cancel` |
| GET | `/plans/{id}` · `/plans/{id}/tasks` · `/tasks/{id}` · `/tasks/{id}/events?after_seq=` |
| POST | `/tasks/{id}/retry` · `/tasks/{id}/cancel` |

## 9. Terminal
| Method | Path |
|---|---|
| POST | `/projects/{p}/terminals` → `{id, ws_url}` |
| GET | `/projects/{p}/terminals` |
| DELETE | `/terminals/{id}` |
| WS | `/terminals/{id}/stream` (binary frames: input/output; JSON control frames: resize, ping, approval prompts) |

## 10. Git
| Method | Path |
|---|---|
| GET, POST, DELETE | `/workspaces/{ws}/git-connections[/{id}]` |
| GET, POST | `/projects/{p}/repos` (connect or clone) |
| GET | `/repos/{r}/status` · `/repos/{r}/branches` · `/repos/{r}/commits?ref=` · `/repos/{r}/diff?base=&head=` · `/repos/{r}/tags` |
| POST | `/repos/{r}/fetch` · `/pull` · `/push` · `/commits` · `/branches` · `/checkout` · `/merge` · `/tags` · `/remotes` |

Push, force operations and merges into the default branch create an approval when the project
requires one.

## 11. Builds and artifacts
| Method | Path |
|---|---|
| GET, POST, PATCH, DELETE | `/projects/{p}/build-configs[/{id}]` |
| POST | `/projects/{p}/builds` (start) · `/builds/{id}/cancel` · `/builds/{id}/retry` |
| GET | `/projects/{p}/builds` · `/builds/{id}` · `/builds/{id}/logs?after_seq=` · `/builds/{id}/tests` |
| GET | `/projects/{p}/artifacts` · `/artifacts/{id}` · `/artifacts/{id}/download` (short-lived signed URL) |

## 12. Preview, MCP, usage, notifications, audit
| Method | Path |
|---|---|
| POST | `/projects/{p}/previews` → preview URL on a separate sandbox origin |
| GET, POST, DELETE | `/workspaces/{ws}/mcp-connections[/{id}]` |
| GET | `/workspaces/{ws}/usage?from=&to=&group_by=` |
| GET | `/me/notifications` · POST `/me/notifications/{id}/read` · POST `/me/push-tokens` |
| GET | `/workspaces/{ws}/audit-logs` (admin+) |
| GET | `/me/diagnostics` (versions, connectivity checks, recent errors with request IDs) |

## 13. Real-time events

- **SSE:** `GET /api/v1/events?topics=job:{id},task:{id},project:{p}` with `Last-Event-ID`.
- **WebSocket:** `GET /api/v1/ws` → client sends `{"op":"subscribe","topic":"job:…","after_seq":N}`.
- Event envelope:
  ```json
  {"topic":"job:…","seq":42,"kind":"agent.tool_finished","ts":"…","payload":{…}}
  ```
- `kind` values: `job.status`, `job.log`, `tool.progress`, `tool.result`, `agent.<upstream AgentEvent
  type>`, `approval.requested`, `approval.decided`, `build.log`, `build.status`, `notification`.
- Every subscription is authorized against the topic's workspace when it is opened.
