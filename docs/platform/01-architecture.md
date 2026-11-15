# Architecture

Status: **proposed** (Phase 0). Nothing in this document is implemented yet.

## 1. Overview

```
 ┌──────────────────────┐     ┌──────────────────────────────┐
 │ Android app (Kotlin) │     │ Browser (desktop / mobile)   │
 │ Compose shell        │     │                              │
 │  └ WebView ──────────┼──┐  │  React app (apps/web)        │
 └─────────┬────────────┘  │  └──────────────┬───────────────┘
           │ REST (Bearer)  │ same React app  │ REST + SSE/WebSocket (cookie session)
           ▼                ▼                 ▼
 ┌──────────────────────────────────────────────────────────────┐
 │ apps/api  (Rust, Axum, Tokio)                                 │
 │ auth · RBAC · projects · approvals · jobs · events · audit    │
 └──────┬───────────────────────────┬───────────────────────────┘
        │ SQLx                       │ worker protocol (JSON lines over a Unix socket)
        ▼                            ▼
 ┌──────────────┐    ┌──────────────────────────────────────────────────┐
 │ PostgreSQL   │    │ apps/workers (Rust), one sandbox per workspace   │
 │ state, queue,│    │  role engine   : Engine + AiHub (original crates) │
 │ events       │    │  role terminal : PTY sessions                     │
 └──────────────┘    │  role build    : build/test runners               │
                     │ non-root · CPU/mem/pids quotas · egress proxy     │
                     └──────────────────────────────────────────────────┘
```

## 2. Components

### apps/api: the control plane
Axum on Tokio. It is the only component clients talk to. Responsibilities: authentication,
sessions and devices, RBAC, project and workspace management, tool-call authorization and
approvals, the job queue, event fan-out (SSE and WebSocket), Git and build orchestration,
artifacts, usage, notifications, audit logging. It never runs engine tools or user code itself.

### apps/workers: the data plane
A single Rust binary (`craft-worker`) with three roles, started by a supervisor inside an
isolated sandbox per workspace:

- **engine**: links the original crates (`engine`, `ai`, `platform`, `engines`). Owns exactly
  one `Engine` and one `AiHub` for one workspace. Each `Engine::call` runs on a blocking thread;
  `Conversation::poll()` is drained on a timer and forwarded as events.
- **terminal**: PTY sessions using `portable-pty` (already a dependency of `crates/platform`).
- **build**: runs build and test commands for a configured toolchain image, streams logs,
  collects artifacts.

Why a separate process per workspace instead of an `Engine` inside the API:

| Constraint found in the audit | Consequence |
|---|---|
| One `Mutex<Session>` per `Engine` | One engine per workspace, otherwise tenants block each other |
| Config dir, engine store and keys come from process environment and home paths | Tenants must not share a process |
| No OS sandbox; tools can run foreign code and external programs | The process must be inside a container with quotas and no host access |
| Blocking `ureq`, threads, `catch_unwind` | Fine in a dedicated process; a panic or hang does not take down the API |

### Worker protocol
JSON lines over a Unix domain socket mounted into the sandbox. Messages:

- `tool.call {call_id, tool, params, consent, deadline_ms}` → `tool.progress` … `tool.result`
- `agent.send / agent.consent / agent.stop` → `agent.event` (an upstream `AgentEvent`, unchanged)
- `cancel {call_id}`, `health`, `shutdown`

The API is the only side that decides `consent`. Workers have no database credentials.

### PostgreSQL
System of record, job queue (`SELECT … FOR UPDATE SKIP LOCKED`), and event log. A Redis-compatible
queue is not planned; add it only if measured queue load requires it.

### Event delivery
Every progress item is appended to `job_events` with a per-job sequence number, then pushed via
`LISTEN/NOTIFY` to API instances. Clients subscribe over SSE (`Last-Event-ID` = sequence) or
WebSocket (resume from `after_seq`). A reconnecting client replays missed events from the table,
so long-running jobs survive client disconnects and API restarts.

## 3. Frontends

- **apps/web** (React, TypeScript, Vite): the full product. It runs on its own in any modern browser
  and is also what the Android WebView loads. Layouts: one column below 768 px, two panels from
  768 px, a multi-panel workspace from 1280 px.
- **apps/android** (Kotlin, Compose, Material 3): native shell for splash, onboarding, server
  connection, login/registration, the dashboard shell, project list, native settings, permissions,
  file picker, notifications, artifact sharing, offline state and diagnostics. Project screens open
  the web app in a WebView restricted to one trusted origin.
