import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type EmployeeInput } from "../bindings";
import { AddressTab, ChildTab, DocumentTab, SalaryTab } from "../components/employee-tabs";
import { EmployeeForm, useDropdowns } from "../components/EmployeeForm";
import { formatDate, formatDateLong } from "../lib/format";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/employees/$employeeId")({
  component: EmployeeDetailPage,
});

const TABS = [
  "Ringkasan",
  "Ubah Data",
  "Keluarga",
  "Pendidikan",
  "Pengalaman",
  "Kontak",
  "Kontrak",
  "Karier",
  "Alamat",
  "Dokumen",
  "Gaji",
] as const;

const CHILD_SLUG: Record<string, string> = {
  Keluarga: "families",
  Pendidikan: "educations",
  Pengalaman: "experiences",
  Kontak: "contacts",
  Kontrak: "contracts",
  Karier: "career-histories",
};

function EmployeeDetailPage() {
  const { employeeId } = Route.useParams();
  const id = Number(employeeId);
  const { data: session } = useSession();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [tab, setTab] = useState<(typeof TABS)[number]>("Ringkasan");

  const detail = useQuery({
    queryKey: ["employee", id],
    queryFn: () => unwrap(commands.employeeDetail(id)),
  });
  const childTypes = useQuery({
    queryKey: ["childTypes"],
    queryFn: () => unwrap(commands.employeeChildTypes()),
  });

  const remove = useMutation({
    mutationFn: () => unwrap(commands.employeeDelete(id)),
    onSuccess: () => {
      toast.success("Karyawan dihapus.");
      void navigate({ to: "/employees" });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  if (detail.isPending) return <p className="text-sm text-text-tertiary">Memuat…</p>;
  if (!detail.data) return <p className="text-sm text-text-error-primary">Karyawan tidak ditemukan.</p>;

  const d = detail.data;
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["employee", id] });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-display-sm font-semibold">
            {d.first_name} {d.last_name ?? ""}
          </h1>
          <p className="text-sm text-text-tertiary">
            {d.employee_number} • {d.position_name ?? "-"} • {d.department_name ?? "-"}
          </p>
        </div>
        {can(session, "employee.delete") && (
          <button
            type="button"
            onClick={() => {
              if (window.confirm(`Hapus ${d.first_name}?`)) remove.mutate();
            }}
            className="rounded-lg border border-border-error px-4 py-2 text-sm font-medium text-text-error-primary hover:bg-bg-error-primary"
          >
            Hapus
          </button>
        )}
      </div>
      <div className="flex flex-wrap gap-1 rounded-xl border border-border-secondary bg-bg-primary p-1">
        {TABS.map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={`rounded-lg px-3 py-1.5 text-sm font-medium transition ${
              tab === t
                ? "bg-bg-brand-primary text-text-brand-primary"
                : "text-text-secondary hover:bg-bg-primary_hover"
            }`}
          >
            {t}
          </button>
        ))}
      </div>
      {tab === "Ringkasan" && <Overview employeeId={id} />}
      {tab === "Ubah Data" && (
        <EditTab employeeId={id} onSaved={refresh} canEdit={can(session, "employee.update")} />
      )}
      {CHILD_SLUG[tab] && (
        <ChildTab
          meta={
            childTypes.data?.find((c) => c.slug === CHILD_SLUG[tab]) ?? {
              slug: CHILD_SLUG[tab],
              title: tab,
              fields: [],
            }
          }
          employeeId={id}
        />
      )}
      {tab === "Alamat" && <AddressTab employeeId={id} />}
      {tab === "Dokumen" && <DocumentTab employeeId={id} />}
      {tab === "Gaji" && <SalaryTab employeeId={id} />}
    </div>
  );
}

