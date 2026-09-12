import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type Notification } from "../bindings";
import { formatDateTime } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/notifications")({
  component: NotificationsPage,
});

function NotificationsPage() {
  const queryClient = useQueryClient();
  const [recentOnly, setRecentOnly] = useState(false);
  const list = useQuery({
    queryKey: ["notifications", recentOnly ? "recent" : "all"],
    queryFn: () =>
      unwrap(recentOnly ? commands.notificationRecent() : commands.notificationAll()),
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["notifications"] });
  };
  const markOne = useMutation({
    mutationFn: (id: number) => unwrap(commands.notificationMarkRead(id)),
    onSuccess: refresh,
    onError: (e: Error) => toast.error(e.message),
  });
  const markAll = useMutation({
    mutationFn: () => unwrap(commands.notificationMarkAll()),
    onSuccess: () => {
      toast.success("Semua notifikasi ditandai dibaca.");
      refresh();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const items: Notification[] = list.data ?? [];
  const unread = items.filter((n) => !n.is_read).length;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-display-sm font-semibold">Notifikasi</h1>
          <p className="mt-1 text-sm text-text-tertiary">
            {unread} belum dibaca.
          </p>
        </div>
        <span className="flex gap-2">
          <button
            type="button"
            onClick={() => setRecentOnly((v) => !v)}
            className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover"
          >
            {recentOnly ? "Tampilkan 100" : "Hanya 8 terbaru"}
          </button>
          <button
            type="button"
            onClick={() => markAll.mutate()}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
          >
            Tandai semua dibaca
          </button>
        </span>
      </div>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {["Judul", "Pesan", "Waktu", "Status", "Aksi"].map((h) => (
                <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {items.map((n) => (
              <tr key={n.id} className="border-b border-border-tertiary last:border-0">
                <td className="px-4 py-2.5 font-medium">{n.title}</td>
                <td className="max-w-md px-4 py-2.5 text-text-secondary">{n.message ?? "-"}</td>
                <td className="whitespace-nowrap px-4 py-2.5 text-text-secondary">
                  {formatDateTime(n.created_at)}
                </td>
                <td className="px-4 py-2.5">
                  {n.is_read ? (
                    <span className="text-text-tertiary">Dibaca</span>
                  ) : (
                    <span className="rounded-full bg-bg-brand-primary px-2 py-0.5 text-xs font-medium text-text-brand-primary">
                      Baru
                    </span>
                  )}
                </td>
                <td className="px-4 py-2.5">
                  {!n.is_read && (
                    <button
                      type="button"
                      onClick={() => markOne.mutate(n.id)}
                      className="text-sm font-medium text-text-brand-secondary hover:underline"
                    >
                      Tandai dibaca
                    </button>
                  )}
                </td>
              </tr>
            ))}
            {items.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-6 text-center text-sm text-text-tertiary">
                  {list.isPending ? "Memuat…" : "Tidak ada notifikasi."}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
