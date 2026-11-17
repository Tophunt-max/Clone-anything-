import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter } from "react-router";
import { ApiError, type ApiClient } from "@forge/api-client";
import type { EvidenceRow, Page, Tool, User, Workspace } from "@forge/shared-types";
import { App, makeQueryClient } from "./App.tsx";
import { routes } from "./routes.tsx";
import { useCurrentWorkspaceId } from "./lib/workspace.ts";

const now = "2026-11-18T00:00:00Z";
const user: User = { id: "u1", email: "a@b.co", display_name: "Ada", created_at: now };
const ws = (): Workspace => ({ id: "w1", name: "Team", slug: "team", role: "owner", created_at: now });
const page = <T,>(items: T[]): Page<T> => ({ items, next_cursor: null });

const tool = (id: string, toolset: string, policy: Tool["policy"], extra: Partial<Tool> = {}): Tool => ({
  id,
  layer: "engine",
  toolset,
  title: id,
  summary: `summary of ${id}`,
  effects: [],
  needs_consent: policy === "approval_gate",
  read_only: policy === "viewer",
  requires: "nothing",
  available_here: true,
  policy,
  input_schema: {},
  ...extra,
});

function fakeApi(over: Partial<ApiClient> = {}): ApiClient {
  const base: Partial<ApiClient> = { me: vi.fn(async () => user), workspaces: vi.fn(async () => page([ws()])) };
  return new Proxy({}, { get: (_t, p: string) => (p in over ? over[p as keyof ApiClient] : p in base ? base[p as keyof ApiClient] : vi.fn(async () => Promise.reject(new Error(`unexpected ${p}`)))) }) as ApiClient;
}

function renderAt(path: string, api: ApiClient) {
  const router = createMemoryRouter(routes, { initialEntries: [path] });
  const qc = makeQueryClient();
  qc.setDefaultOptions({ queries: { retry: false } });
  render(<App router={router} api={api} queryClient={qc} />);
  return router;
}

afterEach(() => {
  useCurrentWorkspaceId.setState({ id: null });
  localStorage.clear();
});

describe("tool registry", () => {
  it("groups tools by toolset and shows policy, filtered by search", async () => {
    const tools = vi.fn(async () => [tool("core_session_show", "core", "viewer"), tool("build_command_run", "build", "approval_gate"), tool("engines_install", "setup", "operator_only")]);
    renderAt("/tools", fakeApi({ tools }));
    expect(await screen.findByText("core_session_show")).toBeInTheDocument();
    expect(screen.getByText("needs approval")).toBeInTheDocument();
    expect(screen.getByText("operator only")).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("Search"), "build");
    expect(screen.getByText("build_command_run")).toBeInTheDocument();
    expect(screen.queryByText("core_session_show")).not.toBeInTheDocument();
  });
});

describe("evidence explorer", () => {
  const rows: EvidenceRow[] = [
    { evidence_id: "ev_" + "a".repeat(64), operation: "triage", confidence: "high", summary: "A shell script", tool: "core_target_triage", subject: { kind: "file", name: "x.sh" }, created_at: now },
    { evidence_id: "ev_" + "b".repeat(64), operation: "native.functions", confidence: "medium", summary: "42 functions", tool: "native_functions_list", subject: { kind: "binary" }, created_at: now },
  ];

  it("lists evidence, filters by operation, and shows a record", async () => {
    const evidence = vi.fn(async (_p: string, a: { operation?: string } = {}) => page(a.operation ? rows.filter((r) => r.operation === a.operation) : rows));
    const evidenceRecord = vi.fn(async () => ({ id: rows[0]!.evidence_id, operation: "triage", confidence: "high", summary: "A shell script", authority: "first_party", environment: { engine: "built-in" }, limitations: ["static_only"] }));
    renderAt("/p/p1/evidence", fakeApi({ evidence: evidence as unknown as ApiClient["evidence"], evidenceRecord }));
    expect(await screen.findByText("A shell script")).toBeInTheDocument();
    expect(screen.getByText("42 functions")).toBeInTheDocument();

    await userEvent.click(screen.getByText("A shell script"));
    expect(await screen.findByText("Evidence record")).toBeInTheDocument();
    expect(screen.getByText("static_only")).toBeInTheDocument();

    await userEvent.selectOptions(screen.getByLabelText("Filter by operation"), "native.functions");
    await waitFor(() => expect(screen.queryByRole("button", { name: /A shell script/ })).not.toBeInTheDocument());
    expect(screen.getByText("42 functions")).toBeInTheDocument();
  });

  it("shows the empty state", async () => {
    renderAt("/p/p1/evidence", fakeApi({ evidence: vi.fn(async () => page([])) as unknown as ApiClient["evidence"] }));
    expect(await screen.findByText(/No evidence yet/)).toBeInTheDocument();
  });
});

