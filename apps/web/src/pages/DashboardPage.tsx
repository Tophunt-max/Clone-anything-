import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { useApi } from "../api.ts";
import { useMe } from "../auth.tsx";
import { StateView } from "../components/StateView.tsx";
import { Badge, Card, PageHeader, formatDate } from "../components/ui.tsx";
import { atLeast, useCurrentWorkspace } from "../lib/workspace.ts";

const RECENT = 5;

export function DashboardPage() {
  const api = useApi();
  const me = useMe();
  const { workspace, query: wsQuery } = useCurrentWorkspace();
  const projects = useQuery({
    queryKey: ["projects", workspace?.id, { limit: RECENT }],
    queryFn: ({ signal }) => api.projects(workspace?.id ?? "", { limit: RECENT, signal }),
    enabled: !!workspace,
  });
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: ({ signal }) => api.sessions(signal) });
  const canCreate = atLeast(workspace?.role, "maintainer");

  return (
    <div className="mx-auto max-w-5xl">
      <PageHeader title={`Hello, ${me.data?.display_name ?? ""}`} subtitle={workspace ? `Workspace: ${workspace.name}` : undefined} />
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <Card
          className="lg:col-span-2"
          title="Recent projects"
          actions={
            <Link to="/projects" className="text-small text-accent underline">
              All projects
            </Link>
          }
        >
          <StateView
            status={workspace ? projects.status : wsQuery.status}
            data={workspace ? projects.data : undefined}
            error={projects.error ?? wsQuery.error}
            onRetry={() => void (workspace ? projects.refetch() : wsQuery.refetch())}
            isEmpty={(p) => p.items.length === 0}
            empty={
              <div className="py-4 text-center">
                <p className="text-text-secondary">No projects in this workspace yet.</p>
                {canCreate && (
                  <Link to="/projects/new" className="mt-3 inline-block rounded-md bg-accent-strong px-3 py-1.5 text-small text-on-accent">
                    Create a project
                  </Link>
                )}
              </div>
            }
          >
            {(p) => (
              <ul className="divide-y divide-border-subtle">
                {p.items.map((proj) => (
                  <li key={proj.id} className="flex items-center justify-between gap-2 py-2">
                    <Link to={`/p/${proj.id}`} className="truncate font-medium hover:underline">
                      {proj.name}
                    </Link>
                    <span className="flex shrink-0 items-center gap-2 text-caption text-text-muted">
                      <Badge>{proj.kind}</Badge>
                      <span className="hidden md:inline">{formatDate(proj.updated_at)}</span>
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </StateView>
        </Card>
        <Card
          title="Signed-in devices"
          actions={
            <Link to="/settings/security" className="text-small text-accent underline">
              Manage
            </Link>
          }
        >
          <StateView status={sessions.status} data={sessions.data} error={sessions.error} onRetry={() => void sessions.refetch()}>
            {(s) => (
              <p className="text-text-secondary">
                <span className="text-title font-semibold text-text-primary">{s.length}</span> active {s.length === 1 ? "session" : "sessions"}
              </p>
            )}
          </StateView>
        </Card>
      </div>
    </div>
  );
}
