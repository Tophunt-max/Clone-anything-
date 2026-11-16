import { useQuery } from "@tanstack/react-query";
import { ApiError } from "@forge/api-client";
import { useApi } from "../api.ts";
import { Badge, Button, Card, PageHeader, formatDate } from "../components/ui.tsx";
import { useDiagnostics } from "../lib/diagnostics.ts";
import { callNative, inAndroidApp } from "../lib/native.ts";

// Works signed in or not: it is where people go when something is broken.
export function HelpPage() {
  const api = useApi();
  const meta = useQuery({ queryKey: ["diag", "meta"], queryFn: ({ signal }) => api.meta(signal), retry: false });
  const health = useQuery({ queryKey: ["diag", "health"], queryFn: ({ signal }) => api.health(signal), retry: false });
  const ready = useQuery({ queryKey: ["diag", "ready"], queryFn: ({ signal }) => api.ready(signal), retry: false });
  const app = useQuery({ queryKey: ["diag", "app"], queryFn: () => callNative("getAppInfo"), enabled: inAndroidApp(), retry: false });
  const { errors, clear } = useDiagnostics();
  const recheck = () => void Promise.all([meta.refetch(), health.refetch(), ready.refetch()]);

  return (
    <div className="mx-auto max-w-3xl p-4 md:p-0">
      <PageHeader title="Help and diagnostics" subtitle="What to check when something doesn't work, and what to quote when you report it." actions={<Button variant="secondary" onClick={recheck}>Check again</Button>} />
      <div className="flex flex-col gap-4">
        <Card title="Server">
          <ul className="flex flex-col gap-2">
            <Check label="Reachable" q={health} ok={() => "The server answers."} />
            <Check label="Ready" q={ready} ok={() => "The database is reachable and up to date."} />
            <Check label="Version" q={meta} ok={(m) => `${m.name} ${m.version}, API ${m.api_version}`} />
          </ul>
        </Card>
        <Card title="This browser">
          <dl className="grid grid-cols-1 gap-2 text-small">
            <Row label="App" value={!inAndroidApp() ? "Web browser" : app.data?.ok ? `Android app ${app.data.result?.version ?? ""}` : app.isError ? "Android app (not answering)" : "Android app…"} />
            <Row label="Address" value={window.location.origin} />
            <Row label="Screen" value={`${window.innerWidth} × ${window.innerHeight}`} />
            <Row label="Online" value={navigator.onLine ? "yes" : "no"} />
            <Row label="Browser" value={navigator.userAgent} />
          </dl>
        </Card>
        <Card title="Recent errors" actions={errors.length > 0 && <Button variant="secondary" onClick={clear}>Clear</Button>}>
          {errors.length === 0 ? (
            <p className="text-text-secondary">No errors in this tab.</p>
          ) : (
            <ul className="divide-y divide-border-subtle">
              {errors.map((e, i) => (
                <li key={`${e.at}-${i}`} className="py-2 text-small">
                  <p>
                    <Badge tone="warning">{e.status || "offline"}</Badge> <span className="font-mono">{e.code}</span> · {formatDate(e.at)}
                  </p>
                  <p className="text-text-secondary">{e.message}</p>
                  {e.requestId && <p className="font-mono text-tiny text-text-muted">Request ID: {e.requestId}</p>}
                </li>
              ))}
            </ul>
          )}
          <p className="mt-3 text-caption text-text-muted">These stay in this tab only. When you report a problem, include the request ID: it points to the server's log entry.</p>
        </Card>
      </div>
    </div>
  );
}

function Check<T>({ label, q, ok }: { label: string; q: { status: string; data: T | undefined; error: unknown }; ok: (d: T) => string }) {
  const e = q.error instanceof ApiError ? q.error : undefined;
  const [tone, text] =
    q.status === "pending" ? ["text-text-muted", "Checking…"] : q.status === "success" && q.data !== undefined ? ["text-success", ok(q.data)] : ["text-danger", e ? `${e.message}${e.requestId ? ` (request ${e.requestId})` : ""}` : "Failed."];
  return (
    <li className="flex flex-col md:flex-row md:gap-3">
      <span className="w-24 shrink-0 font-medium">{label}</span>
      <span className={`text-small ${tone}`}>{text}</span>
    </li>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col md:flex-row md:gap-3">
      <dt className="w-24 shrink-0 text-text-muted">{label}</dt>
      <dd className="break-all font-mono text-tiny">{value}</dd>
    </div>
  );
}
