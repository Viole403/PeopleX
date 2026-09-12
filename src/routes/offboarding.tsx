import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { formatDate } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/offboarding")({
  component: OffboardingPage,
});

const STAGE_LABEL: Record<string, string> = {
  pending: "Menunggu supervisor",
  supervisor_approved: "Disetujui supervisor",
  hr_approved: "Disetujui HR",
  finance_approved: "Disetujui finance",
  completed: "Selesai",
  rejected: "Ditolak",
};

function OffboardingPage() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [detailId, setDetailId] = useState<number | null>(null);
  const list = useQuery({
    queryKey: ["offboarding"],
    queryFn: () => unwrap(commands.offboardingList()),
  });
  const mine = useQuery({
    queryKey: ["offboardingMine"],
    queryFn: () => unwrap(commands.offboardingMy()),
  });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Offboarding</h1>
        <button
          type="button"
          onClick={() => setOpen(true)}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
        >
          Ajukan resign
        </button>
      </div>
      {(mine.data ?? []).length > 0 && (
        <div className="rounded-xl border border-border-brand bg-bg-brand-primary p-4">
          <p className="text-sm font-semibold text-text-brand-primary">Pengajuan saya</p>
          {mine.data!.map((r) => (
            <button
              key={r.id}
              type="button"
              onClick={() => setDetailId(r.id)}
              className="mt-1 block text-sm text-text-brand-secondary hover:underline"
            >
              {formatDate(r.resignation_date)} → {formatDate(r.last_working_date)} • {STAGE_LABEL[r.status] ?? r.status}
            </button>
          ))}
        </div>
      )}
      <div className="space-y-2">
        {(list.data ?? []).map((r) => (
          <button
            key={r.id}
            type="button"
            onClick={() => setDetailId(r.id)}
            className="flex w-full flex-wrap items-center justify-between gap-2 rounded-xl border border-border-secondary bg-bg-primary px-4 py-3 text-left hover:bg-bg-primary_hover"
          >
            <span>
              <span className="block text-sm font-medium">
                {r.employee_name} <span className="font-normal text-text-tertiary">({r.employee_number})</span>
              </span>
              <span className="text-xs text-text-tertiary">
                {formatDate(r.last_working_date)} hari terakhir
              </span>
            </span>
            <span className="text-[13px] font-medium">{STAGE_LABEL[r.status] ?? r.status}</span>
          </button>
        ))}
        {list.data && list.data.length === 0 && (
          <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
            Belum ada proses resign.
          </p>
        )}
      </div>
      {open && (
        <ResignForm
          onClose={() => {
            setOpen(false);
            void queryClient.invalidateQueries({ queryKey: ["offboarding"] });
            void queryClient.invalidateQueries({ queryKey: ["offboardingMine"] });
          }}
        />
      )}
      {detailId !== null && (
        <DetailDialog
          id={detailId}
          onClose={() => {
            setDetailId(null);
            void queryClient.invalidateQueries({ queryKey: ["offboarding"] });
            void queryClient.invalidateQueries({ queryKey: ["offboardingMine"] });
          }}
        />
      )}
    </div>
  );
}

