// Wire types shared by the web app and TypeScript packages. They mirror the Rust API
// (apps/api) and docs/platform/05-api-spec.md; change both together.

/** RFC 9457 problem details: the body of every API error. Decide on `code`, never on text. */
export interface Problem {
  type: string;
  title: string;
  status: number;
  code: string;
  detail: string;
  /** The request field a validation error is about. */
  field?: string;
  request_id?: string;
}

/** `GET /healthz`, `GET /readyz` */
export interface Health {
  status: "ok" | "ready";
}

/** `GET /api/v1/meta` */
export interface Meta {
  name: string;
  version: string;
  api_version: string;
  /** Features the server has turned on; clients hide what is not listed. */
  features: string[];
}

/** Cursor-paginated list. `next_cursor` is null on the last page. */
export interface Page<T> {
  items: T[];
  next_cursor: string | null;
}

/** One real-time event (SSE or WebSocket). `seq` increases per topic and is used to resume. */
export interface EventEnvelope<P = unknown> {
  topic: string;
  seq: number;
  kind: string;
  ts: string;
  payload: P;
}

export interface User {
  id: string;
  email: string;
  display_name: string;
  created_at: string;
}

/** Answer of register, login and refresh. Web clients never receive tokens, only `csrf_token`. */
export interface SessionStart {
  user: User;
  session_id: string;
  access_expires_at: string;
  csrf_token?: string;
}

/** `GET /api/v1/me/sessions` */
export interface DeviceSession {
  id: string;
  device_kind: "web" | "android";
  device_name: string;
  created_at: string;
  last_refreshed_at: string;
  expires_at: string;
  current: boolean;
}

export type Role = "viewer" | "editor" | "maintainer" | "admin" | "owner";
export const ROLES: readonly Role[] = ["viewer", "editor", "maintainer", "admin", "owner"];

export interface Workspace {
  id: string;
  name: string;
  slug: string;
  /** The caller's role in it. */
  role: Role;
  created_at: string;
}

export interface Member {
  user_id: string;
  email: string;
  display_name: string;
  role: Role;
  created_at: string;
}

export type ToolPolicy = "viewer" | "editor" | "editor_worker" | "approval_gate" | "operator_only" | "unavailable";

/** One entry of `GET /api/v1/tools`. */
export interface Tool {
  id: string;
  layer: "engine" | "ai_or_setup";
  toolset: string;
  title: string;
  summary: string;
  effects: string[];
  needs_consent: boolean;
  read_only: boolean;
  requires: string;
  available_here: boolean;
  policy: ToolPolicy;
  input_schema: unknown;
}

/** Result of `POST /projects/{id}/tool-calls` (when it runs). */
export interface ToolCallResult {
  status: "succeeded";
  tool: string;
  policy: ToolPolicy;
  output: unknown;
  evidence_id: string | null;
  duration_ms: number;
}

/** One past tool run. */
export interface ToolExecution {
  id: string;
  tool: string;
  policy: string;
  status: "succeeded" | "failed";
  error_code: string | null;
  evidence_id: string | null;
  duration_ms: number | null;
  created_at: string;
}

/** An evidence record in the project index (summary row). */
export interface EvidenceRow {
  evidence_id: string;
  operation: string;
  confidence: string | null;
  summary: string;
  tool: string | null;
  subject: { kind?: string; name?: string; sha256?: string } & Record<string, unknown>;
  created_at: string;
}

/** A pending approval for a tool that runs code or changes a device. */
export interface Approval {
  id: string;
  project_id: string;
  subject_kind: "tool_call" | "deep_mode";
  tool: string | null;
  summary: string;
  status: string;
  created_at: string;
  expires_at: string;
}

export type ProjectKind = "code" | "analysis" | "rebuild";

export interface Project {
  id: string;
  workspace_id: string;
  name: string;
  kind: ProjectKind;
  deep_mode: boolean;
  created_at: string;
  updated_at: string;
  archived_at: string | null;
}
