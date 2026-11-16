# Development roadmap and status

Feature status uses exactly these labels: **Implemented and verified**, **Implemented but not
live-tested**, **Partially implemented**, **Blocked**, **Not implemented**.

## Phases

Each phase is one branch and one pull request. Every phase ends with: tests run, regressions fixed,
docs updated, this file updated (completed work, blockers, untested features).

| Phase | Scope | Exit criteria | Can be verified in this sandbox? |
|---|---|---|---|
| 0 | Repository audit and feasibility | Audit, architecture, integration map, tool inventory, DB design, API spec, UI plan, threat model, roadmap | Yes |
| 1 | Monorepo skeleton, design tokens, CI | Cargo `exclude` + xtask skip list; pnpm workspace; tokens generate CSS/Tailwind/Kotlin; CI runs Rust, web and Android jobs | Rust + web yes; Android only in CI |
| 2 | Axum API, auth, PostgreSQL | Migrations, register/login/refresh/revoke, RBAC, tenant isolation tests, OpenAPI, health, rate limits | Yes: portable PostgreSQL 16.15 runs here (Phase 2) |
| 3 | React app and API integration | Auth flows, dashboard, projects, settings, notifications, help; responsive tests | Yes |
| 4 | Android shell and secure WebView | Native screens, WebView bridge with security tests, back navigation, offline | Build, unit and Robolectric Compose UI tests yes; a real WebView on a device/emulator no |
| 5 | Engine adapter and tool registry | `forge-worker` engine role, worker protocol, policy + approvals, evidence index, analyzers (screens 16–20) | Engine yes; sandbox isolation no |
| 6 | AI assistant and multi-agent orchestration | Providers, chat, plans, tasks, worktrees, streaming, cost tracking | Partly (needs a real AI key for live tests) |
| 7 | Monaco editor, terminal, Git | Editor with ETag conflicts, PTY terminal, Git operations with credentials | Partly (terminal isolation needs containers) |
| 8 | Build workers and artifacts | Queue, toolchain images, logs, tests, artifacts, quotas | CI / target host only |
| 9 | Security review, testing, monitoring | Threat-model controls tested, E2E suite, backup/restore test, dashboards | Partly |
| 10 | Deployment and release | Compose stack on Ubuntu 24.04, TLS, runbooks, signed Android release | Target host only |

## Current status (end of Phase 4)

### Phase 0
| Item | Status |
|---|---|
| Upstream imported with full history | Implemented and verified |
| Upstream headless test suite (4,025 passed, 4 environment-specific failures explained) | Implemented and verified |
| Tool inventory generated from the registry (326 tools) | Implemented and verified |
| Phase 0 documents | Implemented and verified (reviewed and accepted) |

