import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Navigate, Outlet, useLocation, useNavigate } from "react-router";
import { ApiError } from "@forge/api-client";
import { useApi } from "./api.ts";
import { StateView } from "./components/StateView.tsx";

export const ME = ["me"] as const;

export function useMe() {
  const api = useApi();
  return useQuery({ queryKey: ME, queryFn: ({ signal }) => api.me(signal), retry: false, staleTime: 60_000 });
}

/** Only same-site paths, so `?next=` can't send someone to another site after sign-in. */
export function safeNext(next: string | null): string {
  return next && next.startsWith("/") && !next.startsWith("//") && !next.startsWith("/\\") ? next : "/";
}

/** Screens inside need a signed-in user; anyone else goes to sign-in and comes back after. */
export function RequireAuth() {
  const me = useMe();
  const loc = useLocation();
  if (me.status === "error" && me.error instanceof ApiError && me.error.status === 401) {
    const next = `${loc.pathname}${loc.search}`;
    return <Navigate to={`/login?next=${encodeURIComponent(next)}`} replace />;
  }
  if (me.status !== "success") {
    return (
      <div className="mx-auto max-w-md p-8">
        <StateView status={me.status} data={me.data} error={me.error} onRetry={() => void me.refetch()} loadingLabel="Loading your account…">
          {() => null}
        </StateView>
      </div>
    );
  }
  return <Outlet />;
}

export function useSignOut() {
  const api = useApi();
  const qc = useQueryClient();
  const navigate = useNavigate();
  return async () => {
    try {
      await api.logout();
    } finally {
      qc.clear();
      navigate("/login", { replace: true });
    }
  };
}
