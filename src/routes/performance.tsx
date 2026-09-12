import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/performance")({
  component: PerformancePage,
});

const PERIOD_LABEL: Record<string, string> = {
  monthly: "Bulanan",
  quarterly: "Kuartalan",
  semester: "Semester",
  annual: "Tahunan",
};
const ROLE_LABEL: Record<string, string> = {
  self: "Diri",
  supervisor: "Supervisor",
  manager: "Manajer",
  hr: "HR",
};

function PerformancePage() {
  const queryClient = useQueryClient();
  const { data: session } = useSession();
  const [periodId, setPeriodId] = useState<number | null>(null);
  const [reviewId, setReviewId] = useState<number | null>(null);
  const [periodForm, setPeriodForm] = useState(false);
  const [kpiForm, setKpiForm] = useState(false);

  const periods = useQuery({
    queryKey: ["perfPeriods"],
    queryFn: () => unwrap(commands.performancePeriods()),
  });
  const kpis = useQuery({
    queryKey: ["perfKpis"],
    queryFn: () => unwrap(commands.performanceKpis()),
  });
  const selected = periods.data?.find((p) => p.id === periodId) ?? periods.data?.[0];
  const reviews = useQuery({
    queryKey: ["perfReviews", selected?.id],
    queryFn: () => unwrap(commands.performanceReviews(selected?.id ?? 0)),
    enabled: !!selected,
  });
  const mine = useQuery({
    queryKey: ["perfMine"],
    queryFn: () => unwrap(commands.performanceMyReviews()),
  });

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["perfPeriods"] });
    void queryClient.invalidateQueries({ queryKey: ["perfKpis"] });
    void queryClient.invalidateQueries({ queryKey: ["perfReviews"] });
    void queryClient.invalidateQueries({ queryKey: ["perfMine"] });
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Kinerja</h1>
        {can(session, "performance.create") && (
          <span className="flex gap-2">
            <button
              type="button"
              onClick={() => setPeriodForm(true)}
              className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover"
            >
              Periode baru
            </button>
            <button
              type="button"
              onClick={() => setKpiForm(true)}
              className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
            >
              KPI baru
            </button>
          </span>
        )}
      </div>
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="space-y-2">
          <p className="text-xs font-semibold uppercase text-text-tertiary">Periode</p>
          {(periods.data ?? []).map((p) => (
            <button
              key={p.id}
              type="button"
              onClick={() => setPeriodId(p.id)}
              className={`w-full rounded-xl border px-4 py-3 text-left transition ${
                selected?.id === p.id
                  ? "border-border-brand bg-bg-brand-primary"
                  : "border-border-secondary bg-bg-primary hover:bg-bg-primary_hover"
              }`}
            >
              <p className="text-sm font-semibold">{p.name}</p>
              <p className="text-xs text-text-tertiary">
                {PERIOD_LABEL[p.period_type] ?? p.period_type} • {p.status}
              </p>
            </button>
          ))}
          <p className="text-xs font-semibold uppercase text-text-tertiary">Katalog KPI</p>
          <div className="rounded-xl border border-border-secondary bg-bg-primary p-3">
            {(kpis.data ?? []).map((k) => (
              <p key={k.id} className="py-0.5 text-sm">
                {k.name}
                {k.department_name && (
                  <span className="text-text-tertiary"> • {k.department_name}</span>
                )}
              </p>
            ))}
            {(kpis.data ?? []).length === 0 && (
              <p className="text-sm text-text-tertiary">Belum ada KPI.</p>
            )}
          </div>
        </div>
        <div className="space-y-2 lg:col-span-2">
          <p className="text-xs font-semibold uppercase text-text-tertiary">
            Review {selected ? `• ${selected.name}` : ""}
          </p>
          {(reviews.data ?? []).map((r) => (
            <button
              key={r.id}
              type="button"
              onClick={() => setReviewId(r.id)}
              className="flex w-full flex-wrap items-center justify-between gap-2 rounded-xl border border-border-secondary bg-bg-primary px-4 py-3 text-left hover:bg-bg-primary_hover"
            >
              <span>
                <span className="block text-sm font-medium">{r.employee_name}</span>
                <span className="text-xs text-text-tertiary">
                  {r.employee_number} • {r.status}
                </span>
              </span>
              <span className="text-sm font-semibold">
                {r.final_score ?? "-"}
                {r.final_rating ? ` (R${r.final_rating})` : ""}
              </span>
            </button>
          ))}
          {reviews.data && reviews.data.length === 0 && (
            <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
              Belum ada review. Tugaskan KPI ke karyawan untuk memulai.
            </p>
          )}
          <p className="pt-2 text-xs font-semibold uppercase text-text-tertiary">Review saya</p>
          {(mine.data ?? []).map((r) => (
            <button
              key={r.id}
              type="button"
              onClick={() => setReviewId(r.id)}
              className="flex w-full items-center justify-between rounded-xl border border-border-secondary bg-bg-primary px-4 py-3 text-left text-sm hover:bg-bg-primary_hover"
            >
              <span className="font-medium">{r.period_name}</span>
              <span className="font-semibold">
                {r.final_score ?? "-"}
                {r.final_rating ? ` (R${r.final_rating})` : ""}
              </span>
            </button>
          ))}
        </div>
      </div>
      {periodForm && (
        <PeriodForm
          onClose={() => {
            setPeriodForm(false);
            refresh();
          }}
        />
      )}
      {kpiForm && (
        <KpiForm
          onClose={() => {
            setKpiForm(false);
            refresh();
          }}
        />
      )}
      {reviewId !== null && (
        <ReviewDialog
          id={reviewId}
          onClose={() => {
            setReviewId(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function PeriodForm({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("");
  const [type, setType] = useState("quarterly");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.performancePeriodSave(null, {
          name,
          period_type: type,
          start_date: start,
          end_date: end,
          status: "open",
        }),
      ),
    onSuccess: () => {
      toast.success("Periode disimpan.");
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
        <h2 className="text-display-xs font-semibold">Periode baru</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Nama</span>
          <input value={name} onChange={(e) => setName(e.target.value)} required maxLength={100} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Tipe</span>
          <select value={type} onChange={(e) => setType(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            {Object.entries(PERIOD_LABEL).map(([v, l]) => (
              <option key={v} value={v}>{l}</option>
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
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}

function KpiForm({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.performanceKpiSave(null, {
          name,
          description: description || null,
          department_id: null,
        }),
      ),
    onSuccess: () => {
      toast.success("KPI disimpan.");
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
        <h2 className="text-display-xs font-semibold">KPI baru</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Nama</span>
          <input value={name} onChange={(e) => setName(e.target.value)} required maxLength={150} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
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

function ReviewDialog({ id, onClose }: { id: number; onClose: () => void }) {
  const queryClient = useQueryClient();
  const detail = useQuery({
    queryKey: ["perfReview", id],
    queryFn: () => unwrap(commands.performanceReviewDetail(id)),
  });
  const kpis = useQuery({
    queryKey: ["perfKpis"],
    queryFn: () => unwrap(commands.performanceKpis()),
  });
  const [actual, setActual] = useState<Record<number, string>>({});
  const [scores, setScores] = useState<Record<string, string>>({});
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["perfReview", id] });
    void queryClient.invalidateQueries({ queryKey: ["perfReviews"] });
    void queryClient.invalidateQueries({ queryKey: ["perfMine"] });
  };
  const fail = (e: Error) => toast.error(e.message);

  const d = detail.data ?? undefined;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <div className="max-h-[88vh] w-full max-w-2xl overflow-y-auto rounded-2xl bg-bg-primary p-6 shadow-2xl">
        {detail.isPending ? (
          <p className="text-sm text-text-tertiary">Memuat…</p>
        ) : !d ? (
          <p className="text-sm text-text-error-primary">Tidak ditemukan.</p>
        ) : (
          <div className="space-y-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h2 className="text-display-xs font-semibold">{d.employee_name}</h2>
                <p className="text-sm text-text-tertiary">
                  {d.period_name} • {d.status} • final {d.final_score ?? "-"}
                  {d.final_rating ? ` (R${d.final_rating})` : ""}
                </p>
              </div>
              <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm">Tutup</button>
            </div>
            <AssignKpiForm
              employeeId={d.employee_id}
              kpis={kpis.data ?? []}
              onDone={refresh}
            />
            <div className="rounded-xl border border-border-secondary p-4">
              <h3 className="mb-2 text-sm font-semibold">KPI & realisasi</h3>
              {d.kpis.map((k) => (
                <form
                  key={k.id}
                  className="flex flex-wrap items-center gap-2 border-t border-border-tertiary py-2 text-sm"
                  onSubmit={(e) => {
                    e.preventDefault();
                    const v = actual[k.id];
                    if (v === undefined || v === "") return;
                    unwrap(commands.performanceSubmitActual(k.id, Number(v)))
                      .then(() => {
                        toast.success(`Skor ${k.kpi_name}: ${k.score ?? ""}`);
                        refresh();
                      })
                      .catch(fail);
                  }}
                >
                  <span className="min-w-0 flex-1 font-medium">
                    {k.kpi_name}
                    <span className="text-text-tertiary"> • target {k.target} • bobot {k.weight}</span>
                  </span>
                  <span className="font-semibold">{k.score ?? "-"}</span>
                  <input
                    type="number"
                    value={actual[k.id] ?? ""}
                    onChange={(e) => setActual({ ...actual, [k.id]: e.target.value })}
                    placeholder="Aktual"
                    className="w-28 rounded-lg border border-border-primary bg-bg-primary px-2 py-1.5 text-sm"
                  />
                  <button type="submit" className="rounded-lg border border-border-primary px-3 py-1.5 text-[13px] font-medium">
                    Simpan
                  </button>
                </form>
              ))}
              {d.kpis.length === 0 && (
                <p className="text-sm text-text-tertiary">Belum ada KPI ditugaskan.</p>
              )}
            </div>
            <div className="rounded-xl border border-border-secondary p-4">
              <h3 className="mb-2 text-sm font-semibold">Penilaian peran (0–100)</h3>
              {Object.entries(ROLE_LABEL).map(([role, label]) => (
                <form
                  key={role}
                  className="flex flex-wrap items-center gap-2 border-t border-border-tertiary py-2 text-sm"
                  onSubmit={(e) => {
                    e.preventDefault();
                    const v = scores[role];
                    if (v === undefined || v === "") return;
                    unwrap(commands.performanceSubmitReview(d.id, role, Number(v), null))
                      .then(() => {
                        toast.success(`Nilai ${label} disimpan.`);
                        refresh();
                      })
                      .catch(fail);
                  }}
                >
                  <span className="w-24 font-medium">{label}</span>
                  <input
                    type="number"
                    min={0}
                    max={100}
                    value={scores[role] ?? ""}
                    onChange={(e) => setScores({ ...scores, [role]: e.target.value })}
                    className="w-28 rounded-lg border border-border-primary bg-bg-primary px-2 py-1.5 text-sm"
                  />
                  <button type="submit" className="rounded-lg border border-border-primary px-3 py-1.5 text-[13px] font-medium">
                    Simpan
                  </button>
                </form>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function AssignKpiForm({
  employeeId,
  kpis,
  onDone,
}: {
  employeeId: number;
  kpis: { id: number; name: string }[];
  onDone: () => void;
}) {
  const [kpi, setKpi] = useState("");
  const [target, setTarget] = useState("");
  const [weight, setWeight] = useState("");
  const [periodId, setPeriodId] = useState<number | null>(null);
  const save = useMutation({
    mutationFn: () =>
      unwrap(commands.performanceAssignKpi(periodId ?? 0, employeeId, Number(kpi), Number(target), Number(weight))),
    onSuccess: () => {
      toast.success("KPI ditugaskan.");
      onDone();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  return (
    <form
      className="flex flex-wrap items-end gap-2 rounded-xl border border-border-secondary p-4"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <span className="w-full text-sm font-semibold">Tugaskan KPI</span>
      <label className="block min-w-0 flex-1">
        <span className="mb-1 block text-xs text-text-tertiary">KPI</span>
        <select value={kpi} onChange={(e) => setKpi(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-2 py-1.5 text-sm">
          <option value="">Pilih…</option>
          {kpis.map((k) => (
            <option key={k.id} value={k.id}>{k.name}</option>
          ))}
        </select>
      </label>
      <label className="block">
        <span className="mb-1 block text-xs text-text-tertiary">Target</span>
        <input type="number" value={target} onChange={(e) => setTarget(e.target.value)} required className="w-24 rounded-lg border border-border-primary bg-bg-primary px-2 py-1.5 text-sm" />
      </label>
      <label className="block">
        <span className="mb-1 block text-xs text-text-tertiary">Bobot</span>
        <input type="number" min={0} max={100} value={weight} onChange={(e) => setWeight(e.target.value)} required className="w-24 rounded-lg border border-border-primary bg-bg-primary px-2 py-1.5 text-sm" />
      </label>
      <PeriodPicker onPick={setPeriodId} />
      <button type="submit" disabled={save.isPending || periodId === null} className="rounded-lg bg-bg-brand-solid px-3 py-1.5 text-sm font-semibold text-white disabled:opacity-60">
        Tugaskan
      </button>
    </form>
  );
}

function PeriodPicker({ onPick }: { onPick: (id: number | null) => void }) {
  const periods = useQuery({
    queryKey: ["perfPeriods"],
    queryFn: () => unwrap(commands.performancePeriods()),
  });
  return (
    <label className="block">
      <span className="mb-1 block text-xs text-text-tertiary">Periode</span>
      <select
        onChange={(e) => onPick(e.target.value === "" ? null : Number(e.target.value))}
        className="rounded-lg border border-border-primary bg-bg-primary px-2 py-1.5 text-sm"
        defaultValue=""
      >
        <option value="">Pilih…</option>
        {(periods.data ?? []).map((p) => (
          <option key={p.id} value={p.id}>{p.name}</option>
        ))}
      </select>
    </label>
  );
}