### Phase 1
| Item | Status | Evidence |
|---|---|---|
| Product name **Tophunt Forge**, Android id `com.tophunt.forge` | Implemented and verified | `PRODUCT_NAME` in `apps/api/src/lib.rs`, `apps/android/app/build.gradle.kts` |
| Cargo `exclude` for `apps/web`, `apps/android`; xtask walks skip them | Implemented and verified | `cargo xtask layers`, `locks`, `status`, `i18n-coverage`, `secrets` pass with `node_modules` present; xtask tests 16/16 |
| `apps/api` (forge-api): `/healthz`, `/api/v1/meta`, request IDs, problem+json errors, body limit, timeout, graceful shutdown, env config | Implemented and verified | 8 unit tests; binary run: health/meta/404 answered, SIGTERM shut down cleanly, bad config exit 2 |
| `apps/workers` (forge-worker): links the original engine, `catalog` command | Implemented and verified | 2 tests; `catalog` output renders a byte-identical inventory to Phase 0 |
| Standalone inventory exporter replaced by `forge-worker catalog` | Implemented and verified | same output |
| `packages/design-tokens`: one JSON → CSS variables, Tailwind v4 theme, Kotlin `ForgeTokens` | Implemented and verified | 7 tests incl. WCAG AA contrast for 16 pairs × 2 themes; `--check` for stale output |
| `packages/shared-types`, `packages/api-client` | Implemented and verified | api-client 4 tests (request ID, cookies, problem → `ApiError`, network errors) |
| `apps/web` skeleton: responsive shell, home page reading `/api/v1/meta` with loading/error/retry, 404 route | Implemented and verified (unit level) | 4 tests; `vite build` succeeds. Not yet tested in a real browser at the three breakpoints (Playwright arrives in Phase 3) |
| `apps/android` skeleton: Compose + Material 3 theme from tokens | Implemented and verified (build level) | `assembleDebug`, `assembleRelease`, 3 JVM unit tests pass here; release APK has `allowBackup=false`, cleartext off, not debuggable. Not run on a device or emulator |
| CI workflow `.github/workflows/platform.yml` (rust, web, android jobs) | Implemented and verified | All three jobs green on GitHub Actions for PR #2; upstream gate (`ci.yml`) green too |
| `packages/ui`, `packages/websocket-client`, `packages/validation`, `packages/editor` | Not implemented | Created in the phase that first needs them (3, 3, 2, 7) instead of as empty folders |
| `infrastructure/`, `tests/` (E2E) | Not implemented | Phases 2, 8, 9 |
| Everything in Phases 2–10 | Not implemented | |

### Phase 2
| Item | Status | Evidence |
|---|---|---|
| PostgreSQL schema: users, devices, sessions, refresh tokens, workspaces, members, projects, audit log; migrations embedded in the binary, `forge-api migrate` | Implemented and verified | applied on PostgreSQL 16.15 here; idempotent re-run test; CI uses `postgres:16` |
| `/readyz` (DB reachable, no pending migration) | Implemented and verified | tests for ready and not-ready |
| Register / login / logout, Argon2id, same answer for wrong password and unknown email | Implemented and verified | HTTP tests; hashes checked in the DB |
| Short-lived access tokens (HMAC-signed, 10 min) checked against a live session on every request | Implemented and verified | forged, garbage and revoked tokens refused |
| Rotating refresh tokens with reuse detection (reuse revokes the session) | Implemented and verified | rotation and replay tests |
| Browser sessions: HttpOnly `SameSite=Strict` cookies + double-submit CSRF; Android: Bearer | Implemented and verified | cookie attributes, CSRF refusal and acceptance tests; manual curl run |
| Session list and revocation by the user | Implemented and verified | other device's token stops working at once |
| RBAC (viewer < editor < maintainer < admin < owner), permission table, last-owner rule | Implemented and verified | role tests per action |
| Tenant isolation: other tenants' IDs answer 404 on every workspace, member and project route | Implemented and verified | 12-route isolation test, no side effects |
| Cursor pagination, archived projects | Implemented and verified | paging test |
| Problem+json for every error incl. bad JSON, bad query, bad path, 415, 413 | Implemented and verified | tests |
| Rate limits (auth per IP, login per email, refresh per IP) with `Retry-After` | Implemented and verified (single instance) | in-memory; a shared store is needed once the API runs several instances |
| Audit log with request ID and hashed client address | Implemented and verified | tests check entries and that no password is logged |
| Security headers (nosniff, DENY, no-referrer, CSP none, no-store) | Implemented and verified | test |
| Graceful shutdown with the DB pool | Implemented and verified | manual run |
| Row-level security (backstop for tenancy) | Not implemented | Planned before Phase 5 exposes engine data |
| OpenAPI document | Not implemented | `docs/platform/05-api-spec.md §3a` is the current reference |
| Optional TOTP MFA | Not implemented | |
| WebView login code exchange | Not implemented | Phase 4 |
| Encrypted provider/Git secrets | Not implemented | Phase 6–7, when those secrets exist |

