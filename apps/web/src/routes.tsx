import type { RouteObject } from "react-router";
import { Link } from "react-router";
import { RequireAuth } from "./auth.tsx";
import { AppShell } from "./shell/AppShell.tsx";
import { DashboardPage } from "./pages/DashboardPage.tsx";
import { HelpPage } from "./pages/HelpPage.tsx";
import { NotFoundPage } from "./pages/NotFoundPage.tsx";
import { RouteErrorPage } from "./pages/RouteErrorPage.tsx";
import { LoginPage } from "./pages/auth/LoginPage.tsx";
import { RegisterPage } from "./pages/auth/RegisterPage.tsx";
import { CreateProjectPage } from "./pages/projects/CreateProjectPage.tsx";
import { ProjectListPage } from "./pages/projects/ProjectListPage.tsx";
import { ProjectOverviewPage } from "./pages/projects/ProjectOverviewPage.tsx";
import { ToolRegistryPage } from "./pages/ToolRegistryPage.tsx";
import { EvidenceExplorerPage } from "./pages/analyze/EvidenceExplorerPage.tsx";
import { BinaryAnalyzerPage } from "./pages/analyze/BinaryAnalyzerPage.tsx";
import { ApkAnalyzerPage } from "./pages/analyze/ApkAnalyzerPage.tsx";
import { WebsiteAnalyzerPage } from "./pages/analyze/WebsiteAnalyzerPage.tsx";
import { SecurityPage } from "./pages/settings/SecurityPage.tsx";
import { WorkspaceSettingsPage } from "./pages/settings/WorkspaceSettingsPage.tsx";

// Only screens that work are routed. Each later phase adds its screens here
// (docs/platform/06-ui-plan.md §3).
export const routes: RouteObject[] = [
  { path: "/login", element: <LoginPage />, errorElement: <RouteErrorPage /> },
  { path: "/register", element: <RegisterPage />, errorElement: <RouteErrorPage /> },
  {
    // Reachable signed out too: it is where people go when sign-in itself is broken.
    path: "/help",
    errorElement: <RouteErrorPage />,
    element: (
      <div className="min-h-full py-6 md:p-8">
        <p className="mx-auto mb-4 max-w-3xl px-4 md:px-0">
          <Link to="/" className="text-small text-accent underline">
            ← Back to Tophunt Forge
          </Link>
        </p>
        <HelpPage />
      </div>
    ),
  },
  {
    element: <RequireAuth />,
    errorElement: <RouteErrorPage />,
    children: [
      {
        element: <AppShell />,
        children: [
          { index: true, element: <DashboardPage /> },
          { path: "projects", element: <ProjectListPage /> },
          { path: "projects/new", element: <CreateProjectPage /> },
          { path: "p/:id", element: <ProjectOverviewPage /> },
          { path: "p/:id/evidence", element: <EvidenceExplorerPage /> },
          { path: "p/:id/analyze/binary", element: <BinaryAnalyzerPage /> },
          { path: "p/:id/analyze/apk", element: <ApkAnalyzerPage /> },
          { path: "p/:id/analyze/web", element: <WebsiteAnalyzerPage /> },
          { path: "tools", element: <ToolRegistryPage /> },
          { path: "settings/workspace", element: <WorkspaceSettingsPage /> },
          { path: "settings/security", element: <SecurityPage /> },
          { path: "*", element: <NotFoundPage /> },
        ],
      },
    ],
  },
];
