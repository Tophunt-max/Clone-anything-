# Development

## Requirements

| Part | Tools |
|---|---|
| Rust (`apps/api`, `apps/workers`, upstream crates) | Rust 1.97 or newer |
| Web (`apps/web`, `packages/*`) | Node 22 (`.nvmrc`), pnpm 12 (`packageManager` in `package.json`, via `corepack enable`) |
| Android (`apps/android`) | JDK 17 or 21, Android SDK platform 35. Gradle comes from the wrapper |

## Commands

```sh
# Rust (the API needs PostgreSQL 16; see "Database" below)
cargo run -p forge-api -- migrate           # apply migrations and exit
cargo run -p forge-api                      # API on 127.0.0.1:8080; migrates on start by default
cargo test -p forge-api -p forge-worker     # DB tests skip unless FORGE_TEST_DATABASE_URL is set
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
| `FORGE_DATABASE_URL` | required | `postgres://user:pass@host:5432/db` |
| `FORGE_DATABASE_MAX_CONNECTIONS` | `10` | Pool size (1–500) |
| `FORGE_TOKEN_KEY` | required | base64 of ≥ 32 random bytes (`openssl rand -base64 32`). Rotating it signs everyone out |
| `FORGE_COOKIE_SECURE` | `true` | Only `false` for plain-HTTP local development |
| `FORGE_TRUST_PROXY` | `false` | Use the last `X-Forwarded-For` entry as the client address; only behind our reverse proxy |
| `FORGE_MIGRATE_ON_START` | `true` | Apply pending migrations before serving |
| `RUST_LOG` | `info` | Log filter; logs are JSON lines with the request ID |

## Database

Any PostgreSQL 16 works, e.g. `docker run -e POSTGRES_USER=forge -e POSTGRES_PASSWORD=dev -p 5432:5432 postgres:16`.
Tests create one throwaway database per test, so the test user needs `CREATEDB`:

```sh
export FORGE_TEST_DATABASE_URL=postgres://forge:dev@127.0.0.1:5432/postgres
cargo test -p forge-api
```

`FORGE_REQUIRE_DB_TESTS=1` (set in CI) makes the tests fail instead of skipping when the URL is
missing. Test databases are named `forge_test_<id>` and are not dropped; remove them with
`dropdb` when a local server fills up.

## Rules carried over from upstream

`AGENTS.md` applies to the Rust code: no `unwrap`/`expect`/`panic` outside tests, workspace lints,
`cargo fmt`, `clippy -D warnings`, `cargo xtask layers|locks|secrets`, and no secrets or local
paths in commits. Screens never hard-code colours or sizes; they use the tokens.

## Pitfall: a local `[patch]` for `craft-libs` rewrites `Cargo.lock`

Where `github.com/storytold/craft-libs` can't be fetched with git, a `[patch."https://github.com/storytold/craft-libs"]`
pointing at a local copy works for building, but every cargo command then drops the
`source = "git+…craft-libs…"` line of `craft-update` from `Cargo.lock`. The upstream gate fails on
that diff. Before committing, check `git diff Cargo.lock` and put the line back.
