import { Link, Outlet, useNavigate, useRouterState } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import {
  Announcement01,
  Award01,
  BankNote01,
  BarChartSquare01,
  Bell01,
  Briefcase02,
  CalendarCheck01,
  CalendarDate,
  ChevronDown,
  ChevronLeft,
  ClipboardCheck,
  Clock,
  Command,
  Database01,
  FileCheck01,
  Globe01,
  GraduationHat01,
  Home01,
  Key01,
  List,
  LogOut01,
  Menu01,
  Moon01,
  Package,
  SearchSm,
  Settings01,
  Shield01,
  Sun,
  Truck01,
  Users01,
  Wallet01,
} from "@untitledui/icons";
import { useEffect, useState } from "react";
import { can, useLogout, useSession } from "../lib/session";
import { unwrap } from "../lib/query";
import { commands } from "../bindings";
import { useTheme } from "../lib/theme";
import { useUi } from "../lib/ui";

type NavItem = { to: string; label: string; icon: typeof Home01; perm?: string | null };
type NavSection = { id: string; title: string; items: NavItem[] };

const NAV_SECTIONS: NavSection[] = [
  {
    id: "utama",
    title: "Utama",
    items: [
      { to: "/", label: "Dasbor", icon: Home01 },
      { to: "/employees", label: "Karyawan", icon: Users01 },
      { to: "/announcements", label: "Pengumuman", icon: Announcement01 },
      { to: "/reports", label: "Laporan", icon: BarChartSquare01 },
    ],
  },
  {
    id: "kehadiran",
    title: "Kehadiran",
    items: [
      { to: "/attendance", label: "Absensi", icon: Clock },
      { to: "/leave", label: "Cuti & Izin", icon: CalendarCheck01 },
      { to: "/schedules", label: "Shift & Jadwal", icon: CalendarDate, perm: "attendance.view" },
    ],
  },
  {
    id: "talenta",
    title: "Talenta",
    items: [
      { to: "/recruitment", label: "Rekrutmen", icon: Briefcase02 },
      { to: "/career", label: "Halaman Karir", icon: Globe01 },
      { to: "/onboarding", label: "Onboarding", icon: ClipboardCheck },
      { to: "/offboarding", label: "Offboarding", icon: FileCheck01 },
      { to: "/performance", label: "Kinerja", icon: Award01 },
      { to: "/goals", label: "OKR & 360", icon: BarChartSquare01 },
      { to: "/training", label: "Training", icon: GraduationHat01 },
    ],
  },
  {
    id: "keuangan",
    title: "Keuangan",
    items: [
      { to: "/payroll", label: "Penggajian", icon: BankNote01 },
      { to: "/travel", label: "Dinas & Reimburse", icon: Truck01 },
      { to: "/assets", label: "Aset", icon: Package },
      { to: "/global", label: "Global & FAQ", icon: Globe01 },
      { to: "/payroll-settings", label: "Pengaturan Gaji", icon: Wallet01, perm: "system.manage" },
    ],
  },
  {
    id: "admin",
    title: "Administrasi",
    items: [
      { to: "/organization", label: "Organisasi", icon: Globe01, perm: "organization.view" },
      { to: "/users", label: "Pengguna", icon: Key01, perm: "rbac.manage" },
      { to: "/roles", label: "Peran", icon: Shield01, perm: "rbac.manage" },
      { to: "/settings", label: "Pengaturan", icon: Settings01, perm: "settings.manage" },
      { to: "/workforce", label: "Workforce", icon: Users01, perm: "organization.view" },
      { to: "/it", label: "IT & ESOP", icon: Shield01, perm: "asset.view" },
      { to: "/audit", label: "Audit Log", icon: List, perm: "audit.view" },
      { to: "/status", label: "Status Basis Data", icon: Database01 },
    ],
  },
];

const NAV_OPEN_KEY = "peoplex.nav-open";

function loadNavOpen(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(NAV_OPEN_KEY);
    if (raw) return JSON.parse(raw) as Record<string, boolean>;
  } catch {
    // abaikan penyimpanan rusak, pakai default semua terbuka
  }
  return {};
}

function IconButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className="rounded-lg p-2 text-fg-quaternary transition hover:bg-bg-primary_hover hover:text-fg-secondary"
    >
      {children}
    </button>
  );
}

