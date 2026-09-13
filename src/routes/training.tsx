import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type Training } from "../bindings";
import { formatDate } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/training")({
  component: TrainingPage,
});

const STATUS_LABEL: Record<string, string> = {
  scheduled: "Terjadwal",
  ongoing: "Berlangsung",
  completed: "Selesai",
  cancelled: "Dibatalkan",
  registered: "Terdaftar",
  attended: "Hadir",
  absent: "Absen",
};

function TrainingPage() {
  const [tab, setTab] = useState("Katalog");
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Training</h1>
      <div className="flex flex-wrap gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
        {["Katalog", "Sertifikasi", "Skill"].map((t) => (
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
      {tab === "Katalog" && <CatalogTab />}
      {tab === "Sertifikasi" && <CertTab />}
      {tab === "Skill" && <SkillTab />}
    </div>
  );
}

function CatalogTab() {
  const queryClient = useQueryClient();
  const [detailId, setDetailId] = useState<number | null>(null);
  const [materialId, setMaterialId] = useState<number | null>(null);
  const [form, setForm] = useState<Training | "new" | null>(null);
  const list = useQuery({ queryKey: ["trainings"], queryFn: () => unwrap(commands.trainingList()) });
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["trainings"] });
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.trainingDelete(id)),
    onSuccess: () => {
      toast.success("Training dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => setForm("new")}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Training baru
        </button>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        {(list.data ?? []).map((t) => (
          <div key={t.id} className="rounded-xl border border-border-secondary bg-bg-primary p-4">
            <div className="flex items-start justify-between gap-2">
              <div>
                <p className="font-semibold">{t.title}</p>
                <p className="text-xs text-text-tertiary">
                  {formatDate(t.start_date)} – {formatDate(t.end_date)} • {STATUS_LABEL[t.status] ?? t.status} • {t.participant_count} peserta
                </p>
                {t.trainer_name && <p className="text-xs text-text-tertiary">Trainer: {t.trainer_name}</p>}
              </div>
            </div>
            <div className="mt-2 flex gap-3 text-[13px]">
              <button type="button" onClick={() => setDetailId(t.id)} className="font-medium text-text-brand-secondary hover:underline">
                Peserta
              </button>
              <button type="button" onClick={() => setMaterialId(t.id)} className="font-medium text-text-brand-secondary hover:underline">
                Materi
              </button>
              <button type="button" onClick={() => setForm(t)} className="font-medium text-text-brand-secondary hover:underline">
                Ubah
              </button>
              <button
                type="button"
                onClick={() => {
                  if (window.confirm(`Hapus training ${t.title}?`)) remove.mutate(t.id);
                }}
                className="font-medium text-text-error-primary hover:underline"
              >
                Hapus
              </button>
            </div>
          </div>
        ))}
      </div>
      {form && (
        <TrainingForm
          initial={form === "new" ? null : form}
          onClose={() => {
            setForm(null);
            refresh();
          }}
        />
      )}
      {detailId !== null && (
        <ParticipantDialog
          id={detailId}
          onClose={() => {
            setDetailId(null);
            refresh();
          }}
        />
      )}
      {materialId !== null && (
        <MaterialDialog id={materialId} onClose={() => setMaterialId(null)} />
      )}
    </div>
  );
}

