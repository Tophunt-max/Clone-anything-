import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiError } from "@forge/api-client";
import type { ToolCallResult } from "@forge/shared-types";
import { useApi } from "../api.ts";

/** The outcome of a tool call that needs a human approval before it can run. */
export interface NeedsApproval {
  kind: "approval";
  approvalId: string;
}

/** Runs a project tool, turning the 409 "approval required" into a typed result the UI handles. */
export function useRunTool(projectId: string) {
  const api = useApi();
  const qc = useQueryClient();
  return useMutation<ToolCallResult | NeedsApproval, ApiError, { tool: string; params?: unknown; approvalId?: string }>({
    mutationFn: async ({ tool, params, approvalId }) => {
      try {
        const res = await api.runTool(projectId, { tool, ...(params !== undefined ? { params } : {}), ...(approvalId ? { approval_id: approvalId } : {}) });
        return res;
      } catch (e) {
        if (e instanceof ApiError && e.status === 409 && e.code === "approval_required" && e.field) {
          return { kind: "approval", approvalId: e.field };
        }
        throw e;
      }
    },
    onSuccess: (res) => {
      if ("kind" in res) return;
      void qc.invalidateQueries({ queryKey: ["evidence", projectId] });
      void qc.invalidateQueries({ queryKey: ["executions", projectId] });
    },
  });
}

export function isApproval(r: ToolCallResult | NeedsApproval | undefined): r is NeedsApproval {
  return !!r && "kind" in r && r.kind === "approval";
}

export function useEvidence(projectId: string, operation?: string) {
  const api = useApi();
  return useQuery({ queryKey: ["evidence", projectId, operation ?? null], queryFn: ({ signal }) => api.evidence(projectId, { limit: 100, signal, ...(operation ? { operation } : {}) }) });
}

/** A short, human line for an evidence subject. */
export function subjectLabel(subject: { kind?: string; name?: string }): string {
  return subject.name ?? subject.kind ?? "evidence";
}
