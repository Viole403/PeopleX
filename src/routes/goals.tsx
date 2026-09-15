import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/goals")({
  component: GoalsPage,
});

function GoalsPage() {
  const queryClient = useQueryClient();
  const { data: session } = useSession();
  const [periodId, setPeriodId] = useState<number | null>(null);
  const [title, setTitle] = useState("");
  const [level, setLevel] = useState("company");
  const [parentId, setParentId] = useState<number | null>(null);
  const [fbEmp, setFbEmp] = useState("");
  const [fbRev, setFbRev] = useState("");
  const [fbRel, setFbRel] = useState("peer");
  const [fbScore, setFbScore] = useState("");
  const [calEmp, setCalEmp] = useState("");
  const [calScore, setCalScore] = useState("");
  const [sucPos, setSucPos] = useState("");
  const [sucEmp, setSucEmp] = useState("");
  const [sucReady, setSucReady] = useState("developing");

  const periods = useQuery({
    queryKey: ["perfPeriods"],
    queryFn: () => unwrap(commands.performancePeriods()),
  });
  const selected = periods.data?.find((p) => p.id === periodId) ?? periods.data?.[0];
  const tree = useQuery({
    queryKey: ["goalTree", selected?.id],
    queryFn: () => unwrap(commands.performanceGoalTree(selected?.id ?? 0)),
    enabled: !!selected,
  });
  const sucs = useQuery({
    queryKey: ["succession"],
    queryFn: () => unwrap(commands.performanceSuccessionList()),
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["goalTree"] });
    void queryClient.invalidateQueries({ queryKey: ["succession"] });
  };
  const saveGoal = useMutation({
    mutationFn: () =>
      unwrap(
        commands.performanceGoalSave(selected?.id ?? 0, null, {
          parent_id: parentId,
          level,
          title,
          owner_employee_id: null,
          department_id: null,
          target: 100,
          weight: 100,
        }),
      ),
    onSuccess: () => {
      toast.success("Goal disimpan.");
      setTitle("");
      setParentId(null);
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const saveFb = useMutation({
    mutationFn: () =>
      unwrap(
        commands.performanceFb360Save(
          selected?.id ?? 0,
          Number(fbEmp),
          Number(fbRev),
          fbRel,
          Number(fbScore),
          null,
        ),
      ),
    onSuccess: () => {
      toast.success("Feedback disimpan.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const calibrate = useMutation({
    mutationFn: () =>
      unwrap(commands.performanceCalibrate(selected?.id ?? 0, Number(calEmp), Number(calScore), null)),
    onSuccess: () => toast.success("Kalibrasi diterapkan."),
    onError: (e: Error) => toast.error(e.message),
  });
  const saveSuc = useMutation({
    mutationFn: () =>
      unwrap(
        commands.performanceSuccessionSave({
          position_id: Number(sucPos),
          successor_employee_id: Number(sucEmp),
          readiness: sucReady,
          notes: null,
        }),
      ),
    onSuccess: () => {
      toast.success("Succession disimpan.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const mayEdit = can(session, "performance.create", "system.manage");
  const mayReview = can(session, "performance.review", "system.manage");
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">OKR, 360, Kalibrasi, Succession</h1>
      <select
        value={selected?.id ?? ""}
        onChange={(e) => setPeriodId(e.target.value ? Number(e.target.value) : null)}
        className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
      >
        <option value="">Pilih periode…</option>
        {(periods.data ?? []).map((p) => (
          <option key={p.id} value={p.id}>{p.name}</option>
        ))}
      </select>

      <div className="rounded-xl border border-border-secondary bg-bg-primary p-4">
        <h2 className="text-sm font-semibold">Pohon goal</h2>
        <div className="mt-2 space-y-1 text-sm">
          {(tree.data ?? []).map((g) => (
            <p key={g.id} className={g.level === "company" ? "font-semibold" : g.level === "department" ? "ml-4" : "ml-8"}>
              {g.title} <span className="text-text-tertiary">({g.level}{g.actual !== null ? ` · aktual ${g.actual}` : ""})</span>
            </p>
          ))}
          {(tree.data ?? []).length === 0 && <p className="text-text-tertiary">Belum ada goal.</p>}
        </div>
        {mayEdit && (
          <div className="mt-3 flex flex-wrap gap-2">
            <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Judul goal" className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <select value={level} onChange={(e) => setLevel(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="company">Perusahaan</option>
              <option value="department">Departemen</option>
              <option value="individual">Individu</option>
            </select>
            <input value={parentId ?? ""} onChange={(e) => setParentId(e.target.value ? Number(e.target.value) : null)} placeholder="ID induk (opsional)" className="w-40 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <button type="button" onClick={() => saveGoal.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Simpan goal</button>
          </div>
        )}
      </div>

      {mayReview && (
        <div className="grid gap-4 md:grid-cols-2">
          <div className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
            <h2 className="text-sm font-semibold">Feedback 360</h2>
            <div className="flex flex-wrap gap-2">
              <input value={fbEmp} onChange={(e) => setFbEmp(e.target.value)} placeholder="ID karyawan" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <input value={fbRev} onChange={(e) => setFbRev(e.target.value)} placeholder="ID reviewer" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <select value={fbRel} onChange={(e) => setFbRel(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
                <option value="manager">Atasan</option>
                <option value="peer">Rekan</option>
                <option value="subordinate">Bawahan</option>
                <option value="self">Diri</option>
              </select>
              <input value={fbScore} onChange={(e) => setFbScore(e.target.value)} placeholder="Skor 0-100" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <button type="button" onClick={() => saveFb.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Simpan</button>
            </div>
          </div>
          <div className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
            <h2 className="text-sm font-semibold">Kalibrasi</h2>
            <div className="flex flex-wrap gap-2">
              <input value={calEmp} onChange={(e) => setCalEmp(e.target.value)} placeholder="ID karyawan" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <input value={calScore} onChange={(e) => setCalScore(e.target.value)} placeholder="Skor final" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <button type="button" onClick={() => calibrate.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Terapkan</button>
            </div>
          </div>
        </div>
      )}

      <div className="rounded-xl border border-border-secondary bg-bg-primary p-4">
        <h2 className="text-sm font-semibold">Succession</h2>
        <div className="mt-2 space-y-1 text-sm">
          {(sucs.data ?? []).map((s) => (
            <p key={s.id}>{s.position_name ?? `Posisi ${s.position_id}`} → {s.successor_name} ({s.readiness})</p>
          ))}
        </div>
        {mayEdit && (
          <div className="mt-3 flex flex-wrap gap-2">
            <input value={sucPos} onChange={(e) => setSucPos(e.target.value)} placeholder="ID posisi" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <input value={sucEmp} onChange={(e) => setSucEmp(e.target.value)} placeholder="ID pengganti" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <select value={sucReady} onChange={(e) => setSucReady(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="ready">Siap</option>
              <option value="developing">Dibina</option>
              <option value="not_ready">Belum siap</option>
            </select>
            <button type="button" onClick={() => saveSuc.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Simpan</button>
          </div>
        )}
      </div>
    </div>
  );
}
