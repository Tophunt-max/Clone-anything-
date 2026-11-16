import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { createBrowserRouter } from "react-router";
import "@fontsource-variable/inter";
import "@fontsource/jetbrains-mono";
import "./index.css";
import { App } from "./App.tsx";
import { routes } from "./routes.tsx";
import { createApiClient } from "@forge/api-client";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App router={createBrowserRouter(routes)} api={createApiClient()} />
    </StrictMode>,
  );
}
