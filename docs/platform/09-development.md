# Development

## Requirements

| Part | Tools |
|---|---|
| Rust (`apps/api`, `apps/workers`, upstream crates) | Rust 1.97 or newer |
| Web (`apps/web`, `packages/*`) | Node 22 (`.nvmrc`), pnpm 12 (`packageManager` in `package.json`, via `corepack enable`) |
| Android (`apps/android`) | JDK 17 or 21, Android SDK platform 35. Gradle comes from the wrapper |

## Commands

```sh
# Rust
cargo run -p forge-api                      # API on 127.0.0.1:8080 (FORGE_API_ADDR to change)
cargo test -p forge-api -p forge-worker
cargo run -q -p forge-worker -- catalog     # tool catalogue JSON from the engine registry

# Web and packages (from the repository root)
pnpm install
pnpm --filter @forge/web dev                # proxies /api and /healthz to the API
pnpm check && pnpm typecheck && pnpm test && pnpm build

# Design tokens: edit packages/design-tokens/tokens.json, then
pnpm tokens                                 # writes CSS, Tailwind theme and ForgeTokens.kt

# Android
cd apps/android && ./gradlew assembleDebug testDebugUnitTest
```

## API settings (environment)

| Variable | Default | Meaning |
|---|---|---|
| `FORGE_API_ADDR` | `127.0.0.1:8080` | Listen address |
| `FORGE_API_REQUEST_TIMEOUT_SECS` | `30` | Per-request timeout (1–3600) |
| `FORGE_API_BODY_LIMIT_BYTES` | `1048576` | Largest request body |
| `RUST_LOG` | `info` | Log filter; logs are JSON lines with the request ID |

## Rules carried over from upstream

`AGENTS.md` applies to the Rust code: no `unwrap`/`expect`/`panic` outside tests, workspace lints,
`cargo fmt`, `clippy -D warnings`, `cargo xtask layers|locks|secrets`, and no secrets or local
paths in commits. Screens never hard-code colours or sizes; they use the tokens.

## Pitfall: a local `[patch]` for `craft-libs` rewrites `Cargo.lock`

Where `github.com/storytold/craft-libs` can't be fetched with git, a `[patch."https://github.com/storytold/craft-libs"]`
pointing at a local copy works for building, but every cargo command then drops the
`source = "git+…craft-libs…"` line of `craft-update` from `Cargo.lock`. The upstream gate fails on
that diff. Before committing, check `git diff Cargo.lock` and put the line back.
