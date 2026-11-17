import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useApi } from "../../api.ts";
import { useMe, useSignOut } from "../../auth.tsx";
import { StateView } from "../../components/StateView.tsx";
import { Badge, Button, Card, FormError, PageHeader, formatDate } from "../../components/ui.tsx";

export function SecurityPage() {
  const api = useApi();
  const qc = useQueryClient();
  const me = useMe();
  const signOut = useSignOut();
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: ({ signal }) => api.sessions(signal) });
  const revoke = useMutation({ mutationFn: (id: string) => api.revokeSession(id), onSuccess: () => qc.invalidateQueries({ queryKey: ["sessions"] }) });
  return (
    <div className="mx-auto max-w-3xl">
      <PageHeader title="Security" subtitle="Your account and where you are signed in." />
      <div className="flex flex-col gap-4">
        <Card title="Account">
          <dl className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <div>
              <dt className="text-caption text-text-muted">Name</dt>
              <dd>{me.data?.display_name}</dd>
            </div>
            <div>
              <dt className="text-caption text-text-muted">Email</dt>
              <dd className="truncate">{me.data?.email}</dd>
            </div>
          </dl>
        </Card>
        <Card title="Signed-in devices">
          <StateView status={sessions.status} data={sessions.data} error={sessions.error} onRetry={() => void sessions.refetch()}>
            {(list) => (
              <ul className="divide-y divide-border-subtle">
                {list.map((s) => (
                  <li key={s.id} className="flex flex-col gap-2 py-3 md:flex-row md:items-center md:justify-between">
                    <div>
                      <p className="font-medium">
                        {s.device_name} <Badge>{s.device_kind}</Badge> {s.current && <Badge tone="accent">this device</Badge>}
                      </p>
                      <p className="text-caption text-text-muted">
                        Signed in {formatDate(s.created_at)} · last active {formatDate(s.last_refreshed_at)}
                      </p>
                    </div>
                    {s.current ? (
                      <Button variant="secondary" onClick={() => void signOut()}>
                        Sign out
                      </Button>
                    ) : (
                      <Button variant="danger" busy={revoke.isPending && revoke.variables === s.id} onClick={() => revoke.mutate(s.id)}>
                        Sign out this device
                      </Button>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </StateView>
          <FormError error={revoke.error} />
        </Card>
      </div>
    </div>
  );
}