### Phase 3
| Item | Status | Evidence |
|---|---|---|
| API client: CSRF header on writes, one shared refresh on 401 then one retry, session-ended callback, typed endpoints for auth, sessions, workspaces, members, projects | Implemented and verified | 11 unit tests |
| Screen 4 Login, 5 Registration (validation, server field errors, safe `next` redirect) | Implemented and verified | component tests + Playwright against the real API |
| Screen 6 Dashboard (recent projects, active sessions) | Implemented and verified | tests + E2E |
| Screen 7 Project List (paging, archived filter, empty/forbidden states), 8 Create Project, 9 Project Overview (rename, archive, delete with confirmation, role-based controls) | Implemented and verified | tests + E2E |
| Screen 34 Workspace Settings (rename, members: add, change role, remove, leave; create and delete workspace) | Implemented and verified (component level) | component tests; member flows not yet in E2E |
| Screen 35 Security and Permissions (account, signed-in devices, sign out a device) | Implemented and verified | tests + E2E. MFA not available (not built in the API) |
| Screen 38 Help and Diagnostics (server reachability, readiness, version, browser info, recent errors with request IDs); works signed out | Implemented and verified | tests + E2E |
| Responsive shell: bottom navigation < 768 px, rail 768–1279 px, sidebar ≥ 1280 px; dark theme by default | Implemented and verified | Playwright at 375, 900, 1440 px: right navigation visible, no horizontal overflow, content next to the sidebar; screenshots reviewed |
| Sessions in HttpOnly cookies, not readable by page scripts | Implemented and verified | E2E reads `document.cookie` |
| Expired access cookie renewed transparently; refresh cut off by navigation keeps the session | Implemented and verified | 2 E2E tests; needed an API fix (below) |
| Screen 37 Notifications | Not implemented | The API has no notifications yet; moved to the phase that produces the first notifications (agent and build events, Phases 6 and 8) |
| Light theme switch | Partially implemented | Tokens exist; no toggle in the UI yet |
| E2E job in CI | Implemented and verified | Green on GitHub Actions for PR #4 |

**API fix found by the E2E tests.** When a browser navigates while a refresh is in flight, the
server rotates the token but the new cookie never arrives; the next request presents the old
token, and the strict reuse rule ended the session. A token used less than 30 seconds ago may
now be presented again: it rotates once more and marks the lost token as long used, so whoever
presents that one later still ends the session. Trade-off: a token copied and replayed within
those 30 seconds also works once.

### Phase 4
| Item | Status | Evidence |
|---|---|---|
| API: `POST /auth/login-code` (single-use, 60 s, hash stored) and `POST /auth/exchange` (form POST → HttpOnly cookies + 303 to a same-site path; refuses `Sec-Fetch-Site: cross-site`, reused, expired codes and codes of ended sessions) | Implemented and verified | 4 API tests (52 total) |
| Android screens 1 Splash, 2 Onboarding, 3 Server Connection (HTTPS only; plain HTTP only for local hosts in debug), 4 Login, 5 Registration, 6 Dashboard shell, 7 Project List (workspace picker, empty, offline, error with request ID) | Implemented and verified (JVM) | 7 Robolectric Compose UI navigation tests |
| Native settings (account, server, sign out), native Help and Diagnostics | Implemented and verified (JVM) | navigation tests cover sign out |
| HTTP client: Retrofit + OkHttp, Bearer tokens, one shared refresh on 401 then one retry, refused refresh ends the session | Implemented and verified | MockWebServer tests incl. 4 concurrent calls → 1 refresh |
| Same backend for both clients | Implemented and verified | `LiveApiTest`: the app's real client against a running forge-api: register, workspaces, projects, WebView exchange (303 + HttpOnly cookies), sign out, sign in. Also in CI |
| Tokens encrypted at rest (AES-256-GCM, Android Keystore key, DataStore) | Implemented and verified (crypto part) | `SealerTest` with a software key; the Keystore itself only exists on a device |
| Secure WebView: one trusted origin, other HTTPS pages in Custom Tabs, other schemes blocked; no file/content access; no mixed content; Safe Browsing; no third-party cookies; no extra windows | Implemented but not live-tested | Policy unit-tested; the WebView itself needs a device or emulator |
| Bridge: `addWebMessageListener` limited to the trusted origin, main frame only, strict JSON schema, 4 methods (`getAppInfo`, `haptic`, `share`, `openExternal` https-only), size limits | Implemented and verified (logic) | 6 bridge security tests incl. 21 refused messages; web side `callNative` tested with a fake bridge |
| WebView debugging off in release; no plain HTTP in release | Implemented and verified | `ReleaseSecurityTest` (release unit tests); release APK manifest: `allowBackup=false`, cleartext off, HTTPS-only network security config |
| Android back: web history first, then the native screen | Implemented but not live-tested | needs a real WebView |
| Offline: banner, offline errors with retry, WebView load errors as a native error screen | Implemented and verified (native screens) | Robolectric offline test; WebView part not live-tested |
| Room, WorkManager, notifications, file picker, artifact sharing | Not implemented | No offline cache, downloads or notifications exist yet to use them; planned with artifacts (Phase 8) and notifications (Phases 6, 8) |
| App icon | Not implemented | still the default icon |

