import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { formatDate } from "../lib/format";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/leave")({
  component: LeavePage,
});

function yearNow(): number {
  return new Date().getFullYear();
}

function monthNow(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

const STATUS: Record<string, string> = {
  pending: "Menunggu",
  approved: "Disetujui",
  rejected: "Ditolak",
  cancelled: "Dibatalkan",
};

function LeavePage() {
  const { data: session } = useSession();
  const [tab, setTab] = useState("Saya");
  const canApprove =
    can(session, "leave.approve") ||
    can(session, "overtime.approve") ||
    can(session, "permission.approve");
  const tabs = ["Saya", "Lembur", "Izin", "Kalender", ...(canApprove ? ["Persetujuan"] : [])];

  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Cuti, Lembur & Izin</h1>
      <div className="flex flex-wrap gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
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
      {tab === "Saya" && <MyLeaveTab />}
      {tab === "Lembur" && <OvertimeTab />}
      {tab === "Izin" && <PermissionTab />}
      {tab === "Kalender" && <CalendarTab />}
      {tab === "Persetujuan" && <ApprovalsTab />}
    </div>
  );
}

function MyLeaveTab() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const balances = useQuery({
    queryKey: ["leaveBalances"],
    queryFn: () => unwrap(commands.leaveBalances(yearNow())),
  });
  const mine = useQuery({
    queryKey: ["leaveMy"],
    queryFn: () => unwrap(commands.leaveMy()),
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["leaveBalances"] });
    void queryClient.invalidateQueries({ queryKey: ["leaveMy"] });
  };
  const cancel = useMutation({
    mutationFn: (id: number) => unwrap(commands.leaveCancel(id)),
    onSuccess: () => {
      toast.success("Pengajuan dibatalkan.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {(balances.data ?? []).map((b) => (
          <div key={b.leave_type_id} className="rounded-xl border border-border-secondary bg-bg-primary p-4">
            <p className="text-sm font-medium">{b.name}</p>
            <p className="mt-1 text-display-xs font-semibold">
              {b.remaining ?? 0} <span className="text-xs font-normal text-text-tertiary">hari</span>
            </p>
            <p className="text-xs text-text-tertiary">
              {b.used_days ?? 0} terpakai dari {(b.allocated_days ?? 0) + (b.carried_days ?? 0) + (b.adjustment_days ?? 0)}
            </p>
          </div>
        ))}
      </div>
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => setOpen(true)}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Ajukan cuti
        </button>
      </div>
      <div className="space-y-2">
        {(mine.data ?? []).map((r) => (
          <div key={r.id} className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border-secondary bg-bg-primary px-4 py-3">
            <div>
              <p className="text-sm font-medium">
                {r.leave_type_name} • {formatDate(r.start_date)} – {formatDate(r.end_date)} ({r.total_days} hari)
              </p>
              <p className="text-xs text-text-tertiary">{r.reason || "Tanpa keterangan"}</p>
            </div>
            <span className="flex items-center gap-3">
              <span className="text-[13px] font-medium">{STATUS[r.status] ?? r.status}</span>
              {(r.status === "pending" || r.status === "approved") && (
                <button
                  type="button"
                  onClick={() => {
                    if (window.confirm("Batalkan pengajuan ini?")) cancel.mutate(r.id);
                  }}
                  className="text-[13px] font-medium text-text-error-primary hover:underline"
                >
                  Batalkan
                </button>
              )}
            </span>
          </div>
        ))}
        {mine.data && mine.data.length === 0 && (
          <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
            Belum ada pengajuan cuti.
          </p>
        )}
      </div>
      {open && (
        <LeaveForm
          onClose={() => {
            setOpen(false);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function LeaveForm({ onClose }: { onClose: () => void }) {
  const types = useQuery({ queryKey: ["leaveTypes"], queryFn: () => unwrap(commands.leaveTypes()) });
  const [typeId, setTypeId] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [reason, setReason] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.leaveCreate({
          leave_type_id: Number(typeId),
          start_date: start,
          end_date: end,
          reason: reason || null,
        }),
      ),
    onSuccess: () => {
      toast.success("Pengajuan cuti dikirim.");
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
        <h2 className="text-display-xs font-semibold">Ajukan cuti</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Jenis cuti</span>
          <select value={typeId} onChange={(e) => setTypeId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            <option value="">Pilih…</option>
            {(types.data ?? []).map((t) => (
              <option key={t.id} value={t.id}>{t.name}</option>
            ))}
          </select>
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Mulai</span>
            <input type="date" value={start} onChange={(e) => setStart(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Selesai</span>
            <input type="date" value={end} onChange={(e) => setEnd(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Alasan</span>
          <textarea value={reason} onChange={(e) => setReason(e.target.value)} rows={2} maxLength={255} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand" />
        </label>
        <p className="text-xs text-text-tertiary">Hari kerja Senin–Jumat di luar libur dihitung otomatis.</p>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Kirim</button>
        </div>
      </form>
    </div>
  );
}

function OvertimeTab() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const mine = useQuery({ queryKey: ["overtimeMy"], queryFn: () => unwrap(commands.overtimeMy()) });
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["overtimeMy"] });
  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => setOpen(true)}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Ajukan lembur
        </button>
      </div>
      {(mine.data ?? []).map((o) => (
        <div key={o.id} className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border-secondary bg-bg-primary px-4 py-3">
          <div>
            <p className="text-sm font-medium">
              {formatDate(o.date)} • {o.start_time.slice(11, 16)} – {o.end_time.slice(11, 16)} ({o.duration_minutes} mnt)
            </p>
            <p className="text-xs text-text-tertiary">{o.reason || "Tanpa keterangan"}</p>
          </div>
          <span className="text-[13px] font-medium">{STATUS[o.status] ?? o.status}</span>
        </div>
      ))}
      {mine.data && mine.data.length === 0 && (
        <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
          Belum ada pengajuan lembur.
        </p>
      )}
      {open && (
        <OvertimeForm
          onClose={() => {
            setOpen(false);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function OvertimeForm({ onClose }: { onClose: () => void }) {
  const [date, setDate] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [reason, setReason] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(commands.overtimeCreate({ date, start_time: start, end_time: end, reason: reason || null })),
    onSuccess: () => {
      toast.success("Pengajuan lembur dikirim.");
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
        <h2 className="text-display-xs font-semibold">Ajukan lembur</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal</span>
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Mulai (JJ:MM)</span>
            <input value={start} onChange={(e) => setStart(e.target.value)} required placeholder="18:00" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Selesai (JJ:MM)</span>
            <input value={end} onChange={(e) => setEnd(e.target.value)} required placeholder="21:00" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Alasan</span>
          <textarea value={reason} onChange={(e) => setReason(e.target.value)} rows={2} maxLength={255} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand" />
        </label>
        <p className="text-xs text-text-tertiary">Lewat tengah malam otomatis +1 hari. Minimal 30 menit.</p>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Kirim</button>
        </div>
      </form>
    </div>
  );
}

function PermissionTab() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const types = useQuery({ queryKey: ["permissionTypes"], queryFn: () => unwrap(commands.permissionTypes()) });
  const mine = useQuery({ queryKey: ["permissionMy"], queryFn: () => unwrap(commands.permissionMy()) });
  const [typeId, setTypeId] = useState("");
  const [date, setDate] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [reason, setReason] = useState("");

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.permissionCreate({
          permission_type_id: Number(typeId),
          date,
          start_time: start || null,
          end_time: end || null,
          reason,
        }),
      ),
    onSuccess: () => {
      toast.success("Pengajuan izin dikirim.");
      setOpen(false);
      setTypeId("");
      setDate("");
      setStart("");
      setEnd("");
      setReason("");
      void queryClient.invalidateQueries({ queryKey: ["permissionMy"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => setOpen(!open)}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Ajukan izin
        </button>
      </div>
      {open && (
        <form
          className="grid gap-3 rounded-xl border border-border-secondary bg-bg-primary p-5 sm:grid-cols-2"
          onSubmit={(e) => {
            e.preventDefault();
            save.mutate();
          }}
        >
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Jenis izin</span>
            <select value={typeId} onChange={(e) => setTypeId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="">Pilih…</option>
              {(types.data ?? []).map((t) => (
                <option key={t.id} value={t.id}>{t.name}</option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal</span>
            <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Dari jam (opsional)</span>
            <input value={start} onChange={(e) => setStart(e.target.value)} placeholder="10:00" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Sampai jam (opsional)</span>
            <input value={end} onChange={(e) => setEnd(e.target.value)} placeholder="12:00" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block sm:col-span-2">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Alasan</span>
            <input value={reason} onChange={(e) => setReason(e.target.value)} required maxLength={255} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <div className="sm:col-span-2 flex justify-end">
            <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">
              Kirim
            </button>
          </div>
        </form>
      )}
      {(mine.data ?? []).map((p) => (
        <div key={p.id} className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border-secondary bg-bg-primary px-4 py-3">
          <div>
            <p className="text-sm font-medium">
              {p.permission_type_name} • {formatDate(p.date)}
              {p.start_time ? ` ${p.start_time}–${p.end_time ?? ""}` : ""}
            </p>
            <p className="text-xs text-text-tertiary">{p.reason}</p>
          </div>
          <span className="text-[13px] font-medium">{STATUS[p.status] ?? p.status}</span>
        </div>
      ))}
      {mine.data && mine.data.length === 0 && (
        <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
          Belum ada pengajuan izin.
        </p>
      )}
    </div>
  );
}

function CalendarTab() {
  const [month, setMonth] = useState(monthNow());
  const cal = useQuery({
    queryKey: ["leaveCalendar", month],
    queryFn: () => unwrap(commands.leaveCalendar(month)),
  });
  return (
    <div className="space-y-3">
      <input
        type="month"
        value={month}
        onChange={(e) => setMonth(e.target.value)}
        className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
      />
      <div className="space-y-2">
        {(cal.data ?? []).map((d, i) => (
          <div
            key={`${d.date}-${i}`}
            className="flex flex-wrap items-center gap-3 rounded-xl border border-border-secondary bg-bg-primary px-4 py-2.5"
          >
            <span className="w-28 shrink-0 text-sm font-medium">{formatDate(d.date)}</span>
            <span className="text-sm">{d.label}</span>
            <span className="ml-auto rounded-full bg-bg-brand-primary px-2 py-0.5 text-xs text-text-brand-primary">
              {d.kind === "holiday" ? "Libur" : "Cuti"}
            </span>
          </div>
        ))}
        {cal.data && cal.data.length === 0 && (
          <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
            Tidak ada cuti atau libur bulan ini.
          </p>
        )}
      </div>
    </div>
  );
}

function ApprovalsTab() {
  const queryClient = useQueryClient();
  const leave = useQuery({ queryKey: ["leavePending"], queryFn: () => unwrap(commands.leavePending()) });
  const overtime = useQuery({ queryKey: ["overtimePending"], queryFn: () => unwrap(commands.overtimePending()) });
  const permission = useQuery({
    queryKey: ["permissionPending"],
    queryFn: () => unwrap(commands.permissionPending()),
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["leavePending"] });
    void queryClient.invalidateQueries({ queryKey: ["overtimePending"] });
    void queryClient.invalidateQueries({ queryKey: ["permissionPending"] });
  };
  const onOk = () => {
    toast.success("Keputusan disimpan.");
    refresh();
  };
  const onErr = (e: Error) => toast.error(e.message);

  const leaveDecide = useMutation({
    mutationFn: (v: { id: number; d: string }) => unwrap(commands.leaveDecide(v.id, v.d, null)),
    onSuccess: onOk,
    onError: onErr,
  });
  const overtimeDecide = useMutation({
    mutationFn: (v: { id: number; d: string }) => unwrap(commands.overtimeDecide(v.id, v.d, null)),
    onSuccess: onOk,
    onError: onErr,
  });
  const permissionDecide = useMutation({
    mutationFn: (v: { id: number; d: string }) => unwrap(commands.permissionDecide(v.id, v.d)),
    onSuccess: onOk,
    onError: onErr,
  });

  const group = (
    title: string,
    items: { id: number; label: string; sub: string }[] | undefined,
    act: (id: number, d: string) => void,
  ) => (
    <div className="rounded-xl border border-border-secondary bg-bg-primary p-5">
      <h3 className="font-semibold">{title}</h3>
      <div className="mt-2 space-y-2">
        {(items ?? []).map((it) => (
          <div key={`${title}-${it.id}`} className="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-bg-secondary px-3 py-2">
            <div>
              <p className="text-sm font-medium">{it.label}</p>
              <p className="text-xs text-text-tertiary">{it.sub}</p>
            </div>
            <span className="flex gap-2">
              <button
                type="button"
                onClick={() => act(it.id, "approved")}
                className="rounded-lg bg-bg-brand-solid px-3 py-1.5 text-[13px] font-semibold text-white"
              >
                Setujui
              </button>
              <button
                type="button"
                onClick={() => act(it.id, "rejected")}
                className="rounded-lg border border-border-error px-3 py-1.5 text-[13px] font-medium text-text-error-primary"
              >
                Tolak
              </button>
            </span>
          </div>
        ))}
        {(!items || items.length === 0) && (
          <p className="text-sm text-text-tertiary">Tidak ada antrean.</p>
        )}
      </div>
    </div>
  );

  return (
    <div className="space-y-3">
      {group(
        "Cuti",
        leave.data?.map((r) => ({
          id: r.id,
          label: `${r.employee_name} • ${r.leave_type_name} • ${formatDate(r.start_date)} – ${formatDate(r.end_date)}`,
          sub: `${r.total_days} hari • ${r.reason || "Tanpa keterangan"}`,
        })),
        (id, d) => leaveDecide.mutate({ id, d }),
      )}
      {group(
        "Lembur",
        overtime.data?.map((o) => ({
          id: o.id,
          label: `${o.employee_name} • ${formatDate(o.date)} • ${o.duration_minutes} mnt`,
          sub: o.reason || "Tanpa keterangan",
        })),
        (id, d) => overtimeDecide.mutate({ id, d }),
      )}
      {group(
        "Izin",
        permission.data?.map((p) => ({
          id: p.id,
          label: `${p.employee_name} • ${p.permission_type_name} • ${formatDate(p.date)}`,
          sub: p.reason,
        })),
        (id, d) => permissionDecide.mutate({ id, d }),
      )}
    </div>
  );
}
