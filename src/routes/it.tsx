import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/it")({
  component: ItPage,
});

function ItPage() {
  const queryClient = useQueryClient();
  const { data: session } = useSession();
  const [name, setName] = useState("");
  const [seats, setSeats] = useState("10");
  const [asgLic, setAsgLic] = useState("");
  const [asgEmp, setAsgEmp] = useState("");
  const [esopEmp, setEsopEmp] = useState("");
  const [esopShares, setEsopShares] = useState("1200");
  const [esopDate, setEsopDate] = useState("2026-01-01");

  const lics = useQuery({ queryKey: ["licenses"], queryFn: () => unwrap(commands.licenseList()) });
  const esops = useQuery({ queryKey: ["esops"], queryFn: () => unwrap(commands.compensationEsopList()) });
  const attr = useQuery({ queryKey: ["attrition"], queryFn: () => unwrap(commands.employeeAttrition()) });
  const vest = useMutation({
    mutationFn: () => unwrap(commands.compensationEsopVest(new Date().toISOString().slice(0, 10))),
    onSuccess: () => {
      toast.success("Vesting dijalankan.");
      void queryClient.invalidateQueries({ queryKey: ["esops"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const saveLic = useMutation({
    mutationFn: () =>
      unwrap(commands.licenseSave(null, { name, vendor: null, total_seats: Number(seats) || 1, cost: 0, expires_at: null })),
    onSuccess: () => {
      toast.success("Lisensi disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["licenses"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const assign = useMutation({
    mutationFn: () => unwrap(commands.licenseAssign(Number(asgLic), Number(asgEmp))),
    onSuccess: () => {
      toast.success("Lisensi ditugaskan.");
      void queryClient.invalidateQueries({ queryKey: ["licenses"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const grant = useMutation({
    mutationFn: () =>
      unwrap(
        commands.compensationEsopGrant({
          employee_id: Number(esopEmp),
          total_shares: Number(esopShares) || 0,
          grant_date: esopDate,
          vest_months: 48,
          cliff_months: 12,
        }),
      ),
    onSuccess: () => {
      toast.success("Grant ESOP disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["esops"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const mayEdit = can(session, "asset.create", "system.manage");
  const mayPay = can(session, "payroll.update", "system.manage");
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">IT, ESOP & Attrition</h1>
      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
          <h2 className="text-sm font-semibold">Lisensi software</h2>
          {(lics.data ?? []).map((l) => (
            <p key={l.id} className="text-sm">{l.name} · {l.used_seats}/{l.total_seats} kursi</p>
          ))}
          {mayEdit && (
            <div className="flex flex-wrap gap-2">
              <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Nama lisensi" className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <input value={seats} onChange={(e) => setSeats(e.target.value)} className="w-24 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <button type="button" onClick={() => saveLic.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Simpan</button>
              <input value={asgLic} onChange={(e) => setAsgLic(e.target.value)} placeholder="ID lisensi" className="w-28 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <input value={asgEmp} onChange={(e) => setAsgEmp(e.target.value)} placeholder="ID karyawan" className="w-28 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <button type="button" onClick={() => assign.mutate()} className="rounded-lg border border-border-primary px-4 py-2 text-sm">Tugaskan</button>
            </div>
          )}
        </div>
        <div className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
          <h2 className="text-sm font-semibold">ESOP</h2>
          {(esops.data ?? []).map((g) => (
            <p key={g.id} className="text-sm">{g.employee_name} · {g.vested_shares}/{g.total_shares} vested</p>
          ))}
          {mayPay && (
            <div className="flex flex-wrap gap-2">
              <input value={esopEmp} onChange={(e) => setEsopEmp(e.target.value)} placeholder="ID karyawan" className="w-28 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <input value={esopShares} onChange={(e) => setEsopShares(e.target.value)} className="w-28 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <input type="date" value={esopDate} onChange={(e) => setEsopDate(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <button type="button" onClick={() => grant.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Grant</button>
              <button type="button" onClick={() => vest.mutate()} className="rounded-lg border border-border-primary px-4 py-2 text-sm">Jalankan vesting</button>
            </div>
          )}
        </div>
      </div>
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-4">
        <h2 className="text-sm font-semibold">Risiko attrition tertinggi</h2>
        <div className="mt-2 space-y-1 text-sm">
          {(attr.data ?? []).slice(0, 20).map((a) => (
            <p key={a.employee_id}>{a.employee_name} · skor {(a.score ?? 0).toFixed(0)} ({a.level}) · {a.signals.join("; ") || "-"}</p>
          ))}
        </div>
      </div>
    </div>
  );
}
