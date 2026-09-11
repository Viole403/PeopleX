import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { formatDate, formatDateLong } from "../lib/format";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/attendance")({
  component: AttendancePage,
});

function todayId(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function monthId(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

const STATUS_LABEL: Record<string, string> = {
  present: "Hadir",
  late: "Terlambat",
  absent: "Absen",
  sick: "Sakit",
  permission: "Izin",
  leave: "Cuti",
  wfh: "WFH",
  business_trip: "Dinas",
  early_checkout: "Pulang cepat",
};

function AttendancePage() {
  const { data: session } = useSession();
  const [tab, setTab] = useState("Saya");
  const showHr = can(session, "attendance.view") || can(session, "attendance.approve");
  const tabs = ["Saya", ...(showHr ? ["Rekap", "Koreksi", "Manual"] : [])];

  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Absensi</h1>
      <div className="flex gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
        {tabs.map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={`rounded-lg px-4 py-1.5 text-sm font-medium transition ${
              tab === t
                ? "bg-bg-brand-primary text-text-brand-primary"
                : "text-text-secondary hover:bg-bg-primary_hover"
            }`}
          >
            {t}
          </button>
        ))}
      </div>
      {tab === "Saya" && <SelfTab />}
      {tab === "Rekap" && <RecapTab />}
      {tab === "Koreksi" && <CorrectionsTab />}
      {tab === "Manual" && <ManualTab />}
    </div>
  );
}

