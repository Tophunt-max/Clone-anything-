import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import type { ProjectKind } from "@forge/shared-types";
import { useApi } from "../../api.ts";
import { StateView } from "../../components/StateView.tsx";
import { Button, Card, FormError, PageHeader, TextField, fieldError } from "../../components/ui.tsx";
import { atLeast, useCurrentWorkspace } from "../../lib/workspace.ts";

const KINDS: { kind: ProjectKind; label: string; about: string }[] = [
  { kind: "code", label: "Code", about: "Write and build software." },
  { kind: "analysis", label: "Analysis", about: "Study an app, website or file you are allowed to analyze, with evidence." },
  { kind: "rebuild", label: "Rebuild", about: "Study something, then build your own version of it." },
];

export function CreateProjectPage() {
  const api = useApi();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const { workspace, query } = useCurrentWorkspace();
  const [name, setName] = useState("");
  const [kind, setKind] = useState<ProjectKind>("code");
  const create = useMutation({
    mutationFn: () => api.createProject(workspace?.id ?? "", { name, kind }),
    onSuccess: async (p) => {
      await qc.invalidateQueries({ queryKey: ["projects", workspace?.id] });
      navigate(`/p/${p.id}`);
    },
  });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    create.mutate();
  };
  return (
    <div className="mx-auto max-w-2xl">
      <PageHeader title="New project" subtitle={workspace ? `In ${workspace.name}` : undefined} />
      <StateView status={query.status} data={workspace} error={query.error} onRetry={() => void query.refetch()}>
        {(ws) =>
          !atLeast(ws.role, "maintainer") ? (
            <div role="alert" className="rounded-lg border border-border-strong bg-bg-surface p-4">
              <p className="font-semibold text-warning">You don't have access</p>
              <p className="mt-1 text-small text-text-secondary">Creating projects needs the maintainer role or higher in this workspace. You are {ws.role}.</p>
            </div>
          ) : (
            <Card>
              <form onSubmit={submit} className="flex flex-col gap-4" noValidate>
                <TextField label="Name" required maxLength={100} value={name} onChange={(e) => setName(e.target.value)} error={fieldError(create.error, "name")} />
                <fieldset className="flex flex-col gap-2">
                  <legend className="mb-1 text-small font-medium text-text-secondary">Kind</legend>
                  {KINDS.map((k) => (
                    <label key={k.kind} className={`flex cursor-pointer gap-3 rounded-md border p-3 ${kind === k.kind ? "border-accent bg-bg-raised" : "border-border-subtle"}`}>
                      <input type="radio" name="kind" value={k.kind} checked={kind === k.kind} onChange={() => setKind(k.kind)} />
                      <span>
                        <span className="block font-medium">{k.label}</span>
                        <span className="block text-caption text-text-secondary">{k.about}</span>
                      </span>
                    </label>
                  ))}
                </fieldset>
                {create.error && !fieldError(create.error, "name") && <FormError error={create.error} />}
                <div className="flex justify-end gap-2">
                  <Button variant="secondary" onClick={() => navigate(-1)}>
                    Cancel
                  </Button>
                  <Button type="submit" busy={create.isPending} disabled={!name.trim()}>
                    Create project
                  </Button>
                </div>
              </form>
            </Card>
          )
        }
      </StateView>
    </div>
  );
}
