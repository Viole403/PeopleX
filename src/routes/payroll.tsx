import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type PayrollDetail } from "../bindings";
import { formatDate, formatRupiah } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/payroll")({
  component: PayrollPage,
});

function PayrollPage() {
  const queryClient = useQueryClient();
  const [periodId, setPeriodId] = useState<number | null>(null);
  const [creating, setCreating] = useState(false);
  const [detailId, setDetailId] = useState<number | null>(null);

  const periods = useQuery({
    queryKey: ["payrollPeriods"],
    queryFn: () => unwrap(commands.payrollPeriods()),
  });
  const selected = periods.data?.find((p) => p.id === periodId) ?? periods.data?.[0];
  const rows = useQuery({
    queryKey: ["payrollRows", selected?.id],
    queryFn: () => unwrap(commands.payrollRows(selected?.id ?? 0)),
    enabled: !!selected,
  });
  const slips = useQuery({
    queryKey: ["mySlips"],
    queryFn: () => unwrap(commands.payrollMySlips()),
  });

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["payrollPeriods"] });
    void queryClient.invalidateQueries({ queryKey: ["payrollRows"] });
    void queryClient.invalidateQueries({ queryKey: ["mySlips"] });
  };

  const flow = (fn: (id: number) => Promise<unknown>, label: string) => ({
    mutationFn: () => fn(selected?.id ?? 0),
    onSuccess: () => {
      toast.success(`${label} berhasil.`);
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const generate = useMutation(flow((id) => unwrap(commands.payrollGenerate(id)), "Generate"));
  const approve = useMutation(flow((id) => unwrap(commands.payrollApprove(id)), "Persetujuan"));
  const pay = useMutation(flow((id) => unwrap(commands.payrollPay(id)), "Tandai bayar"));
  const lock = useMutation(flow((id) => unwrap(commands.payrollLock(id)), "Kunci"));

  const openPdf = useMutation({
    mutationFn: (payrollId: number) => unwrap(commands.payrollPayslipFile(payrollId)),
    onSuccess: (f) => {
      const blob = new Blob([new Uint8Array(f.bytes)], { type: f.mime });
      window.open(URL.createObjectURL(blob), "_blank");
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Penggajian</h1>
        <button
          type="button"
          onClick={() => setCreating(true)}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Periode baru
        </button>
      </div>
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="space-y-2">
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
                {formatDate(p.start_date)} – {formatDate(p.end_date)} • {p.status} •{" "}
                {p.employee_count} orang
              </p>
            </button>
          ))}
          {periods.data && periods.data.length === 0 && (
            <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
              Belum ada periode.
            </p>
          )}
        </div>
        <div className="space-y-3 lg:col-span-2">
          {selected && (
            <>
              <div className="flex flex-wrap items-center gap-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
                <span className="text-sm font-semibold">{selected.name}</span>
                <span className="rounded-full bg-bg-brand-primary px-2 py-0.5 text-xs text-text-brand-primary">
                  {selected.status}
                </span>
                <span className="ml-auto flex flex-wrap gap-2">
                  {selected.status === "draft" && (
                    <FlowButton label="Generate" run={() => generate.mutate()} pending={generate.isPending} />
                  )}
                  {selected.status === "review" && (
                    <FlowButton label="Setujui" run={() => approve.mutate()} pending={approve.isPending} />
                  )}
                  {selected.status === "approved" && (
                    <FlowButton label="Tandai bayar" run={() => pay.mutate()} pending={pay.isPending} />
                  )}
                  {selected.status === "paid" && (
                    <FlowButton label="Kunci" run={() => lock.mutate()} pending={lock.isPending} />
                  )}
                </span>
              </div>
              <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
                <table className="w-full text-left text-sm">
                  <thead>
                    <tr className="border-b border-border-secondary text-xs text-text-tertiary">
                      {["Karyawan", "Pokok", "Pendapatan", "Potongan", "Bersih", "Aksi"].map((h) => (
                        <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {(rows.data ?? []).map((r) => (
                      <tr key={r.id} className="border-b border-border-tertiary last:border-0">
                        <td className="px-4 py-2.5">
                          <p className="font-medium">{r.name}</p>
                          <p className="text-xs text-text-tertiary">{r.employee_number}</p>
                        </td>
                        <td className="whitespace-nowrap px-4 py-2.5">{formatRupiah(r.basic_salary ?? 0)}</td>
                        <td className="whitespace-nowrap px-4 py-2.5">{formatRupiah(r.total_income ?? 0)}</td>
                        <td className="whitespace-nowrap px-4 py-2.5">{formatRupiah(r.total_deduction ?? 0)}</td>
                        <td className="whitespace-nowrap px-4 py-2.5 font-semibold">{formatRupiah(r.net_salary ?? 0)}</td>
                        <td className="px-4 py-2.5">
                          <span className="flex gap-3 text-[13px]">
                            <button type="button" onClick={() => setDetailId(r.id)} className="font-medium text-text-brand-secondary hover:underline">
                              Rincian
                            </button>
                            <button type="button" onClick={() => openPdf.mutate(r.id)} className="font-medium text-text-brand-secondary hover:underline">
                              Slip
                            </button>
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
          <div className="rounded-xl border border-border-secondary bg-bg-primary p-5">
            <h3 className="font-semibold">Slip gaji saya</h3>
            <ul className="mt-2 space-y-1 text-sm">
              {(slips.data ?? []).map((s) => (
                <li key={s.id} className="flex items-center justify-between border-t border-border-tertiary pt-1">
                  <span>{s.period_name} • {s.payslip_number}</span>
                  <span className="flex items-center gap-3">
                    <span className="font-medium">{formatRupiah(s.net_salary ?? 0)}</span>
                    {s.pdf_ready ? (
                      <span className="text-xs text-text-tertiary">PDF siap (minta HRD mengirimkan berkas)</span>
                    ) : (
                      <span className="text-xs text-text-tertiary">PDF belum dibuat</span>
                    )}
                  </span>
                </li>
              ))}
              {slips.data && slips.data.length === 0 && (
                <li className="text-sm text-text-tertiary">Belum ada slip.</li>
              )}
            </ul>
          </div>
        </div>
      </div>
      {creating && (
        <PeriodForm
          onClose={() => {
            setCreating(false);
            refresh();
          }}
        />
      )}
      {detailId !== null && (
        <DetailDialog id={detailId} onClose={() => setDetailId(null)} />
      )}
    </div>
  );
}

function FlowButton({ label, run, pending }: { label: string; run: () => void; pending: boolean }) {
  return (
    <button
      type="button"
      disabled={pending}
      onClick={() => {
        if (window.confirm(`${label} periode ini?`)) run();
      }}
      className="rounded-lg bg-bg-brand-solid px-3 py-1.5 text-[13px] font-semibold text-white disabled:opacity-60"
    >
      {label}
    </button>
  );
}

function PeriodForm({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [payment, setPayment] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.payrollPeriodCreate({
          name,
          start_date: start,
          end_date: end,
          payment_date: payment || null,
        }),
      ),
    onSuccess: () => {
      toast.success("Periode dibuat.");
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
          <input value={name} onChange={(e) => setName(e.target.value)} required maxLength={100} placeholder="Gaji Januari 2026" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
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
          <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal bayar (opsional)</span>
          <input type="date" value={payment} onChange={(e) => setPayment(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Buat</button>
        </div>
      </form>
    </div>
  );
}

function DetailDialog({ id, onClose }: { id: number; onClose: () => void }) {
  const detail = useQuery({
    queryKey: ["payrollDetail", id],
    queryFn: () => unwrap(commands.payrollDetail(id)),
  });
  const render = useMutation({
    mutationFn: () => unwrap(commands.payrollPayslipRender(id)),
    onSuccess: () => toast.success("PDF slip dibuat."),
    onError: (e: Error) => toast.error(e.message),
  });
  const d: PayrollDetail | null | undefined = detail.data ?? undefined;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <div className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-2xl bg-bg-primary p-6 shadow-2xl">
        {detail.isPending ? (
          <p className="text-sm text-text-tertiary">Memuat…</p>
        ) : !d ? (
          <p className="text-sm text-text-error-primary">Tidak ditemukan.</p>
        ) : (
          <>
            <div className="flex items-start justify-between">
              <div>
                <h2 className="text-display-xs font-semibold">{d.name}</h2>
                <p className="text-sm text-text-tertiary">
                  {d.employee_number} • {d.period_name} • {d.status}
                </p>
              </div>
              <button
                type="button"
                onClick={() => render.mutate()}
                className="rounded-lg border border-border-primary px-3 py-1.5 text-[13px] font-medium hover:bg-bg-primary_hover"
              >
                Buat PDF
              </button>
            </div>
            <div className="mt-4">
              <p className="text-xs font-semibold uppercase text-text-tertiary">Pendapatan</p>
              {d.lines.filter((l) => l.line_type === "income").map((l, i) => (
                <p key={i} className="flex justify-between py-1 text-sm">
                  <span>{l.component_name}</span>
                  <span className="font-medium">{formatRupiah(l.amount ?? 0)}</span>
                </p>
              ))}
              <p className="flex justify-between border-t border-border-secondary py-1 text-sm font-semibold">
                <span>Total</span>
                <span>{formatRupiah(d.total_income ?? 0)}</span>
              </p>
            </div>
            <div className="mt-3">
              <p className="text-xs font-semibold uppercase text-text-tertiary">Potongan</p>
              {d.lines.filter((l) => l.line_type === "deduction").map((l, i) => (
                <p key={i} className="flex justify-between py-1 text-sm">
                  <span>{l.component_name}</span>
                  <span className="font-medium">{formatRupiah(l.amount ?? 0)}</span>
                </p>
              ))}
              <p className="flex justify-between border-t border-border-secondary py-1 text-sm font-semibold">
                <span>Total</span>
                <span>{formatRupiah(d.total_deduction ?? 0)}</span>
              </p>
            </div>
            <p className="mt-3 flex justify-between rounded-lg bg-bg-brand-primary px-3 py-2 font-semibold text-text-brand-primary">
              <span>Gaji bersih</span>
              <span>{formatRupiah(d.net_salary ?? 0)}</span>
            </p>
            <p className="mt-2 text-xs text-text-tertiary">
              PPh 21 pada slip adalah estimasi, bukan perhitungan pajak resmi.
            </p>
          </>
        )}
        <div className="mt-4 flex justify-end">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">
            Tutup
          </button>
        </div>
      </div>
    </div>
  );
}