export function Shell() {
  const { sidebarOpen, toggleSidebar, setPaletteOpen } = useUi();
  const { effective, toggle } = useTheme();
  const { data: session, isPending } = useSession();
  const logout = useLogout();
  const navigate = useNavigate();
  const unread = useQuery({
    queryKey: ["notifications", "unread"],
    queryFn: () => unwrap(commands.notificationUnread()),
    enabled: !!session && !session.must_change_password,
    staleTime: 30_000,
  });
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  const bare = pathname === "/login" || pathname === "/change-password";

  useEffect(() => {
    if (isPending || bare) return;
    if (!session) {
      void navigate({ to: "/login" });
    } else if (session.must_change_password) {
      void navigate({ to: "/change-password" });
    }
  }, [isPending, session, bare, navigate]);

  if (isPending) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg-secondary">
        <p className="text-sm text-text-tertiary">Memuat sesi…</p>
      </div>
    );
  }

  if (bare || !session) {
    return <Outlet />;
  }

  const [navOpen, setNavOpen] = useState<Record<string, boolean>>(loadNavOpen);

  const toggleSection = (id: string) => {
    setNavOpen((prev) => {
      const next = { ...prev, [id]: !(prev[id] ?? true) };
      try {
        localStorage.setItem(NAV_OPEN_KEY, JSON.stringify(next));
      } catch {
        // penyimpanan opsional; state sesi tetap berlaku
      }
      return next;
    });
  };

  const isOpen = (section: NavSection) => {
    if (!sidebarOpen) return true;
    return navOpen[section.id] ?? true;
  };

  const visibleSections = NAV_SECTIONS.map((section) => ({
    ...section,
    items: section.items.filter((item) =>
      !item.perm ? true : can(session, item.perm, "system.manage"),
    ),
  })).filter((section) => section.items.length > 0);

  const navLink = (item: NavItem) => {
    const active = pathname === item.to;
    const Icon = item.icon;
    return (
      <Link
        key={item.to}
        to={item.to}
        title={item.label}
        className={`flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition ${
          active
            ? "bg-bg-brand-primary font-semibold text-text-brand-primary"
            : "text-text-secondary hover:bg-bg-primary_hover"
        }`}
      >
        <Icon size={20} className="shrink-0" />
        {sidebarOpen && <span className="truncate">{item.label}</span>}
      </Link>
    );
  };

  return (
    <div className="flex h-screen overflow-hidden bg-bg-secondary text-text-primary">
      <aside
        className={`flex shrink-0 flex-col border-r border-border-secondary bg-bg-primary transition-all ${
          sidebarOpen ? "w-60" : "w-16"
        }`}
      >
        <div className="flex h-14 items-center gap-2 border-b border-border-secondary px-3">
          <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-bg-brand-solid text-sm font-bold text-text-primary_on-brand">
            PX
          </div>
          {sidebarOpen && (
            <div className="min-w-0">
              <p className="truncate text-sm font-semibold">PeopleX</p>
              <p className="truncate text-xs text-text-tertiary">Kepegawaian</p>
            </div>
          )}
        </div>
        <nav className="flex-1 space-y-1 overflow-y-auto p-2">
          {visibleSections.map((section) => {
            const activeIn = section.items.some((item) => pathname === item.to);
            return (
              <div key={section.id}>
                {sidebarOpen && (
                  <button
                    type="button"
                    onClick={() => toggleSection(section.id)}
                    aria-expanded={isOpen(section)}
                    className={`flex w-full items-center gap-2 rounded-lg px-3 pb-1 pt-3 text-xs font-semibold uppercase transition hover:text-text-secondary ${
                      activeIn ? "text-text-brand-primary" : "text-text-tertiary"
                    }`}
                  >
                    <span className="flex-1 truncate text-left">{section.title}</span>
                    <ChevronDown
                      size={14}
                      className={`shrink-0 transition-transform ${isOpen(section) ? "" : "-rotate-90"}`}
                    />
                  </button>
                )}
                {isOpen(section) && section.items.map(navLink)}
              </div>
            );
          })}
        </nav>
        <div className="space-y-1 border-t border-border-secondary p-2">
          <div
            title={session.username}
            className="flex items-center gap-3 rounded-lg px-3 py-2 text-sm"
          >
            <span className="flex size-8 shrink-0 items-center justify-center rounded-full bg-bg-brand-primary text-xs font-bold text-text-brand-primary">
              {session.username.slice(0, 1).toUpperCase()}
            </span>
            {sidebarOpen && (
              <span className="min-w-0 flex-1 truncate font-medium">{session.username}</span>
            )}
          </div>
          <button
            type="button"
            onClick={toggleSidebar}
            title={sidebarOpen ? "Ciutkan sidebar" : "Bentangkan sidebar"}
            className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-text-secondary transition hover:bg-bg-primary_hover"
          >
            {sidebarOpen ? <ChevronLeft size={20} /> : <Menu01 size={20} />}
            {sidebarOpen && <span>Ciutkan</span>}
          </button>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center gap-2 border-b border-border-secondary bg-bg-primary px-4">
          <button
            type="button"
            onClick={() => setPaletteOpen(true)}
            className="flex min-w-0 flex-1 items-center gap-2 rounded-lg border border-border-secondary bg-bg-secondary px-3 py-1.5 text-sm text-text-placeholder transition hover:border-border-primary"
          >
            <SearchSm size={16} />
            <span className="truncate">Cari menu, karyawan, aksi…</span>
            <kbd className="ml-auto hidden items-center gap-1 rounded border border-border-secondary bg-bg-primary px-1.5 py-0.5 text-xs text-text-tertiary sm:flex">
              <Command size={12} />K
            </kbd>
          </button>
          <span className="relative">
            <IconButton label="Notifikasi" onClick={() => void navigate({ to: "/notifications" })}>
              <Bell01 size={20} />
            </IconButton>
            {(unread.data ?? 0) > 0 && (
              <span className="absolute -right-0.5 -top-0.5 flex size-4 items-center justify-center rounded-full bg-red-600 text-[10px] font-bold text-white">
                {(unread.data ?? 0) > 9 ? "9+" : unread.data}
              </span>
            )}
          </span>
          <IconButton
            label={effective === "dark" ? "Mode terang" : "Mode gelap"}
            onClick={() => void toggle()}
          >
            {effective === "dark" ? <Sun size={20} /> : <Moon01 size={20} />}
          </IconButton>
          <IconButton label="Keluar" onClick={() => logout.mutate()}>
            <LogOut01 size={20} />
          </IconButton>
        </header>
        <main className="flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-container px-6 py-6">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}

