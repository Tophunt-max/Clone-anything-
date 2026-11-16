import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter } from "react-router";
import { QueryClient } from "@tanstack/react-query";
import { ApiError, type ApiClient } from "@forge/api-client";
import { App } from "./App.tsx";
import { routes } from "./routes.tsx";

const meta = { name: "Tophunt Forge", version: "0.1.0", api_version: "v1", features: [] };

function renderAt(path: string, api: Partial<ApiClient>) {
  const router = createMemoryRouter(routes, { initialEntries: [path] });
  // Same retry count as production, without the back-off delay.
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 1, retryDelay: 0 } } });
  return render(<App router={router} queryClient={queryClient} api={{ health: vi.fn(), meta: vi.fn(), ...api } as ApiClient} />);
}

describe("web app", () => {
  it("shows loading, then the server's metadata", async () => {
    let resolve: (v: typeof meta) => void = () => {};
    const api = { meta: vi.fn(() => new Promise<typeof meta>((r) => (resolve = r))) };
    renderAt("/", api);
    expect(screen.getByRole("status")).toHaveTextContent("Checking the server");
    resolve(meta);
    expect(await screen.findByText("Tophunt Forge", { selector: "dd" })).toBeInTheDocument();
    expect(screen.getByText("v1")).toBeInTheDocument();
  });

  it("shows the error with its request id, and retries", async () => {
    const meta1 = vi
      .fn()
      .mockRejectedValueOnce(new ApiError(0, "network_error", "Can't reach the server", "Check your connection.", "rid-9"))
      .mockRejectedValueOnce(new ApiError(0, "network_error", "Can't reach the server", "Check your connection.", "rid-9"))
      .mockResolvedValue(meta);
    renderAt("/", { meta: meta1 });
    expect(await screen.findByRole("alert")).toHaveTextContent("Can't reach the server");
    expect(screen.getByText(/rid-9/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("v1")).toBeInTheDocument();
  });

  it("routes unknown paths to the not-found page inside the shell", () => {
    renderAt("/does/not/exist", {});
    expect(screen.getByRole("heading", { name: "Page not found" })).toBeInTheDocument();
    expect(screen.getByTestId("app-shell")).toBeInTheDocument();
  });

  it("renders both navigations so CSS can pick one per breakpoint", () => {
    renderAt("/does/not/exist", {});
    expect(screen.getByRole("complementary", { name: "Main navigation" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Bottom navigation" })).toBeInTheDocument();
  });
});
