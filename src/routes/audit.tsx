import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import { commands } from "../bindings";
import { formatDateTime } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/audit")({
  component: AuditPage,
});

function AuditPage() {
  const [module, setModule] = useState("");
  const entries = useQuery({
    queryKey: ["audit", module],
    queryFn: () => unwrap(commands.auditList(module || null, 200)),
  });

  const modules = useMemo(() => {
    const set = new Set<string>();
    for (const e of entries.data ?? []) set.add(e.module);
    return [...set].sort();
  }, [entries.data]);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Audit Log</h1>
        <select
          value={module}
          onChange={(e) => setModule(e.target.value)}
          className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
        >
          <option value="">Semua modul</option>
          {modules.map((m) => (
            <option key={m} value={m}>{m}</option>
          ))}
        </select>
      </div>
      {entries.isPending ? (
        <p className="text-sm text-text-tertiary">Memuat…</p>
      ) : entries.isError ? (
        <p className="text-sm text-text-error-primary">Gagal memuat audit log.</p>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b border-border-secondary text-xs text-text-tertiary">
                {["Waktu", "Aksi", "Modul", "Record", "Keterangan"].map((h) => (
                  <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {entries.data.map((e) => (
                <tr key={e.id} className="border-b border-border-tertiary last:border-0">
                  <td className="whitespace-nowrap px-4 py-2.5 text-text-secondary">
                    {formatDateTime(e.created_at)}
                  </td>
                  <td className="px-4 py-2.5 font-medium">{e.action}</td>
                  <td className="px-4 py-2.5 text-text-secondary">{e.module}</td>
                  <td className="px-4 py-2.5 text-text-secondary">{e.record_id ?? "-"}</td>
                  <td className="px-4 py-2.5 text-text-secondary">{e.description ?? "-"}</td>
                </tr>
              ))}
              {entries.data.length === 0 && (
                <tr>
                  <td colSpan={5} className="px-4 py-6 text-center text-sm text-text-tertiary">
                    Belum ada entri audit.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
