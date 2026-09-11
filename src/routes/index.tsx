import { createFileRoute, Link } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
  component: Dashboard,
});

function Dashboard() {
  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-display-sm font-semibold">Dasbor</h1>
        <p className="mt-1 text-sm text-text-tertiary">
          Ringkasan kepegawaian akan tampil di sini setelah modul backend tersedia.
        </p>
      </div>
      <div className="rounded-xl border border-border-secondary bg-bg-primary p-6">
        <p className="text-sm text-text-secondary">
          Fondasi selesai: basis data 87 tabel, seed awal, dan bindings Rust-ke-TypeScript.
          Periksa kesehatan basis data di halaman{" "}
          <Link to="/status" className="font-medium text-text-brand-secondary hover:underline">
            Status Basis Data
          </Link>
          .
        </p>
      </div>
    </div>
  );
}