function SelfTab() {
  const queryClient = useQueryClient();
  const [month, setMonth] = useState(monthId());
  const [reqOpen, setReqOpen] = useState(false);
  const today = useQuery({
    queryKey: ["attendanceToday"],
    queryFn: () => unwrap(commands.attendanceToday()),
  });
  const history = useQuery({
    queryKey: ["attendanceHistory", month],
    queryFn: () => unwrap(commands.attendanceHistory(month)),
  });
  const mine = useQuery({
    queryKey: ["myCorrections"],
    queryFn: () => unwrap(commands.attendanceMyCorrections()),
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["attendanceToday"] });
    void queryClient.invalidateQueries({ queryKey: ["attendanceHistory"] });
    void queryClient.invalidateQueries({ queryKey: ["myCorrections"] });
  };

  const clockIn = useMutation({
    mutationFn: () => unwrap(commands.attendanceClockIn(null, null)),
    onSuccess: (r) => {
      toast.success(r.message);
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const clockOut = useMutation({
    mutationFn: () => unwrap(commands.attendanceClockOut(null, null)),
    onSuccess: (r) => {
      toast.success(r.message);
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const t = today.data;
  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-6">
        <p className="text-sm text-text-tertiary">{formatDateLong(todayId())}</p>
        {today.isPending ? (
          <p className="mt-2 text-sm text-text-tertiary">Memuat…</p>
        ) : !t ? (
          <div className="mt-3">
            <p className="text-sm text-text-secondary">Belum absen hari ini.</p>
            <button
              type="button"
              disabled={clockIn.isPending}
              onClick={() => clockIn.mutate()}
              className="mt-3 rounded-lg bg-bg-brand-solid px-6 py-2.5 text-sm font-semibold text-white disabled:opacity-60"
            >
              Clock In
            </button>
          </div>
        ) : (
          <div className="mt-3 flex flex-wrap items-center gap-4">
            <div className="text-sm">
              <p>Masuk: <span className="font-semibold">{t.clock_in?.slice(11, 16) ?? "-"}</span></p>
              <p>Keluar: <span className="font-semibold">{t.clock_out?.slice(11, 16) ?? "-"}</span></p>
              <p>Status: <span className="font-semibold">{STATUS_LABEL[t.status] ?? t.status}</span></p>
            </div>
            {!t.clock_out && (
              <button
                type="button"
                disabled={clockOut.isPending}
                onClick={() => clockOut.mutate()}
                className="rounded-lg bg-bg-brand-solid px-6 py-2.5 text-sm font-semibold text-white disabled:opacity-60"
              >
                Clock Out
              </button>
            )}
          </div>
        )}
      </div>
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-5">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="font-semibold">Riwayat</h3>
          <input
            type="month"
            value={month}
            onChange={(e) => setMonth(e.target.value)}
            className="rounded-lg border border-border-primary bg-bg-primary px-3 py-1.5 text-sm"
          />
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b border-border-secondary text-xs text-text-tertiary">
                {["Tanggal", "Masuk", "Keluar", "Status", "Telat", "Kerja"].map((h) => (
                  <th key={h} className="px-3 py-2 font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {(history.data ?? []).map((h) => (
                <tr key={h.id} className="border-b border-border-tertiary last:border-0">
                  <td className="whitespace-nowrap px-3 py-2">{formatDate(h.date)}</td>
                  <td className="px-3 py-2">{h.clock_in?.slice(11, 16) ?? "-"}</td>
                  <td className="px-3 py-2">{h.clock_out?.slice(11, 16) ?? "-"}</td>
                  <td className="px-3 py-2">{STATUS_LABEL[h.status] ?? h.status}</td>
                  <td className="px-3 py-2">{h.late_minutes > 0 ? `${h.late_minutes} mnt` : "-"}</td>
                  <td className="px-3 py-2">{h.work_minutes > 0 ? `${h.work_minutes} mnt` : "-"}</td>
                </tr>
              ))}
              {history.data && history.data.length === 0 && (
                <tr><td colSpan={6} className="px-3 py-4 text-center text-sm text-text-tertiary">Kosong.</td></tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-5">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="font-semibold">Koreksi saya</h3>
          <button
            type="button"
            onClick={() => setReqOpen(true)}
            className="rounded-lg border border-border-primary px-3 py-1.5 text-sm font-medium hover:bg-bg-primary_hover"
          >
            Ajukan koreksi
          </button>
        </div>
        <ul className="space-y-2 text-sm">
          {(mine.data ?? []).map((c) => (
            <li key={c.id} className="flex flex-wrap justify-between gap-2 rounded-lg bg-bg-secondary px-3 py-2">
              <span>{formatDate(c.date)} • {c.reason}</span>
              <span className="font-medium">{c.status}</span>
            </li>
          ))}
          {mine.data && mine.data.length === 0 && (
            <li className="text-sm text-text-tertiary">Belum ada pengajuan.</li>
          )}
        </ul>
      </div>
      {reqOpen && (
        <CorrectionForm
          onClose={() => {
            setReqOpen(false);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function CorrectionForm({ onClose }: { onClose: () => void }) {
  const [date, setDate] = useState(todayId());
  const [cin, setCin] = useState("");
  const [cout, setCout] = useState("");
  const [reason, setReason] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.attendanceRequestCorrection({
          date,
          requested_clock_in: cin || null,
          requested_clock_out: cout || null,
          reason,
        }),
      ),
    onSuccess: () => {
      toast.success("Koreksi diajukan.");
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
        <h2 className="text-display-xs font-semibold">Ajukan koreksi</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal</span>
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Masuk (JJ:MM)</span>
            <input value={cin} onChange={(e) => setCin(e.target.value)} placeholder="08:00" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Pulang (JJ:MM)</span>
            <input value={cout} onChange={(e) => setCout(e.target.value)} placeholder="17:00" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Alasan</span>
          <textarea value={reason} onChange={(e) => setReason(e.target.value)} required maxLength={255} rows={2} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand" />
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Ajukan</button>
        </div>
      </form>
    </div>
  );
}

function RecapTab() {
  const [date, setDate] = useState(todayId());
  const [search, setSearch] = useState("");
  const [applied, setApplied] = useState("");
  const recap = useQuery({
    queryKey: ["recap", date, applied],
    queryFn: () => unwrap(commands.attendanceRecap(date, applied)),
  });
  return (
    <div className="space-y-3">
      <form
        className="flex flex-wrap gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void recap.refetch();
          setApplied(search);
        }}
      >
        <input type="date" value={date} onChange={(e) => setDate(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Cari nama, nomor…"
          className="w-full max-w-xs rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none placeholder:text-text-placeholder focus:border-border-brand"
        />
        <button type="submit" className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover">Tampilkan</button>
      </form>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {["Nomor", "Nama", "Departemen", "Masuk", "Keluar", "Status"].map((h) => (
                <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(recap.data ?? []).map((r) => (
              <tr key={r.employee_id} className="border-b border-border-tertiary last:border-0">
                <td className="whitespace-nowrap px-4 py-2 font-medium">{r.employee_number}</td>
                <td className="px-4 py-2">{r.name}</td>
                <td className="px-4 py-2 text-text-secondary">{r.department_name ?? "-"}</td>
                <td className="px-4 py-2">{r.clock_in?.slice(11, 16) ?? "-"}</td>
                <td className="px-4 py-2">{r.clock_out?.slice(11, 16) ?? "-"}</td>
                <td className="px-4 py-2">{r.status ? (STATUS_LABEL[r.status] ?? r.status) : <span className="text-text-tertiary">Belum absen</span>}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function CorrectionsTab() {
  const queryClient = useQueryClient();
  const pending = useQuery({
    queryKey: ["pendingCorrections"],
    queryFn: () => unwrap(commands.attendancePendingCorrections()),
  });
  const decide = useMutation({
    mutationFn: (v: { id: number; decision: string }) =>
      unwrap(commands.attendanceDecideCorrection(v.id, v.decision, null)),
    onSuccess: () => {
      toast.success("Keputusan disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["pendingCorrections"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  return (
    <div className="space-y-3">
      {(pending.data ?? []).map((c) => (
        <div key={c.id} className="rounded-xl border border-border-secondary bg-bg-primary p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <p className="font-medium">{c.employee_name} • {formatDate(c.date)}</p>
            <span className="flex gap-2">
              <button
                type="button"
                onClick={() => decide.mutate({ id: c.id, decision: "approved" })}
                className="rounded-lg bg-bg-brand-solid px-3 py-1.5 text-[13px] font-semibold text-white"
              >
                Setujui
              </button>
              <button
                type="button"
                onClick={() => decide.mutate({ id: c.id, decision: "rejected" })}
                className="rounded-lg border border-border-error px-3 py-1.5 text-[13px] font-medium text-text-error-primary"
              >
                Tolak
              </button>
            </span>
          </div>
          <p className="mt-1 text-sm text-text-secondary">
            Minta: {c.requested_clock_in?.slice(11, 16) ?? "-"} → {c.requested_clock_out?.slice(11, 16) ?? "-"} • {c.reason}
          </p>
        </div>
      ))}
      {pending.data && pending.data.length === 0 && (
        <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
          Tidak ada koreksi menunggu.
        </p>
      )}
    </div>
  );
}

function ManualTab() {
  const [employeeId, setEmployeeId] = useState("");
  const [date, setDate] = useState(todayId());
  const [cin, setCin] = useState("");
  const [cout, setCout] = useState("");
  const [status, setStatus] = useState("present");
  const [notes, setNotes] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.attendanceManual({
          employee_id: Number(employeeId),
          date,
          clock_in: cin || null,
          clock_out: cout || null,
          status,
          notes: notes || null,
        }),
      ),
    onSuccess: () => toast.success("Absensi manual disimpan."),
    onError: (e: Error) => toast.error(e.message),
  });
  return (
    <form
      className="max-w-lg space-y-4 rounded-xl border border-border-secondary bg-bg-primary p-6"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <label className="block">
        <span className="mb-1 block text-sm font-medium text-text-secondary">ID karyawan</span>
        <input type="number" value={employeeId} onChange={(e) => setEmployeeId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
      </label>
      <label className="block">
        <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal</span>
        <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
      </label>
      <div className="grid grid-cols-2 gap-3">
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Masuk (JJ:MM)</span>
          <input value={cin} onChange={(e) => setCin(e.target.value)} placeholder="08:00" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Pulang (JJ:MM)</span>
          <input value={cout} onChange={(e) => setCout(e.target.value)} placeholder="17:00" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
      </div>
      <label className="block">
        <span className="mb-1 block text-sm font-medium text-text-secondary">Status</span>
        <select value={status} onChange={(e) => setStatus(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
          {Object.entries(STATUS_LABEL).map(([v, l]) => (
            <option key={v} value={v}>{l}</option>
          ))}
        </select>
      </label>
      <label className="block">
        <span className="mb-1 block text-sm font-medium text-text-secondary">Catatan (opsional)</span>
        <input value={notes} onChange={(e) => setNotes(e.target.value)} maxLength={255} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
      </label>
      <div className="flex justify-end">
        <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">
          Simpan
        </button>
      </div>
    </form>
  );
}
