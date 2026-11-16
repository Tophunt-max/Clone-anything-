import { describe, expect, it, vi } from "vitest";
import { ApiError, createApiClient, csrfFromCookie } from "./index.ts";

const json = (status: number, body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", ...headers } });
const problem = (status: number, code: string) => json(status, { type: "about:blank", title: code, status, code, detail: `${code} detail`, request_id: "srv" });
const meta = { name: "Tophunt Forge", version: "0.1.0", api_version: "v1", features: [] };
const user = { id: "u1", email: "a@b.co", display_name: "A", created_at: "2026-01-01T00:00:00Z" };

type Call = [string, RequestInit];
const calls = (f: ReturnType<typeof vi.fn>) => f.mock.calls as unknown as Call[];
const header = (c: Call, h: string) => (c[1].headers as Record<string, string>)[h];

describe("api client", () => {
  it("sends a request id and cookies, and returns the body", async () => {
    const fetch = vi.fn(async () => json(200, meta));
    const api = createApiClient({ fetch, newRequestId: () => "rid-1" });
    expect((await api.meta()).api_version).toBe("v1");
    const [c] = calls(fetch);
    expect(c![0]).toBe("/api/v1/meta");
    expect(c![1].credentials).toBe("include");
    expect(header(c!, "X-Request-Id")).toBe("rid-1");
  });

  it("turns a problem body into an ApiError with code, field and request id", async () => {
    const fetch = vi.fn(async () => json(422, { type: "x", title: "Invalid input", status: 422, code: "bad_input", detail: "Enter a valid email address.", field: "email", request_id: "srv-1" }));
    const err = await createApiClient({ fetch }).login({ email: "x", password: "y" }).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ status: 422, code: "bad_input", field: "email", requestId: "srv-1", message: "Enter a valid email address." });
  });

  it("reports a network failure as network_error", async () => {
    const fetch = vi.fn(async () => {
      throw new TypeError("Failed to fetch");
    });
    const err = await createApiClient({ fetch, newRequestId: () => "rid-2" }).health().catch((e: unknown) => e);
    expect(err).toMatchObject({ status: 0, code: "network_error", requestId: "rid-2" });
  });

  it("reports a non-problem error and a non-JSON success honestly", async () => {
    const e1 = await createApiClient({ fetch: async () => new Response("oops", { status: 502, headers: { "x-request-id": "h-1" } }) }).health().catch((e: unknown) => e);
    expect(e1).toMatchObject({ status: 502, code: "http_error", requestId: "h-1" });
    const e2 = await createApiClient({ fetch: async () => new Response("<html>", { status: 200 }) }).health().catch((e: unknown) => e);
    expect(e2).toMatchObject({ code: "bad_response" });
  });

  it("sends JSON bodies and the CSRF token on writes only, and asks web sessions", async () => {
    const fetch = vi.fn(async () => json(200, { user, session_id: "s", access_expires_at: "x" }));
    const api = createApiClient({ fetch, readCsrf: () => "csrf-1" });
    await api.login({ email: "a@b.co", password: "pw" });
    await api.me();
    const [post, get] = calls(fetch);
    expect(post![1].method).toBe("POST");
    expect(header(post!, "X-CSRF-Token")).toBe("csrf-1");
    expect(header(post!, "Content-Type")).toBe("application/json");
    expect(JSON.parse(post![1].body as string)).toEqual({ email: "a@b.co", password: "pw", client: "web" });
    expect(header(get!, "X-CSRF-Token")).toBeUndefined();
  });

  it("204 answers resolve to undefined", async () => {
    const api = createApiClient({ fetch: async () => new Response(null, { status: 204 }) });
    await expect(api.deleteProject("p")).resolves.toBeUndefined();
  });

  it("refreshes once on 401 and retries, sharing one refresh between concurrent calls", async () => {
    let refreshed = false;
    const fetch = vi.fn(async (url: string) => {
      if (url === "/api/v1/auth/refresh") {
        refreshed = true;
        return json(200, { user, session_id: "s", access_expires_at: "x" });
      }
      return refreshed ? json(200, user) : problem(401, "unauthenticated");
    });
    const api = createApiClient({ fetch: fetch as unknown as typeof globalThis.fetch });
    const [a, b] = await Promise.all([api.me(), api.me()]);
    expect([a.id, b.id]).toEqual(["u1", "u1"]);
    expect(calls(fetch).filter((c) => c[0] === "/api/v1/auth/refresh")).toHaveLength(1);
  });

  it("ends the session when the refresh fails, and never refreshes for auth calls", async () => {
    const ended = vi.fn();
    const fetch = vi.fn(async () => problem(401, "unauthenticated"));
    const api = createApiClient({ fetch, onSessionEnded: ended });
    await expect(api.me()).rejects.toMatchObject({ status: 401 });
    expect(ended).toHaveBeenCalledOnce();
    fetch.mockClear();
    await expect(api.login({ email: "a@b.co", password: "x" })).rejects.toMatchObject({ status: 401 });
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("does not refresh for other 401 codes or 403", async () => {
    const fetch = vi.fn(async () => problem(403, "insufficient_role"));
    await expect(createApiClient({ fetch }).deleteProject("p")).rejects.toMatchObject({ code: "insufficient_role" });
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("builds query strings and escapes path parts", async () => {
    const fetch = vi.fn(async () => json(200, { items: [], next_cursor: null }));
    const api = createApiClient({ fetch });
    await api.projects("w/1", { cursor: "c", limit: 5, includeArchived: true });
    await api.projects("w");
    expect(calls(fetch).map((c) => c[0])).toEqual(["/api/v1/workspaces/w%2F1/projects?cursor=c&limit=5&include_archived=true", "/api/v1/workspaces/w/projects"]);
  });

  it("reads the CSRF cookie", () => {
    expect(csrfFromCookie("a=1; forge_csrf=abc%3D; b=2")).toBe("abc=");
    expect(csrfFromCookie("a=1")).toBeUndefined();
  });
});
