import type { RouteObject } from "react-router";
import { AppShell } from "./shell/AppShell.tsx";
import { HomePage } from "./pages/HomePage.tsx";
import { NotFoundPage } from "./pages/NotFoundPage.tsx";
import { RouteErrorPage } from "./pages/RouteErrorPage.tsx";

// Only screens that work are routed. Each later phase adds its screens here
// (docs/platform/06-ui-plan.md §3).
export const routes: RouteObject[] = [
  {
    element: <AppShell />,
    errorElement: <RouteErrorPage />,
    children: [
      { index: true, element: <HomePage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
];
