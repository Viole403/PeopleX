import { useQuery } from "@tanstack/react-query";
import { commands, type Dropdowns, type EmployeeInput } from "../bindings";
import { unwrap } from "../lib/query";

export const EMPTY_INPUT: EmployeeInput = {
  nik: null,
  first_name: "",
  last_name: null,
  gender: "male",
  birth_place: null,
  birth_date: null,
  religion: null,
  marital_status: "single",
  phone: null,
  personal_email: null,
  company_id: 0,
  branch_id: null,
  department_id: null,
  division_id: null,
  section_id: null,
  position_id: null,
  job_level_id: null,
  job_grade_id: null,
  work_location_id: null,
  cost_center_id: null,
  supervisor_id: null,
  manager_id: null,
  join_date: "",
  appointment_date: null,
  employment_status: "probation",
  employment_type: "contract",
  bank_name: null,
  bank_account_number: null,
  bank_account_holder: null,
  npwp: null,
  ptkp_status: null,
  bpjs_health_number: null,
  bpjs_employment_number: null,
};

export function useDropdowns() {
  return useQuery({
    queryKey: ["employeeDropdowns"],
    queryFn: () => unwrap(commands.employeeDropdowns()),
  });
}

type StrKey =
  | "nik" | "first_name" | "last_name" | "birth_place" | "birth_date" | "religion"
  | "phone" | "personal_email" | "join_date" | "appointment_date" | "bank_name"
  | "bank_account_number" | "bank_account_holder" | "npwp" | "ptkp_status"
  | "bpjs_health_number" | "bpjs_employment_number";
type EnumKey = "gender" | "marital_status" | "employment_status" | "employment_type";
type IdKey =
  | "company_id" | "branch_id" | "department_id" | "division_id" | "section_id"
  | "position_id" | "job_level_id" | "job_grade_id" | "work_location_id"
  | "cost_center_id" | "supervisor_id" | "manager_id";

const ENUMS: Record<EnumKey, [string, string][]> = {
  gender: [["male", "Laki-laki"], ["female", "Perempuan"]],
  marital_status: [["single", "Belum menikah"], ["married", "Menikah"], ["divorced", "Cerai"], ["widowed", "Janda/Duda"]],
  employment_status: [["active", "Aktif"], ["probation", "Probation"], ["resigned", "Resign"], ["terminated", "PHK"]],
  employment_type: [["permanent", "Tetap"], ["contract", "Kontrak"], ["intern", "Magang"], ["daily", "Harian"], ["freelance", "Freelance"]],
};

