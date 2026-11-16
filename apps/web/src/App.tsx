import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, type createBrowserRouter } from "react-router";
import type { ApiClient } from "@forge/api-client";
import { ApiContext } from "./api.ts";

export function App({ router, api, queryClient: given }: { router: ReturnType<typeof createBrowserRouter>; api: ApiClient; queryClient?: QueryClient }) {
  const [queryClient] = useState(
    () =>
      given ??
      new QueryClient({
        defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: false } },
      }),
  );
  return (
    <ApiContext.Provider value={api}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </ApiContext.Provider>
  );
}
