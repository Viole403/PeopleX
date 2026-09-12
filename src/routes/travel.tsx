import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type Reimburse, type Trip } from "../bindings";
import { formatDate, formatRupiah } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/travel")({
  component: TravelPage,
});

const STATUS_LABEL: Record<string, string> = {
  pending: "Menunggu",
  approved: "Disetujui",
  rejected: "Ditolak",
  completed: "Selesai",
  settled: "Selesai",
  paid: "Dibayar",
};

type Tab = "dinas" | "dinas-appr" | "reimburse" | "reimburse-appr" | "kategori";

function TravelPage() {
  const [tab, setTab] = useState<Tab>("dinas");
  const tabs: [Tab, string][] = [
    ["dinas", "Dinas Saya"],
    ["dinas-appr", "Persetujuan Dinas"],
    ["reimburse", "Reimburse Saya"],
    ["reimburse-appr", "Persetujuan Reimburse"],
    ["kategori", "Kategori Reimburse"],
  ];
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Dinas & Reimburse</h1>
      <div className="flex flex-wrap gap-2">
        {tabs.map(([v, l]) => (
          <button
            key={v}
            type="button"
            onClick={() => setTab(v)}
            className={`rounded-lg px-4 py-2 text-sm font-medium ${tab === v ? "bg-bg-brand-solid text-white" : "border border-border-primary hover:bg-bg-primary_hover"}`}
          >
            {l}
          </button>
        ))}
      </div>
      {tab === "dinas" && <MyTrips />}
      {tab === "dinas-appr" && <PendingTrips />}
      {tab === "reimburse" && <MyReimburse />}
      {tab === "reimburse-appr" && <PendingReimburse />}
      {tab === "kategori" && <CategoryList />}
    </div>
  );
}

function statusBadge(s: string) {
  return (
    <span className="rounded-full bg-bg-secondary px-2 py-0.5 text-xs font-medium">
      {STATUS_LABEL[s] ?? s}
    </span>
  );
}

function MyTrips() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [detail, setDetail] = useState<Trip | null>(null);
  const list = useQuery({ queryKey: ["trips", "my"], queryFn: () => unwrap(commands.tripMy()) });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["trips"] });
  };
  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button type="button" onClick={() => setOpen(true)} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">
          Ajukan dinas
        </button>
      </div>
      <div className="overflow-x-auto rounded-xl border border-border-primary">
        <table className="w-full text-left text-sm">
          <thead className="bg-bg-secondary text-text-secondary">
            <tr>
              <th className="px-4 py-2">Tujuan</th>
              <th className="px-4 py-2">Tanggal</th>
              <th className="px-4 py-2">Anggaran</th>
              <th className="px-4 py-2">Terpakai</th>
              <th className="px-4 py-2">Status</th>
            </tr>
          </thead>
          <tbody>
            {(list.data ?? []).map((t) => (
              <tr key={t.id} className="cursor-pointer border-t border-border-secondary hover:bg-bg-primary_hover" onClick={() => setDetail(t)}>
                <td className="px-4 py-2 font-medium">{t.destination}</td>
                <td className="px-4 py-2">{formatDate(t.start_date)} – {formatDate(t.end_date)}</td>
                <td className="px-4 py-2">{t.budget == null ? "—" : formatRupiah(t.budget)}</td>
                <td className="px-4 py-2">{formatRupiah(t.expense_total ?? 0)}</td>
                <td className="px-4 py-2">{statusBadge(t.status)}</td>
              </tr>
            ))}
            {(list.data ?? []).length === 0 && (
              <tr><td colSpan={5} className="px-4 py-6 text-center text-text-tertiary">Belum ada perjalanan dinas.</td></tr>
            )}
          </tbody>
        </table>
      </div>
      {open && <TripForm onClose={() => { setOpen(false); refresh(); }} />}
      {detail && <TripDetail trip={detail} onClose={() => { setDetail(null); refresh(); }} />}
    </div>
  );
}

