import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type CompanyNode, type EntityMeta } from "../bindings";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/organization")({
  component: OrganizationPage,
});

const PER_PAGE = 20;

function OrganizationPage() {
  const entities = useQuery({
    queryKey: ["orgEntities"],
    queryFn: () => unwrap(commands.orgEntities()),
  });
  const [slug, setSlug] = useState("companies");
  const [view, setView] = useState<"data" | "chart">("data");

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Organisasi</h1>
        <div className="flex gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
          {(["data", "chart"] as const).map((v) => (
            <button
              key={v}
              type="button"
              onClick={() => setView(v)}
              className={`rounded-lg px-4 py-1.5 text-sm font-medium transition ${
                view === v
                  ? "bg-bg-brand-primary text-text-brand-primary"
                  : "text-text-secondary hover:bg-bg-primary_hover"
              }`}
            >
              {v === "data" ? "Data" : "Bagan"}
            </button>
          ))}
        </div>
      </div>
      {view === "chart" ? (
        <ChartView />
      ) : entities.isPending ? (
        <p className="text-sm text-text-tertiary">Memuat…</p>
      ) : entities.isError ? (
        <p className="text-sm text-text-error-primary">Gagal memuat entitas.</p>
      ) : (
        <>
          <div className="flex flex-wrap gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
            {entities.data.map((e) => (
              <button
                key={e.slug}
                type="button"
                onClick={() => setSlug(e.slug)}
                className={`rounded-lg px-3 py-1.5 text-sm font-medium transition ${
                  slug === e.slug
                    ? "bg-bg-brand-primary text-text-brand-primary"
                    : "text-text-secondary hover:bg-bg-primary_hover"
                }`}
              >
                {e.title}
              </button>
            ))}
          </div>
          <EntityTable
            key={slug}
            entity={entities.data.find((e) => e.slug === slug) ?? entities.data[0]}
          />
        </>
      )}
    </div>
  );
}

