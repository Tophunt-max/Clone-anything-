-- Phase 4: single-use codes that hand a signed-in Android app's session to its WebView
-- (docs/platform/01-architecture.md §4). Only the SHA-256 of a code is stored.
create table login_codes (
    code_hash         bytea primary key check (length(code_hash) = 32),
    user_id           uuid not null references users (id) on delete cascade,
    source_session_id uuid not null references sessions (id) on delete cascade,
    created_at        timestamptz not null default now(),
    expires_at        timestamptz not null,
    used_at           timestamptz
);
create index login_codes_expiry_idx on login_codes (expires_at);
