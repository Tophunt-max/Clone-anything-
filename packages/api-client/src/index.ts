// HTTP client for the platform API. Every request carries an `X-Request-Id`, sends cookies
// (the browser session is an HttpOnly cookie; this code never sees a token), and turns error
// responses into `ApiError` built from the server's problem body.
import type { Health, Meta, Problem } from "@forge/shared-types";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly title: string;
  readonly requestId: string | undefined;

  constructor(status: number, code: string, title: string, detail: string, requestId: string | undefined) {
    super(detail || title);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.title = title;
    this.requestId = requestId;
  }
}

export interface ApiClientOptions {
  /** Origin and prefix, e.g. "" (same origin) or "https://api.example.com". No trailing slash. */
  baseUrl?: string;
  fetch?: typeof fetch;
  newRequestId?: () => string;
}

function isProblem(v: unknown): v is Problem {
  return typeof v === "object" && v !== null && typeof (v as Problem).code === "string" && typeof (v as Problem).status === "number";
}

export function createApiClient(opts: ApiClientOptions = {}) {
  const base = opts.baseUrl ?? "";
  const doFetch = opts.fetch ?? ((input, init) => globalThis.fetch(input, init));
  const newId = opts.newRequestId ?? (() => crypto.randomUUID());

  async function request<T>(method: string, path: string, signal?: AbortSignal): Promise<T> {
    const requestId = newId();
    let resp: Response;
    try {
      resp = await doFetch(`${base}${path}`, {
        method,
        credentials: "include",
        headers: { Accept: "application/json", "X-Request-Id": requestId },
        ...(signal ? { signal } : {}),
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
      if (isProblem(body)) throw new ApiError(resp.status, body.code, body.title, body.detail, body.request_id ?? id);
      throw new ApiError(resp.status, "http_error", `HTTP ${resp.status}`, "The server answered with an unexpected error.", id);
    }
    if (body === undefined) throw new ApiError(resp.status, "bad_response", "Unexpected answer", "The server's answer was not JSON.", id);
    return body as T;
  }

  return {
    health: (signal?: AbortSignal) => request<Health>("GET", "/healthz", signal),
    meta: (signal?: AbortSignal) => request<Meta>("GET", "/api/v1/meta", signal),
  };
}

export type ApiClient = ReturnType<typeof createApiClient>;
