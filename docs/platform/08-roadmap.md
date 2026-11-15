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
| 2 | Axum API, auth, PostgreSQL | Migrations, register/login/refresh/revoke, RBAC, tenant isolation tests, OpenAPI, health, rate limits | Only if portable Postgres runs here; otherwise CI |
| 3 | React app and API integration | Auth flows, dashboard, projects, settings, notifications, help; responsive tests | Yes |
| 4 | Android shell and secure WebView | Native screens, WebView bridge with security tests, back navigation, offline | CI only (no Android SDK here) |
| 5 | Engine adapter and tool registry | `craft-worker` engine role, worker protocol, policy + approvals, evidence index, analyzers (screens 16–20) | Engine yes; sandbox isolation no |
| 6 | AI assistant and multi-agent orchestration | Providers, chat, plans, tasks, worktrees, streaming, cost tracking | Partly (needs a real AI key for live tests) |
| 7 | Monaco editor, terminal, Git | Editor with ETag conflicts, PTY terminal, Git operations with credentials | Partly (terminal isolation needs containers) |
| 8 | Build workers and artifacts | Queue, toolchain images, logs, tests, artifacts, quotas | CI / target host only |
| 9 | Security review, testing, monitoring | Threat-model controls tested, E2E suite, backup/restore test, dashboards | Partly |
| 10 | Deployment and release | Compose stack on Ubuntu 24.04, TLS, runbooks, signed Android release | Target host only |

## Current status (end of Phase 0)

| Item | Status |
|---|---|
| Upstream imported with full history | Implemented and verified |
| Rust 1.97 toolchain on the audit host | Implemented and verified |
| Upstream headless test suite (4,025 passed, 4 environment-specific failures explained) | Implemented and verified |
| Tool inventory generated from the registry (326 tools) | Implemented and verified |
| Phase 0 documents (this folder) | Implemented and verified (written; review pending) |
| Everything in Phases 1–10 | Not implemented |

## Blockers and risks

| # | Blocker / risk | Affects | Mitigation |
|---|---|---|---|
| B1 | No Android SDK or Gradle in this sandbox | Phase 4, acceptance criteria 1, 13 | Build and test Android in GitHub Actions; report CI results only |
| B2 | Containers do not start in this sandbox (Podman without `conmon`) | Worker isolation, terminal isolation, builds (criteria 8, 10) | Test on CI runners or the target Ubuntu host |
| B3 | No PostgreSQL in this sandbox | Phase 2 migrations and tenant isolation (criteria 11, 12) | Try portable Postgres binaries; otherwise GitHub Actions service container |
| B4 | Git proxy cannot fetch `storytold/craft-libs` | Building `ui-egui` locally | Local tarball patch on the audit host only; CI unaffected |
| B5 | No AI provider key available | Live agent tests (criterion 7) | You add a key as a secret; until then agent streaming is "Implemented but not live-tested" |
| B6 | No authenticated test Git repository | Criterion 9 | A dedicated test repo and token are needed |
| B7 | Product name and icon | Android app id, store listing | ArtCraft brand cannot be used; you choose a name |
| R1 | Scope: 38 screens, 3 runtimes, isolation infrastructure | Schedule | Strict phase gates; no screen is counted done without API integration and all states |
| R2 | Upstream moves fast (1,050 commits) | Merge conflicts | Minimal upstream edits; inventory diff on each sync |

## Untested features

None implemented yet. This section lists untested items at the end of each phase.
