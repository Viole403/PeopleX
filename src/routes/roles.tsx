import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { commands, type Permission, type Role } from "../bindings";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/roles")({
  component: RolesPage,
});

function RolesPage() {
  const queryClient = useQueryClient();
  const roles = useQuery({ queryKey: ["roles"], queryFn: () => unwrap(commands.listRoles()) });
  const [editing, setEditing] = useState<Role | "new" | null>(null);
  const [permRole, setPermRole] = useState<Role | null>(null);

  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["roles"] });

  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.deleteRole(id)),
    onSuccess: () => {
      toast.success("Peran dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-display-sm font-semibold">Peran</h1>
        <button
          type="button"
          onClick={() => setEditing("new")}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white transition hover:bg-bg-brand-solid_hover"
        >
          Tambah peran
        </button>
      </div>
      {roles.isPending ? (
        <p className="text-sm text-text-tertiary">Memuat…</p>
      ) : roles.isError ? (
        <p className="text-sm text-text-error-primary">Gagal memuat peran.</p>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          {roles.data.map((r) => (
            <div
              key={r.id}
              className="rounded-xl border border-border-secondary bg-bg-primary p-4"
            >
              <div className="flex items-start justify-between gap-2">
                <div>
                  <p className="font-semibold">{r.name}</p>
                  <p className="text-xs text-text-tertiary">
                    {r.slug}
                    {r.is_system ? " • sistem" : ""} • {r.user_count} pengguna
                  </p>
                  {r.description && (
                    <p className="mt-1 text-sm text-text-secondary">{r.description}</p>
                  )}
                </div>
              </div>
              <div className="mt-3 flex flex-wrap gap-3 text-[13px]">
                <button
                  type="button"
                  onClick={() => setPermRole(r)}
                  className="font-medium text-text-brand-secondary hover:underline"
                >
                  Izin
                </button>
                {!r.is_system && (
                  <>
                    <button
                      type="button"
                      onClick={() => setEditing(r)}
                      className="font-medium text-text-brand-secondary hover:underline"
                    >
                      Ubah
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        if (window.confirm(`Hapus peran ${r.name}?`)) remove.mutate(r.id);
                      }}
                      className="font-medium text-text-error-primary hover:underline"
                    >
                      Hapus
                    </button>
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
      {editing && (
        <RoleForm
          initial={editing === "new" ? null : editing}
          onClose={() => {
            setEditing(null);
            refresh();
          }}
        />
      )}
      {permRole && (
        <PermissionDialog role={permRole} onClose={() => setPermRole(null)} />
      )}
    </div>
  );
}

function RoleForm({ initial, onClose }: { initial: Role | null; onClose: () => void }) {
  const [slug, setSlug] = useState(initial?.slug ?? "");
  const [name, setName] = useState(initial?.name ?? "");
  const [description, setDescription] = useState(initial?.description ?? "");

  const save = useMutation({
    mutationFn: () =>
      initial
        ? unwrap(commands.updateRole(initial.id, name, description || null))
        : unwrap(commands.createRole(slug, name, description || null)),
    onSuccess: () => {
      toast.success("Peran disimpan.");
      onClose();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <form
        className="w-full max-w-md space-y-4 rounded-2xl bg-bg-primary p-6 shadow-2xl"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <h2 className="text-display-xs font-semibold">
          {initial ? "Ubah peran" : "Tambah peran"}
        </h2>
        {!initial && (
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">
              Slug (huruf kecil, angka, strip)
            </span>
            <input
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
              required
              pattern="[a-z0-9-]+"
              className="w-full rounded-lg border border-border-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
            />
          </label>
        )}
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Nama</span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            className="w-full rounded-lg border border-border-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
          />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Deskripsi</span>
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={2}
            className="w-full rounded-lg border border-border-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
          />
        </label>
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
            disabled={save.isPending}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
          >
            Simpan
          </button>
        </div>
      </form>
    </div>
  );
}

function PermissionDialog({ role, onClose }: { role: Role; onClose: () => void }) {
  const perms = useQuery({
    queryKey: ["permissions"],
    queryFn: () => unwrap(commands.listPermissions()),
  });
  const current = useQuery({
    queryKey: ["rolePermissions", role.id],
    queryFn: () => unwrap(commands.rolePermissionIds(role.id)),
  });
  const [checked, setChecked] = useState<Set<number> | null>(null);
  const ids = checked ?? new Set(current.data ?? []);

  const grouped = useMemo(() => {
    const map = new Map<string, Permission[]>();
    for (const p of perms.data ?? []) {
      const list = map.get(p.module) ?? [];
      list.push(p);
      map.set(p.module, list);
    }
    return [...map.entries()];
  }, [perms.data]);

  const save = useMutation({
    mutationFn: () => unwrap(commands.syncRolePermissions(role.id, [...ids])),
    onSuccess: () => {
      toast.success("Izin peran disimpan.");
      onClose();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <div className="flex max-h-[85vh] w-full max-w-lg flex-col rounded-2xl bg-bg-primary p-6 shadow-2xl">
        <h2 className="text-display-xs font-semibold">Izin: {role.name}</h2>
        {role.is_system ? (
          <p className="mt-3 text-sm text-text-tertiary">
            Peran sistem memiliki seluruh izin dan tidak dapat diubah.
          </p>
        ) : perms.isPending || current.isPending ? (
          <p className="mt-4 text-sm text-text-tertiary">Memuat…</p>
        ) : (
          <div className="mt-4 flex-1 space-y-4 overflow-y-auto">
            {grouped.map(([module, list]) => (
              <div key={module}>
                <p className="mb-1 text-xs font-semibold uppercase text-text-tertiary">
                  {module}
                </p>
                <div className="space-y-1">
                  {list.map((p) => (
                    <label
                      key={p.id}
                      className="flex items-center gap-3 rounded-lg border border-border-secondary px-3 py-1.5 text-sm"
                    >
                      <input
                        type="checkbox"
                        checked={ids.has(p.id)}
                        onChange={() => {
                          const next = new Set(ids);
                          if (next.has(p.id)) next.delete(p.id);
                          else next.add(p.id);
                          setChecked(next);
                        }}
                        className="size-4 accent-brand-600"
                      />
                      {p.name}
                    </label>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium"
          >
            {role.is_system ? "Tutup" : "Batal"}
          </button>
          {!role.is_system && (
            <button
              type="button"
              disabled={save.isPending}
              onClick={() => save.mutate()}
              className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
            >
              Simpan
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
