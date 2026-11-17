-- Phase 5: engine sessions, tool executions, the evidence index and approvals
-- (docs/platform/04-database-design.md). Evidence bodies also live in the workspace volume;
-- this table is the searchable, durable index.

create table engine_sessions (
    id            uuid primary key,
    workspace_id  uuid not null references workspaces (id) on delete cascade,
    project_id    uuid not null references projects (id) on delete cascade,
    mode          text not null default 'clean_room' check (mode in ('clean_room', 'deep')),
    status        text not null default 'ready' check (status in ('starting', 'ready', 'stopped', 'failed')),
    started_at    timestamptz not null default now(),
    stopped_at    timestamptz
);
create index engine_sessions_project_idx on engine_sessions (project_id);

create table approvals (
    id             uuid primary key,
    workspace_id   uuid not null references workspaces (id) on delete cascade,
    project_id     uuid not null references projects (id) on delete cascade,
    subject_kind   text not null check (subject_kind in ('tool_call', 'deep_mode')),
    tool           text,
    summary        text not null,
    details        jsonb not null default '{}'::jsonb,
    status         text not null default 'pending' check (status in ('pending', 'approved', 'rejected', 'expired', 'used')),
    requested_by   uuid references users (id) on delete set null,
    decided_by     uuid references users (id) on delete set null,
    created_at     timestamptz not null default now(),
    decided_at     timestamptz,
    expires_at     timestamptz not null
);
create index approvals_pending_idx on approvals (workspace_id, created_at desc) where status = 'pending';

create table tool_executions (
    id              uuid primary key,
    workspace_id    uuid not null references workspaces (id) on delete cascade,
    project_id      uuid not null references projects (id) on delete cascade,
    user_id         uuid references users (id) on delete set null,
    tool            text not null,
    params_redacted jsonb not null default '{}'::jsonb,
    policy          text not null,
    approval_id     uuid references approvals (id) on delete set null,
    status          text not null check (status in ('succeeded', 'failed')),
    error_code      text,
    evidence_id     text,
    duration_ms     integer,
    created_at      timestamptz not null default now()
);
create index tool_executions_project_idx on tool_executions (project_id, created_at desc);

create table evidence_records (
    workspace_id uuid not null references workspaces (id) on delete cascade,
    project_id   uuid not null references projects (id) on delete cascade,
    evidence_id  text not null check (evidence_id ~ '^ev_[0-9a-f]{64}$'),
    operation    text not null,
    subject      jsonb not null default '{}'::jsonb,
    confidence   text,
    summary      text not null default '',
    tool         text,
    body         jsonb not null,
    created_at   timestamptz not null default now(),
    primary key (workspace_id, evidence_id)
);
create index evidence_project_idx on evidence_records (project_id, created_at desc);
create index evidence_operation_idx on evidence_records (project_id, operation);
