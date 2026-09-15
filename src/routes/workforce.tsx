import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/workforce")({
  component: WorkforcePage,
});

function WorkforcePage() {
  const queryClient = useQueryClient();
  const { data: session } = useSession();
  const [year, setYear] = useState(String(new Date().getFullYear()));
  const [deptId, setDeptId] = useState("");
  const [planned, setPlanned] = useState("");
  const [budget, setBudget] = useState("");

  const view = useQuery({
    queryKey: ["workforce", year],
    queryFn: () => unwrap(commands.workforceOverview(Number(year) || 0)),
  });
  const drill = useQuery({
    queryKey: ["drilldown"],
    queryFn: () => unwrap(commands.organizationDrilldown()),
  });
  const depts = useQuery({
    queryKey: ["departments"],
    queryFn: () => unwrap(commands.orgList("departments", "", 1, 100)),
  });
  const savePlan = useMutation({
    mutationFn: () => unwrap(commands.workforcePlanSave(Number(year) || 0, Number(deptId), Number(planned))),
    onSuccess: () => {
      toast.success("Rencana disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["workforce"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const saveBudget = useMutation({
    mutationFn: () => unwrap(commands.workforceBudgetSave(Number(year) || 0, Number(deptId), Number(budget))),
    onSuccess: () => {
      toast.success("Budget disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["workforce"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const mayEdit = can(session, "organization.create", "system.manage");
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Workforce & Drill-down</h1>
      <input value={year} onChange={(e) => setYear(e.target.value)} className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {["Departemen", "Rencana", "Aktual", "Selisih", "Budget", "Biaya payroll"].map((h) => (
                <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(view.data ?? []).map((v) => (
              <tr key={v.department_id} className="border-b border-border-tertiary last:border-0">
                <td className="px-4 py-2.5 font-medium">{v.department_name}</td>
                <td className="px-4 py-2.5">{v.planned}</td>
                <td className="px-4 py-2.5">{v.actual}</td>
                <td className="px-4 py-2.5">{v.gap}</td>
                <td className="px-4 py-2.5">{(v.budget ?? 0).toLocaleString("id-ID")}</td>
                <td className="px-4 py-2.5">{(v.payroll_cost ?? 0).toLocaleString("id-ID")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {mayEdit && (
        <div className="flex flex-wrap gap-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
          <select value={deptId} onChange={(e) => setDeptId(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            <option value="">Pilih departemen…</option>
            {((depts.data as unknown as { rows?: { id: string; name?: string }[] })?.rows ?? []).map((d) => (
              <option key={d.id} value={d.id}>{d.name ?? d.id}</option>
            ))}
          </select>
          <input value={planned} onChange={(e) => setPlanned(e.target.value)} placeholder="Rencana headcount" className="w-44 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => savePlan.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Simpan rencana</button>
          <input value={budget} onChange={(e) => setBudget(e.target.value)} placeholder="Budget" className="w-44 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          <button type="button" onClick={() => saveBudget.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Simpan budget</button>
        </div>
      )}
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-4">
        <h2 className="text-sm font-semibold">Drill-down perusahaan → departemen → jabatan</h2>
        <div className="mt-2 space-y-2 text-sm">
          {(drill.data ?? []).map((c) => (
            <div key={c.id}>
              <p className="font-semibold">{c.label} ({c.headcount})</p>
              {c.children.map((d) => (
                <div key={d.id} className="ml-4">
                  <p>{d.label} ({d.headcount})</p>
                  <p className="ml-4 text-text-secondary">{d.children.map((p) => `${p.label} (${p.headcount})`).join(" · ")}</p>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
