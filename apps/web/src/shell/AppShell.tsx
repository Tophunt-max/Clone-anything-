import { NavLink, Outlet } from "react-router";

const NAV = [{ to: "/", label: "Home", end: true }];

// Responsive frame (docs/platform/06-ui-plan.md §4):
//   < md (768px): top bar + bottom navigation, one column
//   md – lg: navigation rail + content
//   ≥ lg (1280px): wide sidebar + content
export function AppShell() {
  return (
    <div className="flex h-full flex-col md:flex-row" data-testid="app-shell">
      <aside
        aria-label="Main navigation"
        className="hidden shrink-0 border-r border-border-subtle bg-bg-surface md:flex md:w-16 md:flex-col md:items-center md:gap-2 md:py-4 lg:w-60 lg:items-stretch lg:px-3"
      >
        <div className="mb-4 px-2 text-small font-semibold text-text-primary lg:text-lead">
          <span className="lg:hidden" aria-hidden>
            TF
          </span>
          <span className="hidden lg:inline">Tophunt Forge</span>
        </div>
        <NavItems />
      </aside>

      <header className="flex h-12 items-center border-b border-border-subtle bg-bg-surface px-4 md:hidden">
        <span className="text-body font-semibold">Tophunt Forge</span>
      </header>

      <main className="min-w-0 flex-1 overflow-y-auto p-4 md:p-6 lg:p-8">
        <Outlet />
      </main>

      <nav aria-label="Bottom navigation" className="flex h-14 border-t border-border-subtle bg-bg-surface md:hidden">
        <NavItems compact />
      </nav>
    </div>
  );
}

function NavItems({ compact = false }: { compact?: boolean }) {
  return (
    <>
      {NAV.map((n) => (
        <NavLink
          key={n.to}
          to={n.to}
          end={n.end}
          className={({ isActive }) =>
            [
              compact ? "flex flex-1 items-center justify-center text-caption" : "rounded-md px-3 py-2 text-small",
              isActive ? "bg-bg-raised text-text-primary" : "text-text-secondary hover:text-text-primary",
            ].join(" ")
          }
        >
          {n.label}
        </NavLink>
      ))}
    </>
  );
}
