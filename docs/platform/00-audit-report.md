# Phase 0 audit report

Audit of `storytold/craft-anything` at upstream commit `febf096` (version 0.6.1), imported into
this repository with full history on branch `phase-0-audit`. Everything below was read from source
or produced by commands run during the audit. Statements that were not verified are marked
**Unverified**.

## 1. What the upstream project is

A Rust workspace (edition 2024, `rust-version = 1.97`, resolver 3) for evidence-based analysis of
software: native binaries, Android/Java, Apple, .NET, JavaScript/Electron, websites, firmware,
game archives, packet captures. It ships three apps:

| App | What it is | Relevant to the platform |
|---|---|---|
| `apps/craft-anything` | Desktop app, eframe/egui + wgpu. UI lives in `crates/ui-egui`. | No. Kept as-is. |
| `apps/craft-anything-cli` | Headless CLI. Every tool is a subcommand; also `mcp`, `chat`, `rebuild`, `ai`, `setup`, `doctor`. | Yes. Reference for constructing the engine headless (`make_engine`). |
| `apps/craft-anything-web` | A small egui app compiled to wasm that triages a file locally in the browser using only `formats`, `analysis`, `evidence`. No engine, AI or MCP. | Not a frontend for the engine. Kept as-is; unrelated to the new React app. |

## 2. Crates (lines of Rust, including tests)

| Crate | Layer | Lines | Role |
|---|---|---|---|
| `evidence` | L0 | 980 | Evidence records, content-addressed ids (`ev_` + SHA-256 of canonical JSON), `ToolError` contract |
| `formats` | L0 | 73,218 | Pure parsers: ELF/PE/Mach-O/WASM/DEX/APK/plist/.NET/Go and more. wasm-safe |
| `disasm` | L1 | 7,100 | Built-in disassembler, functions, CFG, xrefs, pseudo-C |
| `js` | L1 | 7,668 | JavaScript bundles, module graph, source maps (oxc parser) |
| `analysis` | L2 | 11,818 | Analyzers producing evidence |
| `platform` | L2 | 7,574 | Owned processes (kill process group on timeout/cancel/drop), PTY capture (`portable-pty` + vt100), loopback static server, custom `Mutex` |
| `engines` | L3 | 10,060 | External engines: Deep mode bridge, adb, CDP (Chromium), Frida, JADX, Apktool…; pinned downloads with SHA-256 check |
| `engine` | L4 | 66,379 | `Engine`, `Session`, projects (`.cany`), workspaces, inbox, journal, all 305 tools |
| `ai` | L5 | 39,794 | AI backends, agent loop, roles, rebuild flow, key store, prices |
| `mcp` | L5 | 1,668 | MCP server (JSON-RPC over stdio), progressive toolsets, restricted mode |
| `setup` | L5 | 3,178 | Connect agent apps to the MCP server; doctor; uninstall |
| `ui-egui` | L6 | 24,803 | Desktop UI |
| `testkit` | — | 8,492 | Fixture generators for tests |

Layering is enforced by `cargo xtask layers`: a crate may only depend on lower layers, and apps
sit on top. The workspace forbids `unsafe_code` and denies `unwrap`/`expect`/`panic` in crates.

## 3. Engine dispatch (`crates/engine/src/lib.rs`)

`Engine::call(&self, id, params: serde_json::Value, opts: CallOptions) -> Result<ToolOutput, ToolError>`
is the single entry point used by the desktop UI, CLI, MCP server and AI agent.

- **Blocking and synchronous.** No async runtime anywhere in the upstream code.
- **One session per engine.** `Engine` holds one `Mutex<Session>`. Every call waits on it (with
  cancellation and stuck-lock detection), so calls on one engine run one at a time.
- **Pipeline:** name lookup (dashes normalised, `mcp__server__` prefix stripped) → session lock →
  Clean room gate → availability (`Requires`) → consent gate → run inside `catch_unwind` →
  record evidence → append a `tool_call` entry to the project journal (with secret-named fields
  redacted, `journal.rs:46-66`).
- **Consent gate.** Tools whose `Effects` include `EXECUTES_FOREIGN_CODE` or `MUTATES_DEVICE` return
  `ConsentRequired` unless `CallOptions.consent == Granted`, the person turned off "ask before
  running", or the project pre-approved the tool. 29 tools carry these flags.
- **Input validation** is done by each tool through serde (`parse_params`, `deny_unknown_fields`
  common) with JSON Pointer error paths. JSON Schemas (schemars) are generated for advertising;
  the dispatcher does not validate against them.
- **Output.** `ToolOutput::Evidence(Box<Evidence>)` or `ToolOutput::Value(Value)`. No per-tool
  output schemas exist.
