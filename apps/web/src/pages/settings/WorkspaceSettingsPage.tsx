import { useState, type FormEvent } from "react";
import { useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { ROLES, type Role, type Workspace } from "@forge/shared-types";
import { useApi } from "../../api.ts";
import { useMe } from "../../auth.tsx";
import { StateView } from "../../components/StateView.tsx";
import { Badge, Button, Card, FormError, PageHeader, SelectField, TextField, fieldError } from "../../components/ui.tsx";
import { atLeast, useCurrentWorkspace, useCurrentWorkspaceId } from "../../lib/workspace.ts";

export function WorkspaceSettingsPage() {
  const { workspace, query } = useCurrentWorkspace();
  return (
    <div className="mx-auto max-w-4xl">
      <PageHeader title="Workspace settings" subtitle={workspace?.name} />
      <div className="flex flex-col gap-4">
        <StateView status={query.status} data={workspace} error={query.error} onRetry={() => void query.refetch()}>
          {(ws) => (
            <>
              <General ws={ws} />
              <Members ws={ws} />
            </>
          )}
        </StateView>
        <NewWorkspace />
      </div>
    </div>
  );
}

function General({ ws }: { ws: Workspace }) {
  const api = useApi();
  const qc = useQueryClient();
  const [name, setName] = useState(ws.name);
  const [confirm, setConfirm] = useState("");
  const rename = useMutation({ mutationFn: () => api.renameWorkspace(ws.id, name), onSuccess: () => qc.invalidateQueries({ queryKey: ["workspaces"] }) });
  const remove = useMutation({ mutationFn: () => api.deleteWorkspace(ws.id), onSuccess: () => qc.invalidateQueries({ queryKey: ["workspaces"] }) });
  const canRename = atLeast(ws.role, "admin");
  return (
    <Card title="General">
      <dl className="mb-4 grid grid-cols-1 gap-3 md:grid-cols-2">
        <div>
          <dt className="text-caption text-text-muted">Short name</dt>
          <dd className="font-mono text-code">{ws.slug}</dd>
        </div>
        <div>
          <dt className="text-caption text-text-muted">Your role</dt>
          <dd>{ws.role}</dd>
        </div>
      </dl>
      {canRename && (
        <form
          className="flex flex-col gap-3 md:flex-row md:items-end"
          onSubmit={(e) => {
            e.preventDefault();
            rename.mutate();
          }}
        >
          <div className="flex-1">
            <TextField label="Name" value={name} maxLength={100} onChange={(e) => setName(e.target.value)} error={fieldError(rename.error, "name")} />
          </div>
          <Button type="submit" busy={rename.isPending} disabled={!name.trim() || name === ws.name}>
            Save
          </Button>
        </form>
      )}
      {rename.isSuccess && <p className="mt-2 text-caption text-success">Saved.</p>}
      {ws.role === "owner" && (
        <div className="mt-6 border-t border-border-subtle pt-4">
          <p className="mb-2 text-small text-text-secondary">
            Deleting the workspace deletes all its projects for every member. Type <span className="font-mono">{ws.slug}</span> to confirm.
          </p>
          <div className="flex flex-col gap-2 md:flex-row md:items-end">
            <div className="flex-1">
              <TextField label="Confirm short name" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
            </div>
            <Button variant="danger" busy={remove.isPending} disabled={confirm !== ws.slug} onClick={() => remove.mutate()}>
              Delete workspace
            </Button>
          </div>
          <FormError error={remove.error} />
        </div>
      )}
    </Card>
  );
}

function Members({ ws }: { ws: Workspace }) {
  const api = useApi();
  const qc = useQueryClient();
  const me = useMe();
  const key = ["members", ws.id];
  const members = useInfiniteQuery({
    queryKey: key,
    queryFn: ({ pageParam, signal }) => api.members(ws.id, { limit: 100, signal, ...(pageParam ? { cursor: pageParam } : {}) }),
    initialPageParam: "",
    getNextPageParam: (l) => l.next_cursor ?? undefined,
  });
  const refresh = () => qc.invalidateQueries({ queryKey: key });
  const change = useMutation({ mutationFn: (v: { user: string; role: Role }) => api.changeMember(ws.id, v.user, v.role), onSuccess: refresh });
  const remove = useMutation({
    mutationFn: (user: string) => api.removeMember(ws.id, user),
    onSuccess: (_, user) => (user === me.data?.id ? qc.invalidateQueries({ queryKey: ["workspaces"] }) : refresh()),
  });
  const isAdmin = atLeast(ws.role, "admin");
  // Owners can give any role; admins only roles below their own (the server enforces this).
  const assignable = ROLES.filter((r) => ws.role === "owner" || ROLES.indexOf(r) < ROLES.indexOf(ws.role));
  const list = members.data?.pages.flatMap((p) => p.items);
  return (
    <Card title="Members">
      <StateView status={members.status} data={list} error={members.error} onRetry={() => void members.refetch()}>
        {(items) => (
          <ul className="divide-y divide-border-subtle">
            {items.map((m) => {
              const self = m.user_id === me.data?.id;
              const manageable = isAdmin && !self && (ws.role === "owner" || ROLES.indexOf(m.role) < ROLES.indexOf(ws.role));
              return (
                <li key={m.user_id} className="flex flex-col gap-2 py-3 md:flex-row md:items-center md:justify-between">
                  <div className="min-w-0">
                    <p className="truncate font-medium">
                      {m.display_name} {self && <Badge tone="accent">you</Badge>}
                    </p>
                    <p className="truncate text-caption text-text-muted">{m.email}</p>
                  </div>
                  <div className="flex items-center gap-2">
                    {manageable ? (
                      <select
                        aria-label={`Role of ${m.display_name}`}
                        className="h-9 rounded-md border border-border-subtle bg-bg-field px-2 text-small"
                        value={m.role}
                        onChange={(e) => change.mutate({ user: m.user_id, role: e.target.value as Role })}
                      >
                        {(assignable.includes(m.role) ? assignable : [m.role, ...assignable]).map((r) => (
                          <option key={r} value={r}>
                            {r}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <Badge>{m.role}</Badge>
                    )}
                    {(manageable || self) && (
                      <Button variant="danger" onClick={() => remove.mutate(m.user_id)}>
                        {self ? "Leave" : "Remove"}
                      </Button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </StateView>
      <div className="mt-2 flex flex-col gap-2">
        <FormError error={change.error ?? remove.error} />
      </div>
      {isAdmin && <AddMember ws={ws} roles={assignable} onAdded={refresh} />}
    </Card>
  );
}

function AddMember({ ws, roles, onAdded }: { ws: Workspace; roles: readonly Role[]; onAdded: () => void }) {
  const api = useApi();
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<Role>("editor");
  const add = useMutation({ mutationFn: () => api.addMember(ws.id, { email, role }), onSuccess: () => (setEmail(""), onAdded()) });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    add.mutate();
  };
  return (
    <form onSubmit={submit} className="mt-4 flex flex-col gap-3 border-t border-border-subtle pt-4 md:flex-row md:items-end">
      <div className="flex-1">
        <TextField label="Add a member by email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} error={fieldError(add.error, "email")} hint="They need an account already." />
      </div>
      <SelectField label="Role" value={role} onChange={(e) => setRole(e.target.value as Role)}>
        {roles.map((r) => (
          <option key={r} value={r}>
            {r}
          </option>
        ))}
      </SelectField>
      <Button type="submit" busy={add.isPending} disabled={!email}>
        Add
      </Button>
      {add.error && !fieldError(add.error, "email") && <FormError error={add.error} />}
    </form>
  );
}

function NewWorkspace() {
  const api = useApi();
  const qc = useQueryClient();
  const select = useCurrentWorkspaceId((s) => s.set);
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const create = useMutation({
    mutationFn: () => api.createWorkspace({ name, slug }),
    onSuccess: async (w) => {
      await qc.invalidateQueries({ queryKey: ["workspaces"] });
      select(w.id);
      setName("");
      setSlug("");
    },
  });
  return (
    <Card title="New workspace">
      <form
        className="flex flex-col gap-3 md:flex-row md:items-end"
        onSubmit={(e) => {
          e.preventDefault();
          create.mutate();
        }}
      >
        <div className="flex-1">
          <TextField label="Name" value={name} maxLength={100} onChange={(e) => setName(e.target.value)} error={fieldError(create.error, "name")} />
        </div>
        <div className="flex-1">
          <TextField label="Short name" value={slug} maxLength={40} onChange={(e) => setSlug(e.target.value.toLowerCase())} error={fieldError(create.error, "slug")} hint="Letters, digits and dashes." />
        </div>
        <Button type="submit" busy={create.isPending} disabled={!name.trim() || slug.length < 3}>
          Create
        </Button>
      </form>
      {create.error && !fieldError(create.error, "name") && !fieldError(create.error, "slug") && <FormError error={create.error} />}
    </Card>
  );
}
