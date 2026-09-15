import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/global")({
  component: GlobalPage,
});

function GlobalPage() {
  const queryClient = useQueryClient();
  const { data: session } = useSession();
  const [code, setCode] = useState("USD");
  const [rate, setRate] = useState("");
  const [asOf, setAsOf] = useState(new Date().toISOString().slice(0, 10));
  const [cName, setCName] = useState("");
  const [cCountry, setCCountry] = useState("MY");
  const [cCur, setCCur] = useState("USD");
  const [payCtr, setPayCtr] = useState("");
  const [payPeriod, setPayPeriod] = useState("");
  const [payAmount, setPayAmount] = useState("");
  const [q, setQ] = useState("");
  const [answer, setAnswer] = useState("");

  const rates = useQuery({ queryKey: ["fx"], queryFn: () => unwrap(commands.payrollCurrencyList()) });
  const ctrs = useQuery({ queryKey: ["contractors"], queryFn: () => unwrap(commands.contractorList()) });
  const pays = useQuery({ queryKey: ["contractorPays"], queryFn: () => unwrap(commands.contractorPayments(null)) });
  const saveFx = useMutation({
    mutationFn: () => unwrap(commands.payrollCurrencySave(code, Number(rate), asOf)),
    onSuccess: () => {
      toast.success("Kurs disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["fx"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const saveCtr = useMutation({
    mutationFn: () =>
      unwrap(commands.contractorSave(null, { name: cName, country: cCountry, currency: cCur, email: null, phone: null })),
    onSuccess: () => {
      toast.success("Kontraktor disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["contractors"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const pay = useMutation({
    mutationFn: () => unwrap(commands.contractorPay(Number(payCtr), payPeriod, Number(payAmount))),
    onSuccess: () => {
      toast.success("Pembayaran dicatat.");
      void queryClient.invalidateQueries({ queryKey: ["contractorPays"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const ask = useMutation({
    mutationFn: () => unwrap(commands.complianceAsk(q)),
    onSuccess: (a) => setAnswer(a),
    onError: (e: Error) => toast.error(e.message),
  });

  const mayEdit = can(session, "payroll.create", "system.manage");
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Global: Kurs, Kontraktor, FAQ Regulasi</h1>
      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
          <h2 className="text-sm font-semibold">Kurs ke IDR</h2>
          {(rates.data ?? []).map((r) => (
            <p key={`${r.code}-${r.as_of}`} className="text-sm">{r.code} = {(r.rate_to_idr ?? 0).toLocaleString("id-ID")} ({r.as_of})</p>
          ))}
          {mayEdit && (
            <div className="flex flex-wrap gap-2">
              <input value={code} onChange={(e) => setCode(e.target.value)} className="w-24 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <input value={rate} onChange={(e) => setRate(e.target.value)} placeholder="Kurs" className="w-36 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <input type="date" value={asOf} onChange={(e) => setAsOf(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
              <button type="button" onClick={() => saveFx.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Simpan</button>
            </div>
          )}
        </div>
        <div className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
          <h2 className="text-sm font-semibold">Tanya regulasi</h2>
          <div className="flex gap-2">
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="mis. tarif pph21?" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <button type="button" onClick={() => ask.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Tanya</button>
          </div>
          {answer && <p className="text-sm">{answer}</p>}
        </div>
      </div>
      <div className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
        <h2 className="text-sm font-semibold">Kontraktor lintas negara</h2>
        {(ctrs.data ?? []).map((c) => (
          <p key={c.id} className="text-sm">{c.name} · {c.country} · {c.currency}</p>
        ))}
        {mayEdit && (
          <div className="flex flex-wrap gap-2">
            <input value={cName} onChange={(e) => setCName(e.target.value)} placeholder="Nama" className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <input value={cCountry} onChange={(e) => setCCountry(e.target.value)} className="w-24 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <input value={cCur} onChange={(e) => setCCur(e.target.value)} className="w-24 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <button type="button" onClick={() => saveCtr.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Simpan</button>
          </div>
        )}
      </div>
      <div className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
        <h2 className="text-sm font-semibold">Pembayaran</h2>
        {(pays.data ?? []).slice(0, 20).map((p) => (
          <p key={p.id} className="text-sm">{p.contractor_name} · {p.period} · {(p.amount ?? 0).toLocaleString()} {p.currency} = Rp {(p.amount_idr ?? 0).toLocaleString("id-ID")} ({p.status})</p>
        ))}
        {mayEdit && (
          <div className="flex flex-wrap gap-2">
            <input value={payCtr} onChange={(e) => setPayCtr(e.target.value)} placeholder="ID kontraktor" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <input value={payPeriod} onChange={(e) => setPayPeriod(e.target.value)} placeholder="YYYY-MM" className="w-32 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <input value={payAmount} onChange={(e) => setPayAmount(e.target.value)} placeholder="Nominal" className="w-36 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
            <button type="button" onClick={() => pay.mutate()} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">Bayar</button>
          </div>
        )}
      </div>
    </div>
  );
}
