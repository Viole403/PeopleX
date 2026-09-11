import { useMutation } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands, type EmployeeInput } from "../bindings";
import { EMPTY_INPUT, EmployeeForm, useDropdowns } from "../components/EmployeeForm";
import { unwrap } from "../lib/query";

export const Route = createFileRoute("/employees/new")({
  component: NewEmployeePage,
});

function NewEmployeePage() {
  const navigate = useNavigate();
  const dropdowns = useDropdowns();
  const [input, setInput] = useState<EmployeeInput>(EMPTY_INPUT);
  const [photo, setPhoto] = useState<{
    data: { name: string; mime: string; bytes: number[] } | null;
    label: string;
  }>({ data: null, label: "" });

  const save = useMutation({
    mutationFn: () => unwrap(commands.employeeCreate(input, photo.data)),
    onSuccess: (id) => {
      toast.success("Karyawan ditambahkan.");
      void navigate({ to: "/employees/$employeeId", params: { employeeId: String(id) } });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <form
      className="space-y-4"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <div className="flex items-center justify-between">
        <h1 className="text-display-sm font-semibold">Tambah Karyawan</h1>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void navigate({ to: "/employees" })}
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
      </div>
      <EmployeeForm
        input={input}
        setInput={setInput}
        dropdowns={dropdowns.data}
        photoName={photo.label}
        setPhoto={(data, label) => setPhoto({ data, label })}
      />
    </form>
  );
}