function ResignForm({ onClose }: { onClose: () => void }) {
  const [resign, setResign] = useState("");
  const [last, setLast] = useState("");
  const [reason, setReason] = useState("");
  const save = useMutation({
    mutationFn: () =>
      unwrap(commands.offboardingCreate({ resignation_date: resign, last_working_date: last, reason: reason || null })),
    onSuccess: () => {
      toast.success("Pengajuan resign dikirim.");
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
        <h2 className="text-display-xs font-semibold">Ajukan resign</h2>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Tgl resign</span>
            <input type="date" value={resign} onChange={(e) => setResign(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Hari terakhir</span>
            <input type="date" value={last} onChange={(e) => setLast(e.target.value)} required className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm" />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm font-medium text-text-secondary">Alasan</span>
          <textarea value={reason} onChange={(e) => setReason(e.target.value)} rows={2} maxLength={255} className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand" />
        </label>
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium">Batal</button>
          <button type="submit" disabled={save.isPending} className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60">Kirim</button>
        </div>
      </form>
    </div>
  );
}

function DetailDialog({ id, onClose }: { id: number; onClose: () => void }) {
  const queryClient = useQueryClient();
  const detail = useQuery({
    queryKey: ["offboarding", id],
    queryFn: () => unwrap(commands.offboardingGet(id)),
  });
  const [feedback, setFeedback] = useState("");
  const [recommend, setRecommend] = useState(true);
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["offboarding", id] });
    void queryClient.invalidateQueries({ queryKey: ["offboarding"] });
  };

  const decide = useMutation({
    mutationFn: (action: string) => unwrap(commands.offboardingDecide(id, action)),
    onSuccess: (next) => {
      toast.success(`Tahap menjadi: ${STAGE_LABEL[next] ?? next}`);
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const toggle = useMutation({
    mutationFn: (v: { item: number; done: boolean }) =>
      unwrap(commands.offboardingClearance(v.item, v.done, null)),
    onSuccess: () => refresh(),
    onError: (e: Error) => toast.error(e.message),
  });
  const saveExit = useMutation({
    mutationFn: () =>
      unwrap(
        commands.offboardingExitSave(id, {
          feedback: feedback || null,
          reason_category: null,
          would_recommend: recommend,
        }),
      ),
    onSuccess: () => {
      toast.success("Exit interview disimpan.");
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
                <h2 className="text-display-xs font-semibold">{d.employee_name}</h2>
                <p className="text-sm text-text-tertiary">
                  {STAGE_LABEL[d.status] ?? d.status} • tahap {d.current_step} • resign {formatDate(d.resignation_date)} • terakhir {formatDate(d.last_working_date)}
                </p>
                {d.reason && <p className="mt-1 text-sm">{d.reason}</p>}
              </div>
              <button type="button" onClick={onClose} className="rounded-lg border border-border-primary px-3 py-1.5 text-sm">Tutup</button>
            </div>
            {d.status !== "completed" && d.status !== "rejected" && (
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => decide.mutate("approve")}
                  className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white"
                >
                  Setujui tahap
                </button>
                <button
                  type="button"
                  onClick={() => {
                    if (window.confirm("Tolak proses resign ini?")) decide.mutate("reject");
                  }}
                  className="rounded-lg border border-border-error px-4 py-2 text-sm font-medium text-text-error-primary"
                >
                  Tolak
                </button>
              </div>
            )}
            <div className="rounded-xl border border-border-secondary p-4">
              <h3 className="mb-2 text-sm font-semibold">Clearance</h3>
              <ul className="space-y-1">
                {d.clearance_items.map((c) => (
                  <li key={c.id}>
                    <label className="flex cursor-pointer items-center gap-3 rounded-lg px-2 py-1.5 text-sm hover:bg-bg-primary_hover">
                      <input
                        type="checkbox"
                        checked={c.is_cleared}
                        onChange={(e) => toggle.mutate({ item: c.id, done: e.target.checked })}
                        className="size-4 accent-brand-600"
                      />
                      <span className={c.is_cleared ? "text-text-tertiary line-through" : ""}>
                        {c.item_name}
                      </span>
                    </label>
                  </li>
                ))}
              </ul>
            </div>
            <div className="rounded-xl border border-border-secondary p-4">
              <h3 className="mb-2 text-sm font-semibold">Exit interview</h3>
              {d.exit_interview?.feedback ? (
                <p className="text-sm">{d.exit_interview.feedback}</p>
              ) : (
                <div className="flex flex-wrap items-end gap-2">
                  <input
                    value={feedback}
                    onChange={(e) => setFeedback(e.target.value)}
                    placeholder="Masukan karyawan…"
                    className="min-w-0 flex-1 rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
                  />
                  <label className="flex items-center gap-2 text-sm">
                    <input type="checkbox" checked={recommend} onChange={(e) => setRecommend(e.target.checked)} className="size-4 accent-brand-600" />
                    Rekomendasikan
                  </label>
                  <button
                    type="button"
                    onClick={() => saveExit.mutate()}
                    className="rounded-lg border border-border-primary px-3 py-2 text-sm font-medium"
                  >
                    Simpan
                  </button>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
