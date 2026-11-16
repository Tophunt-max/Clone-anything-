# Security threat model

Status: **proposed** (Phase 0). Phase 2 implemented and tested T1 (without MFA), T2, T3 (no tokens in JS), T4, T5 (without RLS), T6, T18 (passwords and refresh tokens only) and T21 (auth, workspace, member and project events). Method: STRIDE per trust boundary. Each threat lists the control
and the phase that implements and tests it.

## 1. Assets

User credentials and sessions; AI provider keys; Git tokens and SSH keys; source code in
workspaces; analysis targets uploaded by users (may be confidential); evidence and reports;
build artifacts and signing keys; the host machines; audit logs.

## 2. Trust boundaries

1. Internet ↔ reverse proxy / API.
2. Android native code ↔ WebView JavaScript.
3. API ↔ workspace sandboxes (workers).
4. Sandbox ↔ untrusted content inside it (APKs, binaries, websites, repositories, user code, AI output).
5. Sandbox ↔ outside network (AI providers, Git hosts, package registries, analyzed websites).
6. API ↔ PostgreSQL and object storage.

## 3. Threats and controls

| # | Boundary | Threat | Control | Phase |
|---|---|---|---|---|
| T1 | 1 | Credential stuffing, brute force | Argon2id, per-IP and per-account rate limits, lockout with backoff, optional TOTP | 2 |
| T2 | 1 | Stolen refresh token reused | Rotation with reuse detection revokes the session family; device list with revoke | 2 |
| T3 | 1 | XSS steals tokens | No tokens in JS (HttpOnly cookies), strict CSP (no inline script), Trusted Types where supported | 2–3 |
| T4 | 1 | CSRF | `SameSite=Strict` + double-submit CSRF header on state-changing requests | 2 |
| T5 | 1 | IDOR / cross-tenant access by changing IDs | Server-side membership check on every ID, 404 for unseen resources, RLS as backstop, tenant isolation test suite | 2, 9 |
| T6 | 1 | Oversized or malformed requests | Body limits, `deny_unknown_fields`, timeouts, upload size quotas | 2 |
| T7 | 2 | Remote page calls native bridge | Bridge only via `addWebMessageListener` with the trusted origin; non-trusted origins never loaded in the app WebView | 4 |
| T8 | 2 | Trusted page compromised (XSS) abuses bridge | Small allowlist of harmless methods, schema validation, no file paths or shell, user confirmation for share/open | 4 |
| T9 | 2 | Debugging a release WebView | `setWebContentsDebuggingEnabled(false)` in release; test asserts it | 4 |
| T10 | 3 | Worker impersonates API or another tenant | One Unix socket per sandbox, mounted only into that sandbox; API binds socket to workspace id | 5 |
| T11 | 3 | Agent bypasses approvals | Consent decided only by the API; engine runs with `ask_before_running = true`; `CRAFT_ANYTHING_AGENT` env stops nested CLI from loosening approvals (upstream) | 5–6 |
| T12 | 4 | Malicious target exploits a parser | Parsers run inside the sandbox, never in the API; per-call timeouts; upstream has no `unsafe` code; memory and CPU quotas | 5 |
| T13 | 4 | Foreign code escapes the sandbox | Rootless containers, gVisor runtime, read-only root fs, no Docker socket, no host mounts, seccomp default profile, dropped capabilities | 8–9 |
| T14 | 4 | Resource exhaustion (fork bomb, disk fill, crypto mining) | `pids-limit`, CPU and memory caps, disk quota, wall-clock limits, idle stop, usage alerts | 8 |
| T15 | 5 | Sandbox reaches internal network or cloud metadata | Default-deny egress; allowlisting proxy; block RFC1918, link-local and metadata addresses | 8 |
| T16 | 5 | Analyzed website or AI output carries prompt injection | Tool results with `CARRIES_TARGET_CONTENT` are marked untrusted to the model; sensitive actions always need human approval regardless of what the model asks | 6 |
| T17 | 5 | Secret leakage in logs or AI context | Redaction in API logs (same field list as upstream `journal::redacted`), keys injected as env only for the call, never written to disk in the sandbox, never sent to clients after creation | 2, 6 |
| T18 | 6 | Database dump exposes secrets | Provider keys and Git credentials encrypted with AES-256-GCM, KEK outside the DB; password and token hashes only | 2 |
| T19 | 6 | Lost data | Nightly dumps + WAL archiving; tested restore | 9 |
| T20 | all | Unauthorized reverse engineering | Clean room by default; Deep mode needs maintainer role and a recorded authorization statement; dynamic analysis tools (`EXECUTES_FOREIGN_CODE`) always approval-gated; device and instrumentation tools unavailable in hosted mode | 5 |
| T21 | all | Repudiation | Append-only audit log of auth events, approvals, tool calls, Git pushes, Deep mode changes, with request IDs | 2+ |
| T22 | 1 | Terminal used as a root shell | PTY only inside the workspace sandbox as a non-root user; no host shell exists to connect to; session expiry; sensitive-command approval (e.g. `git push --force`, credential files) | 7 |

## 4. Out of scope (will not be built)

Unauthorized access to third-party systems, credential theft, DRM or anti-cheat bypass, malware
creation or deployment, exploitation of systems the user does not own or have written permission
to test. The platform supports authorized analysis only, and static analysis is the default.

## 5. Open questions

- Which KMS (if any) will hold the key-encryption key on the target host? Default: a file
  readable only by the API service user, outside the repository.
- Whether gVisor is available on the target Ubuntu host's kernel and container runtime.
- Push notifications provider (Firebase Cloud Messaging) and its data-handling terms.
