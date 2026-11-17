import { useState } from "react";
import { Link, useParams } from "react-router";
import { useApi } from "../../api.ts";
import { Badge, Button, Card, FormError, PageHeader } from "../../components/ui.tsx";
import { isApproval, useRunTool } from "../../lib/analysis.ts";
import type { ToolCallResult } from "@forge/shared-types";

// Screen 17: study a website from the outside. Opening a site runs a browser in the sandbox,
// which the engine needs the Chromium engine for and which is approval-gated.
export function WebsiteAnalyzerPage() {
  const { id = "" } = useParams();
  const api = useApi();
  const [url, setUrl] = useState("");
  const run = useRunTool(id);
  const [approvalId, setApprovalId] = useState<string | null>(null);
  const result = run.data && !isApproval(run.data) ? (run.data as ToolCallResult) : undefined;

  const open = (approval?: string) =>
    run.mutate({ tool: "web_site_open", params: { url }, ...(approval ? { approvalId: approval } : {}) }, { onSuccess: (r) => setApprovalId(isApproval(r) ? r.approvalId : null) });

  return (
    <div className="mx-auto max-w-3xl">
      <PageHeader title="Website analyzer" subtitle="Open a site in a sandboxed browser and capture what it shows." actions={<Link to={`/p/${id}/evidence`} className="text-small text-accent underline">All evidence</Link>} />
      <Card title="Open a website">
        <p className="mb-3 text-small text-text-secondary">
          This visits the site from the server as an ordinary visitor would and records what it sees. Only analyze sites you are allowed to. It needs the Chromium engine installed on the server, and
          a maintainer's approval because it runs a browser.
        </p>
        <form
          className="flex flex-col gap-3 md:flex-row md:items-end"
          onSubmit={(e) => {
            e.preventDefault();
            setApprovalId(null);
            open();
          }}
        >
          <div className="flex-1">
            <label className="mb-1 block text-small font-medium text-text-secondary" htmlFor="url">
              Website address
            </label>
            <input id="url" type="url" required placeholder="https://example.com" value={url} onChange={(e) => setUrl(e.target.value)} className="h-10 w-full rounded-md border border-border-subtle bg-bg-field px-3 text-body" />
          </div>
          <Button type="submit" busy={run.isPending} disabled={!url}>
            Open site
          </Button>
        </form>
        {approvalId && (
          <div className="mt-3 rounded-md border border-warning bg-bg-raised p-3 text-small">
            <p className="text-warning">Opening a site runs a browser, so it needs approval.</p>
            <Button className="mt-2" variant="secondary" onClick={() => api.approve(approvalId).then(() => open(approvalId)).catch(() => {})}>
              Approve and open (maintainer)
            </Button>
          </div>
        )}
        {run.error && (
          <div className="mt-3">
            <FormError error={run.error} />
            <p className="mt-1 text-caption text-text-muted">If the server says the browser engine is missing, an operator needs to install Chromium on the worker.</p>
          </div>
        )}
        {result && (
          <p className="mt-3 text-small">
            <Badge tone="accent">opened in {result.duration_ms} ms</Badge>{" "}
            {result.evidence_id && (
              <Link to={`/p/${id}/evidence`} className="text-accent underline">
                View evidence
              </Link>
            )}
          </p>
        )}
      </Card>
    </div>
  );
}