- **Shared packages**: `packages/design-tokens` (one JSON source generating CSS variables, a
  Tailwind preset and Kotlin constants), `packages/api-client` (generated from the OpenAPI spec),
  `packages/websocket-client` (reconnect and resume), `packages/shared-types`,
  `packages/validation` (zod schemas), `packages/ui`, `packages/editor` (Monaco wrapper).

## 4. Authentication model

- Passwords hashed with Argon2id. Optional TOTP MFA.
- Access token: signed JWT (EdDSA), 10-minute lifetime, carries user id, session id, device id.
- Refresh token: opaque random 256-bit value, stored hashed, rotated on every use. Reuse of an old
  token revokes the whole session family.
- **Browser**: the API sets the refresh token and a short-lived session cookie as `HttpOnly`,
  `Secure`, `SameSite=Strict` cookies; the React app never sees a token. State-changing requests
  also carry a CSRF header (double-submit).
- **Android native screens**: Retrofit uses Bearer tokens. The refresh token lives in DataStore
  encrypted with an Android Keystore key.
- **Android WebView**: native code asks the API for a single-use, 60-second login code, then loads
  the web origin's `/auth/exchange` endpoint with the code in a POST body. The API answers with the
  same HttpOnly cookies a browser gets. No token is placed in a URL, in `localStorage`, or in any
  JavaScript-readable place.

## 5. Authorization model

Request → authenticated user → workspace membership → project membership → role → permission →
for tool calls, the tool's policy (from [03-tool-inventory.md](03-tool-inventory.md)) →
optional human approval → worker call. Roles: `owner`, `admin`, `maintainer`, `editor`,
`viewer`. Identifiers in the path or body are always re-checked against the session's memberships
on the server. PostgreSQL row-level security on tenant tables is a second line of defence.

Deep mode (reading inside a target) is a per-project setting that only `maintainer` and above can
enable. Enabling it records who did it and the stated authorization in the audit log.

## 6. Multi-agent orchestration

Built on the upstream agent loop, not a replacement for it.

- A **plan** is a DAG of tasks. The Planner role produces it; a person can edit and approve it.
- Each task is assigned a role (Planner, Architect, Research, Reverse Engineering, Android, Backend,
  Frontend, Rust, Database, Security Reviewer, Code Reviewer, QA, Build Engineer, Documentation).
  A role is a system prompt, a toolset allowlist and a model choice. Upstream roles (Lead, Worker,
  Reviewer, Builder, Extractor) are the execution mechanism underneath.
- Tasks with no dependency between them run in parallel, each in its own Git worktree and branch
  inside the workspace sandbox. Results merge back through a review task; conflicts stop the plan
  and ask a person.
- Every tool call from an agent passes the same policy and approval checks as a person's.
- Token use and cost come from upstream `TurnDone` events and are stored per task.

## 7. Isolation and deployment (target host: Ubuntu 24.04 LTS)

Docker Compose to start: reverse proxy (TLS, security headers, request size limits), `api`,
`postgres`, `worker-supervisor`, and monitoring (Prometheus + Grafana, Loki for logs). Workspace
sandboxes are rootless containers started by the supervisor:

- non-root UID, read-only root filesystem, one writable workspace volume;
- `--cpus`, `--memory`, `--pids-limit`, disk quota per volume;
- no Docker socket, no host mounts, no host network; egress only through an allowlisting proxy;
- gVisor (`runsc`) as the container runtime where available;
- idle timeout, then stop; volumes kept per retention policy.

The React app is deployed as static files (CDN or the reverse proxy). Android APK/AAB is built in
GitHub Actions with a pinned Gradle wrapper and signed with keys held in CI secrets.

## 8. Alternatives considered

| Decision | Chosen | Rejected and why |
|---|---|---|
| Engine placement | Per-workspace worker process | In-process in the API: shared session and global state, no isolation |
| Engine transport | Own JSON-lines protocol calling `Engine::call` | Upstream MCP server: consent only server-wide, no per-call approval, no `AiHub` events |
| Queue | PostgreSQL `SKIP LOCKED` | Redis: extra service before there is load to justify it |
| Web auth | HttpOnly cookies | Tokens in JS memory or storage: readable by any XSS |
| WebView bridge | `WebViewCompat.addWebMessageListener` with an origin allowlist | `addJavascriptInterface`: injected into every frame and origin |
| Non-Rust apps location | `apps/web`, `apps/android`, excluded from Cargo | `clients/…`: avoids touching upstream files but departs from the agreed layout |