function TrainingForm({ initial, onClose }: { initial: Training | null; onClose: () => void }) {
  const [title, setTitle] = useState(initial?.title ?? "");
  const [trainer, setTrainer] = useState(initial?.trainer_name ?? "");
  const [start, setStart] = useState(initial?.start_date ?? "");
  const [end, setEnd] = useState(initial?.end_date ?? "");
  const [location, setLocation] = useState(initial?.location ?? "");
  const [cost, setCost] = useState(initial ? String(initial.cost ?? 0) : "0");
  const [quota, setQuota] = useState(initial?.quota ? String(initial.quota) : "");
  const [status, setStatus] = useState(initial?.status ?? "scheduled");
  const [description, setDescription] = useState(initial?.description ?? "");

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.trainingSave(initial?.id ?? null, {
          title,
          description: description || null,
          trainer_name: trainer || null,
          start_date: start,
          end_date: end,
          location: location || null,
          cost: Number(cost),
          quota: quota === "" ? null : Number(quota),
          status,
        }),
      ),
    onSuccess: () => {
      toast.success("Training disimpan.");
      onClose();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <form
        className="max-h-[85vh] w-full max-w-lg space-y-4 overflow-y-auto rounded-2xl bg-bg-primary p-6 shadow-2xl"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <h2 className="text-display-xs font-semibold">{initial ? "Ubah" : "Tambah"} training</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Judul</span>
          <input value={title} onChange={(e) => setTitle(e.target.value)} required maxLength={150} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
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
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Trainer</span>
            <input value={trainer} onChange={(e) => setTrainer(e.target.value)} maxLength={150} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Lokasi</span>
            <input value={location} onChange={(e) => setLocation(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <div className="grid grid-cols-3 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Biaya (Rp)</span>
            <input type="number" min={0} value={cost} onChange={(e) => setCost(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Kuota</span>
            <input type="number" min={1} value={quota} onChange={(e) => setQuota(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Status</span>
            <select value={status} onChange={(e) => setStatus(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              {["scheduled", "ongoing", "completed", "cancelled"].map((s) => (
                <option key={s} value={s}>{STATUS_LABEL[s]}</option>
              ))}
            </select>
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Deskripsi</span>
          <textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={2} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}

function ParticipantDialog({ id, onClose }: { id: number; onClose: () => void }) {
  const queryClient = useQueryClient();
  const list = useQuery({
    queryKey: ["participants", id],
    queryFn: () => unwrap(commands.trainingParticipants(id)),
  });
  const employees = useQuery({
    queryKey: ["employeeDropdowns"],
    queryFn: () => unwrap(commands.employeeDropdowns()),
  });
  const [employeeId, setEmployeeId] = useState("");
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["participants", id] });

  const add = useMutation({
    mutationFn: () => unwrap(commands.trainingAddParticipant(id, Number(employeeId))),
    onSuccess: () => {
      toast.success("Peserta ditambahkan.");
      setEmployeeId("");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const setStatus = useMutation({
    mutationFn: (v: { pid: number; status: string }) =>
      unwrap(commands.trainingParticipantStatus(v.pid, v.status)),
    onSuccess: () => {
      toast.success("Status diperbarui.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <div className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-2xl bg-bg-primary p-6 shadow-2xl">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-display-xs font-semibold">Peserta</h2>
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm">Tutup</button>
        </div>
        <form
          className="mb-3 flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            add.mutate();
          }}
        >
          <select value={employeeId} onChange={(e) => setEmployeeId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            <option value="">Pilih karyawan…</option>
            {(employees.data?.employees ?? []).map((e) => (
              <option key={e.id} value={e.id}>{e.name}</option>
            ))}
          </select>
          <button type="submit" className="shrink-0 rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">
            Tambah
          </button>
        </form>
        <ul className="space-y-1">
          {(list.data ?? []).map((p) => (
            <li key={p.id} className="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-bg-secondary px-3 py-2 text-sm">
              <span className="font-medium">{p.employee_name}</span>
              <select
                value={p.status}
                onChange={(e) => setStatus.mutate({ pid: p.id, status: e.target.value })}
                className="rounded-lg border border-border-primary bg-bg-primary px-2 py-1 text-[13px]"
              >
                {["registered", "attended", "absent", "completed"].map((s) => (
                  <option key={s} value={s}>{STATUS_LABEL[s]}</option>
                ))}
              </select>
            </li>
          ))}
          {list.data && list.data.length === 0 && (
            <li className="text-sm text-text-tertiary">Belum ada peserta.</li>
          )}
        </ul>
      </div>
    </div>
  );
}

const MATERIAL_KINDS = ["tautan", "dokumen", "teks"] as const;

function MaterialDialog({ id, onClose }: { id: number; onClose: () => void }) {
  const queryClient = useQueryClient();
  const list = useQuery({
    queryKey: ["materials", id],
    queryFn: () => unwrap(commands.trainingMaterials(id)),
  });
  const [title, setTitle] = useState("");
  const [kind, setKind] = useState<string>("tautan");
  const [url, setUrl] = useState("");
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["materials", id] });

  const add = useMutation({
    mutationFn: () => unwrap(commands.trainingMaterialAdd(id, title, kind, url || null)),
    onSuccess: () => {
      toast.success("Materi ditambahkan.");
      setTitle("");
      setUrl("");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const remove = useMutation({
    mutationFn: (mid: number) => unwrap(commands.trainingMaterialDelete(mid)),
    onSuccess: () => {
      toast.success("Materi dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <div className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-2xl bg-bg-primary p-6 shadow-2xl">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-display-xs font-semibold">Materi</h2>
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm">Tutup</button>
        </div>
        <form
          className="mb-3 space-y-2"
          onSubmit={(e) => {
            e.preventDefault();
            add.mutate();
          }}
        >
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            required
            placeholder="Judul materi…"
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
          />
          <div className="flex gap-2">
            <select value={kind} onChange={(e) => setKind(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              {MATERIAL_KINDS.map((k) => (
                <option key={k} value={k}>{k}</option>
              ))}
            </select>
            <input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="Tautan (opsional)…"
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
            />
            <button type="submit" className="shrink-0 rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">
              Tambah
            </button>
          </div>
        </form>
        <ul className="space-y-1">
          {(list.data ?? []).map((m) => (
            <li key={m.id} className="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-bg-secondary px-3 py-2 text-sm">
              <span>
                <span className="font-medium">{m.title}</span>
                <span className="text-text-tertiary"> • {m.kind}</span>
                {m.url && (
                  <span className="text-text-tertiary"> • {m.url}</span>
                )}
              </span>
              <button
                type="button"
                onClick={() => {
                  if (window.confirm(`Hapus materi ${m.title}?`)) remove.mutate(m.id);
                }}
                className="font-medium text-text-error-primary hover:underline"
              >
                Hapus
              </button>
            </li>
          ))}
          {list.data && list.data.length === 0 && (
            <li className="text-sm text-text-tertiary">Belum ada materi.</li>
          )}
        </ul>
      </div>
    </div>
  );
}

function CertTab() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const list = useQuery({
    queryKey: ["certifications"],
    queryFn: () => unwrap(commands.trainingCertifications()),
  });
  const employees = useQuery({
    queryKey: ["employeeDropdowns"],
    queryFn: () => unwrap(commands.employeeDropdowns()),
  });
  const [employeeId, setEmployeeId] = useState("");
  const [name, setName] = useState("");
  const [issuer, setIssuer] = useState("");
  const [issued, setIssued] = useState("");
  const [expiry, setExpiry] = useState("");

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.trainingCertificationAdd({
          employee_id: Number(employeeId),
          name,
          issuer: issuer || null,
          certificate_number: null,
          issued_date: issued || null,
          expiry_date: expiry || null,
        }),
      ),
    onSuccess: () => {
      toast.success("Sertifikasi disimpan.");
      setOpen(false);
      setEmployeeId("");
      setName("");
      setIssuer("");
      setIssued("");
      setExpiry("");
      void queryClient.invalidateQueries({ queryKey: ["certifications"] });
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
          Tambah sertifikasi
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
            <span className="mb-1 block text-sm font-medium text-text-secondary">Karyawan</span>
            <select value={employeeId} onChange={(e) => setEmployeeId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="">Pilih…</option>
              {(employees.data?.employees ?? []).map((e) => (
                <option key={e.id} value={e.id}>{e.name}</option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Nama sertifikasi</span>
            <input value={name} onChange={(e) => setName(e.target.value)} required maxLength={150} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Penerbit</span>
            <input value={issuer} onChange={(e) => setIssuer(e.target.value)} maxLength={150} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal terbit</span>
            <input type="date" value={issued} onChange={(e) => setIssued(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Kedaluarsa</span>
            <input type="date" value={expiry} onChange={(e) => setExpiry(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <div className="flex items-end justify-end">
            <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">
              Simpan
            </button>
          </div>
        </form>
      )}
      <div className="space-y-2">
        {(list.data ?? []).map((c) => (
          <div key={c.id} className="rounded-xl border border-border-secondary bg-bg-primary px-4 py-3">
            <p className="text-sm font-medium">{c.name}</p>
            <p className="text-xs text-text-tertiary">
              {c.employee_name}{c.issuer ? ` • ${c.issuer}` : ""}
              {c.issued_date ? ` • terbit ${formatDate(c.issued_date)}` : ""}
              {c.expiry_date ? ` • berlaku s.d. ${formatDate(c.expiry_date)}` : ""}
            </p>
          </div>
        ))}
        {list.data && list.data.length === 0 && (
          <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
            Belum ada sertifikasi.
          </p>
        )}
      </div>
    </div>
  );
}

function SkillTab() {
  const queryClient = useQueryClient();
  const matrix = useQuery({
    queryKey: ["skillMatrix"],
    queryFn: () => unwrap(commands.trainingSkillMatrix()),
  });
  const employees = useQuery({
    queryKey: ["employeeDropdowns"],
    queryFn: () => unwrap(commands.employeeDropdowns()),
  });
  const [employeeId, setEmployeeId] = useState("");
  const [skill, setSkill] = useState("");
  const [level, setLevel] = useState("3");

  const save = useMutation({
    mutationFn: () => unwrap(commands.trainingSkillSet(Number(employeeId), skill, Number(level))),
    onSuccess: () => {
      toast.success("Skill disimpan.");
      setSkill("");
      void queryClient.invalidateQueries({ queryKey: ["skillMatrix"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const skills = [...new Set((matrix.data ?? []).map((c) => c.skill_name))].sort();

  return (
    <div className="space-y-3">
      <form
        className="flex flex-wrap items-end gap-2 rounded-xl border border-border-secondary bg-bg-primary p-4"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <label className="block min-w-0 flex-1">
          <span className="mb-1 block text-xs text-text-tertiary">Karyawan</span>
          <select value={employeeId} onChange={(e) => setEmployeeId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            <option value="">Pilih…</option>
            {(employees.data?.employees ?? []).map((e) => (
              <option key={e.id} value={e.id}>{e.name}</option>
            ))}
          </select>
        </label>
        <label className="block min-w-0 flex-1">
          <span className="mb-1 block text-xs text-text-tertiary">Skill</span>
          <input value={skill} onChange={(e) => setSkill(e.target.value)} required list="skill-list" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          <datalist id="skill-list">
            {skills.map((s) => (
              <option key={s} value={s} />
            ))}
          </datalist>
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-text-tertiary">Level 1–5</span>
          <select value={level} onChange={(e) => setLevel(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            {[1, 2, 3, 4, 5].map((l) => (
              <option key={l} value={l}>{l}</option>
            ))}
          </select>
        </label>
        <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">
          Simpan
        </button>
      </form>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              <th className="px-4 py-2.5 font-medium">Karyawan</th>
              <th className="px-4 py-2.5 font-medium">Skill</th>
              <th className="px-4 py-2.5 font-medium">Level</th>
            </tr>
          </thead>
          <tbody>
            {(matrix.data ?? []).map((c, i) => (
              <tr key={i} className="border-b border-border-tertiary last:border-0">
                <td className="px-4 py-2 font-medium">{c.employee_name}</td>
                <td className="px-4 py-2">{c.skill_name}</td>
                <td className="px-4 py-2">
                  <span className="flex gap-0.5" aria-label={`Level ${c.level}`}>
                    {[1, 2, 3, 4, 5].map((l) => (
                      <span
                        key={l}
                        className={`size-2.5 rounded-full ${l <= c.level ? "bg-bg-brand-solid" : "bg-bg-quaternary"}`}
                      />
                    ))}
                  </span>
                </td>
              </tr>
            ))}
            {matrix.data && matrix.data.length === 0 && (
              <tr>
                <td colSpan={3} className="px-4 py-6 text-center text-sm text-text-tertiary">
                  Belum ada data skill.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
