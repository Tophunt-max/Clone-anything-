// Wire types shared by the web app and TypeScript packages. They mirror the Rust API
// (apps/api) and docs/platform/05-api-spec.md; change both together.

/** RFC 9457 problem details: the body of every API error. Decide on `code`, never on text. */
export interface Problem {
  type: string;
  title: string;
  status: number;
  code: string;
  detail: string;
  request_id?: string;
}

/** `GET /healthz` */
export interface Health {
  status: "ok";
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
