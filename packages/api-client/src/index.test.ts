import { describe, expect, it, vi } from "vitest";
import { ApiError, createApiClient } from "./index.ts";

const json = (status: number, body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", ...headers } });

describe("api client", () => {
  it("sends a request id and cookies, and returns the body", async () => {
    const fetch = vi.fn(async () => json(200, { name: "Tophunt Forge", version: "0.1.0", api_version: "v1", features: [] }));
    const api = createApiClient({ fetch, newRequestId: () => "rid-1" });
    const meta = await api.meta();
    expect(meta.api_version).toBe("v1");
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/v1/meta");
    expect(init.credentials).toBe("include");
    expect((init.headers as Record<string, string>)["X-Request-Id"]).toBe("rid-1");
  });

  it("turns a problem body into an ApiError with code and request id", async () => {
    const fetch = vi.fn(async () => json(404, { type: "about:blank#not_found", title: "Not found", status: 404, code: "not_found", detail: "Nothing here.", request_id: "srv-1" }));
    const err = await createApiClient({ fetch }).meta().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ status: 404, code: "not_found", requestId: "srv-1", message: "Nothing here." });
  });

  it("reports a network failure as network_error", async () => {
    const fetch = vi.fn(async () => {
      throw new TypeError("Failed to fetch");
    });
    const err = await createApiClient({ fetch, newRequestId: () => "rid-2" }).health().catch((e: unknown) => e);
    expect(err).toMatchObject({ status: 0, code: "network_error", requestId: "rid-2" });
  });

  it("reports a non-problem error and a non-JSON success honestly", async () => {
    const e1 = await createApiClient({ fetch: async () => new Response("oops", { status: 502, headers: { "x-request-id": "h-1" } }) })
      .health()
      .catch((e: unknown) => e);
    expect(e1).toMatchObject({ status: 502, code: "http_error", requestId: "h-1" });
    const e2 = await createApiClient({ fetch: async () => new Response("<html>", { status: 200 }) })
      .health()
      .catch((e: unknown) => e);
    expect(e2).toMatchObject({ code: "bad_response" });
  });
});
