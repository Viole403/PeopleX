import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type ReportTable } from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/reports")({
  component: ReportsPage,
});

type Kind =
  | "employees"
  | "headcount"
  | "attendance"
  | "leave"
  | "payroll"
  | "recruitment"
  | "performance"
  | "contracts"
  | "analytics";

const KINDS: { id: Kind; label: string }[] = [
  { id: "employees", label: "Karyawan" },
  { id: "headcount", label: "Headcount" },
  { id: "attendance", label: "Absensi" },
  { id: "leave", label: "Cuti" },
  { id: "payroll", label: "Payroll" },
  { id: "recruitment", label: "Rekrutmen" },
  { id: "performance", label: "Kinerja" },
  { id: "contracts", label: "Kontrak" },
  { id: "analytics", label: "Analitik" },
];

function currentMonth() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

function ReportsPage() {
  const session = useSession();
  const [kind, setKind] = useState<Kind>("employees");
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("");
  const [month, setMonth] = useState(currentMonth());
  const [year, setYear] = useState(String(new Date().getFullYear()));
  const [periodId, setPeriodId] = useState<number | null>(null);
  const [perfId, setPerfId] = useState<number | null>(null);
  const [before, setBefore] = useState("");

  const allowed = can(session.data, "report.view", "system.manage");
  const canExport = can(session.data, "report.export", "system.manage");

  const periods = useQuery({
    queryKey: ["payrollPeriods"],
    queryFn: () => unwrap(commands.payrollPeriods()),
    enabled: allowed && (kind === "payroll"),
  });
  const perfPeriods = useQuery({
    queryKey: ["perfPeriods"],
    queryFn: () => unwrap(commands.performancePeriods()),
    enabled: allowed && kind === "performance",
  });
  const table = useQuery({
    queryKey: ["report", kind, search, status, month, year, periodId, perfId, before],
    queryFn: (): Promise<ReportTable> => {
      switch (kind) {
        case "employees":
          return unwrap(commands.reportEmployees(search || null, null, status || null));
        case "headcount":
          return unwrap(commands.reportHeadcount());
        case "attendance":
          return unwrap(commands.reportAttendance(month, null));
        case "leave":
          return unwrap(commands.reportLeave(Number(year) || 0));
        case "payroll":
          return unwrap(commands.reportPayroll(periodId ?? 0));
        case "recruitment":
          return unwrap(commands.reportRecruitment());
        case "performance":
          return unwrap(commands.reportPerformance(perfId ?? 0));
        case "contracts":
          return unwrap(commands.reportContracts(before || "9999-12-31"));
        case "analytics":
          return unwrap(commands.reportHeadcount());
      }
    },
    enabled: allowed && kind !== "analytics",
    retry: false,
  });
  const analytics = useQuery({
    queryKey: ["reportAnalytics"],
    queryFn: () => unwrap(commands.reportAnalytics()),
    enabled: allowed && kind === "analytics",
    retry: false,
  });

  const downloadCsv = async () => {
    const args: Record<Kind, { arg1: string | null; arg2: number | null }> = {
      employees: { arg1: null, arg2: null },
      headcount: { arg1: null, arg2: null },
      attendance: { arg1: month, arg2: null },
      leave: { arg1: null, arg2: Number(year) || 0 },
      payroll: { arg1: null, arg2: periodId ?? 0 },
      recruitment: { arg1: null, arg2: null },
      performance: { arg1: null, arg2: perfId ?? 0 },
      contracts: { arg1: before || "9999-12-31", arg2: null },
      analytics: { arg1: null, arg2: null },
    };
    try {
      const f = await unwrap(commands.reportExport(kind, args[kind].arg1, args[kind].arg2));
      const blob = new Blob([f.csv], { type: "text/csv;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = f.filename;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  if (!allowed && session.data !== undefined)
    return <p className="text-sm text-text-tertiary">Akses ditolak.</p>;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Laporan</h1>
        {canExport && kind !== "analytics" && (
          <button
            type="button"
            onClick={downloadCsv}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
          >
            Unduh CSV
          </button>
        )}
      </div>
      <div className="flex flex-wrap gap-2">
        {KINDS.map((k) => (
          <button
            key={k.id}
            type="button"
            onClick={() => setKind(k.id)}
            className={`rounded-lg px-4 py-2 text-sm font-medium ${
              kind === k.id
                ? "bg-bg-brand-primary text-text-brand-primary"
                : "border border-border-primary hover:bg-bg-primary_hover"
            }`}
          >
            {k.label}
          </button>
        ))}
      </div>

      {kind === "employees" && (
        <div className="flex flex-wrap gap-2">
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Cari nama / NIP…"
            className="w-full max-w-xs rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
          />
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
          >
            <option value="">Semua status</option>
            <option value="active">Aktif</option>
            <option value="probation">Percobaan</option>
            <option value="resigned">Resign</option>
            <option value="terminated">PHK</option>
          </select>
        </div>
      )}
      {kind === "attendance" && (
        <input
          type="month"
          value={month}
          onChange={(e) => setMonth(e.target.value)}
          className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
        />
      )}
      {kind === "leave" && (
        <input
          type="number"
          value={year}
          onChange={(e) => setYear(e.target.value)}
          className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
        />
      )}
      {kind === "payroll" && (
        <select
          value={periodId ?? ""}
          onChange={(e) => setPeriodId(e.target.value ? Number(e.target.value) : null)}
          className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
        >
          <option value="">Pilih periode…</option>
          {(periods.data ?? []).map((p) => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>
      )}
      {kind === "performance" && (
        <select
          value={perfId ?? ""}
          onChange={(e) => setPerfId(e.target.value ? Number(e.target.value) : null)}
          className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
        >
          <option value="">Pilih periode…</option>
          {(perfPeriods.data ?? []).map((p) => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>
      )}
      {kind === "contracts" && (
        <label className="flex items-center gap-2 text-sm">
          Berakhir s.d.
          <input
            type="date"
            value={before}
            onChange={(e) => setBefore(e.target.value)}
            className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
          />
        </label>
      )}

      {kind === "analytics" ? (
        <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b border-border-secondary text-xs text-text-tertiary">
                {["Departemen", "Karyawan", "Lowongan terbuka", "Rata-rata kinerja"].map((h) => (
                  <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {(analytics.data ?? []).map((d) => (
                <tr key={d.department} className="border-b border-border-tertiary last:border-0">
                  <td className="px-4 py-2.5 font-medium">{d.department}</td>
                  <td className="px-4 py-2.5">{d.employees}</td>
                  <td className="px-4 py-2.5">{d.open_vacancies}</td>
                  <td className="px-4 py-2.5">
                    {d.avg_performance === null ? "-" : d.avg_performance.toFixed(1)}
                  </td>
                </tr>
              ))}
              {(analytics.data ?? []).length === 0 && (
                <tr>
                  <td colSpan={4} className="px-4 py-6 text-center text-sm text-text-tertiary">
                    {analytics.isPending ? "Memuat…" : "Tidak ada data."}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      ) : (
        <ReportGrid table={table.data} loading={table.isPending} error={table.error} />
      )}
    </div>
  );
}

function ReportGrid({
  table,
  loading,
  error,
}: {
  table: ReportTable | undefined;
  loading: boolean;
  error: Error | null;
}) {
  if (loading) return <p className="text-sm text-text-tertiary">Memuat…</p>;
  if (error) return <p className="text-sm text-red-600">{error.message}</p>;
  if (!table) return null;
  return (
    <div className="space-y-2">
      <p className="text-sm text-text-secondary">
        {table.title} · {table.rows.length} baris
      </p>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {table.headers.map((h) => (
                <th key={h} className="whitespace-nowrap px-4 py-2.5 font-medium">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {table.rows.map((row, i) => (
              <tr key={i} className="border-b border-border-tertiary last:border-0">
                {row.map((c, j) => (
                  <td key={j} className="whitespace-nowrap px-4 py-2.5">{c}</td>
                ))}
              </tr>
            ))}
            {table.rows.length === 0 && (
              <tr>
                <td colSpan={table.headers.length} className="px-4 py-6 text-center text-sm text-text-tertiary">
                  Tidak ada data.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
