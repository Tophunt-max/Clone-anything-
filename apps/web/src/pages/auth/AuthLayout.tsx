import type { ReactNode } from "react";
import { Link } from "react-router";

export function AuthLayout({ title, children, footer }: { title: string; children: ReactNode; footer: ReactNode }) {
  return (
    <div className="flex min-h-full items-center justify-center p-4">
      <div className="w-full max-w-sm">
        <p className="mb-6 text-center text-lead font-semibold">Tophunt Forge</p>
        <main className="rounded-lg border border-border-subtle bg-bg-surface p-6 shadow-e2">
          <h1 className="mb-4 text-section font-semibold">{title}</h1>
          {children}
        </main>
        <p className="mt-4 text-center text-small text-text-secondary">{footer}</p>
        <p className="mt-2 text-center text-caption text-text-muted">
          <Link to="/help" className="underline">
            Help and diagnostics
          </Link>
        </p>
      </div>
    </div>
  );
}