- **Cancellation** is cooperative (`CancelToken`). Time limits are per tool, not global
  (e.g. `build_command_run`: 600 s default, clamped to 1–7200 s).

Effects classification (`crates/engine/src/tool.rs:10-63`) is the basis for the platform's
permission model: `MUTATES_SESSION`, `MUTATES_TARGET`, `WRITES_FILESYSTEM`, `LAUNCHES_PROCESS`,
`ACCESSES_NETWORK`, `CHANGES_UI_STATE`, `MAY_DISCARD_DATA`, `EXECUTES_FOREIGN_CODE`,
`MUTATES_DEVICE`, `IDEMPOTENT`, `CARRIES_TARGET_CONTENT`.

## 4. Tool registry

- **326 tools** in the live registry used by the CLI: 305 engine tools in 17 toolsets plus 21
  AI/setup layer tools (`rebuild_*`, `assets_key_*`, `ai_model*`, `setup_*`). The upstream
  `docs/tools.md` states the same 305. `docs/ROADMAP.md` mentions "314"; that figure matches
  neither count and looks stale.
- The full inventory, generated from the registry, is in [03-tool-inventory.md](03-tool-inventory.md)
  and [tool-inventory.json](tool-inventory.json).
- Plugins and proxied external MCP servers can add runtime tools (`registry.rs:87-112`). WASM
  plugins sit behind the off-by-default `wasm-plugins` feature (wasmtime).
- **Clean room is the default mode.** With default settings 215 tools refuse to run because they
  read a target's code, files or page source. Deep mode must be enabled per session.

## 5. AI layer (`crates/ai`)

- Backends: Anthropic (Messages API, streaming, prompt caching), OpenAI, OpenRouter, Gemini (via
  its OpenAI-compatible endpoint), any OpenAI-compatible server (Ollama, LM Studio, vLLM…), and
  agent CLIs (Claude Code, Codex, Copilot, Gemini CLI, OpenCode, Aider and others), optionally
  over SSH.
- HTTP is blocking `ureq` 3 on background threads; SSE is parsed with size caps.
- **Agent loop.** `AgentLoop` runs on its own thread. Tools go through `EngineHost`, which calls
  `Engine::call` as `Caller::Agent`. On `ConsentRequired` the loop emits
  `AgentEvent::ConsentNeeded` and blocks until `answer_consent`. `AgentEvent` is serde-tagged JSON
  (`TextDelta`, `Status`, `Output`, `ToolStarted`, `ToolFinished`, `ConsentNeeded`, `TurnDone`,
  `Error`), so it can be streamed to clients unchanged.
- Token and cost tracking exist: `TurnDone` carries input/output tokens and `cost_usd` (with
  `cost_is_estimate`), from a built-in price table that settings can override.
- Roles: Lead, Worker, Reviewer, Builder, Extractor. The **rebuild** flow (study → build → check →
  refine) drives an agent CLI in a workspace `app/` folder connected to the MCP server.
- **Keys** (`keys.rs`): OS keychain → plaintext `ai-keys.toml` (0600) → environment variables.
  `CRAFT_ANYTHING_NO_KEYCHAIN=1` skips the keychain. There is no application-level encryption;
  the platform must not use the file store.

## 6. MCP server (`crates/mcp`)

Newline-delimited JSON-RPC 2.0 over **stdio only**. The generic `serve(reader, writer)` allows
other transports. It has progressive toolsets, resources (`evidence://…`), prompts, progress,
cancellation, response budgets, and a **restricted mode** that refuses machine-acting and
sign-in tools. Consent is all-or-nothing per server (`--allow-run`); there is no per-call grant.

## 7. Process, file and secret handling

- Child processes run through `platform::process::Process` with wall-clock and idle timeouts,
  output caps, cancellation and process-group kill. **There is no OS-level sandbox** (no
  container, seccomp or namespace isolation).
- `build_*` tools contain paths to the project's `app/` folder. Symlink handling was not
  verified (**Unverified**).
- `engines_install` downloads and installs software and does not need consent.
- `project_delete` is declared `WRITES_FILESYSTEM` only, without `MAY_DISCARD_DATA`, so MCP
  clients do not see it as destructive. Worth reporting upstream.
- State on disk is per process: `CRAFT_ANYTHING_CONFIG_DIR` (else the OS config dir) holds
  `config.toml`, the engine store and logs; projects default to the user's Documents folder.
  The process reads `HOME`-derived paths through `dirs`.

**Consequence for the platform:** an `Engine` cannot safely be shared between tenants in one
process. Each workspace needs its own engine process with its own config directory, inside an
isolated, non-root sandbox. See [01-architecture.md](01-architecture.md).

## 8. Build and test results on the audit host

