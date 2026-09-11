import { useRouter } from "@tanstack/react-router";
import { Command } from "cmdk";
import { useEffect } from "react";
import { useUi } from "../lib/ui";

const ITEMS = [
  { label: "Dasbor", to: "/" },
  { label: "Pengguna", to: "/users" },
  { label: "Peran", to: "/roles" },
  { label: "Organisasi", to: "/organization" },
  { label: "Pengaturan", to: "/settings" },
  { label: "Audit Log", to: "/audit" },
  { label: "Status Basis Data", to: "/status" },
  { label: "Ganti Password", to: "/change-password" },
];

export function Palette() {
  const { paletteOpen, setPaletteOpen } = useUi();
  const router = useRouter();

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
          {ITEMS.map((item) => (
            <Command.Item
              key={item.to}
              value={item.label}
              onSelect={() => {
                setPaletteOpen(false);
                void router.navigate({ to: item.to });
              }}
              className="cursor-pointer rounded-lg px-3 py-2 text-sm aria-selected:bg-bg-brand-primary aria-selected:text-text-brand-primary"
            >
              {item.label}
            </Command.Item>
          ))}
        </Command.Group>
      </Command.List>
    </Command.Dialog>
  );
}
