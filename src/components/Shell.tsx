import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import {
  Bell01,
  ChevronLeft,
  Command,
  Database01,
  Home01,
  LogOut01,
  Menu01,
  Moon01,
  SearchSm,
  Sun,
} from "@untitledui/icons";
import { useTheme } from "../lib/theme";
import { useUi } from "../lib/ui";

const NAV = [
  { to: "/", label: "Dasbor", icon: Home01 },
  { to: "/status", label: "Status Basis Data", icon: Database01 },
];

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
  const pathname = useRouterState({ select: (s) => s.location.pathname });

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
          {NAV.map((item) => {
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
          })}
        </nav>
        <div className="border-t border-border-secondary p-2">
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
          <IconButton label="Notifikasi" onClick={() => {}}>
            <Bell01 size={20} />
          </IconButton>
          <IconButton
            label={effective === "dark" ? "Mode terang" : "Mode gelap"}
            onClick={() => void toggle()}
          >
            {effective === "dark" ? <Sun size={20} /> : <Moon01 size={20} />}
          </IconButton>
          <IconButton label="Keluar (segera hadir)" onClick={() => {}}>
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
