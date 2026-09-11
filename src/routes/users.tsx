import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type Role } from "../bindings";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/users")({
  component: UsersPage,
});

function Badge({ children, tone }: { children: React.ReactNode; tone: string }) {
  return (
    <span className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${tone}`}>
      {children}
    </span>
  );
}

function UsersPage() {
  const queryClient = useQueryClient();
  const users = useQuery({ queryKey: ["users"], queryFn: () => unwrap(commands.listUsers()) });
  const roles = useQuery({ queryKey: ["roles"], queryFn: () => unwrap(commands.listRoles()) });
  const [roleUserId, setRoleUserId] = useState<number | null>(null);
  const [resetUserId, setResetUserId] = useState<number | null>(null);

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["users"] });
    void queryClient.invalidateQueries({ queryKey: ["session"] });
  };

  const toggle = useMutation({
    mutationFn: (v: { id: number; status: string }) =>
      unwrap(commands.toggleUserStatus(v.id, v.status)),
    onSuccess: () => {
      toast.success("Status pengguna diperbarui.");
      invalidate();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const reset = useMutation({
    mutationFn: (v: { id: number; password: string }) =>
      unwrap(commands.adminResetPassword(v.id, v.password)),
    onSuccess: () => {
      toast.success("Password direset. User wajib ganti saat login berikut.");
      setResetUserId(null);
      invalidate();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Pengguna</h1>
      {users.isPending ? (
        <p className="text-sm text-text-tertiary">Memuat…</p>
      ) : users.isError ? (
        <p className="text-sm text-text-error-primary">Gagal memuat pengguna.</p>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b border-border-secondary text-xs text-text-tertiary">
                {["Username", "Email", "Karyawan", "Peran", "Status", "Aksi"].map((h) => (
                  <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {users.data.map((u) => (
                <tr key={u.id} className="border-b border-border-tertiary last:border-0">
                  <td className="px-4 py-2.5 font-medium">{u.username}</td>
                  <td className="px-4 py-2.5 text-text-secondary">{u.email}</td>
                  <td className="px-4 py-2.5 text-text-secondary">{u.employee_name ?? "-"}</td>
                  <td className="px-4 py-2.5">
                    <span className="flex flex-wrap gap-1">
                      {u.roles.length === 0 ? (
                        <span className="text-text-tertiary">-</span>
                      ) : (
                        u.roles.map((r) => (
                          <Badge key={r} tone="bg-bg-brand-primary text-text-brand-primary">
                            {r}
                          </Badge>
                        ))
                      )}
                    </span>
                  </td>
                  <td className="px-4 py-2.5">
                    <span className="flex flex-wrap items-center gap-1">
                      <Badge
                        tone={
                          u.status === "active"
                            ? "bg-bg-success-primary text-text-success-primary"
                            : "bg-bg-error-primary text-text-error-primary"
                        }
                      >
                        {u.status === "active" ? "Aktif" : "Nonaktif"}
                      </Badge>
                      {u.must_change_password && (
                        <Badge tone="bg-bg-warning-primary text-text-warning-primary">
                          Wajib ganti password
                        </Badge>
                      )}
                    </span>
                  </td>
                  <td className="px-4 py-2.5">
                    <span className="flex flex-wrap gap-2 text-[13px]">
                      <button
                        type="button"
                        onClick={() => setRoleUserId(u.id)}
                        className="font-medium text-text-brand-secondary hover:underline"
                      >
                        Peran
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          toggle.mutate({
                            id: u.id,
                            status: u.status === "active" ? "inactive" : "active",
                          })
                        }
                        className="font-medium text-text-brand-secondary hover:underline"
                      >
                        {u.status === "active" ? "Nonaktifkan" : "Aktifkan"}
                      </button>
                      <button
                        type="button"
                        onClick={() => setResetUserId(u.id)}
                        className="font-medium text-text-brand-secondary hover:underline"
                      >
                        Reset password
                      </button>
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {roleUserId !== null && (
        <RoleDialog
          userId={roleUserId}
          roles={roles.data ?? []}
          onClose={() => {
            setRoleUserId(null);
            invalidate();
          }}
        />
      )}
      {resetUserId !== null && (
        <ResetDialog
          onClose={() => setResetUserId(null)}
          onSave={(password) => reset.mutate({ id: resetUserId, password })}
          pending={reset.isPending}
        />
      )}
    </div>
  );
}

function RoleDialog({
  userId,
  roles,
  onClose,
}: {
  userId: number;
  roles: Role[];
  onClose: () => void;
}) {
  const current = useQuery({
    queryKey: ["userRoles", userId],
    queryFn: () => unwrap(commands.userRoleIds(userId)),
  });
  const [checked, setChecked] = useState<Set<number> | null>(null);
  const ids = checked ?? new Set(current.data ?? []);

  const save = useMutation({
    mutationFn: () => unwrap(commands.syncUserRoles(userId, [...ids])),
    onSuccess: () => {
      toast.success("Peran pengguna disimpan.");
      onClose();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <div className="w-full max-w-md rounded-2xl bg-bg-primary p-6 shadow-2xl">
        <h2 className="text-display-xs font-semibold">Kelola peran</h2>
        {current.isPending ? (
          <p className="mt-4 text-sm text-text-tertiary">Memuat…</p>
        ) : (
          <div className="mt-4 max-h-72 space-y-2 overflow-y-auto">
            {roles.map((r) => (
              <label key={r.id} className="flex items-center gap-3 rounded-lg border border-border-secondary px-3 py-2 text-sm">
                <input
                  type="checkbox"
                  checked={ids.has(r.id)}
                  onChange={() => {
                    const next = new Set(ids);
                    if (next.has(r.id)) next.delete(r.id);
                    else next.add(r.id);
                    setChecked(next);
                  }}
                  className="size-4 accent-brand-600"
                />
                <span className="font-medium">{r.name}</span>
                <span className="text-xs text-text-tertiary">{r.slug}</span>
              </label>
            ))}
          </div>
        )}
        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium"
          >
            Batal
          </button>
          <button
            type="button"
            disabled={save.isPending}
            onClick={() => save.mutate()}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
          >
            Simpan
          </button>
        </div>
      </div>
    </div>
  );
}

function ResetDialog({
  onClose,
  onSave,
  pending,
}: {
  onClose: () => void;
  onSave: (password: string) => void;
  pending: boolean;
}) {
  const [password, setPassword] = useState("");
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <form
        className="w-full max-w-sm space-y-4 rounded-2xl bg-bg-primary p-6 shadow-2xl"
        onSubmit={(e) => {
          e.preventDefault();
          onSave(password);
        }}
      >
        <h2 className="text-display-xs font-semibold">Reset password</h2>
        <p className="text-sm text-text-tertiary">
          User wajib mengganti password ini saat login berikut.
        </p>
        <input
          type="text"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          required
          minLength={8}
          placeholder="Password sementara (min. 8 karakter)"
          className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
        />
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium"
          >
            Batal
          </button>
          <button
            type="submit"
            disabled={pending}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
          >
            Reset
          </button>
        </div>
      </form>
    </div>
  );
}
