import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useSearchParams } from "react-router";
import { useApi } from "../../api.ts";
import { ME, safeNext } from "../../auth.tsx";
import { Button, FormError, TextField, fieldError } from "../../components/ui.tsx";
import { AuthLayout } from "./AuthLayout.tsx";

const MIN_PASSWORD = 12;

export function RegisterPage() {
  const api = useApi();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const next = safeNext(params.get("next"));
  const [displayName, setDisplayName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const mismatch = confirm.length > 0 && confirm !== password;
  const tooShort = password.length > 0 && [...password].length < MIN_PASSWORD;
  const register = useMutation({
    mutationFn: () => api.register({ email, password, display_name: displayName }),
    onSuccess: (s) => {
      qc.clear();
      qc.setQueryData(ME, s.user);
      navigate(next, { replace: true });
    },
  });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!mismatch && !tooShort) register.mutate();
  };
  const fieldErr = (f: string) => fieldError(register.error, f);
  const named = ["email", "password", "display_name"].some((f) => fieldErr(f));
  return (
    <AuthLayout title="Create an account" footer={<>Have an account? <Link to="/login" className="text-accent underline">Sign in</Link></>}>
      <form onSubmit={submit} className="flex flex-col gap-4" noValidate>
        <TextField label="Your name" autoComplete="name" required value={displayName} onChange={(e) => setDisplayName(e.target.value)} error={fieldErr("display_name")} />
        <TextField label="Email" type="email" autoComplete="email" required value={email} onChange={(e) => setEmail(e.target.value)} error={fieldErr("email")} />
        <TextField
          label="Password"
          type="password"
          autoComplete="new-password"
          required
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          error={tooShort ? `Use at least ${MIN_PASSWORD} characters.` : fieldErr("password")}
          hint={`At least ${MIN_PASSWORD} characters. A phrase of a few words works well.`}
        />
        <TextField label="Repeat password" type="password" autoComplete="new-password" required value={confirm} onChange={(e) => setConfirm(e.target.value)} error={mismatch ? "The passwords don't match." : undefined} />
        {register.error && !named && <FormError error={register.error} />}
        <Button type="submit" busy={register.isPending} disabled={!displayName || !email || !password || !confirm || mismatch || tooShort}>
          {register.isPending ? "Creating…" : "Create account"}
        </Button>
      </form>
    </AuthLayout>
  );
}
