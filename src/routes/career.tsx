import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { formatDate } from "../lib/format";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/career")({
  component: CareerPage,
});

function CareerPage() {
  const [vacancyId, setVacancyId] = useState<number | null>(null);
  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState("");
  const [phone, setPhone] = useState("");

  const company = useQuery({
    queryKey: ["company"],
    queryFn: () => unwrap(commands.getCompany()),
    retry: false,
  });
  const jobs = useQuery({
    queryKey: ["careerJobs"],
    queryFn: () => unwrap(commands.careerList()),
    retry: false,
  });
  const apply = useMutation({
    mutationFn: (v: { vacancyId: number; fullName: string; email: string; phone: string }) =>
      unwrap(
        commands.careerApply(
          v.vacancyId,
          { full_name: v.fullName, email: v.email, phone: v.phone || null, address: null },
          null,
        ),
      ),
    onSuccess: () => {
      toast.success("Lamaran terkirim.");
      setVacancyId(null);
      setFullName("");
      setEmail("");
      setPhone("");
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const c = company.data;
  return (
    <div className="space-y-6">
      <div className="overflow-hidden rounded-xl border border-border-secondary bg-bg-primary">
        <div className="bg-bg-brand-solid px-6 py-8 text-white">
          <h1 className="text-display-md font-bold">{c?.name ?? "Karir"}</h1>
          {c?.legal_name && c.legal_name !== c.name && (
            <p className="mt-1 text-sm opacity-80">{c.legal_name}</p>
          )}
          <p className="mt-2 max-w-2xl text-sm opacity-90">
            {[c?.city, c?.province].filter(Boolean).join(", ") || "Bergabunglah dengan tim kami"}
            {c?.email ? ` · ${c.email}` : ""}
            {c?.phone ? ` · ${c.phone}` : ""}
          </p>
        </div>
        <div className="flex flex-wrap gap-4 px-6 py-3 text-sm text-text-secondary">
          <span>{(jobs.data ?? []).length} lowongan terbuka</span>
          {c?.address && <span>{c.address}</span>}
          {c?.npwp && <span>NPWP {c.npwp}</span>}
        </div>
      </div>

      {jobs.isPending && <p className="text-sm text-text-tertiary">Memuat lowongan…</p>}
      {jobs.error && <p className="text-sm text-red-600">{(jobs.error as Error).message}</p>}
      <div className="grid gap-4 md:grid-cols-2">
        {(jobs.data ?? []).map((j) => (
          <div key={j.id} className="space-y-2 rounded-xl border border-border-secondary bg-bg-primary p-4">
            <h2 className="font-semibold">{j.title}</h2>
            <p className="text-sm text-text-secondary">
              {[j.department_name, j.position_name, j.employment_type].filter(Boolean).join(" · ")}
            </p>
            {j.description && <p className="text-sm">{j.description}</p>}
            {j.requirements && (
              <p className="text-sm text-text-secondary">Syarat: {j.requirements}</p>
            )}
            <p className="text-xs text-text-tertiary">
              Kuota {j.quota}
              {j.closing_date ? ` · Tutup ${formatDate(j.closing_date)}` : ""}
            </p>
            <button
              type="button"
              onClick={() => setVacancyId(j.id)}
              className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
            >
              Lamar
            </button>
          </div>
        ))}
      </div>
      {(jobs.data ?? []).length === 0 && !jobs.isPending && (
        <p className="text-sm text-text-tertiary">Belum ada lowongan terbuka.</p>
      )}

      {vacancyId !== null && (
        <div className="space-y-3 rounded-xl border border-border-secondary bg-bg-primary p-4">
          <h2 className="font-semibold">Formulir lamaran</h2>
          <input
            value={fullName}
            onChange={(e) => setFullName(e.target.value)}
            placeholder="Nama lengkap"
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
          />
          <input
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="Email"
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
          />
          <input
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            placeholder="Telepon (opsional)"
            className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
          />
          <div className="flex gap-2">
            <button
              type="button"
              disabled={apply.isPending}
              onClick={() => apply.mutate({ vacancyId, fullName, email, phone })}
              className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
            >
              Kirim lamaran
            </button>
            <button
              type="button"
              onClick={() => setVacancyId(null)}
              className="rounded-lg border border-border-primary px-4 py-2 text-sm"
            >
              Batal
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
