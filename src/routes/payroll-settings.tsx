import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type Component } from "../bindings";
import { formatRupiah } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/payroll-settings")({
  component: PayrollSettingsPage,
});

function PayrollSettingsPage() {
  const [tab, setTab] = useState("Komponen");
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Pengaturan Gaji</h1>
      <div className="flex gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
        {["Komponen", "Kasbon", "Kurs"].map((t) => (
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
      {tab === "Komponen" ? <ComponentTab /> : tab === "Kasbon" ? <DeductionTab /> : <FxTab />}
    </div>
  );
}

function ComponentTab() {
  const queryClient = useQueryClient();
  const list = useQuery({
    queryKey: ["payrollComponents"],
    queryFn: () => unwrap(commands.payrollComponents()),
  });
  const [editing, setEditing] = useState<Component | "new" | null>(null);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["payrollComponents"] });
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.payrollComponentDelete(id)),
    onSuccess: () => {
      toast.success("Komponen dihapus.");
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
          Tambah komponen
        </button>
      </div>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {["Kode", "Nama", "Jenis", "Perhitungan", "Pajak", "Aktif", "Aksi"].map((h) => (
                <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(list.data ?? []).map((c) => (
              <tr key={c.id} className="border-b border-border-tertiary last:border-0">
                <td className="px-4 py-2.5 font-medium">{c.code}</td>
                <td className="px-4 py-2.5">{c.name}</td>
                <td className="px-4 py-2.5">{c.component_type === "income" ? "Pendapatan" : "Potongan"}</td>
                <td className="px-4 py-2.5">{c.calculation_type}</td>
                <td className="px-4 py-2.5">{c.is_taxable ? "Ya" : "Tidak"}</td>
                <td className="px-4 py-2.5">{c.is_active ? "Ya" : "Tidak"}</td>
                <td className="px-4 py-2.5">
                  <span className="flex gap-3 text-[13px]">
                    <button type="button" onClick={() => setEditing(c)} className="font-medium text-text-brand-secondary hover:underline">
                      Ubah
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        if (window.confirm(`Hapus ${c.name}?`)) remove.mutate(c.id);
                      }}
                      className="font-medium text-text-error-primary hover:underline"
                    >
                      Hapus
                    </button>
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {editing && (
        <ComponentForm
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

function ComponentForm({ initial, onClose }: { initial: Component | null; onClose: () => void }) {
  const [code, setCode] = useState(initial?.code ?? "");
  const [name, setName] = useState(initial?.name ?? "");
  const [ctype, setCtype] = useState(initial?.component_type ?? "income");
  const [calc, setCalc] = useState(initial?.calculation_type ?? "fixed");
  const [taxable, setTaxable] = useState(initial?.is_taxable ?? false);
  const [active, setActive] = useState(initial?.is_active ?? true);
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.payrollComponentSave(initial?.id ?? null, {
          code,
          name,
          component_type: ctype,
          calculation_type: calc,
          is_taxable: taxable,
          is_active: active,
        }),
      ),
    onSuccess: () => {
      toast.success("Komponen disimpan.");
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
        <h2 className="text-display-xs font-semibold">{initial ? "Ubah" : "Tambah"} komponen</h2>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Kode</span>
            <input value={code} onChange={(e) => setCode(e.target.value)} required maxLength={30} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Nama</span>
            <input value={name} onChange={(e) => setName(e.target.value)} required maxLength={100} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Jenis</span>
            <select value={ctype} onChange={(e) => setCtype(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="income">Pendapatan</option>
              <option value="deduction">Potongan</option>
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Perhitungan</span>
            <select value={calc} onChange={(e) => setCalc(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="fixed">Nominal tetap</option>
              <option value="percentage">Persentase</option>
              <option value="formula">Formula sistem</option>
            </select>
          </label>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={taxable} onChange={(e) => setTaxable(e.target.checked)} className="size-4 accent-brand-600" />
          Kena pajak (masuk bruto PPh21)
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={active} onChange={(e) => setActive(e.target.checked)} className="size-4 accent-brand-600" />
          Aktif
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}

function DeductionTab() {
  const queryClient = useQueryClient();
  const [pendingOnly, setPendingOnly] = useState(true);
  const list = useQuery({
    queryKey: ["deductions", pendingOnly],
    queryFn: () => unwrap(commands.payrollDeductions(pendingOnly)),
  });
  const employees = useQuery({
    queryKey: ["employeeDropdowns"],
    queryFn: () => unwrap(commands.employeeDropdowns()),
  });
  const [open, setOpen] = useState(false);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["deductions"] });
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.payrollDeductionDelete(id)),
    onSuccess: () => {
      toast.success("Kasbon dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={pendingOnly} onChange={(e) => setPendingOnly(e.target.checked)} className="size-4 accent-brand-600" />
          Hanya pending
        </label>
        <button
          type="button"
          onClick={() => setOpen(true)}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Tambah kasbon
        </button>
      </div>
      <div className="space-y-2">
        {(list.data ?? []).map((d) => (
          <div key={d.id} className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border-secondary bg-bg-primary px-4 py-3">
            <div>
              <p className="text-sm font-medium">{d.employee_name} • {d.description}</p>
              <p className="text-xs text-text-tertiary">
                {d.deduction_type} • {formatRupiah(d.amount ?? 0)} • {d.status}
                {d.total_installments ? ` • cicilan ${d.installment_no ?? "-"} dari ${d.total_installments}` : ""}
              </p>
            </div>
            {d.status === "pending" && (
              <button
                type="button"
                onClick={() => {
                  if (window.confirm("Hapus kasbon ini?")) remove.mutate(d.id);
                }}
                className="text-[13px] font-medium text-text-error-primary hover:underline"
              >
                Hapus
              </button>
            )}
          </div>
        ))}
        {list.data && list.data.length === 0 && (
          <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
            Tidak ada kasbon.
          </p>
        )}
      </div>
      {open && (
        <DeductionForm
          employees={employees.data?.employees ?? []}
          onClose={() => {
            setOpen(false);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function DeductionForm({
  employees,
  onClose,
}: {
  employees: { id: number; name: string }[];
  onClose: () => void;
}) {
  const [employeeId, setEmployeeId] = useState("");
  const [dtype, setDtype] = useState("kasbon");
  const [description, setDescription] = useState("");
  const [amount, setAmount] = useState("");
  const [ino, setIno] = useState("");
  const [total, setTotal] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.payrollDeductionSave(null, {
          employee_id: Number(employeeId),
          deduction_type: dtype,
          description,
          amount: Number(amount),
          installment_no: ino === "" ? null : Number(ino),
          total_installments: total === "" ? null : Number(total),
        }),
      ),
    onSuccess: () => {
      toast.success("Kasbon disimpan. Terpotong otomatis saat generate berikutnya.");
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
        <h2 className="text-display-xs font-semibold">Tambah kasbon</h2>
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
            <select value={dtype} onChange={(e) => setDtype(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
              <option value="kasbon">Kasbon</option>
              <option value="loan">Pinjaman</option>
              <option value="other">Lainnya</option>
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Nominal (Rp)</span>
            <input type="number" min={1} value={amount} onChange={(e) => setAmount(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Keterangan</span>
          <input value={description} onChange={(e) => setDescription(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Cicilan ke (opsional)</span>
            <input type="number" min={1} value={ino} onChange={(e) => setIno(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Dari total (opsional)</span>
            <input type="number" min={1} value={total} onChange={(e) => setTotal(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
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

function FxTab() {
  const queryClient = useQueryClient();
  const [code, setCode] = useState("USD");
  const [rate, setRate] = useState("");
  const [asOf, setAsOf] = useState(new Date().toISOString().slice(0, 10));
  const list = useQuery({
    queryKey: ["fxRates"],
    queryFn: () => unwrap(commands.payrollCurrencyList()),
  });
  const save = useMutation({
    mutationFn: () => unwrap(commands.payrollCurrencySave(code, Number(rate), asOf)),
    onSuccess: () => {
      toast.success("Kurs disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["fxRates"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  return (
    <div className="space-y-3 rounded-xl border border-border-secondary bg-bg-primary p-4">
      {(list.data ?? []).map((r) => (
        <p key={`${r.code}-${r.as_of}`} className="text-sm">{r.code} = {(r.rate_to_idr ?? 0).toLocaleString("id-ID")} ({r.as_of})</p>
      ))}
      <div className="flex flex-wrap gap-2">
        <input value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} className="w-24 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        <input value={rate} onChange={(e) => setRate(e.target.value)} placeholder="Kurs ke IDR" className="w-44 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        <input type="date" value={asOf} onChange={(e) => setAsOf(e.target.value)} className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        <button type="button" onClick={() => save.mutate()} disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan kurs</button>
      </div>
    </div>
  );
}
