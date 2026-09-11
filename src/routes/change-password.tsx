import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { unwrap } from "../lib/query";
import { useSession } from "../lib/session";

export const Route = createFileRoute("/change-password")({
  component: ChangePasswordPage,
});

function ChangePasswordPage() {
  const { data: session } = useSession();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");

  const save = useMutation({
    mutationFn: () => {
      if (next !== confirm) throw new Error("Konfirmasi password baru tidak sama.");
      return unwrap(commands.changePassword(current, next));
    },
    onSuccess: () => {
      toast.success("Password berhasil diubah.");
      void queryClient.invalidateQueries({ queryKey: ["session"] });
      void navigate({ to: "/" });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="flex min-h-screen items-center justify-center bg-bg-secondary px-4">
      <form
        className="w-full max-w-sm space-y-4 rounded-2xl border border-border-secondary bg-bg-primary p-8 shadow-lg"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <div>
          <h1 className="text-display-xs font-semibold">Ganti Password</h1>
          <p className="mt-1 text-sm text-text-tertiary">
            {session?.must_change_password
              ? "Akun Anda wajib ganti password sebelum lanjut."
              : `Masuk sebagai ${session?.username ?? ""}.`}
          </p>
        </div>
        {(
          [
            ["Password saat ini", current, setCurrent, "current-password"],
            ["Password baru (min. 8 karakter)", next, setNext, "new-password"],
            ["Ulangi password baru", confirm, setConfirm, "new-password"],
          ] as const
        ).map(([label, value, set, auto]) => (
          <label key={label} className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">{label}</span>
            <input
              type="password"
              value={value}
              onChange={(e) => set(e.target.value)}
              autoComplete={auto}
              required
              minLength={8}
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none placeholder:text-text-placeholder focus:border-border-brand"
            />
          </label>
        ))}
        <button
          type="submit"
          disabled={save.isPending}
          className="w-full rounded-lg bg-bg-brand-solid px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-bg-brand-solid_hover disabled:opacity-60"
        >
          {save.isPending ? "Menyimpan…" : "Simpan password"}
        </button>
      </form>
    </div>
  );
}
