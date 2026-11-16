import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter } from "react-router";
import { ApiError, type ApiClient } from "@forge/api-client";
import type { Member, Page, Project, Role, User, Workspace } from "@forge/shared-types";
import { App, makeQueryClient } from "./App.tsx";
import { routes } from "./routes.tsx";
import { safeNext } from "./auth.tsx";
import { useCurrentWorkspaceId } from "./lib/workspace.ts";
import { useDiagnostics } from "./lib/diagnostics.ts";

const now = "2026-11-16T10:00:00Z";
const user: User = { id: "u1", email: "ada@example.com", display_name: "Ada", created_at: now };
const ws = (role: Role, id = "w1"): Workspace => ({ id, name: `Team ${id}`, slug: `team-${id}`, role, created_at: now });
const project = (id: string, extra: Partial<Project> = {}): Project => ({ id, workspace_id: "w1", name: `Project ${id}`, kind: "code", deep_mode: false, created_at: now, updated_at: now, archived_at: null, ...extra });
const page = <T,>(items: T[], next: string | null = null): Page<T> => ({ items, next_cursor: next });
const unauth = () => new ApiError(401, "unauthenticated", "Sign in required", "Sign in again to continue.", "rid-401");

/** A fake API: every method fails loudly unless the test provides it. */
function fakeApi(over: Partial<ApiClient> = {}): ApiClient {
  const handler: ProxyHandler<object> = {
    get: (_, prop: string) => (prop in over ? over[prop as keyof ApiClient] : vi.fn(async () => Promise.reject(new Error(`unexpected call: ${prop}`)))),
  };
  return new Proxy({}, handler) as ApiClient;
}

function signedIn(role: Role = "owner", over: Partial<ApiClient> = {}): ApiClient {
  return fakeApi({
    me: vi.fn(async () => user),
    workspaces: vi.fn(async () => page([ws(role)])),
    sessions: vi.fn(async () => [{ id: "s1", device_kind: "web" as const, device_name: "Web browser", created_at: now, last_refreshed_at: now, expires_at: now, current: true }]),
    projects: vi.fn(async () => page<Project>([])),
    ...over,
  });
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
  useDiagnostics.getState().clear();
  localStorage.clear();
});

