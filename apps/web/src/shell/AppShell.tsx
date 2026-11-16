import { NavLink, Outlet } from "react-router";
import { useMe, useSignOut } from "../auth.tsx";
import { useCurrentWorkspace, useCurrentWorkspaceId } from "../lib/workspace.ts";

const NAV = [
  { to: "/", label: "Dashboard", short: "Home", end: true },
  { to: "/projects", label: "Projects", short: "Projects", end: false },
  { to: "/settings/workspace", label: "Workspace", short: "Team", end: false },
  { to: "/settings/security", label: "Security", short: "Security", end: false },
  { to: "/help", label: "Help", short: "Help", end: false },
];

// Responsive frame (docs/platform/06-ui-plan.md §4):
//   < md (768px): top bar + bottom navigation, one column
//   md – lg: navigation rail + content
//   ≥ lg (1280px): wide sidebar + content
export function AppShell() {
  return (
    <div className="flex h-full flex-col md:flex-row" data-testid="app-shell">
      <aside
        aria-label="Main navigation"
        className="hidden shrink-0 border-r border-border-subtle bg-bg-surface md:flex md:w-24 md:flex-col md:gap-2 md:py-4 lg:w-64 lg:px-3"
      >
        <div className="mb-2 px-2 text-center text-small font-semibold lg:text-left lg:text-lead">
          <span className="lg:hidden" aria-hidden>
            TF
          </span>
          <span className="hidden lg:inline">Tophunt Forge</span>
        </div>
        <div className="hidden px-2 lg:block">
          <WorkspaceSwitcher />
        </div>
        <nav aria-label="Sections" className="flex flex-col gap-1 px-2 lg:px-0">
          <NavItems variant="side" />
        </nav>
        <div className="mt-auto hidden px-2 lg:block">
          <Account />
        </div>
      </aside>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <header className="flex h-12 items-center justify-between gap-2 border-b border-border-subtle bg-bg-surface px-4 lg:hidden">
          <span className="shrink-0 text-body font-semibold md:hidden">Tophunt Forge</span>
          <div className="min-w-0 max-w-56 flex-1 md:max-w-xs">
            <WorkspaceSwitcher />
          </div>
          <Account compact />
        </header>

        <main className="min-w-0 flex-1 overflow-y-auto p-4 md:p-6 lg:p-8">
          <Outlet />
        </main>
      </div>

      <nav aria-label="Bottom navigation" className="flex h-14 shrink-0 border-t border-border-subtle bg-bg-surface md:hidden">
        <NavItems variant="bottom" />
      </nav>
    </div>
  );
}

function NavItems({ variant }: { variant: "side" | "bottom" }) {
  return (
    <>
      {NAV.map((n) => (
        <NavLink
          key={n.to}
          to={n.to}
          end={n.end}
          className={({ isActive }) =>
            [
              variant === "bottom" ? "flex flex-1 items-center justify-center text-caption" : "rounded-md px-2 py-2 text-center text-caption lg:px-3 lg:text-left lg:text-small",
              isActive ? "bg-bg-raised font-medium text-text-primary" : "text-text-secondary hover:text-text-primary",
            ].join(" ")
          }
        >
          {variant === "bottom" ? n.short : n.label}
        </NavLink>
      ))}
    </>
  );
}

function WorkspaceSwitcher() {
  const { workspace, query } = useCurrentWorkspace();
  const select = useCurrentWorkspaceId((s) => s.set);
  const items = query.data?.items ?? [];
  if (!workspace) return null;
  return (
    <select
      aria-label="Workspace"
      className="h-9 w-full truncate rounded-md border border-border-subtle bg-bg-field px-2 text-small"
      value={workspace.id}
      onChange={(e) => select(e.target.value)}
    >
      {items.map((w) => (
        <option key={w.id} value={w.id}>
          {w.name}
        </option>
      ))}
    </select>
  );
}

function Account({ compact = false }: { compact?: boolean }) {
  const me = useMe();
  const signOut = useSignOut();
  return (
    <div className={compact ? "flex shrink-0 items-center" : "flex flex-col gap-1 border-t border-border-subtle pt-3"}>
      {!compact && <span className="truncate text-small text-text-secondary">{me.data?.display_name}</span>}
      <button type="button" onClick={() => void signOut()} className="text-left text-small text-accent hover:underline">
        Sign out
      </button>
    </div>
  );
}
