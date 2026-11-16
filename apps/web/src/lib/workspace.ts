import { create } from "zustand";
import { persist } from "zustand/middleware";
import { useQuery } from "@tanstack/react-query";
import type { Workspace } from "@forge/shared-types";
import { useApi } from "../api.ts";

// The workspace the person is working in. Only its ID is remembered (not a secret); the server
// decides on every request whether they may use it.
interface CurrentWorkspace {
  id: string | null;
  set: (id: string) => void;
}

export const useCurrentWorkspaceId = create<CurrentWorkspace>()(
  persist((set) => ({ id: null, set: (id) => set({ id }) }), { name: "forge.workspace" }),
);

export function useWorkspaces() {
  const api = useApi();
  // The first 200 cover everyone today; a picker with search comes when people have more.
  return useQuery({ queryKey: ["workspaces"], queryFn: ({ signal }) => api.workspaces({ limit: 200, signal }) });
}

/** The chosen workspace if the person is still a member, otherwise their first one. */
export function useCurrentWorkspace(): { workspace: Workspace | undefined; query: ReturnType<typeof useWorkspaces> } {
  const query = useWorkspaces();
  const id = useCurrentWorkspaceId((s) => s.id);
  const items = query.data?.items ?? [];
  const workspace = items.find((w) => w.id === id) ?? items[0];
  return { workspace, query };
}

const ORDER = ["viewer", "editor", "maintainer", "admin", "owner"] as const;

/** Mirrors the server's permission table, only to hide controls; the server still decides. */
export function atLeast(role: Workspace["role"] | undefined, need: Workspace["role"]): boolean {
  return role !== undefined && ORDER.indexOf(role) >= ORDER.indexOf(need);
}
