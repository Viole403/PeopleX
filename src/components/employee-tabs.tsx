import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type ChildMeta } from "../bindings";
import { formatDate, formatRupiah } from "../lib/format";
import { unwrap } from "../lib/query";

function fmtVal(v: string | undefined, fieldType: string): string {
  if (v === undefined || v === "") return "-";
  if (fieldType === "date" && /^\d{4}-\d{2}-\d{2}$/.test(v)) return formatDate(v);
  if (fieldType === "checkbox") return v === "1" ? "Ya" : "Tidak";
  return v;
}

function optLabel(meta: ChildMeta, field: string, value: string): string {
  const f = meta.fields.find((x) => x.name === field);
  const o = f?.options?.find((x) => x.value === value);
  return o ? o.label : value;
}

export function ChildTab({ meta, employeeId }: { meta: ChildMeta; employeeId: number }) {
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState<Record<string, string> | "new" | null>(null);
  const list = useQuery({
    queryKey: ["child", meta.slug, employeeId],
    queryFn: () => unwrap(commands.employeeChildList(meta.slug, employeeId)),
  });
  const refresh = () =>
    void queryClient.invalidateQueries({ queryKey: ["child", meta.slug, employeeId] });

  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.employeeChildDelete(meta.slug, employeeId, id)),
    onSuccess: () => {
      toast.success("Data dihapus.");
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
          Tambah
        </button>
      </div>
      {list.isPending ? (
        <p className="text-sm text-text-tertiary">Memuat…</p>
      ) : (list.data ?? []).length === 0 ? (
        <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
          Belum ada {meta.title.toLowerCase()}.
        </p>
      ) : (
        <div className="grid gap-3">
          {(list.data ?? []).map((row) => (
            <div
              key={row.id}
              className="rounded-xl border border-border-secondary bg-bg-primary p-4"
            >
              <dl className="grid gap-x-6 gap-y-1 text-sm sm:grid-cols-2">
                {meta.fields.map((f) => (
                  <div key={f.name} className="flex gap-2">
                    <dt className="w-36 shrink-0 text-text-tertiary">{f.label}</dt>
                    <dd>{fmtVal(optLabel(meta, f.name, row[f.name] ?? ""), f.field_type)}</dd>
                  </div>
                ))}
              </dl>
              <div className="mt-2 flex gap-3 text-[13px]">
                <button
                  type="button"
                  onClick={() => setEditing(row)}
                  className="font-medium text-text-brand-secondary hover:underline"
                >
                  Ubah
                </button>
                <button
                  type="button"
                  onClick={() => {
                    if (window.confirm("Hapus data ini?")) remove.mutate(Number(row.id));
                  }}
                  className="font-medium text-text-error-primary hover:underline"
                >
                  Hapus
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
      {editing && (
        <ChildForm
          meta={meta}
          employeeId={employeeId}
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

function ChildForm({
  meta,
  employeeId,
  initial,
  onClose,
}: {
  meta: ChildMeta;
  employeeId: number;
  initial: Record<string, string> | null;
  onClose: () => void;
}) {
  const [values, setValues] = useState<Record<string, string>>(initial ?? {});
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.employeeChildSave(
          meta.slug,
          employeeId,
          initial?.id ? Number(initial.id) : null,
          values,
        ),
      ),
    onSuccess: () => {
      toast.success("Data disimpan.");
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
        <h2 className="text-display-xs font-semibold">
          {initial ? "Ubah" : "Tambah"} {meta.title}
        </h2>
        {meta.fields.map((f) => (
          <label key={f.name} className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">
              {f.label}
              {f.required && <span className="text-text-error-primary"> *</span>}
            </span>
            {f.field_type === "textarea" ? (
              <textarea
                value={values[f.name] ?? ""}
                onChange={(e) => setValues({ ...values, [f.name]: e.target.value })}
                rows={2}
                className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
              />
            ) : f.field_type === "checkbox" ? (
              <input
                type="checkbox"
                checked={(values[f.name] ?? "") === "1"}
                onChange={(e) =>
                  setValues({ ...values, [f.name]: e.target.checked ? "1" : "0" })
                }
                className="size-4 accent-brand-600"
              />
            ) : f.field_type === "select" ? (
              <select
                value={values[f.name] ?? ""}
                onChange={(e) => setValues({ ...values, [f.name]: e.target.value })}
                required={f.required}
                className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
              >
                <option value="">Pilih…</option>
                {(f.options ?? []).map((o) => (
                  <option key={o.value} value={o.value}>{o.label}</option>
                ))}
              </select>
            ) : (
              <input
                type={f.field_type === "number" ? "number" : f.field_type === "date" ? "date" : "text"}
                value={values[f.name] ?? ""}
                onChange={(e) => setValues({ ...values, [f.name]: e.target.value })}
                required={f.required}
                className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
              />
            )}
          </label>
        ))}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium"
          >
            Batal
          </button>
          <button
            type="submit"
            disabled={save.isPending}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
          >
            Simpan
          </button>
        </div>
      </form>
    </div>
  );
}

export function AddressTab({ employeeId }: { employeeId: number }) {
  const queryClient = useQueryClient();
  const data = useQuery({
    queryKey: ["addresses", employeeId],
    queryFn: () => unwrap(commands.employeeAddresses(employeeId)),
  });
  const [draft, setDraft] = useState<Record<string, Record<string, string>> | null>(null);

  const save = useMutation({
    mutationFn: (kind: string) =>
      unwrap(commands.employeeAddressSave(employeeId, kind, draft?.[kind] ?? {})),
    onSuccess: () => {
      toast.success("Alamat disimpan.");
      setDraft(null);
      void queryClient.invalidateQueries({ queryKey: ["addresses", employeeId] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  if (data.isPending) return <p className="text-sm text-text-tertiary">Memuat…</p>;
  const kinds = [["ktp", "Alamat KTP"], ["domicile", "Alamat Domisili"]] as const;

  return (
    <div className="grid gap-4 md:grid-cols-2">
      {kinds.map(([kind, label]) => {
        const cur = kind === "ktp" ? data.data?.ktp : data.data?.domicile;
        const val = (k: string) => draft?.[kind]?.[k] ?? (cur?.[k as keyof typeof cur] as string | null | undefined) ?? "";
        const set = (k: string, v: string) =>
          setDraft({ ...(draft ?? {}), [kind]: { ...(draft?.[kind] ?? {}), [k]: v } });
        return (
          <div key={kind} className="space-y-3 rounded-xl border border-border-secondary bg-bg-primary p-5">
            <h3 className="font-semibold">{label}</h3>
            {(
              [["address", "Alamat"], ["city", "Kota"], ["province", "Provinsi"], ["postal_code", "Kode pos"]] as const
            ).map(([k, l]) => (
              <label key={k} className="block">
                <span className="mb-1 block text-sm font-medium text-text-secondary">{l}</span>
                <input
                  value={val(k)}
                  onChange={(e) => set(k, e.target.value)}
                  className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
                />
              </label>
            ))}
            <div className="flex justify-end">
              <button
                type="button"
                disabled={save.isPending || !draft?.[kind]}
                onClick={() => save.mutate(kind)}
                className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
              >
                Simpan
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}

const DOC_LABELS: Record<string, string> = {
  ktp: "KTP", kk: "KK", npwp: "NPWP", bpjs: "BPJS", ijazah: "Ijazah",
  certificate: "Sertifikat", cv: "CV", contract: "Kontrak",
  appointment_letter: "SK Pengangkatan", promotion_letter: "SK Promosi",
  mutation_letter: "SK Mutasi", other: "Lainnya",
};

export function DocumentTab({ employeeId }: { employeeId: number }) {
  const queryClient = useQueryClient();
  const docs = useQuery({
    queryKey: ["documents", employeeId],
    queryFn: () => unwrap(commands.employeeDocuments(employeeId)),
  });
  const [form, setForm] = useState<{ category: string; name: string; expiry: string; file: File | null }>({
    category: "ktp",
    name: "",
    expiry: "",
    file: null,
  });

  const refresh = () =>
    void queryClient.invalidateQueries({ queryKey: ["documents", employeeId] });

  const upload = useMutation({
    mutationFn: async () => {
      if (!form.file) throw new Error("Pilih berkas dulu.");
      const buf = new Uint8Array(await form.file.arrayBuffer());
      return unwrap(
        commands.employeeDocumentUpload(employeeId, form.category, form.name, form.expiry || null, {
          name: form.file.name,
          mime: form.file.type || "application/octet-stream",
          bytes: [...buf],
        }),
      );
    },
    onSuccess: () => {
      toast.success("Dokumen diunggah.");
      setForm({ category: "ktp", name: "", expiry: "", file: null });
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const open = useMutation({
    mutationFn: (id: number) => unwrap(commands.employeeDocumentBytes(employeeId, id)),
    onSuccess: (d) => {
      const blob = new Blob([new Uint8Array(d.bytes)], { type: d.mime });
      window.open(URL.createObjectURL(blob), "_blank");
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.employeeDocumentDelete(employeeId, id)),
    onSuccess: () => {
      toast.success("Dokumen dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <form
        className="grid gap-3 rounded-xl border border-border-secondary bg-bg-primary p-5 sm:grid-cols-2"
        onSubmit={(e) => {
          e.preventDefault();
          upload.mutate();
        }}
      >
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Kategori</span>
          <select
            value={form.category}
            onChange={(e) => setForm({ ...form, category: e.target.value })}
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
          >
            {Object.entries(DOC_LABELS).map(([v, l]) => (
              <option key={v} value={v}>{l}</option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Nama dokumen</span>
          <input
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            required
            maxLength={150}
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
          />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Kedaluarsa (opsional)</span>
          <input
            type="date"
            value={form.expiry}
            onChange={(e) => setForm({ ...form, expiry: e.target.value })}
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
          />
        </label>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">
            Berkas (jpg/png/pdf/doc, maks 5MB)
          </span>
          <input
            type="file"
            required
            onChange={(e) => setForm({ ...form, file: e.target.files?.[0] ?? null })}
            className="w-full text-sm text-text-secondary file:mr-3 file:rounded-lg file:border file:border-border-primary file:bg-bg-secondary file:px-3 file:py-2 file:text-sm"
          />
        </label>
        <div className="sm:col-span-2 flex justify-end">
          <button
            type="submit"
            disabled={upload.isPending}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
          >
            Unggah
          </button>
        </div>
      </form>
      <div className="grid gap-3">
        {(docs.data ?? []).map((d) => (
          <div
            key={d.id}
            className="flex flex-wrap items-center gap-3 rounded-xl border border-border-secondary bg-bg-primary p-4"
          >
            <div className="min-w-0 flex-1">
              <p className="font-medium">{d.name}</p>
              <p className="text-xs text-text-tertiary">
                {DOC_LABELS[d.category] ?? d.category}
                {d.expiry_date ? ` • berlaku s.d. ${formatDate(d.expiry_date)}` : ""}
              </p>
            </div>
            <button
              type="button"
              onClick={() => open.mutate(d.id)}
              className="text-[13px] font-medium text-text-brand-secondary hover:underline"
            >
              Buka
            </button>
            <button
              type="button"
              onClick={() => {
                if (window.confirm("Hapus dokumen ini?")) remove.mutate(d.id);
              }}
              className="text-[13px] font-medium text-text-error-primary hover:underline"
            >
              Hapus
            </button>
          </div>
        ))}
        {docs.data && docs.data.length === 0 && (
          <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
            Belum ada dokumen.
          </p>
        )}
      </div>
    </div>
  );
}

export function SalaryTab({ employeeId }: { employeeId: number }) {
  const queryClient = useQueryClient();
  const current = useQuery({
    queryKey: ["salaryCurrent", employeeId],
    queryFn: () => unwrap(commands.employeeSalaryCurrent(employeeId)),
  });
  const history = useQuery({
    queryKey: ["salaryHistory", employeeId],
    queryFn: () => unwrap(commands.employeeSalaryHistory(employeeId)),
  });
  const catalog = useQuery({
    queryKey: ["salaryCatalog"],
    queryFn: () => unwrap(commands.employeeAvailableComponents()),
  });
  const attached = useQuery({
    queryKey: ["salaryComponents", current.data?.id],
    queryFn: () => unwrap(commands.employeeSalaryComponents(current.data?.id ?? 0)),
    enabled: !!current.data,
  });
  const [basic, setBasic] = useState("");
  const [effective, setEffective] = useState("");
  const [amounts, setAmounts] = useState<Record<number, string>>({});

  const save = useMutation({
    mutationFn: () => {
      const comps = Object.entries(amounts)
        .filter(([, v]) => v.trim() !== "")
        .map(([k, v]) => [Number(k), Number(v)] as [number, number]);
      return unwrap(commands.employeeSetSalary(employeeId, Number(basic), effective, comps));
    },
    onSuccess: () => {
      toast.success("Gaji ditetapkan.");
      setBasic("");
      setEffective("");
      setAmounts({});
      void queryClient.invalidateQueries({ queryKey: ["salaryCurrent", employeeId] });
      void queryClient.invalidateQueries({ queryKey: ["salaryHistory", employeeId] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-5">
        <h3 className="font-semibold">Gaji aktif</h3>
        {current.isPending ? (
          <p className="mt-2 text-sm text-text-tertiary">Memuat…</p>
        ) : current.data ? (
          <div className="mt-2 text-sm">
            <p className="text-display-xs font-semibold">{formatRupiah(current.data.basic_salary ?? 0)}</p>
            <p className="text-text-tertiary">berlaku sejak {formatDate(current.data.effective_date)}</p>
            {(attached.data ?? []).length > 0 && (
              <ul className="mt-2 space-y-1">
                {attached.data!.map((c) => (
                  <li key={c.id} className="flex justify-between border-t border-border-tertiary pt-1">
                    <span>{c.name}</span>
                    <span className="font-medium">{formatRupiah(c.amount ?? 0)}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        ) : (
          <p className="mt-2 text-sm text-text-tertiary">Belum ada gaji ditetapkan.</p>
        )}
      </div>
      <form
        className="space-y-3 rounded-xl border border-border-secondary bg-bg-primary p-5"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <h3 className="font-semibold">Tetapkan gaji baru</h3>
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Gaji pokok (Rp)</span>
            <input
              type="number"
              min={0}
              value={basic}
              onChange={(e) => setBasic(e.target.value)}
              required
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
            />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Tanggal efektif</span>
            <input
              type="date"
              value={effective}
              onChange={(e) => setEffective(e.target.value)}
              required
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
            />
          </label>
        </div>
        {(catalog.data ?? []).length > 0 && (
          <div>
            <p className="mb-2 text-sm font-medium text-text-secondary">Tunjangan (opsional)</p>
            <div className="grid gap-2 sm:grid-cols-2">
              {(catalog.data ?? []).filter((c) => c.code !== "BASIC").map((c) => (
                <label key={c.id} className="flex items-center gap-2 text-sm">
                  <span className="w-40 shrink-0 truncate">{c.name}</span>
                  <input
                    type="number"
                    min={0}
                    value={amounts[c.id] ?? ""}
                    onChange={(e) => setAmounts({ ...amounts, [c.id]: e.target.value })}
                    placeholder="0"
                    className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-1.5 text-sm outline-none focus:border-border-brand"
                  />
                </label>
              ))}
            </div>
          </div>
        )}
        <div className="flex justify-end">
          <button
            type="submit"
            disabled={save.isPending}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
          >
            Tetapkan
          </button>
        </div>
      </form>
      {(history.data ?? []).length > 1 && (
        <div className="rounded-xl border border-border-secondary bg-bg-primary p-5">
          <h3 className="font-semibold">Riwayat</h3>
          <ul className="mt-2 space-y-1 text-sm">
            {history.data!.map((h) => (
              <li key={h.id} className="flex justify-between border-t border-border-tertiary pt-1">
                <span className="text-text-tertiary">{formatDate(h.effective_date)}</span>
                <span className="font-medium">{formatRupiah(h.basic_salary ?? 0)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
