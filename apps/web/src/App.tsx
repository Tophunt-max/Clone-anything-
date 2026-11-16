import { useState } from "react";
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, type createBrowserRouter } from "react-router";
import { ApiError, type ApiClient } from "@forge/api-client";
import { ApiContext } from "./api.ts";
import { useDiagnostics } from "./lib/diagnostics.ts";

export function makeQueryClient(): QueryClient {
  const record = (e: unknown) => useDiagnostics.getState().record(e);
  return new QueryClient({
    queryCache: new QueryCache({ onError: record }),
    mutationCache: new MutationCache({ onError: record }),
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: false,
        // Client errors won't change by asking again; network and server errors might.
        retry: (n, e) => n < 1 && !(e instanceof ApiError && e.status >= 400 && e.status < 500),
      },
    },
  });
}

export function App({ router, api, queryClient: given }: { router: ReturnType<typeof createBrowserRouter>; api: ApiClient; queryClient?: QueryClient }) {
  const [queryClient] = useState(() => given ?? makeQueryClient());
  return (
    <ApiContext.Provider value={api}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </ApiContext.Provider>
  );
}
