import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type Company, type Workflow } from "../bindings";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/settings")({
  component: SettingsPage,
});

const TABS = [
  { id: "umum", label: "Umum" },
  { id: "perusahaan", label: "Perusahaan" },
  { id: "persetujuan", label: "Alur Persetujuan" },
] as const;

function SettingsPage() {
  const [tab, setTab] = useState<(typeof TABS)[number]["id"]>("umum");
  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Pengaturan</h1>
      <div className="flex gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            className={`rounded-lg px-4 py-2 text-sm font-medium transition ${
              tab === t.id
                ? "bg-bg-brand-primary text-text-brand-primary"
                : "text-text-secondary hover:bg-bg-primary_hover"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tab === "umum" && <GeneralTab />}
      {tab === "perusahaan" && <CompanyTab />}
      {tab === "persetujuan" && <WorkflowTab />}
    </div>
  );
}

function GeneralTab() {
  const queryClient = useQueryClient();
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: () => unwrap(commands.getSettings()),
  });
  const [draft, setDraft] = useState<Record<string, string> | null>(null);
  const values: Record<string, string> = draft ?? Object.fromEntries(
    (settings.data ?? []).map((s) => [s.key, s.value ?? ""]),
  );

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.saveSettings(Object.entries(values).map(([k, v]) => [k, v] as [string, string])),
      ),
    onSuccess: () => {
      toast.success("Pengaturan disimpan.");
      setDraft(null);
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  if (settings.isPending) return <p className="text-sm text-text-tertiary">Memuat…</p>;
  if (settings.isError)
    return <p className="text-sm text-text-error-primary">Gagal memuat pengaturan.</p>;

  return (
    <div className="rounded-xl border border-border-secondary bg-bg-primary p-6">
      <div className="grid gap-4 sm:grid-cols-2">
        {settings.data.map((s) => (
          <label key={s.key} className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">{s.key}</span>
            <input
              value={values[s.key] ?? ""}
              onChange={(e) => setDraft({ ...values, [s.key]: e.target.value })}
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
            />
          </label>
        ))}
      </div>
      <div className="mt-5 flex justify-end">
        <button
          type="button"
          disabled={save.isPending || draft === null}
          onClick={() => save.mutate()}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
        >
          Simpan pengaturan
        </button>
      </div>
    </div>
  );
}