Host: Linux x86-64 sandbox, Rust 1.97.1 installed for the audit (the preinstalled 1.92 is
below the workspace's `rust-version`).

**Blocker found and worked around:** `crates/ui-egui` depends on git crate `craft-update` from
`github.com/storytold/craft-libs` (pinned rev `6ac2a6a`). The sandbox git proxy only reaches
repositories connected to this session, so `cargo fetch` failed with HTTP 404. The repo is
public; the audit downloaded the pinned rev as a tarball and used a local `[patch]` in the
user-level cargo config. Nothing in the repository was changed for this. CI on GitHub will not
have this problem.

Command (headless crates only; the GUI crate and desktop app were not built):

```
CRAFT_ANYTHING_NO_KEYCHAIN=1 cargo test --no-fail-fast -p craft-anything-{evidence,formats,disasm,js,analysis,platform,engines,engine,ai,mcp,setup}
```

Result: **4,025 passed, 4 failed, 34 ignored.** All four failures are environment-specific:

| Test | Cause | Status |
|---|---|---|
| `ai: rebuild::…::a_failed_turn_leaves_just_ask_questions…` | Needs `craft-anything-cli` built first (documented in upstream AGENTS.md) | Passes after `cargo build -p craft-anything-cli` (33/33 in that module) |
| `ai: rebuild::…::a_skipped_scan_does_not_hide_the_builders_own_failure` | Same | Passes after building the CLI |
| `disasm: program::tests::real_system_binary_if_present` | `/usr/bin/ls` on this host is a shell-script shim, not an ELF | Host quirk, not a code bug |
| `engine: tools::native_formats::tests::real_binary_end_to_end_if_present` | Same shim | Host quirk, not a code bug |

Not run in the audit: `cargo xtask ci` in full (needs GUI system libraries for `ui-egui`),
`egui_kittest` snapshot tests, the `wasm-plugins` feature, and live tests that need agent CLIs,
devices or browsers.

## 9. Environment limits that affect later phases

| Need | State in this sandbox | Effect |
|---|---|---|
| Rust ≥ 1.97 | Installed 1.97.1 into the workspace folder | OK |
| Node.js / pnpm | Node 22, pnpm 12 via corepack | Web app can be built and tested here |
| Java | OpenJDK 25 | Present, but no Gradle and no Android SDK |
| Android SDK + Gradle | Missing | **Android APK cannot be built here.** Must be built in GitHub Actions |
| Containers | `docker` is Podman without `conmon`; containers do not start | **Worker isolation and Postgres-in-container cannot be tested here.** Needs CI or the target Ubuntu host |
| PostgreSQL | No server or client | Phase 2 will try portable Postgres binaries in the workspace; otherwise migrations are tested in CI only |

## 10. Licensing and brand

- Code: `MIT OR Apache-2.0`. Our additions use the same dual licence.
- Bundled fonts are OFL-1.1; Lucide icons are ISC. Both may be reused with their licence files.
- **The ArtCraft name, wordmark, logos (`docs/brand/`) and the raven app icon are trademarks and
  not open source.** The platform must use its own name and icon. This repository is named
  "Clone-anything-"; a product name is needed before the Android app ID and store listing.
- `deny.toml` restricts licences and git sources; new Rust dependencies must pass it.

## 11. Upstream conventions worth keeping

From `AGENTS.md` / `CONTRIBUTING.md`: one item per PR, squash merge with a plain present-tense
title, branch prefixes (`feature/`, `bugfix/`, `docs/`…), `cargo fmt`, `clippy -D warnings`,
`cargo xtask layers|locks|secrets`, the `platform::sync::Mutex`, and no secrets or home-folder
paths in commits (`.githooks/pre-commit` runs `cargo xtask secrets`).

## 12. Findings that change the original plan

1. **Engine is single-session and blocking.** The API cannot embed one `Engine` for all users.
   The adapter becomes a per-workspace engine process (`apps/workers`, role `engine`) that the
   API talks to over a JSON protocol.
2. **No OS sandbox upstream.** Isolation (containers, non-root, quotas, egress proxy) is entirely
   new platform work, and it cannot be tested in this sandbox.
3. **Workspace glob `members = ["crates/*", "apps/*", "xtask"]`.** Putting non-Rust folders in
   `apps/` (React, Android) breaks Cargo and `cargo xtask layers`. The integration map proposes a
   two-line change.
4. **Clean room by default.** Matches the brief's "authorization for sensitive analysis": Deep
   mode becomes a per-project permission with an audit entry.
5. **MCP consent is server-wide.** The platform needs per-call approval, so it calls
   `Engine::call` directly with `Consent::Granted` after an approval, rather than going through
   the MCP server.
6. **Multi-agent orchestration with 14 roles, Git worktrees and a remote build system do not exist
   upstream.** They are new code built on the existing agent loop and `build_command_run`.
