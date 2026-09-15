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
  | "pph21annual"
  | "analytics"
  | "builder";

const KINDS: { id: Kind; label: string }[] = [
  { id: "employees", label: "Karyawan" },
  { id: "headcount", label: "Headcount" },
  { id: "attendance", label: "Absensi" },
  { id: "leave", label: "Cuti" },
  { id: "payroll", label: "Payroll" },
  { id: "recruitment", label: "Rekrutmen" },
  { id: "performance", label: "Kinerja" },
  { id: "contracts", label: "Kontrak" },
  { id: "pph21annual", label: "PPh 21 Tahunan" },
  { id: "analytics", label: "Analitik" },
  { id: "builder", label: "Builder" },
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
  const [bFields, setBFields] = useState<string[]>(["nip", "nama", "status"]);
  const [bFilters, setBFilters] = useState<{ field: string; op: string; value: string }[]>([]);

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
    queryKey: ["report", kind, search, status, month, year, periodId, perfId, before, bFields, bFilters],
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
        case "pph21annual":
          return unwrap(commands.reportPph21Annual(Number(year) || 0));
        case "analytics":
          return unwrap(commands.reportHeadcount());
        case "builder":
          return unwrap(
            commands.reportCustom(
              bFields,
              bFilters.map((f) => ({ field: f.field, op: f.op, value: f.value })),
            ),
          );
      }
    },
    enabled: allowed && kind !== "analytics",
    retry: false,
  });
  const fieldList = useQuery({
    queryKey: ["reportCustomFields"],
    queryFn: () => unwrap(commands.reportCustomFields()),
    enabled: allowed && kind === "builder",
    retry: false,
  });
  const analytics = useQuery({
    queryKey: ["reportAnalytics"],
    queryFn: () => unwrap(commands.reportAnalytics()),
    enabled: allowed && kind === "analytics",
    retry: false,
  });

  const download = async (format: "csv" | "xlsx" | "pdf") => {
    const args: Record<Kind, { arg1: string | null; arg2: number | null }> = {
      employees: { arg1: null, arg2: null },
      headcount: { arg1: null, arg2: null },
      attendance: { arg1: month, arg2: null },
      leave: { arg1: null, arg2: Number(year) || 0 },
      payroll: { arg1: null, arg2: periodId ?? 0 },
      recruitment: { arg1: null, arg2: null },
      performance: { arg1: null, arg2: perfId ?? 0 },
      contracts: { arg1: before || "9999-12-31", arg2: null },
      pph21annual: { arg1: null, arg2: Number(year) || 0 },
      analytics: { arg1: null, arg2: null },
      builder: { arg1: null, arg2: null },
    };
    try {
      const f = await unwrap(commands.reportExport(kind, format, args[kind].arg1, args[kind].arg2));
      const blob = new Blob([new Uint8Array(f.bytes)], { type: f.mime });
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
        {canExport && kind !== "analytics" && kind !== "builder" && (
          <div className="flex gap-2">
            {(
              [
                ["csv", "Unduh CSV"],
                ["xlsx", "Unduh XLSX"],
                ["pdf", "Unduh PDF"],
              ] as const
            ).map(([format, label]) => (
              <button
                key={format}
                type="button"
                onClick={() => download(format)}
                className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
              >
                {label}
              </button>
            ))}
          </div>
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
      {kind === "builder" && (
        <div className="space-y-3 rounded-xl border border-border-secondary bg-bg-primary p-4">
          <div>
            <p className="mb-2 text-sm font-medium">Field tampil</p>
            <div className="flex flex-wrap gap-2">
              {(fieldList.data ?? []).map((f) => (
                <label key={f.id} className="flex items-center gap-1.5 rounded-lg border border-border-primary px-3 py-1.5 text-sm">
                  <input
                    type="checkbox"
                    checked={bFields.includes(f.id)}
                    onChange={(e) =>
                      setBFields((prev) =>
                        e.target.checked ? [...prev, f.id] : prev.filter((x) => x !== f.id),
                      )
                    }
                  />
                  {f.label}
                </label>
              ))}
            </div>
          </div>
          <div>
            <p className="mb-2 text-sm font-medium">Filter (semua harus cocok)</p>
            <div className="space-y-2">
              {bFilters.map((fl, i) => (
                <div key={i} className="flex flex-wrap items-center gap-2">
                  <select
                    value={fl.field}
                    onChange={(e) =>
                      setBFilters((prev) => prev.map((x, j) => (j === i ? { ...x, field: e.target.value } : x)))
                    }
                    className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
                  >
                    {(fieldList.data ?? []).map((f) => (
                      <option key={f.id} value={f.id}>{f.label}</option>
                    ))}
                  </select>
                  <select
                    value={fl.op}
                    onChange={(e) =>
                      setBFilters((prev) => prev.map((x, j) => (j === i ? { ...x, op: e.target.value } : x)))
                    }
                    className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
                  >
                    <option value="like">mengandung</option>
                    <option value="eq">sama dengan</option>
                    <option value="gte">≥</option>
                    <option value="lte">≤</option>
                  </select>
                  <input
                    value={fl.value}
                    onChange={(e) =>
                      setBFilters((prev) => prev.map((x, j) => (j === i ? { ...x, value: e.target.value } : x)))
                    }
                    placeholder="Nilai…"
                    className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
                  />
                  <button
                    type="button"
                    onClick={() => setBFilters((prev) => prev.filter((_, j) => j !== i))}
                    className="rounded-lg border border-border-primary px-3 py-2 text-sm hover:bg-bg-primary_hover"
                  >
                    Hapus
                  </button>
                </div>
              ))}
              <button
                type="button"
                onClick={() => setBFilters((prev) => [...prev, { field: "nama", op: "like", value: "" }])}
                className="rounded-lg border border-border-primary px-3 py-2 text-sm hover:bg-bg-primary_hover"
              >
                + Tambah filter
              </button>
            </div>
          </div>
        </div>
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
