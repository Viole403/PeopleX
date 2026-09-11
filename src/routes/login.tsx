import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useLogin, useSession } from "../lib/session";

export const Route = createFileRoute("/login")({
  component: LoginPage,
});

function LoginPage() {
  const { data: session, isPending } = useSession();
  const login = useLogin();
  const navigate = useNavigate();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  useEffect(() => {
    if (!isPending && session) {
      void navigate({
        to: session.must_change_password ? "/change-password" : "/",
      });
    }
  }, [isPending, session, navigate]);

  return (
    <div className="flex min-h-screen items-center justify-center bg-bg-secondary px-4">
      <form
        className="w-full max-w-sm space-y-4 rounded-2xl border border-border-secondary bg-bg-primary p-8 shadow-lg"
        onSubmit={(e) => {
          e.preventDefault();
          login.mutate({ username, password });
        }}
      >
        <div className="flex items-center gap-3">
          <div className="flex size-10 items-center justify-center rounded-xl bg-bg-brand-solid text-base font-bold text-text-primary_on-brand">
            PX
          </div>
          <div>
            <h1 className="text-display-xs font-semibold">PeopleX</h1>
            <p className="text-sm text-text-tertiary">Sistem Informasi Kepegawaian</p>
          </div>
        </div>
        <div className="space-y-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">
              Username atau email
            </span>
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="username"
              autoFocus
              required
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none placeholder:text-text-placeholder focus:border-border-brand"
              placeholder="admin"
            />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Password</span>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              required
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none placeholder:text-text-placeholder focus:border-border-brand"
              placeholder="••••••••"
            />
          </label>
        </div>
        <button
          type="submit"
          disabled={login.isPending}
          className="w-full rounded-lg bg-bg-brand-solid px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-bg-brand-solid_hover disabled:opacity-60"
        >
          {login.isPending ? "Memeriksa…" : "Masuk"}
        </button>
        <p className="text-center text-xs text-text-tertiary">
          Lupa password? Minta token reset ke admin HRD.
        </p>
      </form>
    </div>
  );
}
