import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { commands, type HrDashboard, type MySummary } from "../bindings";
import { formatDate } from "../lib/format";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/")({
  component: Dashboard,
});

function Stat({ label, value, accent }: { label: string; value: number; accent?: boolean }) {
  return (
    <div className="rounded-xl border border-border-secondary bg-bg-primary p-4">
      <p className="text-xs text-text-tertiary">{label}</p>
      <p className={`mt-1 text-2xl font-semibold ${accent ? "text-text-brand-secondary" : ""}`}>
        {value}
      </p>
    </div>
  );
}

function Section({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="rounded-xl border border-border-secondary bg-bg-primary p-4">
      <div className="mb-3 flex items-center justify-between">
        <h2 className="text-sm font-semibold">{title}</h2>
        {action}
      </div>
      {children}
    </div>
  );
}

function Dashboard() {
  const session = useSession();
  const isHr = can(session.data, "employee.view", "payroll.view", "system.manage");
  const hr = useQuery({
    queryKey: ["dashboardHr"],
    queryFn: () => unwrap(commands.dashboardHr()),
    enabled: isHr,
  });
  const me = useQuery({
    queryKey: ["dashboardMe"],
    queryFn: () => unwrap(commands.dashboardMe()),
    enabled: session.data !== undefined && !isHr,
    retry: false,
  });

  if (session.isPending) return <p className="text-sm text-text-tertiary">Memuat…</p>;

  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Dasbor</h1>
      {isHr ? (
        <HrView data={hr.data} loading={hr.isPending} />
      ) : (
        <MeView data={me.data} loading={me.isPending} error={me.error} />
      )}
    </div>
  );
}

function ExpiringContracts() {
  const session = useSession();
  const allowed = can(session.data, "contract.view", "system.manage");
  const list = useQuery({
    queryKey: ["contractsExpiring", 30],
    queryFn: () => unwrap(commands.contractExpiring(30)),
    enabled: allowed,
  });
  if (!allowed) return null;
  if (list.isPending)
    return <p className="text-sm text-text-tertiary">Memuat kontrak jatuh tempo…</p>;
  const rows = list.data ?? [];
  const urgent = rows.filter((r) => r.days_left <= 7);
  return (
    <Section
      title="Kontrak jatuh tempo ≤ 30 hari"
      action={
        urgent.length > 0 ? (
          <span className="text-xs font-semibold text-red-600">
            {urgent.length} mendesak ≤ 7 hari
          </span>
        ) : undefined
      }
    >
      {rows.length === 0 ? (
        <p className="text-sm text-text-tertiary">Tidak ada.</p>
      ) : (
        <ul className="space-y-1.5 text-sm">
          {rows.map((r) => (
            <li key={r.id} className="flex items-center justify-between gap-2">
              <span>
                {r.employee_name} • {r.contract_number} • berakhir {formatDate(r.end_date)}
              </span>
              <span className="shrink-0 font-medium">H-{r.days_left}</span>
            </li>
          ))}
        </ul>
      )}
    </Section>
  );
}

