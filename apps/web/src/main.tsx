import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { createBrowserRouter } from "react-router";
import "@fontsource-variable/inter";
import "@fontsource/jetbrains-mono";
import "./index.css";
import { App, makeQueryClient } from "./App.tsx";
import { routes } from "./routes.tsx";
import { createApiClient } from "@forge/api-client";

const queryClient = makeQueryClient();
const router = createBrowserRouter(routes);
const api = createApiClient({
  // The refresh failed: the session is over. Forget cached data and go to sign-in, then back.
  onSessionEnded: () => {
    queryClient.clear();
    const here = `${window.location.pathname}${window.location.search}`;
    if (!here.startsWith("/login") && !here.startsWith("/register") && !here.startsWith("/help")) {
      void router.navigate(`/login?next=${encodeURIComponent(here)}`, { replace: true });
    }
  },
});

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App router={router} api={api} queryClient={queryClient} />
    </StrictMode>,
  );
}
