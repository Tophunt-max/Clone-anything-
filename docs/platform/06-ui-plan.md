# UI implementation plan

Status: **proposed** (Phase 0). Phase 3 built screens 4–9, 34, 35 and 38 in React; screen 37 waits for a notifications API. See the roadmap.

## 1. Design system

One source of truth: `packages/design-tokens/tokens.json` in the W3C Design Tokens format.
A build script generates:

- `tokens.css` (CSS custom properties for dark and light themes) and a Tailwind preset for
  `apps/web` and `packages/ui`;
- `CraftTokens.kt` (colors, type scale, spacing, radius, elevation) for the Compose `MaterialTheme`
  in `apps/android`.

A CI check fails if the generated files are stale. Screens may not hard-code colours or sizes
(the same rule upstream enforces for egui, `docs/design-system.md`).

Initial values (dark theme first):

| Token group | Values |
|---|---|
| Background | `bg.canvas #0B1020` (deep navy), `bg.surface #111827` (charcoal), `bg.raised #161F33`, `bg.field #1C2740` |
| Border | `border.subtle #24304A`, `border.strong #33415F` |
| Text | `text.primary #E6EAF2`, `text.secondary #A3AECB`, `text.muted #6B7899` |
| Accent | `accent.blue #4C8DFF`, `accent.violet #8B5CF6`, focus ring = blue at 60% |
| Status | `success #22C55E`, `warning #F59E0B`, `danger #EF4444`, `info #38BDF8` |
| Type | Inter (UI): 12 / 13 / 14 / 15 / 17 / 20 / 26; JetBrains Mono (code): 12.5 / 13 / 14 |
| Spacing | 2, 4, 8, 12, 16, 24, 32, 48 |
| Radius | 4 (rows), 6 (controls), 8 (cards), 12 (sheets), full (pills) |
| Elevation | 0 flat, 1 card (subtle shadow + border), 2 popover, 3 dialog |
| Breakpoints | `sm` < 768 (one column), `md` 768–1279 (two panels), `lg` ≥ 1280 (multi-panel) |

Inter and JetBrains Mono are OFL-1.1 and already in `assets/fonts/` with licence files. Icons:
Lucide (ISC), also already bundled. Text/background pairs must meet WCAG AA (4.5:1); a unit test
checks the token pairs. Light theme follows in Phase 3.

The ArtCraft brand and raven icon are **not** used (trademarks, see the audit).

## 2. Component states

Every data screen uses one shared wrapper with five states: `loading` (skeletons), `empty`
(explanation + primary action), `error` (message, request ID, retry), `forbidden` (what permission
is missing and who can grant it), `ready`. Long operations show live progress from the event stream.

## 3. Screens

Legend: **N** = native Compose, **W** = React. "Both" means the screen exists natively on Android
and in React for browsers (the WebView uses the native version). Phase = when it becomes
functional, not just drawn.