describe("sign-in", () => {
  it("sends signed-out people to sign-in and remembers where they were going", async () => {
    const router = renderAt("/projects?x=1", fakeApi({ me: vi.fn(async () => Promise.reject(unauth())) }));
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/login");
    expect(new URLSearchParams(router.state.location.search).get("next")).toBe("/projects?x=1");
  });

  it("signs in and goes back to the page asked for", async () => {
    const login = vi.fn(async () => ({ user, session_id: "s", access_expires_at: now, csrf_token: "c" }));
    const api = signedIn("owner", { login });
    const router = renderAt("/login?next=%2Fsettings%2Fsecurity", api);
    await userEvent.type(screen.getByLabelText("Email"), "ada@example.com");
    await userEvent.type(screen.getByLabelText("Password"), "a long enough passphrase");
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(await screen.findByRole("heading", { name: "Security" })).toBeInTheDocument();
    expect(login).toHaveBeenCalledWith({ email: "ada@example.com", password: "a long enough passphrase" });
    expect(router.state.location.pathname).toBe("/settings/security");
  });

  it("shows the server's message and request id when sign-in fails", async () => {
    const login = vi.fn(async () => Promise.reject(new ApiError(401, "invalid_credentials", "Not signed in", "The email or password is wrong.", "rid-7")));
    renderAt("/login", fakeApi({ login }));
    await userEvent.type(screen.getByLabelText("Email"), "a@b.co");
    await userEvent.type(screen.getByLabelText("Password"), "nope");
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The email or password is wrong.");
    expect(alert).toHaveTextContent("rid-7");
  });

  it("checks the password before registering and shows field errors from the server", async () => {
    const register = vi.fn(async () => Promise.reject(new ApiError(409, "email_taken", "Conflict", "An account already uses this email. Sign in instead.", "rid-8")));
    renderAt("/register", fakeApi({ register }));
    await userEvent.type(screen.getByLabelText("Your name"), "Ada");
    await userEvent.type(screen.getByLabelText("Email"), "ada@example.com");
    await userEvent.type(screen.getByLabelText("Password"), "short");
    expect(screen.getByText("Use at least 12 characters.")).toBeInTheDocument();
    await userEvent.clear(screen.getByLabelText("Password"));
    await userEvent.type(screen.getByLabelText("Password"), "a long enough passphrase");
    await userEvent.type(screen.getByLabelText("Repeat password"), "a different passphrase");
    expect(screen.getByText("The passwords don't match.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create account" })).toBeDisabled();
    await userEvent.clear(screen.getByLabelText("Repeat password"));
    await userEvent.type(screen.getByLabelText("Repeat password"), "a long enough passphrase");
    await userEvent.click(screen.getByRole("button", { name: "Create account" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("already uses this email");
  });

  it("only follows same-site next paths", () => {
    expect(safeNext("/p/1")).toBe("/p/1");
    for (const bad of [null, "", "https://evil.example", "//evil.example", "/\\evil.example"]) expect(safeNext(bad)).toBe("/");
  });
});

describe("shell and dashboard", () => {
  it("renders both navigations so CSS can pick one per breakpoint", async () => {
    renderAt("/", signedIn());
    expect(await screen.findByRole("heading", { name: "Hello, Ada" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "Main navigation" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Bottom navigation" })).toBeInTheDocument();
  });

  it("offers project creation only to roles that may create", async () => {
    renderAt("/", signedIn("owner"));
    expect(await screen.findByRole("link", { name: "Create a project" })).toBeInTheDocument();
  });

  it("hides project creation from viewers", async () => {
    renderAt("/", signedIn("viewer"));
    expect(await screen.findByText("No projects in this workspace yet.")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Create a project" })).not.toBeInTheDocument();
  });

  it("shows server errors with a retry", async () => {
    const projects = vi.fn().mockRejectedValueOnce(new ApiError(503, "timed_out", "Request timed out", "Try again.", "rid-9")).mockResolvedValue(page([project("a")]));
    renderAt("/projects", signedIn("owner", { projects }));
    expect(await screen.findByText("rid-9", { exact: false })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Project a")).toBeInTheDocument();
    expect(useDiagnostics.getState().errors[0]?.requestId).toBe("rid-9");
  });
});

describe("projects", () => {
  it("lists projects page by page", async () => {
    const projects = vi.fn(async (_ws: string, a: { cursor?: string } = {}) => (a.cursor ? page([project("c")]) : page([project("a"), project("b")], "b")));
    renderAt("/projects", signedIn("viewer", { projects: projects as unknown as ApiClient["projects"] }));
    expect(await screen.findByText("Project b")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Load more" }));
    expect(await screen.findByText("Project c")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "New project" })).not.toBeInTheDocument();
  });

  it("creates a project and opens it", async () => {
    const createProject = vi.fn(async () => project("new", { name: "Shop" }));
    const api = signedIn("maintainer", { createProject, project: vi.fn(async () => project("new", { name: "Shop" })) });
    const router = renderAt("/projects/new", api);
    await userEvent.type(await screen.findByLabelText("Name"), "Shop");
    await userEvent.click(screen.getByLabelText(/Analysis/));
    await userEvent.click(screen.getByRole("button", { name: "Create project" }));
    expect(await screen.findByRole("heading", { name: "Shop" })).toBeInTheDocument();
    expect(createProject).toHaveBeenCalledWith("w1", { name: "Shop", kind: "analysis" });
    expect(router.state.location.pathname).toBe("/p/new");
  });

  it("explains why viewers can't create projects", async () => {
    renderAt("/projects/new", signedIn("viewer"));
    expect(await screen.findByRole("alert")).toHaveTextContent("You are viewer");
  });

  it("shows not found for projects the person can't see", async () => {
    renderAt("/p/zzz", signedIn("owner", { project: vi.fn(async () => Promise.reject(new ApiError(404, "not_found", "Not found", "", "r"))) }));
    expect(await screen.findByText("It doesn't exist, or you don't have access to it.")).toBeInTheDocument();
  });

  it("archives, and deletes only after confirming", async () => {
    const updateProject = vi.fn(async () => project("a", { archived_at: now }));
    const deleteProject = vi.fn(async () => undefined);
    const router = renderAt("/p/a", signedIn("admin", { project: vi.fn(async () => project("a")), updateProject, deleteProject }));
    await userEvent.click(await screen.findByRole("button", { name: "Archive" }));
    expect(updateProject).toHaveBeenCalledWith("a", { archived: true });
    expect(await screen.findByText(/Archived/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Delete project" }));
    expect(deleteProject).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Yes, delete it" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/projects"));
    expect(deleteProject).toHaveBeenCalledWith("a");
  });

  it("hides edit controls from viewers", async () => {
    renderAt("/p/a", signedIn("viewer", { project: vi.fn(async () => project("a")) }));
    expect(await screen.findByRole("heading", { name: "Project a" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Archive" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete project" })).not.toBeInTheDocument();
  });
});

describe("settings", () => {
  it("signs out another device", async () => {
    const sessions = vi.fn(async () => [
      { id: "s1", device_kind: "web" as const, device_name: "Web browser", created_at: now, last_refreshed_at: now, expires_at: now, current: true },
      { id: "s2", device_kind: "android" as const, device_name: "Pixel", created_at: now, last_refreshed_at: now, expires_at: now, current: false },
    ]);
    const revokeSession = vi.fn(async () => undefined);
    renderAt("/settings/security", signedIn("owner", { sessions, revokeSession }));
    const row = (await screen.findByText("Pixel")).closest("li") as HTMLElement;
    await userEvent.click(within(row).getByRole("button", { name: "Sign out this device" }));
    expect(revokeSession).toHaveBeenCalledWith("s2");
  });

  it("lets admins assign only roles below their own", async () => {
    const members: Page<Member> = page([
      { user_id: "u1", email: "ada@example.com", display_name: "Ada", role: "admin", created_at: now },
      { user_id: "u2", email: "bo@example.com", display_name: "Bo", role: "editor", created_at: now },
      { user_id: "u3", email: "cy@example.com", display_name: "Cy", role: "owner", created_at: now },
    ]);
    const changeMember = vi.fn(async () => members.items[1]!);
    renderAt("/settings/workspace", signedIn("admin", { members: vi.fn(async () => members), changeMember }));
    const select = await screen.findByLabelText("Role of Bo");
    const options = within(select).getAllByRole("option").map((o) => o.textContent);
    expect(options).toEqual(["viewer", "editor", "maintainer"]);
    expect(screen.queryByLabelText("Role of Cy")).not.toBeInTheDocument();
    await userEvent.selectOptions(select, "maintainer");
    expect(changeMember).toHaveBeenCalledWith("w1", "u2", "maintainer");
    expect(screen.queryByRole("button", { name: "Delete workspace" })).not.toBeInTheDocument();
  });
});

describe("help", () => {
  it("works signed out and reports the server's state", async () => {
    const api = fakeApi({
      health: vi.fn(async () => ({ status: "ok" as const })),
      ready: vi.fn(async () => Promise.reject(new ApiError(503, "not_ready", "Not ready", "Database migrations are pending.", "rid-r"))),
      meta: vi.fn(async () => ({ name: "Tophunt Forge", version: "0.1.0", api_version: "v1", features: [] })),
    });
    renderAt("/help", api);
    expect(await screen.findByText("The server answers.")).toBeInTheDocument();
    // Shown in the readiness check and, with its request ID, under recent errors.
    expect(await screen.findAllByText(/Database migrations are pending/)).toHaveLength(2);
    expect(screen.getByText("Request ID: rid-r")).toBeInTheDocument();
    expect(await screen.findByText("Tophunt Forge 0.1.0, API v1")).toBeInTheDocument();
  });
});
