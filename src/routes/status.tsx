import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { commands } from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/status")({
  component: StatusPage,
});

function StatusPage() {
  const { data, isPending, isError, error, refetch } = useQuery({
    queryKey: ["dbStatus"],
    queryFn: () => unwrap(commands.dbStatus()),
  });

  if (isPending) {
    return <p className="text-sm text-text-tertiary">Memuat status basis data…</p>;
  }

  if (isError) {
    return (
      <div className="space-y-3 rounded-xl border border-border-error bg-bg-error-primary p-6">
        <h1 className="text-display-sm font-semibold text-text-error-primary">
          Basis data tidak dapat dibaca
        </h1>
        <p className="text-sm text-text-secondary">
          {error instanceof Error ? error.message : "Kesalahan tidak dikenal"}
        </p>
        <button
          type="button"
          onClick={() => {
            toast.info("Memuat ulang status…");
            void refetch();
          }}
          className="rounded-lg bg-bg-error-solid px-4 py-2 text-sm font-medium text-white transition hover:opacity-90"
        >
          Coba lagi
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Status Basis Data</h1>
      <dl className="grid gap-3 sm:grid-cols-3">
        {[
          ["Status", data.ok ? "Sehat" : "Bermasalah"],
          ["Jumlah tabel", String(data.tables)],
          ["Jumlah pengguna", String(data.users)],
        ].map(([label, value]) => (
          <div
            key={label}
            className="rounded-xl border border-border-secondary bg-bg-primary p-4"
          >
            <dt className="text-xs text-text-tertiary">{label}</dt>
            <dd className="mt-1 text-display-xs font-semibold">{value}</dd>
          </div>
        ))}
      </dl>
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-4">
        <p className="text-xs text-text-tertiary">Direktori data</p>
        <p className="mt-1 break-all font-mono text-sm">{data.data_dir}</p>
      </div>
      <BackupSection />
    </div>
  );
}

function BackupSection() {
  const queryClient = useQueryClient();
  const session = useSession();
  const allowed = can(session.data, "backup.manage", "system.manage");
  const list = useQuery({
    queryKey: ["backups"],
    queryFn: () => unwrap(commands.backupList()),
    enabled: allowed,
  });
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["backups"] });
  const now = useMutation({
    mutationFn: () => unwrap(commands.backupNow()),
    onSuccess: (name) => {
      toast.success(`Cadangan ${name} dibuat.`);
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const restore = useMutation({
    mutationFn: (name: string) => unwrap(commands.backupRestore(name)),
    onSuccess: () => {
      toast.success("Database dipulihkan dari cadangan.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  if (!allowed) return null;
  return (
    <div className="space-y-3 rounded-xl border border-border-secondary bg-bg-primary p-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold">Cadangan database</h2>
        <button
          type="button"
          onClick={() => now.mutate()}
          disabled={now.isPending}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
        >
          {now.isPending ? "Membuat…" : "Buat cadangan"}
        </button>
      </div>
      {list.isPending ? (
        <p className="text-sm text-text-tertiary">Memuat daftar…</p>
      ) : (list.data ?? []).length === 0 ? (
        <p className="text-sm text-text-tertiary">Belum ada berkas cadangan.</p>
      ) : (
        <ul className="space-y-1.5 text-sm">
          {(list.data ?? []).map((b) => (
            <li key={b.name} className="flex items-center justify-between gap-2">
              <span className="break-all font-mono text-xs">{b.name}</span>
              <button
                type="button"
                onClick={() => {
                  if (window.confirm(`Pulihkan database dari ${b.name}? Data saat ini akan diganti.`)) {
                    restore.mutate(b.name);
                  }
                }}
                className="shrink-0 rounded-lg border border-border-primary px-3 py-1.5 text-xs font-medium"
              >
                Pulihkan
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