function EntityTable({ entity }: { entity: EntityMeta }) {
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [applied, setApplied] = useState("");
  const [page, setPage] = useState(1);
  const [editingId, setEditingId] = useState<number | "new" | null>(null);

  const list = useQuery({
    queryKey: ["orgList", entity.slug, applied, page],
    queryFn: () => unwrap(commands.orgList(entity.slug, applied, page, PER_PAGE)),
  });
  const totalPages = Math.max(1, Math.ceil((list.data?.total ?? 0) / PER_PAGE));

  const refresh = () =>
    void queryClient.invalidateQueries({ queryKey: ["orgList", entity.slug] });

  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.orgDelete(entity.slug, id)),
    onSuccess: () => {
      toast.success("Data dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <form
          className="flex flex-1 gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            setPage(1);
            setApplied(search);
          }}
        >
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={`Cari ${entity.title.toLowerCase()}…`}
            className="w-full max-w-sm rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none placeholder:text-text-placeholder focus:border-border-brand"
          />
          <button
            type="submit"
            className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover"
          >
            Cari
          </button>
        </form>
        <button
          type="button"
          onClick={() => setEditingId("new")}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Tambah
        </button>
      </div>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {entity.columns.map((c) => (
                <th key={c} className="px-4 py-2.5 font-medium">{c}</th>
              ))}
              <th className="px-4 py-2.5 font-medium">Aksi</th>
            </tr>
          </thead>
          <tbody>
            {(list.data?.rows ?? []).map((row) => (
              <tr key={row.id} className="border-b border-border-tertiary last:border-0">
                {entity.columns.map((c) => (
                  <td key={c} className="px-4 py-2.5">
                    {row[c] === "" || row[c] === undefined ? (
                      <span className="text-text-tertiary">-</span>
                    ) : c === "is_head_office" ? (
                      row[c] === "1" ? "Ya" : "Tidak"
                    ) : (
                      row[c]
                    )}
                  </td>
                ))}
                <td className="px-4 py-2.5">
                  <span className="flex gap-3 text-[13px]">
                    <button
                      type="button"
                      onClick={() => setEditingId(Number(row.id))}
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
                  </span>
                </td>
              </tr>
            ))}
            {list.data && list.data.rows.length === 0 && (
              <tr>
                <td
                  colSpan={entity.columns.length + 1}
                  className="px-4 py-6 text-center text-sm text-text-tertiary"
                >
                  Belum ada data.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <div className="flex items-center justify-between text-sm text-text-tertiary">
        <span>
          Total {list.data?.total ?? 0} • Halaman {page} dari {totalPages}
        </span>
        <span className="flex gap-2">
          <button
            type="button"
            disabled={page <= 1}
            onClick={() => setPage((p) => p - 1)}
            className="rounded-lg border border-border-primary px-3 py-1.5 disabled:opacity-50"
          >
            Sebelumnya
          </button>
          <button
            type="button"
            disabled={page >= totalPages}
            onClick={() => setPage((p) => p + 1)}
            className="rounded-lg border border-border-primary px-3 py-1.5 disabled:opacity-50"
          >
            Berikutnya
          </button>
        </span>
      </div>
      {editingId !== null && (
        <EntityForm
          entity={entity}
          id={editingId === "new" ? null : editingId}
          onClose={() => {
            setEditingId(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function EntityForm({
  entity,
  id,
  onClose,
}: {
  entity: EntityMeta;
  id: number | null;
  onClose: () => void;
}) {
  const existing = useQuery({
    queryKey: ["orgGet", entity.slug, id],
    queryFn: () => unwrap(commands.orgGet(entity.slug, id ?? 0)),
    enabled: id !== null,
  });
  const [draft, setDraft] = useState<Record<string, string> | null>(null);
  const values: Record<string, string> = draft ?? existing.data ?? {};

  const save = useMutation({
    mutationFn: () => unwrap(commands.orgSave(entity.slug, id, values)),
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
          {id === null ? "Tambah" : "Ubah"} {entity.title}
        </h2>
        {id !== null && existing.isPending ? (
          <p className="text-sm text-text-tertiary">Memuat…</p>
        ) : (
          entity.fields.map((f) => (
            <FieldInput
              key={f.name}
              slug={entity.slug}
              field={f}
              value={values[f.name] ?? ""}
              onChange={(v) => setDraft({ ...values, [f.name]: v })}
            />
          ))
        )}
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

function FieldInput({
  slug,
  field,
  value,
  onChange,
}: {
  slug: string;
  field: EntityMeta["fields"][number];
  value: string;
  onChange: (v: string) => void;
}) {
  const options = useQuery({
    queryKey: ["orgOptions", slug, field.name],
    queryFn: () => unwrap(commands.orgOptions(slug, field.name)),
    enabled: field.field_type === "select",
  });
  const cls =
    "w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand";
  const label = (
    <span className="mb-1 block text-sm font-medium text-text-secondary">
      {field.label}
      {field.required && <span className="text-text-error-primary"> *</span>}
    </span>
  );

  if (field.field_type === "textarea") {
    return (
      <label className="block">
        {label}
        <textarea value={value} onChange={(e) => onChange(e.target.value)} rows={2} className={cls} />
      </label>
    );
  }
  if (field.field_type === "checkbox") {
    return (
      <label className="flex items-center gap-3 rounded-lg border border-border-secondary px-3 py-2 text-sm">
        <input
          type="checkbox"
          checked={value === "1"}
          onChange={(e) => onChange(e.target.checked ? "1" : "0")}
          className="size-4 accent-brand-600"
        />
        {field.label}
      </label>
    );
  }
  if (field.field_type === "select") {
    return (
      <label className="block">
        {label}
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          required={field.required}
          className={cls}
        >
          <option value="">Pilih…</option>
          {(options.data ?? []).map((o) => (
            <option key={o.id} value={o.id}>{o.name}</option>
          ))}
        </select>
      </label>
    );
  }
  return (
    <label className="block">
      {label}
      <input
        type={field.field_type === "number" ? "number" : field.field_type === "email" ? "email" : field.field_type === "date" ? "date" : "text"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        required={field.required}
        className={cls}
      />
    </label>
  );
}

function ChartView() {
  const chart = useQuery({
    queryKey: ["orgChart"],
    queryFn: () => unwrap(commands.orgChart()),
  });

  if (chart.isPending) return <p className="text-sm text-text-tertiary">Memuat bagan…</p>;
  if (chart.isError)
    return <p className="text-sm text-text-error-primary">Gagal memuat bagan.</p>;

  const renderCompany = (c: CompanyNode) => (
    <div key={c.id} className="rounded-xl border border-border-secondary bg-bg-primary p-5">
      <div className="flex items-center justify-between">
        <p className="font-semibold">{c.name}</p>
        <span className="text-xs text-text-tertiary">{c.employee_count} karyawan</span>
      </div>
      <div className="mt-3 space-y-3">
        {c.branches.map((b) => (
          <div key={b.id} className="ml-4 border-l-2 border-border-brand pl-4">
            <p className="text-sm font-medium">{b.name}</p>
            <div className="mt-2 space-y-2">
              {b.departments.map((d) => (
                <div key={d.id} className="rounded-lg bg-bg-secondary p-3">
                  <div className="flex items-center justify-between text-sm">
                    <span className="font-medium">{d.name}</span>
                    <span className="text-xs text-text-tertiary">
                      {d.employee_count} karyawan{d.head_name ? ` • ${d.head_name}` : ""}
                    </span>
                  </div>
                  {d.divisions.length > 0 && (
                    <div className="mt-1 flex flex-wrap gap-1">
                      {d.divisions.map((v) => (
                        <span
                          key={v.id}
                          className="rounded-full bg-bg-brand-primary px-2 py-0.5 text-xs text-text-brand-primary"
                        >
                          {v.name}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              ))}
              {b.departments.length === 0 && (
                <p className="text-xs text-text-tertiary">Belum ada departemen.</p>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );

  return <div className="space-y-3">{chart.data.map(renderCompany)}</div>;
}
