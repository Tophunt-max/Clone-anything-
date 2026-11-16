# Repository integration map

Status: **proposed** (Phase 0). Upstream code stays where it is. New code is added beside it.

## 1. Target layout

```
Cargo.toml                 upstream workspace (two-line change, see §3)
crates/                    upstream crates, unchanged
  evidence formats disasm js analysis platform engines engine ai mcp setup ui-egui testkit
apps/
  craft-anything/          upstream desktop app, unchanged
  craft-anything-cli/      upstream CLI, unchanged
  craft-anything-web/      upstream wasm triage viewer, unchanged
  api/                     NEW  Rust, Axum control plane, forge-api (Cargo workspace member)
  workers/                 NEW  Rust, forge-worker: engine/terminal/build roles (member)
  web/                     NEW  React + TypeScript + Vite           (excluded from Cargo)
  android/                 NEW  Kotlin + Compose + WebView          (excluded from Cargo)
packages/                  NEW  pnpm workspace
  design-tokens/ ui/ api-client/ websocket-client/ shared-types/ validation/ editor/
infrastructure/            NEW  compose files, reverse proxy, sandbox images, monitoring, backup
scripts/                   NEW  tool-inventory exporter, dev helpers
tests/                     NEW  end-to-end and infrastructure tests
docs/                      upstream docs, unchanged
docs/platform/             NEW  this documentation set
xtask/                     upstream, small change (see §3)
```

## 2. How new code uses upstream crates

| New component | Upstream API used | Notes |
|---|---|---|
| `apps/workers` (engine role) | `craft_anything_engine::{Engine, Session, Settings, CallOptions, Caller, Consent, CancelToken, ToolOutput}` | Construct the same way as `apps/craft-anything-cli/src/main.rs::make_engine`: `Engine::shared_with(session, layer_tools, plugins)` |
| | `craft_anything_ai::{AiHub, Conversation, AgentEvent, tools()}` | Conversations are polled, events forwarded unchanged |
| | `craft_anything_ai::keys::KeyStore` | Keys passed per job as environment variables (`ANTHROPIC_API_KEY` …) with `CRAFT_ANYTHING_NO_KEYCHAIN=1`; the plaintext file store is never used |
| | `craft_anything_engine::Registry` | Serves the tool catalogue and JSON Schemas to the API |
| `apps/workers` (terminal role) | `craft_anything_platform` PTY support (`portable-pty`, vt100) | Reuse rather than add another PTY crate |
| `apps/workers` (build role) | `craft_anything_platform::process::Process` | Timeouts, output caps, process-group kill |
| `apps/api` | `craft_anything_evidence::{Evidence, ToolError, ErrorCode}` | Shared types for evidence storage and error mapping |
| Tool inventory | `Registry`, `Effects`, `Requires` | `forge-worker catalog`, rendered by `scripts/tool-inventory/render.py` |
| External MCP clients (screen 31) | `craft_anything_mcp::serve` | Later phase: expose the upstream MCP server to a user's own MCP client through an authenticated tunnel, restricted mode on |

Tool semantics are not changed. The platform adds policy and approval **in front of**
`Engine::call`; it does not wrap or reimplement any tool.

### Engine settings per workspace
- `CRAFT_ANYTHING_CONFIG_DIR=/workspace/.craft` inside the sandbox.
- Default mode Clean room. Deep mode only when the project setting allows it.
- `ask_before_running = true`, so consent-gated tools always come back as `ConsentRequired`
  unless the API grants them per call after an approval.
- Engine installs (`engines_install`) are disabled for tenants. Sandbox images ship pinned engines
  instead (Chromium, JDK, adb is not useful without a device).

## 3. Changes made in upstream files (Phase 1, done)

Kept minimal so `git merge upstream/main` stays easy.

1. **`Cargo.toml`**: the members glob `apps/*` would pick up `apps/web` and `apps/android`, which
   have no `Cargo.toml`, and Cargo fails (verified during the audit with an empty `apps/zz-probe`).
   Add:
   ```toml
   exclude = ["apps/web", "apps/android"]
   ```
2. **`xtask/src/main.rs`** and **`xtask/src/locks.rs`**: several checks walk every file under
   `apps/` (`layers`, file walks at lines ~258, ~413, ~509, `locks.rs:216`). They would walk
   `node_modules` and Gradle build folders. Add a skip list for `apps/web` and `apps/android`.
   `forge-api` and `forge-worker` need no registration: everything in `apps/` is `APP_LAYER` = 7.
3. **`crates/ui-egui` git dependency**: no change. Documented in the audit as a sandbox-only issue.

## 4. Upstream sync

- Remote `upstream` = `https://github.com/storytold/craft-anything.git`.
- Sync by merging `upstream/main` into a `chore/upstream-sync-<date>` branch, running the full
  test suite plus the inventory exporter, and reviewing the inventory diff for changed tool
  effects before merging.
- The inventory diff is the early warning: a tool that gains `EXECUTES_FOREIGN_CODE` or changes
  `Requires` changes its platform policy.

## 5. Things deliberately not used

- `crates/ui-egui`, `apps/craft-anything`: desktop UI, not part of the hosted product.
- `apps/craft-anything-web`: unrelated wasm viewer; kept, not extended.
- `crates/setup`: connects local agent apps on a person's machine. In hosted mode its tools are
  operator-only.
- The plaintext key file in `crates/ai/src/keys.rs`.
