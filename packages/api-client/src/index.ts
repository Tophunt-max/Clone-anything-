// HTTP client for the platform API.
//
// - Every request carries an `X-Request-Id` and sends cookies. The browser session lives in
//   HttpOnly cookies, so this code never sees an access or refresh token.
// - State-changing requests echo the readable `forge_csrf` cookie in `X-CSRF-Token`.
// - A 401 on a signed-in call triggers one refresh (shared by concurrent callers), then one retry.
// - Error responses become `ApiError`, built from the server's problem body.
import type { DeviceSession, Health, Member, Meta, Page, Problem, Project, ProjectKind, Role, SessionStart, User, Workspace } from "@forge/shared-types";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly title: string;
  readonly field: string | undefined;
  readonly requestId: string | undefined;

  constructor(status: number, code: string, title: string, detail: string, requestId: string | undefined, field?: string) {
    super(detail || title);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.title = title;
    this.requestId = requestId;
    this.field = field;
  }
}

export interface ApiClientOptions {
  /** Origin and prefix, e.g. "" (same origin) or "https://api.example.com". No trailing slash. */
  baseUrl?: string;
  fetch?: typeof fetch;
  newRequestId?: () => string;
  /** Reads the CSRF cookie. Defaults to `document.cookie`. */
  readCsrf?: () => string | undefined;
  /** Called when a refresh fails: the session is over. */
  onSessionEnded?: () => void;
}

function isProblem(v: unknown): v is Problem {
  return typeof v === "object" && v !== null && typeof (v as Problem).code === "string" && typeof (v as Problem).status === "number";
}

export function csrfFromCookie(cookie: string): string | undefined {
  const m = /(?:^|;\s*)forge_csrf=([^;]*)/.exec(cookie);
  return m?.[1] ? decodeURIComponent(m[1]) : undefined;
}

function defaultCsrf(): string | undefined {
  return typeof document === "undefined" ? undefined : csrfFromCookie(document.cookie);
}

interface CallOpts {
  body?: unknown;
  signal?: AbortSignal | undefined;
  /** Try a refresh and retry once on 401. */
  refresh?: boolean;
}

const SAFE = new Set(["GET", "HEAD", "OPTIONS"]);

