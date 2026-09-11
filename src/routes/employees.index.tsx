import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import { commands } from "../bindings";
import { formatDate } from "../lib/format";
import { unwrap } from "../lib/query";
import { can, useSession } from "../lib/session";

export const Route = createFileRoute("/employees/")({
  component: EmployeesPage,
});

const PER_PAGE = 20;

function EmployeesPage() {
  const { data: session } = useSession();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [search, setSearch] = useState("");
  const [applied, setApplied] = useState("");
  const [status, setStatus] = useState("");
  const [page, setPage] = useState(1);

  const list = useQuery({
    queryKey: ["employees", applied, status, page],
    queryFn: () =>
      unwrap(
        commands.employeeList(
          applied,
          {
            department_id: null,
            position_id: null,
            branch_id: null,
            employment_status: status || null,
            gender: null,
            employment_type: null,
          },
          page,
          PER_PAGE,
        ),
      ),
  });
  const totalPages = Math.max(1, Math.ceil((list.data?.total ?? 0) / PER_PAGE));

  const remove = useMutation({
    mutationFn: (id: number) => unwrap(commands.employeeDelete(id)),
    onSuccess: () => {
      toast.success("Karyawan dihapus.");
      void queryClient.invalidateQueries({ queryKey: ["employees"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-display-sm font-semibold">Karyawan</h1>
        {can(session, "employee.create") && (
          <button
            type="button"
            onClick={() => void navigate({ to: "/employees/new" })}
            className="rounded-lg bg-bg-brand-solid px-4 py-2 text-sm font-semibold text-white hover:bg-bg-brand-solid_hover"
          >
            Tambah karyawan
          </button>
        )}
      </div>
      <form
        className="flex flex-wrap gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          setPage(1);
          setApplied(search);
        }}
      >
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Cari nama, NIK, nomor…"
          className="w-full max-w-sm rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm outline-none placeholder:text-text-placeholder focus:border-border-brand"
        />
        <select
          value={status}
          onChange={(e) => {
            setStatus(e.target.value);
            setPage(1);
          }}
          className="rounded-lg border border-border-primary bg-bg-primary px-3 py-2 text-sm"
        >
          <option value="">Semua status</option>
          <option value="active">Aktif</option>
          <option value="probation">Probation</option>
          <option value="resigned">Resign</option>
          <option value="terminated">PHK</option>
        </select>
        <button
          type="submit"
          className="rounded-lg border border-border-primary px-4 py-2 text-sm font-medium hover:bg-bg-primary_hover"
        >
          Cari
        </button>
      </form>
      <div className="overflow-x-auto rounded-xl border border-border-secondary bg-bg-primary">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border-secondary text-xs text-text-tertiary">
              {["Nomor", "Nama", "Departemen", "Jabatan", "Masuk", "Status", "Aksi"].map((h) => (
                <th key={h} className="px-4 py-2.5 font-medium">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(list.data?.rows ?? []).map((r) => (
              <tr key={r.id} className="border-b border-border-tertiary last:border-0">
                <td className="whitespace-nowrap px-4 py-2.5 font-medium">{r.employee_number}</td>
                <td className="px-4 py-2.5">
                  <Link
                    to="/employees/$employeeId"
                    params={{ employeeId: String(r.id) }}
                    className="font-medium text-text-brand-secondary hover:underline"
                  >
                    {r.first_name} {r.last_name ?? ""}
                  </Link>
                </td>
                <td className="px-4 py-2.5 text-text-secondary">{r.department_name ?? "-"}</td>
                <td className="px-4 py-2.5 text-text-secondary">{r.position_name ?? "-"}</td>
                <td className="whitespace-nowrap px-4 py-2.5 text-text-secondary">
                  {formatDate(r.join_date)}
                </td>
                <td className="px-4 py-2.5">
                  <span className="rounded-full bg-bg-brand-primary px-2 py-0.5 text-xs font-medium text-text-brand-primary">
                    {r.employment_status}
                  </span>
                </td>
                <td className="px-4 py-2.5">
                  {can(session, "employee.delete") && (
                    <button
                      type="button"
                      onClick={() => {
                        if (window.confirm(`Hapus ${r.first_name}?`)) remove.mutate(r.id);
                      }}
                      className="text-[13px] font-medium text-text-error-primary hover:underline"
                    >
                      Hapus
                    </button>
                  )}
                </td>
              </tr>
            ))}
            {list.data && list.data.rows.length === 0 && (
              <tr>
                <td colSpan={7} className="px-4 py-6 text-center text-sm text-text-tertiary">
                  Belum ada karyawan.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <div className="flex items-center justify-between text-sm text-text-tertiary">
        <span>Total {list.data?.total ?? 0} • Halaman {page} dari {totalPages}</span>
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
    </div>
  );
}
