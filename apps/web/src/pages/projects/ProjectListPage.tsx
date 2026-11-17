import { useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { useApi } from "../../api.ts";
import { StateView } from "../../components/StateView.tsx";
import { Badge, Button, PageHeader, formatDate } from "../../components/ui.tsx";
import { atLeast, useCurrentWorkspace } from "../../lib/workspace.ts";

const PAGE = 50;

export function ProjectListPage() {
  const api = useApi();
  const { workspace, query: wsQuery } = useCurrentWorkspace();
  const [archived, setArchived] = useState(false);
  const projects = useInfiniteQuery({
    queryKey: ["projects", workspace?.id, { archived }],
    queryFn: ({ pageParam, signal }) => api.projects(workspace?.id ?? "", { limit: PAGE, includeArchived: archived, signal, ...(pageParam ? { cursor: pageParam } : {}) }),
    initialPageParam: "",
    getNextPageParam: (last) => last.next_cursor ?? undefined,
    enabled: !!workspace,
  });
  const canCreate = atLeast(workspace?.role, "maintainer");
  const items = projects.data?.pages.flatMap((p) => p.items);

  return (
    <div className="mx-auto max-w-5xl">
      <PageHeader
        title="Projects"
        subtitle={workspace?.name}
        actions={
          canCreate && (
            <Link to="/projects/new" className="inline-flex h-9 items-center rounded-md bg-accent-strong px-3 text-small font-medium text-on-accent">
              New project
            </Link>
          )
        }
      />
      <label className="mb-3 inline-flex items-center gap-2 text-small text-text-secondary">
        <input type="checkbox" checked={archived} onChange={(e) => setArchived(e.target.checked)} />
        Show archived
      </label>
      <StateView
        status={workspace ? projects.status : wsQuery.status}
        data={workspace ? items : undefined}
        error={projects.error ?? wsQuery.error}
        onRetry={() => void (workspace ? projects.refetch() : wsQuery.refetch())}
        isEmpty={(list) => list.length === 0}
        empty={
          <div className="rounded-lg border border-border-subtle bg-bg-surface p-8 text-center">
            <p className="text-lead font-semibold">No projects yet</p>
            <p className="mt-1 text-text-secondary">{canCreate ? "Create one to start working." : "A maintainer of this workspace can create one."}</p>
          </div>
        }
      >
        {(list) => (
          <>
            <ul className="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-3">
              {list.map((p) => (
                <li key={p.id}>
                  <Link to={`/p/${p.id}`} className="block h-full rounded-lg border border-border-subtle bg-bg-surface p-4 shadow-e1 hover:border-border-strong">
                    <div className="flex items-start justify-between gap-2">
                      <span className="font-semibold">{p.name}</span>
                      <Badge>{p.kind}</Badge>
                    </div>
                    <p className="mt-2 text-caption text-text-muted">Updated {formatDate(p.updated_at)}</p>
                    {p.archived_at && (
                      <p className="mt-2">
                        <Badge tone="warning">archived</Badge>
                      </p>
                    )}
                  </Link>
                </li>
              ))}
            </ul>
            {projects.hasNextPage && (
              <div className="mt-4 text-center">
                <Button variant="secondary" busy={projects.isFetchingNextPage} onClick={() => void projects.fetchNextPage()}>
                  Load more
                </Button>
              </div>
            )}
          </>
        )}
      </StateView>
    </div>
  );
}
