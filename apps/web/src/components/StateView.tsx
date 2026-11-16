import type { ReactNode } from "react";
import { ApiError } from "@forge/api-client";

// The shared wrapper for data screens: loading, error (with request id and retry), empty, ready.
// Forbidden (403-type) states arrive with authentication in Phase 2.
export function StateView<T>({
  status,
  data,
  error,
  onRetry,
  isEmpty,
  empty,
  loadingLabel = "Loading…",
  children,
}: {
  status: "pending" | "error" | "success";
  data: T | undefined;
  error: unknown;
  onRetry: () => void;
  isEmpty?: (d: T) => boolean;
  empty?: ReactNode;
  loadingLabel?: string;
  children: (d: T) => ReactNode;
}) {
  if (status === "pending") {
    return (
      <div role="status" aria-live="polite" className="animate-pulse rounded-lg border border-border-subtle bg-bg-surface p-4 text-text-muted">
        {loadingLabel}
      </div>
    );
  }
  if (status === "error" || data === undefined) {
    const e = error instanceof ApiError ? error : undefined;
    return (
      <div role="alert" className="rounded-lg border border-border-strong bg-bg-surface p-4">
        <p className="font-semibold text-danger">{e?.title ?? "Something went wrong"}</p>
        <p className="mt-1 text-small text-text-secondary">{e?.message ?? "The request failed."}</p>
        {e?.requestId && <p className="mt-2 font-mono text-tiny text-text-muted">Request ID: {e.requestId}</p>}
        <button type="button" onClick={onRetry} className="mt-3 rounded-md bg-accent-strong px-3 py-1.5 text-small text-on-accent hover:opacity-90">
          Try again
        </button>
      </div>
    );
  }
  if (isEmpty?.(data)) return <>{empty}</>;
  return <>{children(data)}</>;
}
