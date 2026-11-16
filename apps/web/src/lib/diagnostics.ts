import { create } from "zustand";
import { ApiError } from "@forge/api-client";

// The last API errors this tab saw, with their request IDs, for Help and Diagnostics. Kept in
// memory only: nothing is stored or sent anywhere.
export interface SeenError {
  at: string;
  status: number;
  code: string;
  message: string;
  requestId: string | undefined;
}

const MAX = 20;

interface DiagnosticsState {
  errors: SeenError[];
  record: (e: unknown) => void;
  clear: () => void;
}

export const useDiagnostics = create<DiagnosticsState>((set) => ({
  errors: [],
  record: (e) => {
    if (!(e instanceof ApiError)) return;
    const item: SeenError = { at: new Date().toISOString(), status: e.status, code: e.code, message: e.message, requestId: e.requestId };
    set((s) => ({ errors: [item, ...s.errors].slice(0, MAX) }));
  },
  clear: () => set({ errors: [] }),
}));