**Bug found by the tests:** the app's JSON encoder left out default values, so `client: "android"`
was never sent and the server answered as to a browser (cookies, no tokens). Defaults are now
always encoded, and `HttpBackendTest` checks the request body.

## Blockers and risks

| # | Blocker / risk | Affects | Mitigation |
|---|---|---|---|
| B1 | ~~No Android SDK or Gradle in this sandbox~~ Resolved in Phase 1: JDK 21, Gradle 8.11.1 and SDK 35 installed in the workspace folder. Still no emulator | Instrumented and Compose UI tests (criterion 13) | Run instrumented tests on a CI emulator in Phase 4 |
| B2 | Containers do not start in this sandbox (Podman without `conmon`) | Worker isolation, terminal isolation, builds (criteria 8, 10) | Test on CI runners or the target Ubuntu host |
| B3 | ~~No PostgreSQL in this sandbox~~ Resolved in Phase 2: portable PostgreSQL 16.15 binaries run as an unprivileged user here; CI uses a `postgres:16` service | — | — |
| B4 | Git proxy cannot fetch `storytold/craft-libs` | Building `ui-egui` locally | Local tarball patch on the audit host only; CI unaffected |
| B5 | No AI provider key available | Live agent tests (criterion 7) | You add a key as a secret; until then agent streaming is "Implemented but not live-tested" |
| B6 | No authenticated test Git repository | Criterion 9 | A dedicated test repo and token are needed |
| B7 | ~~Product name~~ Resolved: Tophunt Forge. App icon still needed | Store listing | Phase 4 uses a plain generated icon until one is designed |
| R1 | Scope: 38 screens, 3 runtimes, isolation infrastructure | Schedule | Strict phase gates; no screen is counted done without API integration and all states |
| R3 | Upstream test `plugins::tests::a_plugin_that_exits_at_once_says_what_it_printed_and_leaves_a_log` is flaky on CI (broken pipe when the plugin exits before its first write); passed 30/30 locally and on re-run | Occasional red upstream gate | Re-run; report upstream if it recurs |
| R2 | Upstream moves fast (1,050 commits) | Merge conflicts | Minimal upstream edits; inventory diff on each sync |

## Untested features

- API behind a real reverse proxy with TLS (`FORGE_TRUST_PROXY`, `Secure` cookies over HTTPS).
- Rate limits with more than one API instance (in-memory today).

- Web layout at real breakpoints in a browser (only jsdom unit tests so far).
- Android app on a device or emulator, including the real WebView, Keystore, Custom Tabs and back navigation.
- `cargo deny` licence check for the new Rust dependencies (axum, tokio, tracing, uuid) was not run; they are MIT or MIT/Apache-2.0, which `deny.toml` allows.
