import { useRef, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Link, useParams } from "react-router";
import { ApiError } from "@forge/api-client";
import type { ToolCallResult } from "@forge/shared-types";
import { useApi } from "../../api.ts";
import { Badge, Button, Card, FormError, PageHeader } from "../../components/ui.tsx";
import { isApproval, useRunTool } from "../../lib/analysis.ts";

export interface Step {
  tool: string;
  label: string;
  /** Builds the tool params from the uploaded file's server path. */
  params: (path: string) => unknown;
}

interface Uploaded {
  path: string;
  name: string;
  sha256: string;
}

/** Shared flow for upload-based analyzers (Binary, APK): upload a file, then run a set of tools
 *  on it in order, showing each step's outcome and linking to the evidence. */
export function UploadAnalyzer({ title, subtitle, accept, steps, note }: { title: string; subtitle: string; accept: string; steps: Step[]; note?: string }) {
  const { id = "" } = useParams();
  const api = useApi();
  const fileRef = useRef<HTMLInputElement>(null);
  const [uploaded, setUploaded] = useState<Uploaded | null>(null);

  const upload = useMutation<Uploaded, ApiError, File>({
    mutationFn: async (file) => {
      const res = await api.uploadTarget(id, file);
      return res;
    },
    onSuccess: (u) => setUploaded(u),
  });

  return (
    <div className="mx-auto max-w-4xl">
      <PageHeader title={title} subtitle={subtitle} actions={<Link to={`/p/${id}/evidence`} className="text-small text-accent underline">All evidence</Link>} />
      <div className="flex flex-col gap-4">
        <Card title="1 · Choose a file">
          {note && <p className="mb-3 text-small text-text-secondary">{note}</p>}
          <p className="mb-3 text-caption text-text-muted">The file is uploaded into this project's sandbox on the server and analyzed there. Only analyze files you are allowed to.</p>
          <div className="flex flex-col gap-3 md:flex-row md:items-center">
            <input
              ref={fileRef}
              type="file"
              accept={accept}
              aria-label="File to analyze"
              className="block w-full text-small file:mr-3 file:rounded-md file:border-0 file:bg-bg-field file:px-3 file:py-1.5 file:text-text-primary"
              onChange={(e) => {
                const f = e.target.files?.[0];
                setUploaded(null);
                if (f) upload.mutate(f);
              }}
            />
            {upload.isPending && <span className="shrink-0 text-small text-text-muted">Uploading…</span>}
          </div>
          <FormError error={upload.error} />
          {uploaded && (
            <p className="mt-3 text-small text-success">
              Uploaded <span className="font-mono">{uploaded.name}</span> · sha256 <span className="font-mono text-tiny">{uploaded.sha256.slice(0, 16)}…</span>
            </p>
          )}
        </Card>
        <Card title="2 · Analyze">
          {!uploaded ? (
            <p className="text-text-secondary">Choose a file first.</p>
          ) : (
            <ol className="flex flex-col gap-3">
              {steps.map((s) => (
                <StepRow key={s.tool} projectId={id} step={s} path={uploaded.path} />
              ))}
            </ol>
          )}
        </Card>
      </div>
    </div>
  );
}

function StepRow({ projectId, step, path }: { projectId: string; step: Step; path: string }) {
  const run = useRunTool(projectId);
  const api = useApi();
  const approve = useMutation<void, ApiError, string>({ mutationFn: (approvalId) => api.approve(approvalId) });
  const [approvalId, setApprovalId] = useState<string | null>(null);

  const start = () => run.mutate({ tool: step.tool, params: step.params(path) }, { onSuccess: (r) => setApprovalId(isApproval(r) ? r.approvalId : null) });
  const result = run.data && !isApproval(run.data) ? (run.data as ToolCallResult) : undefined;

  return (
    <li className="rounded-md border border-border-subtle p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0">
          <p className="font-medium">{step.label}</p>
          <p className="font-mono text-tiny text-text-muted">{step.tool}</p>
        </div>
        <Button busy={run.isPending} onClick={start}>
          {result ? "Run again" : "Run"}
        </Button>
      </div>
      {approvalId && (
        <div className="mt-2 rounded-md border border-warning bg-bg-raised p-2 text-small">
          <p className="text-warning">This tool runs code or changes a device, so it needs approval.</p>
          <div className="mt-2 flex gap-2">
            <Button
              variant="secondary"
              busy={approve.isPending}
              onClick={() => approve.mutate(approvalId, { onSuccess: () => run.mutate({ tool: step.tool, params: step.params(path), approvalId }, { onSuccess: (r) => setApprovalId(isApproval(r) ? r.approvalId : null) }) })}
            >
              Approve and run
            </Button>
            <Link to={`/p/${projectId}`} className="self-center text-caption text-text-muted">
              (needs the maintainer role)
            </Link>
          </div>
          <FormError error={approve.error} />
        </div>
      )}
      {run.error && <div className="mt-2"><FormError error={run.error} /></div>}
      {result && (
        <div className="mt-2 text-small">
          <Badge tone="accent">done in {result.duration_ms} ms</Badge>{" "}
          {result.evidence_id ? (
            <Link to={`/p/${projectId}/evidence`} className="text-accent underline">
              View evidence
            </Link>
          ) : (
            <span className="text-text-secondary">no evidence recorded</span>
          )}
        </div>
      )}
    </li>
  );
}
