import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useSearchParams } from "react-router";
import { useApi } from "../../api.ts";
import { ME, safeNext } from "../../auth.tsx";
import { Button, FormError, TextField, fieldError } from "../../components/ui.tsx";
import { AuthLayout } from "./AuthLayout.tsx";

export function LoginPage() {
  const api = useApi();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const next = safeNext(params.get("next"));
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const login = useMutation({
    mutationFn: () => api.login({ email, password }),
    onSuccess: (s) => {
      qc.clear();
      qc.setQueryData(ME, s.user);
      navigate(next, { replace: true });
    },
  });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    login.mutate();
  };
  return (
    <AuthLayout title="Sign in" footer={<>New here? <Link to={`/register${next !== "/" ? `?next=${encodeURIComponent(next)}` : ""}`} className="text-accent underline">Create an account</Link></>}>
      <form onSubmit={submit} className="flex flex-col gap-4" noValidate>
        <TextField label="Email" type="email" autoComplete="email" required value={email} onChange={(e) => setEmail(e.target.value)} error={fieldError(login.error, "email")} />
        <TextField label="Password" type="password" autoComplete="current-password" required value={password} onChange={(e) => setPassword(e.target.value)} />
        {login.error && !fieldError(login.error, "email") && <FormError error={login.error} />}
        <Button type="submit" busy={login.isPending} disabled={!email || !password}>
          {login.isPending ? "Signing in…" : "Sign in"}
        </Button>
      </form>
    </AuthLayout>
  );
}