function Text({
  label, value, onChange, type, required,
}: {
  label: string; value: string; onChange: (v: string) => void; type?: string; required?: boolean;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-sm font-medium text-text-secondary">
        {label}{required && <span className="text-text-error-primary"> *</span>}
      </span>
      <input
        type={type ?? "text"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        required={required}
        className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none focus:border-border-brand"
      />
    </label>
  );
}

function Select({
  label, value, onChange, options, required, nullable,
}: {
  label: string; value: string; onChange: (v: string) => void;
  options: [string, string][]; required?: boolean; nullable?: boolean;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-sm font-medium text-text-secondary">
        {label}{required && <span className="text-text-error-primary"> *</span>}
      </span>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        required={required}
        className="w-full rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
      >
        {nullable && <option value="">Tidak ada</option>}
        {options.map(([v, l]) => (
          <option key={v} value={v}>{l}</option>
        ))}
      </select>
    </label>
  );
}

export function EmployeeForm({
  input,
  setInput,
  dropdowns,
  photoName,
  setPhoto,
}: {
  input: EmployeeInput;
  setInput: (v: EmployeeInput) => void;
  dropdowns: Dropdowns | undefined;
  photoName: string;
  setPhoto: (f: { name: string; mime: string; bytes: number[] } | null, label: string) => void;
}) {
  const set = (patch: Partial<EmployeeInput>) => setInput({ ...input, ...patch });
  const str = (k: StrKey) => (input[k] ?? "") as string;
  const idVal = (k: IdKey) => (input[k] ?? "") as string | number;
  const setId = (k: IdKey) => (v: string) => {
    const n = v === "" ? null : Number(v);
    if (k === "company_id") set({ company_id: (n ?? 0) as number });
    else set({ [k]: n } as Partial<EmployeeInput>);
  };
  const opts = (list: { id: number; name: string }[] | undefined): [string, string][] =>
    (list ?? []).map((o) => [String(o.id), o.name]);

  const pickPhoto = async (file: File | undefined) => {
    if (!file) {
      setPhoto(null, "");
      return;
    }
    const buf = new Uint8Array(await file.arrayBuffer());
    setPhoto({ name: file.name, mime: file.type || "application/octet-stream", bytes: [...buf] }, file.name);
  };

  return (
    <div className="space-y-6">
      <section className="rounded-xl border border-border-secondary bg-bg-primary p-5">
        <h2 className="mb-4 text-sm font-semibold uppercase text-text-tertiary">Data Pribadi</h2>
        <div className="grid gap-4 sm:grid-cols-3">
          <Text label="Nama depan" value={input.first_name} onChange={(v) => set({ first_name: v })} required />
          <Text label="Nama belakang" value={str("last_name")} onChange={(v) => set({ last_name: v || null })} />
          <Text label="NIK" value={str("nik")} onChange={(v) => set({ nik: v || null })} />
          <Select label="Jenis kelamin" value={input.gender} onChange={(v) => set({ gender: v })} options={ENUMS.gender} required />
          <Text label="Tempat lahir" value={str("birth_place")} onChange={(v) => set({ birth_place: v || null })} />
          <Text label="Tanggal lahir" type="date" value={str("birth_date")} onChange={(v) => set({ birth_date: v || null })} />
          <Text label="Agama" value={str("religion")} onChange={(v) => set({ religion: v || null })} />
          <Select label="Status pernikahan" value={input.marital_status} onChange={(v) => set({ marital_status: v })} options={ENUMS.marital_status} required />
          <Text label="Telepon" value={str("phone")} onChange={(v) => set({ phone: v || null })} />
          <Text label="Email pribadi" type="email" value={str("personal_email")} onChange={(v) => set({ personal_email: v || null })} />
          <label className="block">
            <span className="mb-1 block text-sm font-medium text-text-secondary">Foto {photoName && `(${photoName})`}</span>
            <input
              type="file"
              accept="image/jpeg,image/png,image/webp"
              onChange={(e) => void pickPhoto(e.target.files?.[0])}
              className="w-full text-sm text-text-secondary file:mr-3 file:rounded-lg file:border file:border-border-primary file:bg-bg-secondary file:px-3 file:py-2 file:text-sm"
            />
          </label>
        </div>
      </section>
      <section className="rounded-xl border border-border-secondary bg-bg-primary p-5">
        <h2 className="mb-4 text-sm font-semibold uppercase text-text-tertiary">Kepegawaian</h2>
        <div className="grid gap-4 sm:grid-cols-3">
          <Select label="Perusahaan" value={String(input.company_id || "")} onChange={setId("company_id")} options={opts(dropdowns?.companies)} required />
          <Select label="Cabang" value={String(idVal("branch_id"))} onChange={setId("branch_id")} options={opts(dropdowns?.branches)} nullable />
          <Select label="Departemen" value={String(idVal("department_id"))} onChange={setId("department_id")} options={opts(dropdowns?.departments)} nullable />
          <Select label="Divisi" value={String(idVal("division_id"))} onChange={setId("division_id")} options={opts(dropdowns?.divisions)} nullable />
          <Select label="Seksi" value={String(idVal("section_id"))} onChange={setId("section_id")} options={opts(dropdowns?.sections)} nullable />
          <Select label="Jabatan" value={String(idVal("position_id"))} onChange={setId("position_id")} options={opts(dropdowns?.positions)} nullable />
          <Select label="Job level" value={String(idVal("job_level_id"))} onChange={setId("job_level_id")} options={opts(dropdowns?.job_levels)} nullable />
          <Select label="Job grade" value={String(idVal("job_grade_id"))} onChange={setId("job_grade_id")} options={opts(dropdowns?.job_grades)} nullable />
          <Select label="Lokasi kerja" value={String(idVal("work_location_id"))} onChange={setId("work_location_id")} options={opts(dropdowns?.work_locations)} nullable />
          <Select label="Cost center" value={String(idVal("cost_center_id"))} onChange={setId("cost_center_id")} options={opts(dropdowns?.cost_centers)} nullable />
          <Select label="Supervisor" value={String(idVal("supervisor_id"))} onChange={setId("supervisor_id")} options={opts(dropdowns?.employees)} nullable />
          <Select label="Manajer" value={String(idVal("manager_id"))} onChange={setId("manager_id")} options={opts(dropdowns?.employees)} nullable />
          <Text label="Tanggal masuk" type="date" value={input.join_date} onChange={(v) => set({ join_date: v })} required />
          <Text label="Tanggal pengangkatan" type="date" value={str("appointment_date")} onChange={(v) => set({ appointment_date: v || null })} />
          <Select label="Status" value={input.employment_status} onChange={(v) => set({ employment_status: v })} options={ENUMS.employment_status} required />
          <Select label="Jenis" value={input.employment_type} onChange={(v) => set({ employment_type: v })} options={ENUMS.employment_type} required />
        </div>
      </section>
      <section className="rounded-xl border border-border-secondary bg-bg-primary p-5">
        <h2 className="mb-4 text-sm font-semibold uppercase text-text-tertiary">Bank & Pajak</h2>
        <div className="grid gap-4 sm:grid-cols-3">
          <Text label="Nama bank" value={str("bank_name")} onChange={(v) => set({ bank_name: v || null })} />
          <Text label="Nomor rekening" value={str("bank_account_number")} onChange={(v) => set({ bank_account_number: v || null })} />
          <Text label="Pemilik rekening" value={str("bank_account_holder")} onChange={(v) => set({ bank_account_holder: v || null })} />
          <Text label="NPWP" value={str("npwp")} onChange={(v) => set({ npwp: v || null })} />
          <Text label="Status PTKP" value={str("ptkp_status")} onChange={(v) => set({ ptkp_status: v || null })} />
          <Text label="No. BPJS Kesehatan" value={str("bpjs_health_number")} onChange={(v) => set({ bpjs_health_number: v || null })} />
          <Text label="No. BPJS Ketenagakerjaan" value={str("bpjs_employment_number")} onChange={(v) => set({ bpjs_employment_number: v || null })} />
        </div>
      </section>
    </div>
  );
}
