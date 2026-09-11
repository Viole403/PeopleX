import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import {
  commands,
  type Assignment,
  type Holiday,
  type Schedule,
  type Shift,
} from "../bindings";
import { formatDate } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/schedules")({
  component: SchedulesPage,
});

function SchedulesPage() {
  const [tab, setTab] = useState("Shift");
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Shift & Jadwal</h1>
      <div className="flex flex-wrap gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
        {["Shift", "Jadwal", "Penugasan", "Libur"].map((t) => (
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
      {tab === "Shift" && <ShiftTab />}
      {tab === "Jadwal" && <ScheduleTab />}
      {tab === "Penugasan" && <AssignmentTab />}
      {tab === "Libur" && <HolidayTab />}
    </div>
  );
}

function ShiftTab() {
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["shifts"], queryFn: () => unwrap(commands.shiftList()) });
  const [editing, setEditing] = useState<Shift | "new" | null>(null);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["shifts"] });
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.shiftDelete(id)),
    onSuccess: () => {
      toast.success("Shift dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => setEditing("new")}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Tambah shift
        </button>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        {(list.data ?? []).map((s) => (
          <div key={s.id} className="rounded-xl border border-border-secondary bg-bg-primary p-4">
            <div className="flex items-center justify-between">
              <p className="font-semibold">{s.name}</p>
              {s.is_overnight && (
                <span className="rounded-full bg-bg-brand-primary px-2 py-0.5 text-xs text-text-brand-primary">
                  Malam
                </span>
              )}
            </div>
            <p className="mt-1 text-sm text-text-secondary">
              {s.start_time.slice(0, 5)} – {s.end_time.slice(0, 5)} • toleransi {s.grace_period_minutes} mnt
            </p>
            <div className="mt-2 flex gap-3 text-[13px]">
              <button type="button" onClick={() => setEditing(s)} className="font-medium text-text-brand-secondary hover:underline">Ubah</button>
              <button
                type="button"
                onClick={() => {
                  if (window.confirm(`Hapus shift ${s.name}?`)) remove.mutate(s.id);
                }}
                className="font-medium text-text-error-primary hover:underline"
              >
                Hapus
              </button>
            </div>
          </div>
        ))}
      </div>
      {editing && (
        <ShiftForm
          initial={editing === "new" ? null : editing}
          onClose={() => {
            setEditing(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function ShiftForm({ initial, onClose }: { initial: Shift | null; onClose: () => void }) {
  const [name, setName] = useState(initial?.name ?? "");
  const [start, setStart] = useState(initial?.start_time.slice(0, 5) ?? "08:00");
  const [end, setEnd] = useState(initial?.end_time.slice(0, 5) ?? "17:00");
  const [grace, setGrace] = useState(initial?.grace_period_minutes ?? 15);
  const [overnight, setOvernight] = useState(initial?.is_overnight ?? false);

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.shiftSave(initial?.id ?? null, {
          name,
          start_time: `${start}:00`,
          end_time: `${end}:00`,
          break_start: null,
          break_end: null,
          grace_period_minutes: grace,
          is_overnight: overnight,
        }),
      ),
    onSuccess: () => {
      toast.success("Shift disimpan.");
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
        <h2 className="text-display-xs font-semibold">{initial ? "Ubah" : "Tambah"} shift</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Nama</span>
          <input value={name} onChange={(e) => setName(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Masuk</span>
            <input type="time" value={start} onChange={(e) => setStart(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Pulang</span>
            <input type="time" value={end} onChange={(e) => setEnd(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Toleransi (menit)</span>
          <input type="number" min={0} max={180} value={grace} onChange={(e) => setGrace(Number(e.target.value))} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <label className="flex items-center gap-3 text-sm">
          <input type="checkbox" checked={overnight} onChange={(e) => setOvernight(e.target.checked)} className="size-4 accent-brand-600" />
          Shift malam (lewat tengah malam)
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}

const DOW = ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"];

function ScheduleTab() {
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["schedules"], queryFn: () => unwrap(commands.scheduleList()) });
  const shifts = useQuery({ queryKey: ["shifts"], queryFn: () => unwrap(commands.shiftList()) });
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<Schedule | null>(null);

  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["schedules"] });

  const create = useMutation({
    mutationFn: () => unwrap(commands.scheduleSave(null, { name, description: null })),
    onSuccess: () => {
      toast.success("Jadwal dibuat dengan hari Senin–Jumat aktif.");
      setName("");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.scheduleDelete(id)),
    onSuccess: () => {
      toast.success("Jadwal dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-3">
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          create.mutate();
        }}
      >
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
          placeholder="Nama jadwal baru…"
          className="w-full max-w-sm rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
        />
        <button type="submit" className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">
          Tambah
        </button>
      </form>
      {(list.data ?? []).map((s) => (
        <div key={s.id} className="rounded-xl border border-border-secondary bg-bg-primary p-5">
          <div className="flex items-center justify-between">
            <p className="font-semibold">{s.name}</p>
            <span className="flex gap-2">
              <button type="button" onClick={() => setEditing(s)} className="text-[13px] font-medium text-text-brand-secondary hover:underline">
                Atur hari
              </button>
              <button
                type="button"
                onClick={() => {
                  if (window.confirm(`Hapus jadwal ${s.name}?`)) remove.mutate(s.id);
                }}
                className="text-[13px] font-medium text-text-error-primary hover:underline"
              >
                Hapus
              </button>
            </span>
          </div>
          <div className="mt-2 flex flex-wrap gap-1">
            {s.days.map((d) => (
              <span
                key={d.day_of_week}
                title={shifts.data?.find((x) => x.id === d.shift_id)?.name ?? ""}
                className={`rounded-full px-2.5 py-0.5 text-xs font-medium ${
                  d.is_working_day
                    ? "bg-bg-success-primary text-text-success-primary"
                    : "bg-bg-tertiary text-text-tertiary"
                }`}
              >
                {DOW[d.day_of_week]}
              </span>
            ))}
          </div>
        </div>
      ))}
      {editing && (
        <DaysDialog
          schedule={editing}
          shifts={shifts.data ?? []}
          onClose={() => {
            setEditing(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function DaysDialog({
  schedule,
  shifts,
  onClose,
}: {
  schedule: Schedule;
  shifts: Shift[];
  onClose: () => void;
}) {
  const [days, setDays] = useState(
    schedule.days.map((d) => ({
      dow: d.day_of_week,
      shift: d.shift_id ? String(d.shift_id) : "",
      working: d.is_working_day,
    })),
  );
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.scheduleSaveDays(
          schedule.id,
          days.map((d) => [d.dow, d.shift === "" ? null : Number(d.shift), d.working] as [number, number | null, boolean]),
        ),
      ),
    onSuccess: () => {
      toast.success("Hari jadwal disimpan.");
      onClose();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <form
        className="w-full max-w-lg space-y-3 rounded-2xl bg-bg-primary p-6 shadow-2xl"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <h2 className="text-display-xs font-semibold">Hari: {schedule.name}</h2>
        {days.map((d, i) => (
          <div key={d.dow} className="flex items-center gap-2 text-sm">
            <span className="w-12 font-medium">{DOW[d.dow]}</span>
            <input
              type="checkbox"
              checked={d.working}
              onChange={(e) => {
                const next = [...days];
                next[i] = { ...d, working: e.target.checked };
                setDays(next);
              }}
              className="size-4 accent-brand-600"
            />
            <select
              value={d.shift}
              disabled={!d.working}
              onChange={(e) => {
                const next = [...days];
                next[i] = { ...d, shift: e.target.value };
                setDays(next);
              }}
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-2 py-1.5 text-sm disabled:opacity-50"
            >
              <option value="">Tanpa shift</option>
              {shifts.map((s) => (
                <option key={s.id} value={s.id}>{s.name}</option>
              ))}
            </select>
          </div>
        ))}
        <div className="flex justify-end gap-2 pt-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}

function AssignmentTab() {
  const queryClient = useQueryClient();
  const list = useQuery({
    queryKey: ["assignments"],
    queryFn: () => unwrap(commands.assignmentList()),
  });
  const employees = useQuery({
    queryKey: ["employeeDropdowns"],
    queryFn: () => unwrap(commands.employeeDropdowns()),
  });
  const schedules = useQuery({
    queryKey: ["schedules"],
    queryFn: () => unwrap(commands.scheduleList()),
  });
  const shifts = useQuery({ queryKey: ["shifts"], queryFn: () => unwrap(commands.shiftList()) });
  const [open, setOpen] = useState(false);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["assignments"] });
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.assignmentDelete(id)),
    onSuccess: () => {
      toast.success("Penugasan dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => setOpen(true)}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Tambah penugasan
        </button>
      </div>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {["Karyawan", "Jadwal/Shift", "Tanggal", "Rentang", "Aksi"].map((h) => (
                <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(list.data ?? []).map((a: Assignment) => (
              <tr key={a.id} className="border-b border-border-tertiary last:border-0">
                <td className="px-4 py-2.5 font-medium">{a.employee_name}</td>
                <td className="px-4 py-2.5 text-text-secondary">
                  {a.schedule_name ?? a.shift_name ?? "-"}
                </td>
                <td className="px-4 py-2.5 text-text-secondary">
                  {a.date ? formatDate(a.date) : "-"}
                </td>
                <td className="px-4 py-2.5 text-text-secondary">
                  {formatDate(a.start_date)}{a.end_date ? ` – ${formatDate(a.end_date)}` : ""}
                </td>
                <td className="px-4 py-2.5">
                  <button
                    type="button"
                    onClick={() => {
                      if (window.confirm("Hapus penugasan ini?")) remove.mutate(a.id);
                    }}
                    className="text-[13px] font-medium text-text-error-primary hover:underline"
                  >
                    Hapus
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {open && (
        <AssignmentForm
          employees={employees.data?.employees ?? []}
          schedules={schedules.data ?? []}
          shifts={shifts.data ?? []}
          onClose={() => {
            setOpen(false);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function AssignmentForm({
  employees,
  schedules,
  shifts,
  onClose,
}: {
  employees: { id: number; name: string }[];
  schedules: Schedule[];
  shifts: Shift[];
  onClose: () => void;
}) {
  const [employeeId, setEmployeeId] = useState("");
  const [mode, setMode] = useState<"schedule" | "shift">("schedule");
  const [refId, setRefId] = useState("");
  const [specific, setSpecific] = useState(true);
  const [date, setDate] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.assignmentSave(null, {
          employee_id: Number(employeeId),
          work_schedule_id: mode === "schedule" ? Number(refId) : null,
          shift_id: mode === "shift" ? Number(refId) : null,
          date: specific ? date || null : null,
          start_date: specific ? null : start || null,
          end_date: specific ? null : end || null,
        }),
      ),
    onSuccess: () => {
      toast.success("Penugasan disimpan.");
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
        <h2 className="text-display-xs font-semibold">Tambah penugasan</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Karyawan</span>
          <select value={employeeId} onChange={(e) => setEmployeeId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            <option value="">Pilih…</option>
            {employees.map((e) => (
              <option key={e.id} value={e.id}>{e.name}</option>
            ))}
          </select>
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Jenis</span>
            <select value={mode} onChange={(e) => setMode(e.target.value as "schedule" | "shift")} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="schedule">Jadwal mingguan</option>
              <option value="shift">Shift tanggal spesifik</option>
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">
              {mode === "schedule" ? "Jadwal" : "Shift"}
            </span>
            <select value={refId} onChange={(e) => setRefId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="">Pilih…</option>
              {(mode === "schedule" ? schedules : shifts).map((r) => (
                <option key={r.id} value={r.id}>{r.name}</option>
              ))}
            </select>
          </label>
        </div>
        {mode === "shift" ? (
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal spesifik</span>
            <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        ) : (
          <>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={specific} onChange={(e) => setSpecific(e.target.checked)} className="size-4 accent-brand-600" />
              Berlaku sejak tanggal tertentu (kosongkan rentang bila spesifik)
            </label>
            {specific ? (
              <label className="block">
                <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal spesifik (opsional)</span>
                <input type="date" value={date} onChange={(e) => setDate(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              </label>
            ) : (
              <div className="grid grid-cols-2 gap-3">
                <label className="block">
                  <span className="mb-1 block text-sm font-medium text-text-secondary">Mulai</span>
                  <input type="date" value={start} onChange={(e) => setStart(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
                </label>
                <label className="block">
                  <span className="mb-1 block text-sm font-medium text-text-secondary">Selesai (opsional)</span>
                  <input type="date" value={end} onChange={(e) => setEnd(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
                </label>
              </div>
            )}
          </>
        )}
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}

const HOLIDAY_TYPES: [string, string][] = [
  ["national", "Nasional"],
  ["company", "Perusahaan"],
  ["collective_leave", "Cuti bersama"],
  ["custom", "Khusus"],
];

function HolidayTab() {
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["holidays"], queryFn: () => unwrap(commands.holidayList()) });
  const [editing, setEditing] = useState<Holiday | "new" | null>(null);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["holidays"] });
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.holidayDelete(id)),
    onSuccess: () => {
      toast.success("Libur dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const [name, setName] = useState("");
  const [date, setDate] = useState("");
  const [htype, setHtype] = useState("national");

  const create = useMutation({
    mutationFn: () =>
      unwrap(commands.holidaySave(null, { name, date, holiday_type: htype, description: null })),
    onSuccess: () => {
      toast.success("Libur ditambahkan.");
      setName("");
      setDate("");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-3">
      <form
        className="flex flex-wrap gap-2 rounded-xl border border-border-secondary bg-bg-primary p-4"
        onSubmit={(e) => {
          e.preventDefault();
          create.mutate();
        }}
      >
        <input value={name} onChange={(e) => setName(e.target.value)} required placeholder="Nama libur…" className="min-w-40 flex-1 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand" />
        <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        <select value={htype} onChange={(e) => setHtype(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
          {HOLIDAY_TYPES.map(([v, l]) => (
            <option key={v} value={v}>{l}</option>
          ))}
        </select>
        <button type="submit" className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Tambah</button>
      </form>
      <div className="grid gap-2">
        {(list.data ?? []).map((h) => (
          <div key={h.id} className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border-secondary bg-bg-primary px-4 py-2.5">
            <div>
              <p className="text-sm font-medium">{h.name}</p>
              <p className="text-xs text-text-tertiary">
                {formatDate(h.date)} • {HOLIDAY_TYPES.find(([v]) => v === h.holiday_type)?.[1] ?? h.holiday_type}
              </p>
            </div>
            <span className="flex gap-3 text-[13px]">
              <button type="button" onClick={() => setEditing(h)} className="font-medium text-text-brand-secondary hover:underline">Ubah</button>
              <button
                type="button"
                onClick={() => {
                  if (window.confirm(`Hapus libur ${h.name}?`)) remove.mutate(h.id);
                }}
                className="font-medium text-text-error-primary hover:underline"
              >
                Hapus
              </button>
            </span>
          </div>
        ))}
      </div>
      {editing && editing !== "new" && (
        <HolidayEdit
          holiday={editing}
          onClose={() => {
            setEditing(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function HolidayEdit({ holiday, onClose }: { holiday: Holiday; onClose: () => void }) {
  const [name, setName] = useState(holiday.name);
  const [date, setDate] = useState(holiday.date);
  const [htype, setHtype] = useState(holiday.holiday_type);
  const save = useMutation({
    mutationFn: () =>
      unwrap(commands.holidaySave(holiday.id, { name, date, holiday_type: htype, description: holiday.description })),
    onSuccess: () => {
      toast.success("Libur disimpan.");
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
        <h2 className="text-display-xs font-semibold">Ubah libur</h2>
        <input value={name} onChange={(e) => setName(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        <div className="grid grid-cols-2 gap-3">
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          <select value={htype} onChange={(e) => setHtype(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            {HOLIDAY_TYPES.map(([v, l]) => (
              <option key={v} value={v}>{l}</option>
            ))}
          </select>
        </div>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}