| # | Screen | Where | Route (web) | Backed by | Phase |
|---|---|---|---|---|---|
| 1 | Splash | N | n/a | `/meta` reachability | 4 |
| 2 | Onboarding | N | n/a | local only | 4 |
| 3 | Server Connection | N | n/a (web uses its own origin) | `/meta`, `/readyz`, TLS check | 4 |
| 4 | Login | Both | `/login` | `/auth/login`, MFA | 3 (W), 4 (N) |
| 5 | Registration | Both | `/register` | `/auth/register` | 3, 4 |
| 6 | Dashboard | Both | `/` | projects, recent jobs, approvals, usage | 3, 4 |
| 7 | Project List | Both | `/projects` | `/workspaces/{ws}/projects` | 3, 4 |
| 8 | Create Project | W | `/projects/new` | `POST /projects` | 3 |
| 9 | Project Overview | W | `/p/:id` | project, recent tool calls, builds | 3 |
| 10 | Project Files | W | `/p/:id/files` | files API | 7 |
| 11 | Code Editor | W | `/p/:id/edit/*` | files API with ETag, AI actions | 7 |
| 12 | AI Coding Chat | W | `/p/:id/chat` | conversations + `agent.*` events | 6 |
| 13 | Agent Manager | W | `/agents` | agents API | 6 |
| 14 | Multi-Agent Task Planner | W | `/p/:id/plans/new` | plans API | 6 |
| 15 | Agent Execution Monitor | W | `/p/:id/plans/:plan` | tasks + events | 6 |
| 16 | APK Analyzer | W | `/p/:id/analyze/apk` | `android_package_show`, `android_components_list`, `android_classes_find`, `android_resources_read` … | 5 |
| 17 | Website Analyzer | W | `/p/:id/analyze/web` | `web_site_open` (approval gate), `web_site_capture`, `web_page_js_analyze`, `js_app_map` … | 5 |
| 18 | Binary Analyzer | W | `/p/:id/analyze/binary` | `core_target_triage`, `native_functions_list`, `native_function_pseudocode` … | 5 |
| 19 | Evidence Explorer | W | `/p/:id/evidence` | evidence API (`core_evidence_browse` data) | 5 |
| 20 | Tool Registry | W | `/tools` | `/tools` with policy column | 5 |
| 21 | Remote Terminal | W | `/p/:id/terminal` | terminal WS (xterm.js) | 7 |
| 22 | Git Repositories | W | `/p/:id/git` | repos API | 7 |
| 23 | Git Branches | W | `/p/:id/git/branches` | branches | 7 |
| 24 | Commit History | W | `/p/:id/git/commits` | commits | 7 |
| 25 | Diff Viewer | W | `/p/:id/git/diff` | diff + Monaco diff editor | 7 |
| 26 | Build Configuration | W | `/p/:id/builds/config` | build-configs | 8 |
| 27 | Build Logs | W | `/p/:id/builds/:b` | build logs stream | 8 |
| 28 | Test Reports | W | `/p/:id/builds/:b/tests` | test reports | 8 |
| 29 | Artifact Manager | Both (share on N) | `/p/:id/artifacts` | artifacts + signed download | 8 |
| 30 | Web Preview | W | `/p/:id/preview` | previews API (separate origin, sandboxed iframe) | 8 |
| 31 | MCP Connections | W | `/settings/mcp` | mcp-connections | 6 |
| 32 | AI Provider Settings | W | `/settings/providers` | providers + test | 6 |
| 33 | API Key Management | W | `/settings/keys` | providers (write-only keys), Git credentials | 6 |
| 34 | Workspace Settings | W | `/settings/workspace` | workspace, members, retention | 3 |
| 35 | Security and Permissions | Both | `/settings/security` | sessions, devices, MFA, roles, Deep mode | 3, 4 |
| 36 | Usage and Resource Monitoring | W | `/usage` | usage API | 8 |
| 37 | Notifications | Both | `/notifications` | notifications + push | 3, 4 |
| 38 | Help and Diagnostics | Both | `/help` | `/me/diagnostics`, app/WebView versions | 3, 4 |

## 4. Layouts

- **Mobile (< 768 px):** bottom navigation (Dashboard, Projects, Agents, Notifications, More).
  Project screens use a top tab bar. Editor is read-mostly with a full-screen edit mode.
- **Tablet (768–1279 px):** navigation rail + list/detail panels.
- **Desktop (≥ 1280 px):** activity bar, resizable side panel (files, evidence, git), main editor
  area with tabs, bottom panel (terminal, build logs, agent output), right panel (chat).

## 5. Web stack details

React 19, TypeScript strict, Vite, React Router (data routers with lazy routes), TanStack Query
for server state, Zustand for UI state only (never tokens), Tailwind + shadcn/ui components themed
from tokens, Monaco Editor (lazy-loaded), xterm.js for the terminal, IndexedDB (via `idb`) for
drafts and cached read-only data. Testing: Vitest + Testing Library, MSW for API mocks in tests
only, Playwright for routing, responsive and end-to-end tests.

## 6. Android stack details

Kotlin, Compose + Material 3 (theme from generated tokens), Navigation Compose, coroutines +
StateFlow, Retrofit + OkHttp (certificate checks, auth interceptor with refresh), Room (project
list cache, notifications), DataStore (settings; refresh token encrypted with a Keystore key),
WorkManager (artifact downloads, notification sync), WebView via AndroidX WebKit.

**WebView rules:**
- Loads only the configured web origin over HTTPS; other navigations open in Custom Tabs.
- Bridge: `WebViewCompat.addWebMessageListener` with `allowedOriginRules = {configured origin}`.
  Every message is JSON validated against a Kotlin schema (kotlinx.serialization, unknown fields
  rejected) and dispatched to a fixed allowlist: `pickFile`, `shareArtifact`, `showNotification`,
  `getAppInfo`, `openExternal` (https only, through Custom Tabs), `haptic`.
- `setWebContentsDebuggingEnabled(BuildConfig.DEBUG)`; file and content access off;
  mixed content blocked; Safe Browsing on.
- Back button: if the WebView can go back, go back in history; otherwise pop the native screen.
- Offline and load errors show a native error screen with retry instead of the WebView's page.
