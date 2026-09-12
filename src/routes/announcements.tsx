import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import {
  commands,
  type Announcement,
  type AnnouncementInput,
  type EmployeeFilter,
  type TargetInput,
} from "../bindings";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/announcements")({
  component: AnnouncementsPage,
});

const TARGET_LABEL: Record<string, string> = {
  all: "Semua",
  department: "Departemen",
  branch: "Cabang",
  role: "Peran",
  selected: "Karyawan terpilih",
};

const EMPTY_FILTER: EmployeeFilter = {
  department_id: null,
  position_id: null,
  branch_id: null,
  employment_status: null,
  gender: null,
  employment_type: null,
};

function AnnouncementsPage() {
  const session = useSession();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<"mine" | "manage">("mine");
  const [detailId, setDetailId] = useState<number | null>(null);
  const [formOpen, setFormOpen] = useState(false);

  const canManage =
    can(session.data, "announcement.view", "announcement.create", "system.manage");
  const visible = useQuery({
    queryKey: ["announcementsVisible"],
    queryFn: () => unwrap(commands.announcementVisible()),
  });
  const managed = useQuery({
    queryKey: ["announcementsAll"],
    queryFn: () => unwrap(commands.announcementList()),
    enabled: tab === "manage" && canManage,
  });
  const detail = useQuery({
    queryKey: ["announcement", detailId],
    queryFn: () => unwrap(commands.announcementGet(detailId ?? 0)),
    enabled: detailId !== null,
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["announcementsVisible"] });
    void queryClient.invalidateQueries({ queryKey: ["announcementsAll"] });
    void queryClient.invalidateQueries({ queryKey: ["announcement"] });
    void queryClient.invalidateQueries({ queryKey: ["dashboardMe"] });
  };
  const openDetail = (id: number) => setDetailId(id);
  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.announcementDelete(id)),
    onSuccess: () => {
      toast.success("Pengumuman dihapus.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Pengumuman</h1>
        {tab === "manage" && can(session.data, "announcement.create", "system.manage") && (
          <button
            type="button"
            onClick={() => setFormOpen(true)}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
          >
            Buat pengumuman
          </button>
        )}
      </div>
      <div className="flex gap-2">
        {(["mine", "manage"] as const).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={`rounded-lg px-4 py-2 text-sm font-medium ${
              tab === t
                ? "bg-bg-brand-primary text-text-brand-primary"
                : "border border-border-primary hover:bg-bg-primary_hover"
            }`}
          >
            {t === "mine" ? "Untuk saya" : "Kelola"}
          </button>
        ))}
      </div>

      {tab === "mine" && (
        <div className="space-y-3">
          {(visible.data ?? []).map((a) => (
            <button
              key={a.id}
              type="button"
              onClick={() => openDetail(a.id)}
              className="w-full rounded-xl border border-border-secondary bg-bg-primary p-4 text-left hover:border-border-brand"
            >
              <span className="flex items-center gap-2">
                {!a.read_at && (
                  <span className="rounded-full bg-bg-brand-primary px-2 py-0.5 text-xs font-medium text-text-brand-primary">
                    Baru
                  </span>
                )}
                <span className="font-semibold">{a.title}</span>
              </span>
              <span className="mt-1 block text-sm text-text-tertiary">
                Target: {TARGET_LABEL[a.target_type] ?? a.target_type}
              </span>
            </button>
          ))}
          {(visible.data ?? []).length === 0 && (
            <p className="text-sm text-text-tertiary">
              {visible.isPending ? "Memuat…" : "Tidak ada pengumuman."}
            </p>
          )}
        </div>
      )}

      {tab === "manage" && !canManage && (
        <p className="text-sm text-text-tertiary">Akses ditolak.</p>
      )}
      {tab === "manage" && canManage && (
        <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b border-border-secondary text-xs text-text-tertiary">
                {["Judul", "Target", "Status", "Aksi"].map((h) => (
                  <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {(managed.data ?? []).map((a) => (
                <tr key={a.id} className="border-b border-border-tertiary last:border-0">
                  <td className="px-4 py-2.5 font-medium">{a.title}</td>
                  <td className="px-4 py-2.5 text-text-secondary">
                    {TARGET_LABEL[a.target_type] ?? a.target_type}
                    {a.targets.length > 0 ? ` (${a.targets.length})` : ""}
                  </td>
                  <td className="px-4 py-2.5">{a.status}</td>
                  <td className="px-4 py-2.5">
                    {can(session.data, "announcement.delete", "system.manage") && (
                      <button
                        type="button"
                        onClick={() => {
                          if (window.confirm("Hapus pengumuman ini?")) remove.mutate(a.id);
                        }}
                        className="text-sm font-medium text-red-600 hover:underline"
                      >
                        Hapus
                      </button>
                    )}
                  </td>
                </tr>
              ))}
              {(managed.data ?? []).length === 0 && (
                <tr>
                  <td colSpan={4} className="px-4 py-6 text-center text-sm text-text-tertiary">
                    {managed.isPending ? "Memuat…" : "Belum ada pengumuman."}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {detailId !== null && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
          <div className="max-h-[80vh] w-full max-w-lg overflow-y-auto rounded-xl bg-bg-primary p-5">
            {detail.data ? (
              <DetailBody
                a={detail.data}
                onClose={() => {
                  setDetailId(null);
                  refresh();
                }}
              />
            ) : (
              <p className="text-sm text-text-tertiary">Memuat…</p>
            )}
          </div>
        </div>
      )}
      {formOpen && (
        <AnnouncementForm
          onClose={() => setFormOpen(false)}
          onSaved={() => {
            setFormOpen(false);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function DetailBody({ a, onClose }: { a: Announcement; onClose: () => void }) {
  return (
    <div className="space-y-3">
      <h2 className="text-lg font-semibold">{a.title}</h2>
      <p className="whitespace-pre-wrap text-sm">{a.content}</p>
      <p className="text-xs text-text-tertiary">
        Target: {TARGET_LABEL[a.target_type] ?? a.target_type}
        {a.targets.length > 0 &&
          ` · ${a.targets.map((t) => t.target_name ?? `#${t.target_id}`).join(", ")}`}
      </p>
      <button
        type="button"
        onClick={onClose}
        className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover"
      >
        Tutup
      </button>
    </div>
  );
}

function AnnouncementForm({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [targetType, setTargetType] = useState("all");
  const [publishAt, setPublishAt] = useState("");
  const [expireAt, setExpireAt] = useState("");
  const [picked, setPicked] = useState<number[]>([]);

  const depts = useQuery({
    queryKey: ["annTargets", "department"],
    queryFn: () => unwrap(commands.orgList("departments", "", 1, 200)),
    enabled: targetType === "department",
  });
  const branches = useQuery({
    queryKey: ["annTargets", "branch"],
    queryFn: () => unwrap(commands.orgList("branches", "", 1, 200)),
    enabled: targetType === "branch",
  });
  const roles = useQuery({
    queryKey: ["annTargets", "role"],
    queryFn: () => unwrap(commands.listRoles()),
    enabled: targetType === "role",
  });
  const emps = useQuery({
    queryKey: ["annTargets", "selected"],
    queryFn: () => unwrap(commands.employeeList("", EMPTY_FILTER, 1, 200)),
    enabled: targetType === "selected",
  });

  const options: { id: number; name: string }[] =
    targetType === "department"
      ? (depts.data?.rows ?? []).map((r) => ({
          id: Number(r["id"] ?? 0),
          name: r["name"] ?? `#${r["id"]}`,
        }))
      : targetType === "branch"
        ? (branches.data?.rows ?? []).map((r) => ({
            id: Number(r["id"] ?? 0),
            name: r["name"] ?? `#${r["id"]}`,
          }))
        : targetType === "role"
          ? (roles.data ?? []).map((r) => ({ id: r.id, name: r.name }))
          : (emps.data?.rows ?? []).map((e) => ({
              id: e.id,
              name: `${e.first_name}${e.last_name ? ` ${e.last_name}` : ""} (${e.employee_number})`,
            }));

  const save = useMutation({
    mutationFn: (input: AnnouncementInput) => unwrap(commands.announcementCreate(input)),
    onSuccess: () => {
      toast.success("Pengumuman diterbitkan.");
      onSaved();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const toggle = (id: number) =>
    setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <form
        className="max-h-[85vh] w-full max-w-lg space-y-3 overflow-y-auto rounded-xl bg-bg-primary p-5"
        onSubmit={(e) => {
          e.preventDefault();
          const targets: TargetInput[] =
            targetType === "all"
              ? []
              : picked.map((id) => ({ target_type: targetType === "selected" ? "employee" : targetType, target_id: id }));
          save.mutate({
            title,
            content,
            target_type: targetType,
            publish_at: publishAt || null,
            expire_at: expireAt || null,
            targets,
          });
        }}
      >
        <h2 className="text-lg font-semibold">Pengumuman baru</h2>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Judul"
          className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
        />
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Isi pengumuman"
          rows={4}
          className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
        />
        <label className="block text-sm">
          <span className="mb-1 block text-text-secondary">Target</span>
          <select
            value={targetType}
            onChange={(e) => {
              setTargetType(e.target.value);
              setPicked([]);
            }}
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
          >
            {Object.entries(TARGET_LABEL).map(([v, l]) => (
              <option key={v} value={v}>{l}</option>
            ))}
          </select>
        </label>
        {targetType !== "all" && (
          <div className="max-h-40 space-y-1 overflow-y-auto rounded-lg border border-border-secondary p-2">
            {options.map((o) => (
              <label key={o.id} className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={picked.includes(o.id)}
                  onChange={() => toggle(o.id)}
                />
                {o.name}
              </label>
            ))}
            {options.length === 0 && (
              <p className="text-xs text-text-tertiary">Memuat pilihan…</p>
            )}
          </div>
        )}
        <div className="grid grid-cols-2 gap-2">
          <label className="block text-sm">
            <span className="mb-1 block text-text-secondary">Tayang (opsional)</span>
            <input
              type="date"
              value={publishAt}
              onChange={(e) => setPublishAt(e.target.value)}
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
            />
          </label>
          <label className="block text-sm">
            <span className="mb-1 block text-text-secondary">Berakhir (opsional)</span>
            <input
              type="date"
              value={expireAt}
              onChange={(e) => setExpireAt(e.target.value)}
              className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
            />
          </label>
        </div>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover"
          >
            Batal
          </button>
          <button
            type="submit"
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
          >
            Terbitkan
          </button>
        </div>
      </form>
    </div>
  );
}