function Overview({ employeeId }: { employeeId: number }) {
  const detail = useQuery({
    queryKey: ["employee", employeeId],
    queryFn: () => unwrap(commands.employeeDetail(employeeId)),
  });
  if (!detail.data) return null;
  const d = detail.data;
  const rows: [string, string][] = [
    ["Nomor", d.employee_number],
    ["NIK", d.nik ?? "-"],
    ["Nama", `${d.first_name} ${d.last_name ?? ""}`.trim()],
    ["Jenis kelamin", d.gender === "male" ? "Laki-laki" : "Perempuan"],
    ["Tempat, tanggal lahir", `${d.birth_place ?? "-"}, ${d.birth_date ? formatDateLong(d.birth_date) : "-"}`],
    ["Agama", d.religion ?? "-"],
    ["Status pernikahan", d.marital_status],
    ["Telepon", d.phone ?? "-"],
    ["Email", d.personal_email ?? "-"],
    ["Perusahaan", d.company_name ?? "-"],
    ["Cabang", d.branch_name ?? "-"],
    ["Departemen", d.department_name ?? "-"],
    ["Divisi", d.division_name ?? "-"],
    ["Seksi", d.section_name ?? "-"],
    ["Jabatan", d.position_name ?? "-"],
    ["Level / Grade", `${d.job_level_name ?? "-"} / ${d.job_grade_name ?? "-"}`],
    ["Lokasi", d.work_location_name ?? "-"],
    ["Supervisor", d.supervisor_name ?? "-"],
    ["Manajer", d.manager_name ?? "-"],
    ["Tanggal masuk", formatDate(d.join_date)],
    ["Pengangkatan", d.appointment_date ? formatDate(d.appointment_date) : "-"],
    ["Resign", d.resign_date ? formatDate(d.resign_date) : "-"],
    ["Status", `${d.employment_status} • ${d.employment_type}`],
    ["Bank", [d.bank_name, d.bank_account_number, d.bank_account_holder].filter(Boolean).join(" • ") || "-"],
    ["NPWP", d.npwp ?? "-"],
    ["PTKP", d.ptkp_status ?? "-"],
    ["BPJS Kesehatan", d.bpjs_health_number ?? "-"],
    ["BPJS Ketenagakerjaan", d.bpjs_employment_number ?? "-"],
  ];
  return (
    <dl className="grid gap-x-8 gap-y-2 rounded-xl border border-border-secondary bg-bg-primary p-6 text-sm sm:grid-cols-2">
      {rows.map(([k, v]) => (
        <div key={k} className="flex gap-3">
          <dt className="w-44 shrink-0 text-text-tertiary">{k}</dt>
          <dd className="font-medium">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

function EditTab({
  employeeId,
  onSaved,
  canEdit,
}: {
  employeeId: number;
  onSaved: () => void;
  canEdit: boolean;
}) {
  const detail = useQuery({
    queryKey: ["employee", employeeId],
    queryFn: () => unwrap(commands.employeeDetail(employeeId)),
  });
  const dropdowns = useDropdowns();
  const [input, setInput] = useState<EmployeeInput | null>(null);
  const [photo, setPhoto] = useState<{
    data: { name: string; mime: string; bytes: number[] } | null;
    label: string;
  }>({ data: null, label: "" });
  const [resign, setResign] = useState("");

  const current: EmployeeInput | undefined = input ?? (detail.data
    ? {
        nik: detail.data.nik,
        first_name: detail.data.first_name,
        last_name: detail.data.last_name,
        gender: detail.data.gender,
        birth_place: detail.data.birth_place,
        birth_date: detail.data.birth_date,
        religion: detail.data.religion,
        marital_status: detail.data.marital_status,
        phone: detail.data.phone,
        personal_email: detail.data.personal_email,
        company_id: detail.data.company_id,
        branch_id: detail.data.branch_id,
        department_id: detail.data.department_id,
        division_id: detail.data.division_id,
        section_id: detail.data.section_id,
        position_id: detail.data.position_id,
        job_level_id: detail.data.job_level_id,
        job_grade_id: detail.data.job_grade_id,
        work_location_id: detail.data.work_location_id,
        cost_center_id: detail.data.cost_center_id,
        supervisor_id: detail.data.supervisor_id,
        manager_id: detail.data.manager_id,
        join_date: detail.data.join_date,
        appointment_date: detail.data.appointment_date,
        employment_status: detail.data.employment_status,
        employment_type: detail.data.employment_type,
        bank_name: detail.data.bank_name,
        bank_account_number: detail.data.bank_account_number,
        bank_account_holder: detail.data.bank_account_holder,
        npwp: detail.data.npwp,
        ptkp_status: detail.data.ptkp_status,
        bpjs_health_number: detail.data.bpjs_health_number,
        bpjs_employment_number: detail.data.bpjs_employment_number,
      }
    : undefined);

  const save = useMutation({
    mutationFn: () =>
      unwrap(commands.employeeUpdate(employeeId, current!, resign || null, photo.data)),
    onSuccess: () => {
      toast.success("Data karyawan disimpan.");
      onSaved();
    },
    onError: (e: Error) => toast.error(e.message),
  });

  if (detail.isPending || !current) return <p className="text-sm text-text-tertiary">Memuat…</p>;
  if (!canEdit) return <p className="text-sm text-text-tertiary">Tidak ada izin mengubah.</p>;

  return (
    <form
      className="space-y-4"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <EmployeeForm
        input={current}
        setInput={setInput}
        dropdowns={dropdowns.data}
        photoName={photo.label || detail.data?.photo || ""}
        setPhoto={(data, label) => setPhoto({ data, label })}
      />
      <label className="block max-w-xs">
        <span className="mb-1 block text-sm font-medium text-text-secondary">
          Tanggal resign (opsional)
        </span>
        <input
          type="date"
          value={resign || detail.data?.resign_date || ""}
          onChange={(e) => setResign(e.target.value)}
          className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
        />
      </label>
      <div className="flex justify-end">
        <button
          type="submit"
          disabled={save.isPending}
          className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white disabled:opacity-60"
        >
          Simpan perubahan
        </button>
      </div>
    </form>
  );
}
