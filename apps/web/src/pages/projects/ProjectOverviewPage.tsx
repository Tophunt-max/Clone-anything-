import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router";
import type { Project } from "@forge/shared-types";
import { useApi } from "../../api.ts";
import { StateView } from "../../components/StateView.tsx";
import { Badge, Button, Card, FormError, PageHeader, TextField, fieldError, formatDate } from "../../components/ui.tsx";
import { atLeast, useWorkspaces } from "../../lib/workspace.ts";

export function ProjectOverviewPage() {
  const { id = "" } = useParams();
  const api = useApi();
  const project = useQuery({ queryKey: ["project", id], queryFn: ({ signal }) => api.project(id, signal) });
  return (
    <div className="mx-auto max-w-4xl">
      <StateView status={project.status} data={project.data} error={project.error} onRetry={() => void project.refetch()} loadingLabel="Loading the project…">
        {(p) => <Overview project={p} />}
      </StateView>
    </div>
  );
}

function Overview({ project: p }: { project: Project }) {
  const api = useApi();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const role = useWorkspaces().data?.items.find((w) => w.id === p.workspace_id)?.role;
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(p.name);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const done = (updated: Project) => {
    qc.setQueryData(["project", p.id], updated);
    void qc.invalidateQueries({ queryKey: ["projects", p.workspace_id] });
  };
  const update = useMutation({ mutationFn: (b: { name?: string; archived?: boolean }) => api.updateProject(p.id, b), onSuccess: (u) => (done(u), setEditing(false)) });
  const remove = useMutation({
    mutationFn: () => api.deleteProject(p.id),
    onSuccess: () => {
      qc.removeQueries({ queryKey: ["project", p.id] });
      void qc.invalidateQueries({ queryKey: ["projects", p.workspace_id] });
      navigate("/projects", { replace: true });
    },
  });
  const canEdit = atLeast(role, "maintainer");
  const canDelete = atLeast(role, "admin");

  return (
    <>
      <PageHeader
        title={p.name}
        subtitle={`${p.kind[0]?.toUpperCase()}${p.kind.slice(1)} project`}
        actions={
          canEdit && (
            <>
              <Button variant="secondary" onClick={() => setEditing(true)}>
                Rename
              </Button>
              <Button variant="secondary" busy={update.isPending && !editing} onClick={() => update.mutate({ archived: !p.archived_at })}>
                {p.archived_at ? "Unarchive" : "Archive"}
              </Button>
            </>
          )
        }
      />
      {p.archived_at && (
        <p className="mb-4">
          <Badge tone="warning">Archived {formatDate(p.archived_at)}</Badge>
        </p>
      )}
      {editing && (
        <Card className="mb-4" title="Rename">
          <form
            className="flex flex-col gap-3 md:flex-row md:items-end"
            onSubmit={(e) => {
              e.preventDefault();
              update.mutate({ name });
            }}
          >
            <div className="flex-1">
              <TextField label="Name" value={name} maxLength={100} onChange={(e) => setName(e.target.value)} />
            </div>
            <Button type="submit" busy={update.isPending} disabled={!name.trim()}>
              Save
            </Button>
            <Button variant="secondary" onClick={() => (setEditing(false), setName(p.name))}>
              Cancel
            </Button>
          </form>
        </Card>
      )}
      {update.error && <FormError error={update.error} />}
      {atLeast(role, "maintainer") && <DeepModeToggle project={p} onChanged={done} />}
      <Card title="Analyze" className="mb-4">
        <p className="mb-3 text-small text-text-secondary">Study a target with the engine. Findings are recorded as evidence.</p>
        <div className="flex flex-wrap gap-2">
          <Link to={`/p/${p.id}/analyze/binary`} className="rounded-md border border-border-strong bg-bg-field px-3 py-1.5 text-small hover:bg-bg-raised">Binary</Link>
          <Link to={`/p/${p.id}/analyze/apk`} className="rounded-md border border-border-strong bg-bg-field px-3 py-1.5 text-small hover:bg-bg-raised">APK</Link>
          <Link to={`/p/${p.id}/analyze/web`} className="rounded-md border border-border-strong bg-bg-field px-3 py-1.5 text-small hover:bg-bg-raised">Website</Link>
          <Link to={`/p/${p.id}/evidence`} className="rounded-md border border-border-strong bg-bg-field px-3 py-1.5 text-small hover:bg-bg-raised">Evidence</Link>
        </div>
      </Card>
      <Card title="Details">
        <dl className="grid grid-cols-1 gap-3 md:grid-cols-2">
          <Item label="Kind" value={p.kind} />
          <Item label="Analysis mode" value={p.deep_mode ? "Deep (reads inside targets)" : "Clean room (outside only)"} />
          <Item label="Created" value={formatDate(p.created_at)} />
          <Item label="Updated" value={formatDate(p.updated_at)} />
          <Item label="Your role" value={role ?? "unknown"} />
          <Item label="Project ID" value={p.id} mono />
        </dl>
      </Card>
      {canDelete && (
        <Card className="mt-4" title="Danger zone">
          {!confirmDelete ? (
            <Button variant="danger" onClick={() => setConfirmDelete(true)}>
              Delete project
            </Button>
          ) : (
            <div className="flex flex-col gap-3">
              <p className="text-small text-text-secondary">This deletes “{p.name}” for everyone in the workspace. It can't be undone.</p>
              <div className="flex gap-2">
                <Button variant="danger" busy={remove.isPending} onClick={() => remove.mutate()}>
                  Yes, delete it
                </Button>
                <Button variant="secondary" onClick={() => setConfirmDelete(false)}>
                  Keep it
                </Button>
              </div>
              <FormError error={remove.error} />
            </div>
          )}
        </Card>
      )}
    </>
  );
}

function Item({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="min-w-0">
      <dt className="text-caption text-text-muted">{label}</dt>
      <dd className={`truncate ${mono ? "font-mono text-code" : ""}`}>{value}</dd>
    </div>
  );
}

function DeepModeToggle({ project, onChanged }: { project: Project; onChanged: (p: Project) => void }) {
  const api = useApi();
  const [authorization, setAuthorization] = useState("");
  const set = useMutation({ mutationFn: (enabled: boolean) => api.setDeepMode(project.id, enabled, enabled ? authorization : undefined), onSuccess: onChanged });
  return (
    <Card className="mb-4" title="Analysis mode">
      <p className="text-small text-text-secondary">
        {project.deep_mode
          ? "Deep mode is on: tools may read inside targets (code, files, page source)."
          : "Clean room: tools study targets only from the outside. Turn on Deep mode to read inside, which you must be authorized to do."}
      </p>
      {project.deep_mode ? (
        <Button className="mt-3" variant="secondary" busy={set.isPending} onClick={() => set.mutate(false)}>
          Turn off Deep mode
        </Button>
      ) : (
        <div className="mt-3 flex flex-col gap-2">
          <TextField label="Authorization" placeholder="e.g. we own this software / written permission on file" value={authorization} onChange={(e) => setAuthorization(e.target.value)} error={fieldError(set.error, "authorization")} />
          <div>
            <Button busy={set.isPending} disabled={!authorization.trim()} onClick={() => set.mutate(true)}>
              Turn on Deep mode
            </Button>
          </div>
        </div>
      )}
      {set.error && !fieldError(set.error, "authorization") && <FormError error={set.error} />}
    </Card>
  );
}
