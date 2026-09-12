import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type Asset } from "../bindings";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/assets")({
  component: AssetsPage,
});

const CONDITION_LABEL: Record<string, string> = {
  new: "Baru",
  good: "Baik",
  fair: "Cukup",
  damaged: "Rusak",
  lost: "Hilang",
};
const STATUS_LABEL: Record<string, string> = {
  available: "Tersedia",
  assigned: "Ditugaskan",
  maintenance: "Maintenance",
  disposed: "Dihapus",
};

function AssetsPage() {
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [applied, setApplied] = useState("");
  const [detailId, setDetailId] = useState<number | null>(null);
  const [form, setForm] = useState<Asset | "new" | null>(null);
  const [catOpen, setCatOpen] = useState(false);

  const list = useQuery({
    queryKey: ["assets", applied],
    queryFn: () => unwrap(commands.assetsList(applied)),
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["assets"] });
    void queryClient.invalidateQueries({ queryKey: ["assetCategories"] });
  };
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.assetDelete(id)),
    onSuccess: () => {
      toast.success("Aset dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Aset</h1>
        <span className="flex gap-2">
          <button
            type="button"
            onClick={() => setCatOpen(true)}
            className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover"
          >
            Kategori
          </button>
          <button
            type="button"
            onClick={() => setForm("new")}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
          >
            Tambah aset
          </button>
        </span>
      </div>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          setApplied(search);
        }}
      >
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Cari kode, nama, serial…"
          className="w-full max-w-sm rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none placeholder:text-text-placeholder focus:border-border-brand"
        />
        <button type="submit" className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover">
          Cari
        </button>
      </form>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {["Kode", "Nama", "Kategori", "Kondisi", "Status", "Aksi"].map((h) => (
                <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(list.data ?? []).map((a) => (
              <tr key={a.id} className="border-b border-border-tertiary last:border-0">
                <td className="whitespace-nowrap px-4 py-2.5 font-medium">{a.asset_code}</td>
                <td className="px-4 py-2.5">{a.name}</td>
                <td className="px-4 py-2.5 text-text-secondary">{a.category_name}</td>
                <td className="px-4 py-2.5">{CONDITION_LABEL[a.condition_status] ?? a.condition_status}</td>
                <td className="px-4 py-2.5">
                  <span className="rounded-full bg-bg-brand-primary px-2 py-0.5 text-xs font-medium text-text-brand-primary">
                    {STATUS_LABEL[a.status] ?? a.status}
                  </span>
                </td>
                <td className="px-4 py-2.5">
                  <span className="flex gap-3 text-[13px]">
                    <button type="button" onClick={() => setDetailId(a.id)} className="font-medium text-text-brand-secondary hover:underline">
                      Detail
                    </button>
                    <button type="button" onClick={() => setForm(a)} className="font-medium text-text-brand-secondary hover:underline">
                      Ubah
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        if (window.confirm(`Hapus aset ${a.name}?`)) remove.mutate(a.id);
                      }}
                      className="font-medium text-text-error-primary hover:underline"
                    >
                      Hapus
                    </button>
                  </span>
                </td>
              </tr>
            ))}
            {list.data && list.data.length === 0 && (
              <tr>
                <td colSpan={6} className="px-4 py-6 text-center text-sm text-text-tertiary">
                  Belum ada aset.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      {form && (
        <AssetForm
          initial={form === "new" ? null : form}
          onClose={() => {
            setForm(null);
            refresh();
          }}
        />
      )}
      {detailId !== null && (
        <AssetDetail
          id={detailId}
          onClose={() => {
            setDetailId(null);
            refresh();
          }}
        />
      )}
      {catOpen && (
        <CategoryDialog
          onClose={() => {
            setCatOpen(false);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function AssetForm({ initial, onClose }: { initial: Asset | null; onClose: () => void }) {
  const categories = useQuery({
    queryKey: ["assetCategories"],
    queryFn: () => unwrap(commands.assetCategories()),
  });
  const [code, setCode] = useState(initial?.asset_code ?? "");
  const [name, setName] = useState(initial?.name ?? "");
  const [category, setCategory] = useState(initial ? String(initial.asset_category_id) : "");
  const [brand, setBrand] = useState(initial?.brand ?? "");
  const [serial, setSerial] = useState(initial?.serial_number ?? "");
  const [condition, setCondition] = useState(initial?.condition_status ?? "good");

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.assetSave(initial?.id ?? null, {
          asset_code: code,
          name,
          asset_category_id: Number(category),
          brand: brand || null,
          serial_number: serial || null,
          purchase_date: initial?.purchase_date ?? null,
          purchase_price: initial?.purchase_price ?? null,
          condition_status: condition,
          status: initial?.status ?? "available",
        }),
      ),
    onSuccess: () => {
      toast.success("Aset disimpan.");
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
        <h2 className="text-display-xs font-semibold">{initial ? "Ubah" : "Tambah"} aset</h2>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Kode</span>
            <input value={code} onChange={(e) => setCode(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Nama</span>
            <input value={name} onChange={(e) => setName(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Kategori</span>
          <select value={category} onChange={(e) => setCategory(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            <option value="">Pilih…</option>
            {(categories.data ?? []).map((c) => (
              <option key={c.id} value={c.id}>{c.name}</option>
            ))}
          </select>
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Merek</span>
            <input value={brand} onChange={(e) => setBrand(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Serial</span>
            <input value={serial} onChange={(e) => setSerial(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Kondisi</span>
          <select value={condition} onChange={(e) => setCondition(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            {Object.entries(CONDITION_LABEL).map(([v, l]) => (
              <option key={v} value={v}>{l}</option>
            ))}
          </select>
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}

function CategoryDialog({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const list = useQuery({
    queryKey: ["assetCategories"],
    queryFn: () => unwrap(commands.assetCategories()),
  });
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["assetCategories"] });

  const save = useMutation({
    mutationFn: () => unwrap(commands.assetCategorySave(null, code, name)),
    onSuccess: () => {
      toast.success("Kategori disimpan.");
      setCode("");
      setName("");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.assetCategoryDelete(id)),
    onSuccess: () => {
      toast.success("Kategori dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay/60 p-4">
      <div className="w-full max-w-md space-y-3 rounded-2xl bg-bg-primary p-6 shadow-2xl">
        <div className="flex items-center justify-between">
          <h2 className="text-display-xs font-semibold">Kategori aset</h2>
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm">Tutup</button>
        </div>
        <ul className="space-y-1">
          {(list.data ?? []).map((c) => (
            <li key={c.id} className="flex items-center justify-between rounded-lg bg-bg-secondary px-3 py-2 text-sm">
              <span className="font-medium">{c.name} <span className="text-text-tertiary">({c.code})</span></span>
              <button
                type="button"
                onClick={() => {
                  if (window.confirm(`Hapus kategori ${c.name}?`)) remove.mutate(c.id);
                }}
                className="text-[13px] font-medium text-text-error-primary hover:underline"
              >
                Hapus
              </button>
            </li>
          ))}
        </ul>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            save.mutate();
          }}
        >
          <input value={code} onChange={(e) => setCode(e.target.value)} required placeholder="Kode" className="w-24 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          <input value={name} onChange={(e) => setName(e.target.value)} required placeholder="Nama kategori" className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          <button type="submit" className="shrink-0 rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white">
            Tambah
          </button>
        </form>
      </div>
    </div>
  );
}

function AssetDetail({ id, onClose }: { id: number; onClose: () => void }) {
  const queryClient = useQueryClient();
  const detail = useQuery({
    queryKey: ["asset", id],
    queryFn: () => unwrap(commands.assetDetail(id)),
  });
  const employees = useQuery({
    queryKey: ["employeeDropdowns"],
    queryFn: () => unwrap(commands.employeeDropdowns()),
  });
  const [assignOpen, setAssignOpen] = useState(false);
  const [maintOpen, setMaintOpen] = useState(false);

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["asset", id] });
    void queryClient.invalidateQueries({ queryKey: ["assets"] });
  };
  const doReturn = useMutation({
    mutationFn: (v: { aid: number; cond: string }) =>
      unwrap(
        commands.assetReturn(v.aid, {
          returned_date: new Date().toISOString().slice(0, 10),
          condition_on_return: v.cond,
          notes: null,
        }),
      ),
    onSuccess: () => {
      toast.success("Aset dikembalikan.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const available = useMutation({
    mutationFn: () => unwrap(commands.assetMarkAvailable(id)),
    onSuccess: () => {
      toast.success("Aset tersedia kembali.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

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
                <h2 className="text-display-xs font-semibold">{d.asset.name}</h2>
                <p className="text-sm text-text-tertiary">
                  {d.asset.asset_code} • {d.asset.category_name} • {STATUS_LABEL[d.asset.status] ?? d.asset.status}
                </p>
              </div>
              <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm">Tutup</button>
            </div>
            <div className="flex flex-wrap gap-2">
              {d.asset.status === "available" && (
                <button type="button" onClick={() => setAssignOpen(true)} className="rounded-lg bg-bg-brand-solid px-3 py-1.5 text-sm font-semibold text-white">
                  Tugaskan
                </button>
              )}
              {(d.asset.status === "maintenance" || d.asset.status === "disposed") && (
                <button type="button" onClick={() => available.mutate()} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm font-medium">
                  Tandai tersedia
                </button>
              )}
              <button type="button" onClick={() => setMaintOpen(true)} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm font-medium">
                Catat maintenance
              </button>
            </div>
            <div className="rounded-xl border border-border-secondary p-4">
              <h3 className="mb-2 text-sm font-semibold">Riwayat penugasan</h3>
              <ul className="space-y-1 text-sm">
                {d.assignments.map((a) => (
                  <li key={a.id} className="flex flex-wrap items-center justify-between gap-2 border-t border-border-tertiary pt-1">
                    <span>
                      {a.employee_name} • {a.assigned_date}
                      {a.returned_date ? ` → ${a.returned_date}` : " • dipinjam"}
                    </span>
                    {!a.returned_date && (
                      <span className="flex gap-2 text-[13px]">
                        <button type="button" onClick={() => doReturn.mutate({ aid: a.id, cond: "good" })} className="font-medium text-text-brand-secondary hover:underline">
                          Kembali baik
                        </button>
                        <button type="button" onClick={() => doReturn.mutate({ aid: a.id, cond: "damaged" })} className="font-medium text-text-error-primary hover:underline">
                          Kembali rusak
                        </button>
                      </span>
                    )}
                  </li>
                ))}
                {d.assignments.length === 0 && <li className="text-text-tertiary">Belum pernah ditugaskan.</li>}
              </ul>
            </div>
            <div className="rounded-xl border border-border-secondary p-4">
              <h3 className="mb-2 text-sm font-semibold">Maintenance</h3>
              <ul className="space-y-1 text-sm">
                {d.maintenance.map((m) => (
                  <li key={m.id} className="flex justify-between gap-2 border-t border-border-tertiary pt-1">
                    <span>{m.maintenance_date} • {m.description}</span>
                    <span className="font-medium">{(m.cost ?? 0).toLocaleString("id-ID")}</span>
                  </li>
                ))}
                {d.maintenance.length === 0 && <li className="text-text-tertiary">Belum ada.</li>}
              </ul>
            </div>
          </div>
        )}
      </div>
      {assignOpen && d && (
        <AssignDialog
          assetId={id}
          employees={employees.data?.employees ?? []}
          onClose={() => {
            setAssignOpen(false);
            refresh();
          }}
        />
      )}
      {maintOpen && (
        <MaintenanceDialog
          assetId={id}
          onClose={() => {
            setMaintOpen(false);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function AssignDialog({
  assetId,
  employees,
  onClose,
}: {
  assetId: number;
  employees: { id: number; name: string }[];
  onClose: () => void;
}) {
  const [employeeId, setEmployeeId] = useState("");
  const [date, setDate] = useState(new Date().toISOString().slice(0, 10));
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.assetAssign(assetId, {
          employee_id: Number(employeeId),
          assigned_date: date,
          condition_on_assign: null,
          notes: null,
        }),
      ),
    onSuccess: () => {
      toast.success("Aset ditugaskan.");
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
        <h2 className="text-display-xs font-semibold">Tugaskan aset</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Karyawan</span>
          <select value={employeeId} onChange={(e) => setEmployeeId(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm">
            <option value="">Pilih…</option>
            {employees.map((e) => (
              <option key={e.id} value={e.id}>{e.name}</option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal</span>
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Tugaskan</button>
        </div>
      </form>
    </div>
  );
}

function MaintenanceDialog({ assetId, onClose }: { assetId: number; onClose: () => void }) {
  const [date, setDate] = useState(new Date().toISOString().slice(0, 10));
  const [description, setDescription] = useState("");
  const [cost, setCost] = useState("0");
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.assetMaintenanceAdd(assetId, {
          maintenance_date: date,
          description,
          cost: Number(cost),
          performed_by: null,
        }),
      ),
    onSuccess: () => {
      toast.success("Maintenance dicatat.");
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
        <h2 className="text-display-xs font-semibold">Catat maintenance</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal</span>
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Deskripsi</span>
          <input value={description} onChange={(e) => setDescription(e.target.value)} required maxLength={255} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Biaya (Rp)</span>
          <input type="number" min={0} value={cost} onChange={(e) => setCost(e.target.value)} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Simpan</button>
        </div>
      </form>
    </div>
  );
}
