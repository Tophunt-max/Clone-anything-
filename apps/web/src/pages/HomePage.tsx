import { useQuery } from "@tanstack/react-query";
import { useApi } from "../api.ts";
import { StateView } from "../components/StateView.tsx";

// Phase 1 home: proves the web app reaches the API through the shared client. The dashboard
// (screen 6) replaces it in Phase 3.
export function HomePage() {
  const api = useApi();
  const meta = useQuery({ queryKey: ["meta"], queryFn: ({ signal }) => api.meta(signal) });
  return (
    <section className="mx-auto max-w-3xl">
      <h1 className="text-title font-semibold">Server</h1>
      <p className="mt-1 text-text-secondary">The platform API this app is connected to.</p>
      <div className="mt-6">
        <StateView status={meta.status} data={meta.data} error={meta.error} onRetry={() => void meta.refetch()} loadingLabel="Checking the server…">
          {(m) => (
            <dl className="grid grid-cols-1 gap-3 rounded-lg border border-border-subtle bg-bg-surface p-4 shadow-e1 md:grid-cols-3">
              <Item label="Name" value={m.name} />
              <Item label="Version" value={m.version} mono />
              <Item label="API" value={m.api_version} mono />
            </dl>
          )}
        </StateView>
      </div>
    </section>
  );
}

function Item({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <dt className="text-caption text-text-muted">{label}</dt>
      <dd className={mono ? "font-mono text-code" : "text-body"}>{value}</dd>
    </div>
  );
}
