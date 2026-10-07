import { useRouter } from "@tanstack/react-router";
import { Command } from "cmdk";
import {
  Announcement01,
  Award01,
  BankNote01,
  BarChartSquare01,
  Bell01,
  BookOpen01,
  Briefcase02,
  Building01,
  CalendarCheck01,
  CalendarDate,
  ClipboardCheck,
  Clock,
  Database01,
  FileCheck01,
  GraduationHat01,
  Home01,
  Key01,
  List,
  Lock01,
  Package,
  Settings01,
  Shield01,
  Truck01,
  UserPlus01,
  Users01,
  Wallet01,
} from "@untitledui/icons";
import { useEffect } from "react";
import { useUi } from "../lib/ui";
import { can, useSession } from "../lib/session";

type PaletteItem = {
  label: string;
  to: string;
  icon: typeof Home01;
  perm?: string;
};

const ITEMS: PaletteItem[] = [
  { label: "Dasbor", to: "/", icon: Home01 },
  { label: "Karyawan", to: "/employees", icon: Users01 },
  { label: "Tambah Karyawan", to: "/employees/new", icon: UserPlus01 },
  { label: "Absensi", to: "/attendance", icon: Clock },
  { label: "Cuti & Izin", to: "/leave", icon: CalendarCheck01 },
  { label: "Penggajian", to: "/payroll", icon: BankNote01 },
  { label: "Rekrutmen", to: "/recruitment", icon: Briefcase02 },
  { label: "Onboarding", to: "/onboarding", icon: ClipboardCheck },
  { label: "Offboarding", to: "/offboarding", icon: FileCheck01 },
  { label: "Kinerja", to: "/performance", icon: Award01 },
  { label: "Training", to: "/training", icon: GraduationHat01 },
  { label: "Aset", to: "/assets", icon: Package },
  { label: "Dinas & Reimburse", to: "/travel", icon: Truck01 },
  { label: "Pengumuman", to: "/announcements", icon: Announcement01 },
  { label: "Laporan", to: "/reports", icon: BarChartSquare01 },
  { label: "Notifikasi", to: "/notifications", icon: Bell01 },
  { label: "Pengaturan Gaji", to: "/payroll-settings", icon: Wallet01, perm: "system.manage" },
  { label: "Shift & Jadwal", to: "/schedules", icon: CalendarDate, perm: "attendance.view" },
  { label: "Pengguna", to: "/users", icon: Key01, perm: "rbac.manage" },
  { label: "Peran", to: "/roles", icon: Shield01, perm: "rbac.manage" },
  { label: "Organisasi", to: "/organization", icon: Building01, perm: "organization.view" },
  { label: "Pengaturan", to: "/settings", icon: Settings01, perm: "settings.manage" },
  { label: "Audit Log", to: "/audit", icon: List, perm: "audit.view" },
  { label: "Status Basis Data", to: "/status", icon: Database01 },
  { label: "Ganti Password", to: "/change-password", icon: Lock01 },
  { label: "Global & FAQ", to: "/global", icon: BookOpen01 },
];

export function Palette() {
  const { paletteOpen, setPaletteOpen } = useUi();
  const router = useRouter();
  const { data: session } = useSession();

  const visibleItems = ITEMS.filter((item) =>
    !item.perm ? true : can(session, item.perm, "system.manage"),
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(!paletteOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [paletteOpen, setPaletteOpen]);

  return (
    <Command.Dialog
      open={paletteOpen}
      onOpenChange={setPaletteOpen}
      label="Palet perintah"
      className="fixed left-1/2 top-24 z-50 w-full max-w-lg -translate-x-1/2 overflow-hidden rounded-xl border border-border-secondary bg-bg-primary shadow-2xl"
    >
      <Command.Input
        placeholder="Ketik perintah atau cari menu…"
        className="w-full border-b border-border-secondary bg-transparent px-4 py-3 text-sm outline-none placeholder:text-text-placeholder"
      />
      <Command.List className="max-h-72 overflow-y-auto p-2">
        <Command.Empty className="px-3 py-6 text-center text-sm text-text-tertiary">
          Tidak ada hasil.
        </Command.Empty>
        <Command.Group heading="Navigasi" className="px-2 py-1 text-xs text-text-tertiary">
          {visibleItems.map((item) => (
            <Command.Item
              key={item.to}
              value={item.label}
              onSelect={() => {
                setPaletteOpen(false);
                void router.navigate({ to: item.to });
              }}
              className="flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2 text-sm aria-selected:bg-bg-brand-primary aria-selected:text-text-brand-primary"
            >
              <item.icon size={16} className="shrink-0" />
              <span className="truncate">{item.label}</span>
            </Command.Item>
          ))}
        </Command.Group>
      </Command.List>
    </Command.Dialog>
  );
}