function HrView({ data, loading }: { data: HrDashboard | undefined; loading: boolean }) {
  if (loading) return <p className="text-sm text-text-tertiary">Memuat…</p>;
  if (!data) return <p className="text-sm text-text-tertiary">Data tidak tersedia.</p>;
  const queue = data.pending_leave + data.pending_overtime + data.pending_corrections + data.pending_trips;
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <Stat label="Total karyawan aktif" value={data.total_employees} accent />
        <Stat label="Masuk bulan ini" value={data.new_hires_this_month} />
        <Stat label="Keluar bulan ini" value={data.resigned_this_month} />
        <Stat label="Hadir hari ini" value={data.present_today} />
        <Stat label="Terlambat hari ini" value={data.late_today} />
        <Stat label="Antrean persetujuan" value={queue} accent />
        <Stat label="Kontrak aktif" value={data.active_contracts} />
        <Stat label="Kontrak berakhir ≤ 60 hari" value={data.expiring_contracts} />
      </div>
      <ExpiringContracts />
      <div className="grid gap-4 md:grid-cols-2">
        <Section title="Headcount per departemen">
          <table className="w-full text-left text-sm">
            <tbody>
              {data.headcount_by_department.map((r) => (
                <tr key={r.name} className="border-b border-border-tertiary last:border-0">
                  <td className="py-1.5">{r.name}</td>
                  <td className="py-1.5 text-right font-medium">{r.count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
        <Section
          title="Pengumuman terbaru"
          action={
            <Link to="/announcements" className="text-xs font-medium text-text-brand-secondary hover:underline">
              Semua
            </Link>
          }
        >
          {data.recent_announcements.length === 0 ? (
            <p className="text-sm text-text-tertiary">Belum ada pengumuman.</p>
          ) : (
            <ul className="space-y-1.5 text-sm">
              {data.recent_announcements.map((a) => (
                <li key={a.id}>
                  <Link to="/announcements" className="hover:underline">
                    {a.title}
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Section>
        <Section title="Ulang tahun hari ini">
          {data.birthdays_today.length === 0 ? (
            <p className="text-sm text-text-tertiary">Tidak ada.</p>
          ) : (
            <ul className="space-y-1.5 text-sm">
              {data.birthdays_today.map((b) => (
                <li key={b.employee_number}>
                  {b.name} <span className="text-text-tertiary">({b.employee_number})</span>
                </li>
              ))}
            </ul>
          )}
        </Section>
        <Section title="Libur terdekat">
          {data.upcoming_holidays.length === 0 ? (
            <p className="text-sm text-text-tertiary">Tidak ada.</p>
          ) : (
            <ul className="space-y-1.5 text-sm">
              {data.upcoming_holidays.map((h) => (
                <li key={`${h.date}-${h.name}`} className="flex justify-between gap-2">
                  <span>{h.name}</span>
                  <span className="text-text-tertiary">{h.date}</span>
                </li>
              ))}
            </ul>
          )}
        </Section>
      </div>
    </div>
  );
}

function MeView({
  data,
  loading,
  error,
}: {
  data: MySummary | undefined;
  loading: boolean;
  error: Error | null;
}) {
  if (loading) return <p className="text-sm text-text-tertiary">Memuat…</p>;
  if (!data)
    return (
      <p className="text-sm text-text-tertiary">
        {error ? error.message : "Ringkasan tidak tersedia."}
      </p>
    );
  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-4">
        <p className="text-lg font-semibold">{data.employee_name}</p>
        <p className="mt-0.5 text-sm text-text-tertiary">
          {[data.employee_number, data.position, data.department].filter(Boolean).join(" · ")}
        </p>
        <p className="mt-2 text-sm">
          Status hari ini:{" "}
          <span className="font-medium">{data.today_status ?? "belum absen"}</span>
          {data.today_clock_in ? ` · masuk ${data.today_clock_in}` : ""}
        </p>
      </div>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <Stat label="Cuti menunggu" value={data.pending_leave} />
        <Stat label="Lembur menunggu" value={data.pending_overtime} />
        <Stat label="Koreksi menunggu" value={data.pending_corrections} />
        <Stat label="Aset dipinjam" value={data.my_assets} />
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        <Section title="Saldo cuti tahun berjalan">
          {data.leave_balances.length === 0 ? (
            <p className="text-sm text-text-tertiary">Belum ada saldo.</p>
          ) : (
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-border-secondary text-xs text-text-tertiary">
                  {["Jenis", "Alokasi", "Terpakai", "Sisa"].map((h) => (
                    <th key={h} className="py-2 font-medium">{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {data.leave_balances.map((b) => (
                  <tr key={b.leave_type} className="border-b border-border-tertiary last:border-0">
                    <td className="py-1.5">{b.leave_type}</td>
                    <td className="py-1.5">{b.allocated}</td>
                    <td className="py-1.5">{b.used}</td>
                    <td className="py-1.5 font-medium">{b.remaining}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Section>
        <Section
          title="Pengumuman belum dibaca"
          action={
            <Link to="/announcements" className="text-xs font-medium text-text-brand-secondary hover:underline">
              Buka
            </Link>
          }
        >
          <p className="text-2xl font-semibold">{data.unread_announcements}</p>
        </Section>
      </div>
    </div>
  );
}
