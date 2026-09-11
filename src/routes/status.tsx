import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { commands } from "../bindings";
import { unwrap } from "../lib/query";

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
    </div>
  );
}
