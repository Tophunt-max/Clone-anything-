import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { Tool, ToolPolicy } from "@forge/shared-types";
import { useApi } from "../api.ts";
import { StateView } from "../components/StateView.tsx";
import { Badge, Card, PageHeader, TextField } from "../components/ui.tsx";

const POLICY_TONE: Record<ToolPolicy, { tone: "neutral" | "accent" | "warning"; label: string }> = {
  viewer: { tone: "neutral", label: "view" },
  editor: { tone: "accent", label: "edit" },
  editor_worker: { tone: "accent", label: "worker" },
  approval_gate: { tone: "warning", label: "needs approval" },
  operator_only: { tone: "warning", label: "operator only" },
  unavailable: { tone: "neutral", label: "unavailable" },
};

// Screen 20: the engine tool catalogue with each tool's hosted-mode policy.
export function ToolRegistryPage() {
  const api = useApi();
  const tools = useQuery({ queryKey: ["tools"], queryFn: ({ signal }) => api.tools(signal), staleTime: 5 * 60_000 });
  const [q, setQ] = useState("");

  return (
    <div className="mx-auto max-w-5xl">
      <PageHeader title="Tool registry" subtitle="Every engine tool and how the platform governs it." />
      <StateView status={tools.status} data={tools.data} error={tools.error} onRetry={() => void tools.refetch()} loadingLabel="Loading the catalogue…">
        {(list) => <Registry tools={list} q={q} setQ={setQ} />}
      </StateView>
    </div>
  );
}

function Registry({ tools, q, setQ }: { tools: Tool[]; q: string; setQ: (s: string) => void }) {
  const filtered = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const match = (t: Tool) => !needle || t.id.includes(needle) || t.title.toLowerCase().includes(needle) || t.summary.toLowerCase().includes(needle) || t.toolset.includes(needle);
    const groups = new Map<string, Tool[]>();
    for (const t of tools) if (match(t)) (groups.get(t.toolset) ?? groups.set(t.toolset, []).get(t.toolset)!).push(t);
    return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [tools, q]);

  return (
    <>
      <div className="mb-4 flex items-center gap-3">
        <div className="flex-1">
          <TextField label="Search" placeholder="name, summary or toolset" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <p className="shrink-0 pt-6 text-caption text-text-muted">{tools.length} tools</p>
      </div>
      {filtered.length === 0 ? (
        <p className="text-text-secondary">No tools match “{q}”.</p>
      ) : (
        <div className="flex flex-col gap-4">
          {filtered.map(([toolset, items]) => (
            <Card key={toolset} title={`${toolset} (${items.length})`}>
              <ul className="divide-y divide-border-subtle">
                {items.map((t) => {
                  const p = POLICY_TONE[t.policy];
                  return (
                    <li key={t.id} className="flex flex-col gap-1 py-2 md:flex-row md:items-start md:justify-between md:gap-4">
                      <div className="min-w-0">
                        <p className="font-mono text-code">{t.id}</p>
                        <p className="text-small text-text-secondary">{t.summary}</p>
                      </div>
                      <div className="flex shrink-0 flex-wrap items-center gap-2">
                        {t.needs_consent && <Badge tone="warning">consent</Badge>}
                        {t.read_only && <Badge>read-only</Badge>}
                        {t.requires !== "nothing" && <Badge>{t.requires}</Badge>}
                        <Badge tone={p.tone}>{p.label}</Badge>
                      </div>
                    </li>
                  );
                })}
              </ul>
            </Card>
          ))}
        </div>
      )}
    </>
  );
}
