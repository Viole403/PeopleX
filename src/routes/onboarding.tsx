import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { commands } from "../bindings";
import { formatDate } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/onboarding")({
  component: OnboardingPage,
});

function OnboardingPage() {
  const queryClient = useQueryClient();
  const list = useQuery({
    queryKey: ["onboarding"],
    queryFn: () => unwrap(commands.onboardingList()),
  });
  const toggle = useMutation({
    mutationFn: (v: { id: number; done: boolean }) =>
      unwrap(commands.onboardingToggle(v.id, v.done)),
    onSuccess: ([progress]) => {
      if (progress >= 100) toast.success("Onboarding selesai.");
      void queryClient.invalidateQueries({ queryKey: ["onboarding"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <h1 className="text-display-sm font-semibold">Onboarding</h1>
      {(list.data ?? []).map((o) => (
        <div key={o.id} className="rounded-xl border border-border-secondary bg-bg-primary p-5">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <p className="font-semibold">
                {o.employee_name} <span className="font-normal text-text-tertiary">({o.employee_number})</span>
              </p>
              <p className="text-xs text-text-tertiary">
                Mulai {formatDate(o.start_date)} • {o.status === "completed" ? "Selesai" : "Berjalan"}
              </p>
            </div>
            <span className="text-sm font-semibold">{o.progress_percent}%</span>
          </div>
          <div className="mt-2 h-2 overflow-hidden rounded-full bg-bg-tertiary">
            <div className="h-full bg-bg-brand-solid transition-all" style={{ width: `${o.progress_percent}%` }} />
          </div>
          <ul className="mt-3 space-y-1">
            {o.tasks.map((t) => (
              <li key={t.id}>
                <label className="flex cursor-pointer items-center gap-3 rounded-lg px-2 py-1.5 text-sm hover:bg-bg-primary_hover">
                  <input
                    type="checkbox"
                    checked={t.is_completed}
                    onChange={(e) => toggle.mutate({ id: t.id, done: e.target.checked })}
                    className="size-4 accent-brand-600"
                  />
                  <span className={t.is_completed ? "text-text-tertiary line-through" : ""}>
                    {t.task_name}
                  </span>
                </label>
              </li>
            ))}
          </ul>
        </div>
      ))}
      {list.data && list.data.length === 0 && (
        <p className="rounded-xl border border-border-secondary bg-bg-primary p-6 text-center text-sm text-text-tertiary">
          Belum ada onboarding. Dibuat otomatis saat kandidat di-hire.
        </p>
      )}
    </div>
  );
}