describe("binary analyzer", () => {
  function file(name: string) {
    return new File([new Uint8Array([0x7f, 0x45, 0x4c, 0x46])], name, { type: "application/octet-stream" });
  }

  it("uploads a file then runs triage and links to evidence", async () => {
    const uploadTarget = vi.fn(async (_p: string, f: File) => ({ path: `/ws/p/targets/${f.name}`, name: f.name, size: 4, sha256: "a".repeat(64) }));
    const runTool = vi.fn(async () => ({ status: "succeeded" as const, tool: "core_target_triage", policy: "editor" as const, output: {}, evidence_id: "ev_" + "c".repeat(64), duration_ms: 12 }));
    renderAt("/p/p1/analyze/binary", fakeApi({ uploadTarget, runTool }));
    await screen.findByText("Binary analyzer");
    await userEvent.upload(screen.getByLabelText("File to analyze"), file("thing.so"));
    expect(await screen.findByText(/Uploaded/)).toBeInTheDocument();
    const triage = (await screen.findByText("Triage (what is it?)")).closest("li") as HTMLElement;
    await userEvent.click(within(triage).getByRole("button", { name: "Run" }));
    expect(await within(triage).findByText("View evidence")).toBeInTheDocument();
    expect(uploadTarget).toHaveBeenCalled();
    expect(runTool).toHaveBeenCalledWith("p1", { tool: "core_target_triage", params: { path: "/ws/p/targets/thing.so" } });
  });

  it("shows the approval flow for a tool that needs it", async () => {
    const uploadTarget = vi.fn(async (_p: string, f: File) => ({ path: `/t/${f.name}`, name: f.name, size: 4, sha256: "a".repeat(64) }));
    const runTool = vi
      .fn()
      .mockRejectedValueOnce(new ApiError(409, "approval_required", "Approval needed", "A maintainer must approve it.", "rid-1", "appr-1"))
      .mockResolvedValue({ status: "succeeded", tool: "core_target_open", policy: "editor", output: {}, evidence_id: null, duration_ms: 3 });
    const approve = vi.fn(async () => undefined);
    renderAt("/p/p1/analyze/binary", fakeApi({ uploadTarget, runTool, approve }));
    await userEvent.upload(await screen.findByLabelText("File to analyze"), file("thing.bin"));
    await screen.findByText(/Uploaded/);
    const openStep = (await screen.findByText("Open as the active target")).closest("li") as HTMLElement;
    await userEvent.click(within(openStep).getByRole("button", { name: "Run" }));
    expect(await within(openStep).findByText(/needs approval/)).toBeInTheDocument();
    await userEvent.click(within(openStep).getByRole("button", { name: "Approve and run" }));
    await waitFor(() => expect(approve).toHaveBeenCalledWith("appr-1"));
    expect(runTool).toHaveBeenLastCalledWith("p1", { tool: "core_target_open", params: { path: "/t/thing.bin" }, approval_id: "appr-1" });
  });
});
