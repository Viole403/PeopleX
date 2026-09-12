# PeopleX

Aplikasi HRIS desktop (Tauri v2 + React 19 + TypeScript + SQLite) untuk pengelolaan
kepegawaian: karyawan, kehadiran, cuti, lembur, payroll dan slip gaji PDF, rekrutmen
sampai onboarding/offboarding, penilaian kinerja dan pelatihan, aset, dinas dan
reimburse, pengumuman, notifikasi, serta dasbor dan laporan (ekspor CSV/XLSX/PDF).

## Stack

- Backend: Tauri v2, Rust (`rusqlite` + pool `r2d2` + `rusqlite_migration`), `tauri-specta`
- Frontend: React 19, Vite, TypeScript strict, TanStack (Query/Router/Form/Table, Zod),
  Zustand (state UI saja), Untitled UI + Tailwind CSS
- Package manager: `bun`

## Prasyarat

- Rust toolchain stabil + `bun`
- Linux: dependensi WebKitGTK untuk `tauri dev`/`tauri build`

## Menjalankan

```bash
bun install
bun run dev        # frontend saja (Vite)
bun run tauri dev  # aplikasi desktop (butuh display / Xvfb di headless)
```

## Build

```bash
bun run build      # tsc + vite build
cargo build        # backend (dari src-tauri/)
```

## Tes

```bash
cargo test         # unit test backend (dari src-tauri/)
```

## Versi dan rilis

- Sumber versi rilis adalah `src-tauri/Cargo.toml`.
- Samakan `package.json` dan `src-tauri/tauri.conf.json` sebelum membuat rilis.
- CI berjalan pada push ke `main` dan pull request: build frontend lalu `cargo test`.
- Installer hanya dibuat saat tag `vX.Y.Z` di-push, misalnya `v0.2.0`.
- Workflow rilis menggagalkan proses bila tag tidak sama dengan versi Cargo.

```bash
git tag v0.2.0
git push origin v0.2.0
```

## Kredensial awal

- Username: `admin` / password: `Admin@123`
- Wajib diganti saat login pertama (dipaksa oleh aplikasi).

## Struktur

- `src-tauri/src/` — `db.rs` (pool + migrasi), `services/` (logika bisnis per domain),
  `seed.rs` (data awal: peran, izin, admin), `schema_sqlite.sql`
- `src/routes/` — halaman per domain, `src/components/` — Shell, Palette, UI bersama
- `src/bindings.ts` — hasil generate `tauri-specta` (jangan edit manual)

## Aturan angka lintas batas

Nilai numerik Rust ke TypeScript memakai `i32` (batas presisi number JS);
internal SQLite tetap `i64`.