function TripForm({ onClose }: { onClose: () => void }) {
  const [destination, setDestination] = useState("");
  const [purpose, setPurpose] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [transport, setTransport] = useState("");
  const [hotel, setHotel] = useState("");
  const [budget, setBudget] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(commands.tripCreate({
        destination,
        purpose,
        start_date: start,
        end_date: end,
        transportation: transport || null,
        hotel: hotel || null,
        budget: budget ? Number(budget) : null,
      })),
    onSuccess: () => { toast.success("Pengajuan dinas dikirim."); onClose(); },
    onError: (e: Error) => toast.error(e.message),
  });
  const input = "w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm";
  const label = "mb-1 block text-sm font-medium text-text-secondary";
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <form
        className="w-full max-w-md space-y-4 rounded-2xl bg-bg-primary p-6 shadow-2xl"
        onSubmit={(e) => { e.preventDefault(); save.mutate(); }}
      >
        <h2 className="text-display-xs font-semibold">Pengajuan dinas</h2>
        <label className="block"><span className={label}>Tujuan</span>
          <input value={destination} onChange={(e) => setDestination(e.target.value)} required className={input} /></label>
        <label className="block"><span className={label}>Keperluan</span>
          <textarea value={purpose} onChange={(e) => setPurpose(e.target.value)} required rows={2} className={input} /></label>
        <div className="grid grid-cols-2 gap-3">
          <label className="block"><span className={label}>Berangkat</span>
            <input type="date" value={start} onChange={(e) => setStart(e.target.value)} required className={input} /></label>
          <label className="block"><span className={label}>Pulang</span>
            <input type="date" value={end} onChange={(e) => setEnd(e.target.value)} required className={input} /></label>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <label className="block"><span className={label}>Transportasi</span>
            <input value={transport} onChange={(e) => setTransport(e.target.value)} className={input} /></label>
          <label className="block"><span className={label}>Hotel</span>
            <input value={hotel} onChange={(e) => setHotel(e.target.value)} className={input} /></label>
        </div>
        <label className="block"><span className={label}>Anggaran (Rp)</span>
          <input type="number" min={0} value={budget} onChange={(e) => setBudget(e.target.value)} className={input} /></label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Kirim</button>
        </div>
      </form>
    </div>
  );
}