function CompanyTab() {
  const queryClient = useQueryClient();
  const company = useQuery({
    queryKey: ["company"],
    queryFn: () => unwrap(commands.getCompany()),
  });
  const [draft, setDraft] = useState<Partial<Company> | null>(null);
  const value = (k: keyof Company): string =>
    ((draft?.[k] as string | null | undefined) ?? (company.data?.[k] as string | null | undefined) ?? "");

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.saveCompany({
          name: value("name") || null,
          legal_name: value("legal_name") || null,
          address: value("address") || null,
          city: value("city") || null,
          province: value("province") || null,
          postal_code: value("postal_code") || null,
          phone: value("phone") || null,
          email: value("email") || null,
          npwp: value("npwp") || null,
          established_date: value("established_date") || null,
        }),
      ),
    onSuccess: () => {
      toast.success("Profil perusahaan disimpan.");
      setDraft(null);
      void queryClient.invalidateQueries({ queryKey: ["company"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  if (company.isPending) return <p className="text-sm text-text-tertiary">Memuat…</p>;

  const fields: [keyof Company, string, string][] = [
    ["name", "Nama perusahaan", "text"],
    ["legal_name", "Nama legal", "text"],
    ["address", "Alamat", "text"],
    ["city", "Kota", "text"],
    ["province", "Provinsi", "text"],
    ["postal_code", "Kode pos", "text"],
    ["phone", "Telepon", "text"],
    ["email", "Email", "email"],
    ["npwp", "NPWP", "text"],
    ["established_date", "Tanggal berdiri", "date"],
  ];

  return (
    <div className="rounded-xl border border-border-secondary bg-bg-primary p-6">
      <div className="grid gap-4 sm:grid-cols-2">
        {fields.map(([key, label, type]) => (
          <label key={key} className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">{label}</span>
            <input
              type={type}
              value={value(key)}
              onChange={(e) => setDraft({ ...(draft ?? {}), [key]: e.target.value })}
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
            />
          </label>
        ))}
      </div>
      <div className="mt-5 flex justify-end">
        <button
          type="button"
          disabled={save.isPending || draft === null}
          onClick={() => save.mutate()}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
        >
          Simpan perusahaan
        </button>
      </div>
    </div>
  );
}

const APPROVER_LABELS: Record<string, string> = {
  supervisor: "Atasan langsung",
  manager: "Manajer",
  role: "Peran",
  specific_user: "Pengguna tertentu",
  department_head: "Kepala departemen",
};

function WorkflowTab() {
  const queryClient = useQueryClient();
  const workflows = useQuery({
    queryKey: ["workflows"],
    queryFn: () => unwrap(commands.listWorkflows()),
  });
  const roles = useQuery({ queryKey: ["roles"], queryFn: () => unwrap(commands.listRoles()) });
  const users = useQuery({ queryKey: ["users"], queryFn: () => unwrap(commands.listUsers()) });
  const [addingFor, setAddingFor] = useState<Workflow | null>(null);

  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["workflows"] });

  const remove = useMutation({
    mutationFn: (stepId: number) => unwrap(commands.removeWorkflowStep(stepId)),
    onSuccess: () => {
      toast.success("Tahap dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  if (workflows.isPending) return <p className="text-sm text-text-tertiary">Memuat…</p>;
  if (workflows.isError)
    return <p className="text-sm text-text-error-primary">Gagal memuat alur.</p>;

  return (
    <div className="space-y-3">
      {workflows.data.map((w) => (
        <div key={w.id} className="rounded-xl border border-border-secondary bg-bg-primary p-5">
          <div className="flex items-center justify-between">
            <div>
              <p className="font-semibold">{w.name}</p>
              <p className="text-xs text-text-tertiary">Modul: {w.module}</p>
            </div>
            <button
              type="button"
              onClick={() => setAddingFor(w)}
              className="rounded-lg border border-border-primary px-3 py-1.5 text-sm font-medium hover:bg-bg-primary_hover"
            >
              Tambah tahap
            </button>
          </div>
          <ol className="mt-3 space-y-1">
            {w.steps.map((s, i) => (
              <li
                key={s.id}
                className="flex items-center gap-3 rounded-lg bg-bg-secondary px-3 py-2 text-sm"
              >
                <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-bg-brand-primary text-xs font-bold text-text-brand-primary">
                  {i + 1}
                </span>
                <span className="font-medium">
                  {APPROVER_LABELS[s.approver_type] ?? s.approver_type}
                </span>
                {s.role_name && <span className="text-text-tertiary">({s.role_name})</span>}
                {s.username && <span className="text-text-tertiary">({s.username})</span>}
                <button
                  type="button"
                  onClick={() => {
                    if (window.confirm("Hapus tahap ini?")) remove.mutate(s.id);
                  }}
                  className="ml-auto text-[13px] font-medium text-text-error-primary hover:underline"
                >
                  Hapus
                </button>
              </li>
            ))}
            {w.steps.length === 0 && (
              <li className="text-sm text-text-tertiary">
                Belum ada tahap — pengajuan langsung disetujui.
              </li>
            )}
          </ol>
        </div>
      ))}
      {addingFor && (
        <StepDialog
          workflow={addingFor}
          roles={roles.data ?? []}
          users={users.data ?? []}
          onClose={() => {
            setAddingFor(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function StepDialog({
  workflow,
  roles,
  users,
  onClose,
}: {
  workflow: Workflow;
  roles: { id: number; name: string }[];
  users: { id: number; username: string }[];
  onClose: () => void;
}) {
  const [type, setType] = useState("supervisor");
  const [roleId, setRoleId] = useState("");
  const [userId, setUserId] = useState("");

  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.addWorkflowStep(
          workflow.id,
          type,
          type === "role" ? Number(roleId) : null,
          type === "specific_user" ? Number(userId) : null,
        ),
      ),
    onSuccess: () => {
      toast.success("Tahap ditambahkan di urutan akhir.");
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
        <h2 className="text-display-xs font-semibold">Tambah tahap: {workflow.name}</h2>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Tipe approver</span>
          <select
            value={type}
            onChange={(e) => setType(e.target.value)}
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
          >
            {Object.entries(APPROVER_LABELS).map(([v, l]) => (
              <option key={v} value={v}>{l}</option>
            ))}
          </select>
        </label>
        {type === "role" && (
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Peran</span>
            <select
              value={roleId}
              onChange={(e) => setRoleId(e.target.value)}
              required
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
            >
              <option value="">Pilih peran…</option>
              {roles.map((r) => (
                <option key={r.id} value={r.id}>{r.name}</option>
              ))}
            </select>
          </label>
        )}
        {type === "specific_user" && (
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Pengguna</span>
            <select
              value={userId}
              onChange={(e) => setUserId(e.target.value)}
              required
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
            >
              <option value="">Pilih pengguna…</option>
              {users.map((u) => (
                <option key={u.id} value={u.id}>{u.username}</option>
              ))}
            </select>
          </label>
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
            Tambah
          </button>
        </div>
      </form>
    </div>
  );
}
