import { isRouteErrorResponse, useRouteError } from "react-router";

export function RouteErrorPage() {
  const err = useRouteError();
  const message = isRouteErrorResponse(err) ? `${err.status} ${err.statusText}` : "This screen failed to load.";
  return (
    <div role="alert" className="mx-auto max-w-xl p-8 text-center">
      <h1 className="text-section font-semibold">Something went wrong</h1>
      <p className="mt-2 text-text-secondary">{message}</p>
      <button type="button" onClick={() => window.location.reload()} className="mt-4 rounded-md bg-accent-strong px-3 py-1.5 text-on-accent">
        Reload
      </button>
    </div>
  );
}