export function createApiClient(opts: ApiClientOptions = {}) {
  const base = opts.baseUrl ?? "";
  const doFetch = opts.fetch ?? ((input, init) => globalThis.fetch(input, init));
  const newId = opts.newRequestId ?? (() => crypto.randomUUID());
  const readCsrf = opts.readCsrf ?? defaultCsrf;
  let refreshing: Promise<boolean> | null = null;

  async function raw<T>(method: string, path: string, o: CallOpts): Promise<T> {
    const requestId = newId();
    const headers: Record<string, string> = { Accept: "application/json", "X-Request-Id": requestId };
    if (o.body !== undefined) headers["Content-Type"] = "application/json";
    if (!SAFE.has(method)) {
      const csrf = readCsrf();
      if (csrf) headers["X-CSRF-Token"] = csrf;
    }
    let resp: Response;
    try {
      resp = await doFetch(`${base}${path}`, {
        method,
        credentials: "include",
        headers,
        ...(o.body !== undefined ? { body: JSON.stringify(o.body) } : {}),
        ...(o.signal ? { signal: o.signal } : {}),
      });
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") throw e;
      throw new ApiError(0, "network_error", "Can't reach the server", "Check your connection and the server address, then try again.", requestId);
    }
    const id = resp.headers.get("x-request-id") ?? requestId;
    const text = await resp.text();
    let body: unknown = undefined;
    if (text) {
      try {
        body = JSON.parse(text);
      } catch {
        body = undefined;
      }
    }
    if (!resp.ok) {
      if (isProblem(body)) throw new ApiError(resp.status, body.code, body.title, body.detail, body.request_id ?? id, body.field);
      throw new ApiError(resp.status, "http_error", `HTTP ${resp.status}`, "The server answered with an unexpected error.", id);
    }
    if (resp.status === 204) return undefined as T;
    if (body === undefined) throw new ApiError(resp.status, "bad_response", "Unexpected answer", "The server's answer was not JSON.", id);
    return body as T;
  }

  /** One refresh at a time; everyone waiting gets its result. */
  function refreshOnce(): Promise<boolean> {
    refreshing ??= raw<SessionStart>("POST", "/api/v1/auth/refresh", { refresh: false })
      .then(() => true)
      .catch(() => {
        opts.onSessionEnded?.();
        return false;
      })
      .finally(() => {
        refreshing = null;
      });
    return refreshing;
  }

  async function call<T>(method: string, path: string, o: CallOpts = {}): Promise<T> {
    try {
      return await raw<T>(method, path, o);
    } catch (e) {
      if (o.refresh !== false && e instanceof ApiError && e.status === 401 && e.code === "unauthenticated" && (await refreshOnce())) {
        return raw<T>(method, path, o);
      }
      throw e;
    }
  }

  const q = (params: Record<string, string | number | boolean | undefined>) => {
    const s = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) if (v !== undefined) s.set(k, String(v));
    const t = s.toString();
    return t ? `?${t}` : "";
  };
  const enc = encodeURIComponent;
  type PageArgs = { cursor?: string; limit?: number; signal?: AbortSignal };

  return {
    health: (signal?: AbortSignal) => call<Health>("GET", "/healthz", { signal, refresh: false }),
    ready: (signal?: AbortSignal) => call<Health>("GET", "/readyz", { signal, refresh: false }),
    meta: (signal?: AbortSignal) => call<Meta>("GET", "/api/v1/meta", { signal, refresh: false }),

    register: (b: { email: string; password: string; display_name: string }) => call<SessionStart>("POST", "/api/v1/auth/register", { body: { ...b, client: "web" }, refresh: false }),
    login: (b: { email: string; password: string }) => call<SessionStart>("POST", "/api/v1/auth/login", { body: { ...b, client: "web" }, refresh: false }),
    logout: () => call<void>("POST", "/api/v1/auth/logout"),
    me: (signal?: AbortSignal) => call<User>("GET", "/api/v1/me", { signal }),
    sessions: (signal?: AbortSignal) => call<DeviceSession[]>("GET", "/api/v1/me/sessions", { signal }),
    revokeSession: (id: string) => call<void>("DELETE", `/api/v1/me/sessions/${enc(id)}`),

    workspaces: (a: PageArgs = {}) => call<Page<Workspace>>("GET", `/api/v1/workspaces${q({ cursor: a.cursor, limit: a.limit })}`, { signal: a.signal }),
    createWorkspace: (b: { name: string; slug: string }) => call<Workspace>("POST", "/api/v1/workspaces", { body: b }),
    workspace: (id: string, signal?: AbortSignal) => call<Workspace>("GET", `/api/v1/workspaces/${enc(id)}`, { signal }),
    renameWorkspace: (id: string, name: string) => call<Workspace>("PATCH", `/api/v1/workspaces/${enc(id)}`, { body: { name } }),
    deleteWorkspace: (id: string) => call<void>("DELETE", `/api/v1/workspaces/${enc(id)}`),

    members: (ws: string, a: PageArgs = {}) => call<Page<Member>>("GET", `/api/v1/workspaces/${enc(ws)}/members${q({ cursor: a.cursor, limit: a.limit })}`, { signal: a.signal }),
    addMember: (ws: string, b: { email: string; role: Role }) => call<Member>("POST", `/api/v1/workspaces/${enc(ws)}/members`, { body: b }),
    changeMember: (ws: string, user: string, role: Role) => call<Member>("PATCH", `/api/v1/workspaces/${enc(ws)}/members/${enc(user)}`, { body: { role } }),
    removeMember: (ws: string, user: string) => call<void>("DELETE", `/api/v1/workspaces/${enc(ws)}/members/${enc(user)}`),

    projects: (ws: string, a: PageArgs & { includeArchived?: boolean } = {}) =>
      call<Page<Project>>("GET", `/api/v1/workspaces/${enc(ws)}/projects${q({ cursor: a.cursor, limit: a.limit, include_archived: a.includeArchived || undefined })}`, { signal: a.signal }),
    createProject: (ws: string, b: { name: string; kind: ProjectKind }) => call<Project>("POST", `/api/v1/workspaces/${enc(ws)}/projects`, { body: b }),
    project: (id: string, signal?: AbortSignal) => call<Project>("GET", `/api/v1/projects/${enc(id)}`, { signal }),
    updateProject: (id: string, b: { name?: string; archived?: boolean }) => call<Project>("PATCH", `/api/v1/projects/${enc(id)}`, { body: b }),
    deleteProject: (id: string) => call<void>("DELETE", `/api/v1/projects/${enc(id)}`),
  };
}

export type ApiClient = ReturnType<typeof createApiClient>;
