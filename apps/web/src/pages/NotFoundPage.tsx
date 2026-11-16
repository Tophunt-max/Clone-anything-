import { Link } from "react-router";

export function NotFoundPage() {
  return (
    <section className="mx-auto max-w-xl py-12 text-center">
      <h1 className="text-section font-semibold">Page not found</h1>
      <p className="mt-2 text-text-secondary">There is nothing at this address.</p>
      <Link to="/" className="mt-4 inline-block text-accent underline">
        Go home
      </Link>
    </section>
  );
}