function TripDetail({ trip, onClose }: { trip: Trip; onClose: () => void }) {
  const queryClient = useQueryClient();
  const expenses = useQuery({
    queryKey: ["trips", "expenses", trip.id],
    queryFn: () => unwrap(commands.tripExpenses(trip.id)),
  });
  const [category, setCategory] = useState("");
  const [desc, setDesc] = useState("");
  const [amount, setAmount] = useState("");
  const [receipt, setReceipt] = useState<File | null>(null);
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["trips"] });
    void queryClient.invalidateQueries({ queryKey: ["trips", "expenses", trip.id] });
  };
  const addExpense = useMutation({
    mutationFn: async () => {
      let file: { name: string; mime: string; bytes: number[] } | null = null;
      if (receipt) {
        const buf = new Uint8Array(await receipt.arrayBuffer());
        file = { name: receipt.name, mime: receipt.type || "application/octet-stream", bytes: [...buf] };
      }
      return unwrap(commands.tripExpenseAdd(trip.id, {
        category,
        description: desc || null,
        amount: amount ? Number(amount) : null,
      }, file));
    },
    onSuccess: () => {
      toast.success("Biaya ditambahkan.");
      setCategory(""); setDesc(""); setAmount(""); setReceipt(null);
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const settle = useMutation({
    mutationFn: () => unwrap(commands.tripSettle(trip.id)),
    onSuccess: () => { toast.success("Dinas diselesaikan."); onClose(); },
    onError: (e: Error) => toast.error(e.message),
  });
  const input = "w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm";
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <div className="max-h-[90vh] w-full max-w-lg space-y-4 overflow-y-auto rounded-2xl bg-bg-primary p-6 shadow-2xl">
        <div className="flex items-start justify-between">
          <div>
            <h2 className="text-display-xs font-semibold">{trip.destination}</h2>
            <p className="text-sm text-text-secondary">{trip.purpose}</p>
            <p className="mt-1 text-sm text-text-secondary">
              {formatDate(trip.start_date)} – {formatDate(trip.end_date)} • {statusBadge(trip.status)}
            </p>
          </div>
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm">Tutup</button>
        </div>
        <div>
          <h3 className="mb-2 text-sm font-semibold">Rincian biaya</h3>
          <div className="overflow-x-auto rounded-xl border border-border-primary">
            <table className="w-full text-left text-sm">
              <thead className="bg-bg-secondary text-text-secondary">
                <tr><th className="px-3 py-2">Kategori</th><th className="px-3 py-2">Nominal</th></tr>
              </thead>
              <tbody>
                {(expenses.data ?? []).map((x) => (
                  <tr key={x.id} className="border-t border-border-secondary">
                    <td className="px-3 py-2">{x.category}{x.description ? ` • ${x.description}` : ""}</td>
                    <td className="px-3 py-2">{x.amount == null ? "—" : formatRupiah(x.amount)}</td>
                  </tr>
                ))}
                {(expenses.data ?? []).length === 0 && (
                  <tr><td colSpan={2} className="px-3 py-4 text-center text-text-tertiary">Belum ada biaya.</td></tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
        {trip.status === "approved" && (
          <form
            className="space-y-3 rounded-xl border border-border-primary p-4"
            onSubmit={(e) => { e.preventDefault(); addExpense.mutate(); }}
          >
            <h3 className="text-sm font-semibold">Tambah biaya</h3>
            <div className="grid grid-cols-2 gap-3">
              <input value={category} onChange={(e) => setCategory(e.target.value)} required placeholder="Kategori" className={input} />
              <input type="number" min={0} value={amount} onChange={(e) => setAmount(e.target.value)} placeholder="Nominal (Rp)" className={input} />
            </div>
            <input value={desc} onChange={(e) => setDesc(e.target.value)} placeholder="Keterangan" className={input} />
            <input type="file" onChange={(e) => setReceipt(e.target.files?.[0] ?? null)} className="text-sm" />
            <div className="flex justify-end gap-2">
              <button type="submit" disabled={addExpense.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Tambah</button>
              <button type="button" disabled={settle.isPending} onClick={() => settle.mutate()} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Selesaikan dinas</button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}

function PendingTrips() {
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["trips", "pending"], queryFn: () => unwrap(commands.tripPending()) });
  const decide = useMutation({
    mutationFn: ({ id, d }: { id: number; d: string }) => unwrap(commands.tripDecide(id, d)),
    onSuccess: () => {
      toast.success("Keputusan disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["trips"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  return (
    <div className="overflow-x-auto rounded-xl border border-border-primary">
      <table className="w-full text-left text-sm">
        <thead className="bg-bg-secondary text-text-secondary">
          <tr><th className="px-4 py-2">Karyawan</th><th className="px-4 py-2">Tujuan</th><th className="px-4 py-2">Tanggal</th><th className="px-4 py-2">Tahap</th><th className="px-4 py-2">Aksi</th></tr>
        </thead>
        <tbody>
          {(list.data ?? []).map((t) => (
            <tr key={t.id} className="border-t border-border-secondary">
              <td className="px-4 py-2">{t.employee_name} • {t.employee_number}</td>
              <td className="px-4 py-2 font-medium">{t.destination}</td>
              <td className="px-4 py-2">{formatDate(t.start_date)} – {formatDate(t.end_date)}</td>
              <td className="px-4 py-2">Tahap {t.current_step}</td>
              <td className="px-4 py-2">
                <span className="flex gap-2">
                  <button type="button" disabled={decide.isPending} onClick={() => decide.mutate({ id: t.id, d: "approve" })} className="rounded-lg bg-bg-brand-solid px-3 py-1.5 text-xs font-semibold text-white disabled:opacity-60">Setujui</button>
                  <button type="button" disabled={decide.isPending} onClick={() => decide.mutate({ id: t.id, d: "reject" })} className="rounded-lg border border-border-primary px-3 py-1.5 text-xs font-medium">Tolak</button>
                </span>
              </td>
            </tr>
          ))}
          {(list.data ?? []).length === 0 && (
            <tr><td colSpan={5} className="px-4 py-6 text-center text-text-tertiary">Tidak ada pengajuan menunggu.</td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function MyReimburse() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const list = useQuery({ queryKey: ["reimburse", "my"], queryFn: () => unwrap(commands.reimburseMy()) });
  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button type="button" onClick={() => setOpen(true)} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">
          Ajukan reimburse
        </button>
      </div>
      <div className="overflow-x-auto rounded-xl border border-border-primary">
        <table className="w-full text-left text-sm">
          <thead className="bg-bg-secondary text-text-secondary">
            <tr><th className="px-4 py-2">Kategori</th><th className="px-4 py-2">Nominal</th><th className="px-4 py-2">Keterangan</th><th className="px-4 py-2">Status</th></tr>
          </thead>
          <tbody>
            {(list.data ?? []).map((r) => (
              <tr key={r.id} className="border-t border-border-secondary">
                <td className="px-4 py-2 font-medium">{r.category_name}</td>
                <td className="px-4 py-2">{r.amount == null ? "—" : formatRupiah(r.amount)}</td>
                <td className="px-4 py-2">{r.description ?? "—"}</td>
                <td className="px-4 py-2">{statusBadge(r.status)}</td>
              </tr>
            ))}
            {(list.data ?? []).length === 0 && (
              <tr><td colSpan={4} className="px-4 py-6 text-center text-text-tertiary">Belum ada pengajuan reimburse.</td></tr>
            )}
          </tbody>
        </table>
      </div>
      {open && <ReimburseForm onClose={() => { setOpen(false); void queryClient.invalidateQueries({ queryKey: ["reimburse"] }); }} />}
    </div>
  );
}

function ReimburseForm({ onClose }: { onClose: () => void }) {
  const categories = useQuery({ queryKey: ["reimburseCategories"], queryFn: () => unwrap(commands.reimburseCategories()) });
  const [category, setCategory] = useState("");
  const [amount, setAmount] = useState("");
  const [desc, setDesc] = useState("");
  const [receipt, setReceipt] = useState<File | null>(null);
  const save = useMutation({
    mutationFn: async () => {
      let file: { name: string; mime: string; bytes: number[] } | null = null;
      if (receipt) {
        const buf = new Uint8Array(await receipt.arrayBuffer());
        file = { name: receipt.name, mime: receipt.type || "application/octet-stream", bytes: [...buf] };
      }
      return unwrap(commands.reimburseCreate({
        category_id: Number(category),
        amount: amount ? Number(amount) : null,
        description: desc || null,
      }, file));
    },
    onSuccess: () => { toast.success("Pengajuan reimburse dikirim."); onClose(); },
    onError: (e: Error) => toast.error(e.message),
  });
  const input = "w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm";
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <form
        className="w-full max-w-md space-y-4 rounded-2xl bg-bg-primary p-6 shadow-2xl"
        onSubmit={(e) => { e.preventDefault(); save.mutate(); }}
      >
        <h2 className="text-display-xs font-semibold">Pengajuan reimburse</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Kategori</span>
          <select value={category} onChange={(e) => setCategory(e.target.value)} required className={input}>
            <option value="">Pilih…</option>
            {(categories.data ?? []).map((c) => (
              <option key={c.id} value={c.id}>{c.name}{c.max_amount != null ? ` (maks ${formatRupiah(c.max_amount)})` : ""}</option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Nominal (Rp)</span>
          <input type="number" min={0} value={amount} onChange={(e) => setAmount(e.target.value)} className={input} />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Keterangan</span>
          <textarea value={desc} onChange={(e) => setDesc(e.target.value)} rows={2} className={input} />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Bukti (opsional)</span>
          <input type="file" onChange={(e) => setReceipt(e.target.files?.[0] ?? null)} className="text-sm" />
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Kirim</button>
        </div>
      </form>
    </div>
  );
}

function PendingReimburse() {
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["reimburse", "all"], queryFn: () => unwrap(commands.reimburseAll()) });
  const decide = useMutation({
    mutationFn: ({ id, a }: { id: number; a: string }) => unwrap(commands.reimburseDecide(id, a)),
    onSuccess: (msg) => {
      toast.success(msg || "Keputusan disimpan.");
      void queryClient.invalidateQueries({ queryKey: ["reimburse"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const pending = (list.data ?? []).filter((r: Reimburse) => r.status === "pending");
  return (
    <div className="overflow-x-auto rounded-xl border border-border-primary">
      <table className="w-full text-left text-sm">
        <thead className="bg-bg-secondary text-text-secondary">
          <tr><th className="px-4 py-2">Karyawan</th><th className="px-4 py-2">Kategori</th><th className="px-4 py-2">Nominal</th><th className="px-4 py-2">Tahap</th><th className="px-4 py-2">Aksi</th></tr>
        </thead>
        <tbody>
          {pending.map((r) => (
            <tr key={r.id} className="border-t border-border-secondary">
              <td className="px-4 py-2">{r.employee_name} • {r.employee_number}</td>
              <td className="px-4 py-2 font-medium">{r.category_name}</td>
              <td className="px-4 py-2">{r.amount == null ? "—" : formatRupiah(r.amount)}</td>
              <td className="px-4 py-2">Tahap {r.current_step}</td>
              <td className="px-4 py-2">
                <span className="flex gap-2">
                  <button type="button" disabled={decide.isPending} onClick={() => decide.mutate({ id: r.id, a: "approve" })} className="rounded-lg bg-bg-brand-solid px-3 py-1.5 text-xs font-semibold text-white disabled:opacity-60">Setujui</button>
                  <button type="button" disabled={decide.isPending} onClick={() => decide.mutate({ id: r.id, a: "reject" })} className="rounded-lg border border-border-primary px-3 py-1.5 text-xs font-medium">Tolak</button>
                </span>
              </td>
            </tr>
          ))}
          {pending.length === 0 && (
            <tr><td colSpan={5} className="px-4 py-6 text-center text-text-tertiary">Tidak ada pengajuan menunggu.</td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function CategoryList() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const list = useQuery({ queryKey: ["reimburseCategories"], queryFn: () => unwrap(commands.reimburseCategories()) });
  const refresh = () => { void queryClient.invalidateQueries({ queryKey: ["reimburseCategories"] }); };
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.reimburseCategoryDelete(id)),
    onSuccess: () => { toast.success("Kategori dihapus."); refresh(); },
    onError: (e: Error) => toast.error(e.message),
  });
  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <button type="button" onClick={() => setOpen(true)} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">
          Tambah kategori
        </button>
      </div>
      <div className="overflow-x-auto rounded-xl border border-border-primary">
        <table className="w-full text-left text-sm">
          <thead className="bg-bg-secondary text-text-secondary">
            <tr><th className="px-4 py-2">Kode</th><th className="px-4 py-2">Nama</th><th className="px-4 py-2">Batas</th><th className="px-4 py-2">Aksi</th></tr>
          </thead>
          <tbody>
            {(list.data ?? []).map((c) => (
              <tr key={c.id} className="border-t border-border-secondary">
                <td className="px-4 py-2 font-mono">{c.code}</td>
                <td className="px-4 py-2 font-medium">{c.name}</td>
                <td className="px-4 py-2">{c.max_amount == null ? "—" : formatRupiah(c.max_amount)}</td>
                <td className="px-4 py-2">
                  <button type="button" onClick={() => { if (confirm(`Hapus kategori ${c.name}?`)) remove.mutate(c.id); }} className="rounded-lg border border-border-primary px-3 py-1.5 text-xs font-medium text-text-error">Hapus</button>
                </td>
              </tr>
            ))}
            {(list.data ?? []).length === 0 && (
              <tr><td colSpan={4} className="px-4 py-6 text-center text-text-tertiary">Belum ada kategori.</td></tr>
            )}
          </tbody>
        </table>
      </div>
      {open && <ReimburseCategoryDialog onClose={() => { setOpen(false); refresh(); }} />}
    </div>
  );
}

function ReimburseCategoryDialog({ onClose }: { onClose: () => void }) {
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [max, setMax] = useState("");
  const save = useMutation({
    mutationFn: () => unwrap(commands.reimburseCategorySave(null, code, name, max ? Number(max) : null)),
    onSuccess: () => { toast.success("Kategori disimpan."); onClose(); },
    onError: (e: Error) => toast.error(e.message),
  });
  const input = "w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm";
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <form
        className="w-full max-w-sm space-y-4 rounded-2xl bg-bg-primary p-6 shadow-2xl"
        onSubmit={(e) => { e.preventDefault(); save.mutate(); }}
      >
        <h2 className="text-display-xs font-semibold">Tambah kategori</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Kode</span>
          <input value={code} onChange={(e) => setCode(e.target.value)} required className={input} />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Nama</span>
          <input value={name} onChange={(e) => setName(e.target.value)} required className={input} />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Batas nominal (Rp, kosong = tanpa batas)</span>
          <input type="number" min={0} value={max} onChange={(e) => setMax(e.target.value)} className={input} />
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}
