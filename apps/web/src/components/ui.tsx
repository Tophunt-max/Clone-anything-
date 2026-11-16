import { useId, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes } from "react";
import { ApiError } from "@forge/api-client";

// Small set of shared controls. Colours, sizes and radii come from the design tokens only.

type Variant = "primary" | "secondary" | "danger";
const VARIANT: Record<Variant, string> = {
  primary: "bg-accent-strong text-on-accent hover:opacity-90",
  secondary: "border border-border-strong bg-bg-field text-text-primary hover:bg-bg-raised",
  danger: "border border-danger text-danger hover:bg-bg-raised",
};

export function Button({ variant = "primary", busy = false, className = "", children, ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; busy?: boolean }) {
  return (
    <button
      type="button"
      {...rest}
      disabled={rest.disabled || busy}
      aria-busy={busy || undefined}
      className={`inline-flex h-9 items-center justify-center gap-2 rounded-md px-3 text-small font-medium disabled:cursor-not-allowed disabled:opacity-50 ${VARIANT[variant]} ${className}`}
    >
      {children}
    </button>
  );
}

const FIELD = "h-10 w-full rounded-md border border-border-subtle bg-bg-field px-3 text-body text-text-primary placeholder:text-text-muted focus:border-accent";

export function TextField({ label, error, hint, ...rest }: InputHTMLAttributes<HTMLInputElement> & { label: string; error?: string | undefined; hint?: string }) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="text-small font-medium text-text-secondary">
        {label}
      </label>
      <input id={id} aria-invalid={error ? true : undefined} aria-describedby={error || hint ? `${id}-note` : undefined} className={FIELD} {...rest} />
      {(error || hint) && (
        <p id={`${id}-note`} className={`text-caption ${error ? "text-danger" : "text-text-muted"}`}>
          {error ?? hint}
        </p>
      )}
    </div>
  );
}

export function SelectField({ label, children, ...rest }: SelectHTMLAttributes<HTMLSelectElement> & { label: string; children: ReactNode }) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="text-small font-medium text-text-secondary">
        {label}
      </label>
      <select id={id} className={FIELD} {...rest}>
        {children}
      </select>
    </div>
  );
}

export function Card({ title, actions, children, className = "" }: { title?: string | undefined; actions?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`rounded-lg border border-border-subtle bg-bg-surface p-4 shadow-e1 ${className}`}>
      {(title || actions) && (
        <div className="mb-3 flex items-center justify-between gap-2">
          {title && <h2 className="text-lead font-semibold">{title}</h2>}
          {actions}
        </div>
      )}
      {children}
    </section>
  );
}

export function PageHeader({ title, subtitle, actions }: { title: string; subtitle?: string | undefined; actions?: ReactNode }) {
  return (
    <header className="mb-6 flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
      <div>
        <h1 className="text-title font-semibold">{title}</h1>
        {subtitle && <p className="mt-1 text-text-secondary">{subtitle}</p>}
      </div>
      {actions && <div className="flex gap-2">{actions}</div>}
    </header>
  );
}

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "accent" | "warning" }) {
  const t = { neutral: "text-text-secondary border-border-strong", accent: "text-accent border-accent", warning: "text-warning border-warning" }[tone];
  return <span className={`inline-flex items-center rounded-full border px-2 py-0.5 text-tiny font-medium ${t}`}>{children}</span>;
}

/** Inline error for a form: the server's message, the field it names, and the request ID. */
export function FormError({ error }: { error: unknown }) {
  if (!error) return null;
  const e = error instanceof ApiError ? error : undefined;
  return (
    <div role="alert" className="rounded-md border border-danger bg-bg-raised p-3 text-small">
      <p className="text-danger">{e?.message ?? "Something went wrong."}</p>
      {e?.requestId && <p className="mt-1 font-mono text-tiny text-text-muted">Request ID: {e.requestId}</p>}
    </div>
  );
}

/** The message for one field when the server's 422 names it. */
export function fieldError(error: unknown, field: string): string | undefined {
  return error instanceof ApiError && error.field === field ? error.message : undefined;
}

export function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}
