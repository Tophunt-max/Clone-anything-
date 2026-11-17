import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router";
import type { EvidenceRow } from "@forge/shared-types";
import { useApi } from "../../api.ts";
import { StateView } from "../../components/StateView.tsx";
import { Badge, Card, PageHeader, formatDate } from "../../components/ui.tsx";
import { subjectLabel, useEvidence } from "../../lib/analysis.ts";

// Screen 19: the evidence a project has gathered, with the full record on demand.
export function EvidenceExplorerPage() {
  const { id = "" } = useParams();
  const [operation, setOperation] = useState<string>("");
  const [selected, setSelected] = useState<string | null>(null);
  const evidence = useEvidence(id, operation || undefined);

  return (
    <div className="mx-auto max-w-5xl">
      <PageHeader title="Evidence" subtitle="Findings this project recorded, each with its confidence and coverage." />
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-5">
        <div className="lg:col-span-2">
          <StateView
            status={evidence.status}
            data={evidence.data}
            error={evidence.error}
            onRetry={() => void evidence.refetch()}
            isEmpty={(p) => p.items.length === 0}
            empty={
              <Card>
                <p className="text-text-secondary">No evidence yet. Run an analysis from the Binary or APK analyzer, or any read tool, to gather some.</p>
              </Card>
            }
          >
            {(page) => <EvidenceList rows={page.items} selected={selected} onSelect={setSelected} operation={operation} setOperation={setOperation} />}
          </StateView>
        </div>
        <div className="lg:col-span-3">{selected ? <EvidenceDetail projectId={id} evidenceId={selected} /> : <Card><p className="text-text-secondary">Pick a record to see its full evidence.</p></Card>}</div>
      </div>
    </div>
  );
}

function EvidenceList({ rows, selected, onSelect, operation, setOperation }: { rows: EvidenceRow[]; selected: string | null; onSelect: (id: string) => void; operation: string; setOperation: (s: string) => void }) {
  const operations = [...new Set(rows.map((r) => r.operation))].sort();
  return (
    <Card title="Records">
      <select aria-label="Filter by operation" className="mb-3 h-9 w-full rounded-md border border-border-subtle bg-bg-field px-2 text-small" value={operation} onChange={(e) => setOperation(e.target.value)}>
        <option value="">All operations</option>
        {operations.map((op) => (
          <option key={op} value={op}>
            {op}
          </option>
        ))}
      </select>
      <ul className="flex flex-col gap-2">
        {rows.map((r) => (
          <li key={r.evidence_id}>
            <button
              type="button"
              onClick={() => onSelect(r.evidence_id)}
              className={`w-full rounded-md border p-2 text-left ${selected === r.evidence_id ? "border-accent bg-bg-raised" : "border-border-subtle hover:border-border-strong"}`}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate font-mono text-caption">{r.operation}</span>
                {r.confidence && <Badge tone={r.confidence === "high" ? "accent" : "neutral"}>{r.confidence}</Badge>}
              </div>
              <p className="mt-1 truncate text-small">{r.summary || subjectLabel(r.subject)}</p>
              <p className="text-tiny text-text-muted">{formatDate(r.created_at)}</p>
            </button>
          </li>
        ))}
      </ul>
    </Card>
  );
}

function EvidenceDetail({ projectId, evidenceId }: { projectId: string; evidenceId: string }) {
  const api = useApi();
  const record = useQuery({ queryKey: ["evidence-record", projectId, evidenceId], queryFn: ({ signal }) => api.evidenceRecord(projectId, evidenceId, signal) });
  return (
    <Card title="Evidence record" actions={<span className="font-mono text-tiny text-text-muted">{evidenceId.slice(0, 14)}…</span>}>
      <StateView status={record.status} data={record.data} error={record.error} onRetry={() => void record.refetch()}>
        {(rec) => <EvidenceBody record={rec as Record<string, unknown>} />}
      </StateView>
    </Card>
  );
}

function EvidenceBody({ record }: { record: Record<string, unknown> }) {
  const str = (k: string) => (typeof record[k] === "string" ? (record[k] as string) : undefined);
  const limitations = Array.isArray(record.limitations) ? (record.limitations as unknown[]) : [];
  return (
    <div className="flex flex-col gap-3">
      <dl className="grid grid-cols-2 gap-2 text-small">
        <Field label="Operation" value={str("operation")} mono />
        <Field label="Confidence" value={str("confidence")} />
        <Field label="Authority" value={str("authority")} />
        <Field label="Engine" value={(record.environment as { engine?: string } | undefined)?.engine} />
      </dl>
      {str("summary") && <p className="text-body">{str("summary")}</p>}
      {limitations.length > 0 && (
        <div>
          <p className="text-caption font-medium text-warning">Limitations</p>
          <ul className="list-inside list-disc text-small text-text-secondary">
            {limitations.map((l, i) => (
              <li key={i}>{typeof l === "string" ? l : (l as { code?: string }).code ?? JSON.stringify(l)}</li>
            ))}
          </ul>
        </div>
      )}
      <details>
        <summary className="cursor-pointer text-small text-accent">Full record</summary>
        <pre className="mt-2 max-h-96 overflow-auto rounded-md bg-bg-canvas p-3 font-mono text-tiny">{JSON.stringify(record, null, 2)}</pre>
      </details>
    </div>
  );
}

function Field({ label, value, mono = false }: { label: string; value: string | undefined; mono?: boolean }) {
  if (!value) return null;
  return (
    <div className="min-w-0">
      <dt className="text-caption text-text-muted">{label}</dt>
      <dd className={`truncate ${mono ? "font-mono text-code" : ""}`}>{value}</dd>
    </div>
  );
}
