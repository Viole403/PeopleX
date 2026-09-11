import { create } from "zustand";
import { load } from "@tauri-apps/plugin-store";

export type ThemeChoice = "light" | "dark" | "system";

interface ThemeState {
  choice: ThemeChoice;
  effective: "light" | "dark";
  ready: boolean;
  init: () => Promise<void>;
  setChoice: (c: ThemeChoice) => Promise<void>;
  toggle: () => Promise<void>;
}

function resolveEffective(choice: ThemeChoice): "light" | "dark" {
  if (choice === "dark") return "dark";
  if (choice === "light") return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function apply(effective: "light" | "dark") {
  document.documentElement.classList.toggle("dark-mode", effective === "dark");
  document.documentElement.style.colorScheme = effective;
}

export const useTheme = create<ThemeState>()((set, get) => ({
  choice: "system",
  effective: "light",
  ready: false,
  init: async () => {
    let choice: ThemeChoice = "system";
    try {
      const store = await load("settings.json", { autoSave: false });
      const saved = await store.get<ThemeChoice>("theme");
      if (saved === "light" || saved === "dark" || saved === "system") choice = saved;
    } catch {
      choice = "system";
    }
    const effective = resolveEffective(choice);
    apply(effective);
    set({ choice, effective, ready: true });
  },
  setChoice: async (choice) => {
    const effective = resolveEffective(choice);
    apply(effective);
    set({ choice, effective });
    try {
      const store = await load("settings.json", { autoSave: true });
      await store.set("theme", choice);
      await store.save();
    } catch {
      // penyimpanan preferensi opsional; tema tetap berlaku sesi ini
    }
  },
  toggle: async () => {
    const next = get().effective === "dark" ? "light" : "dark";
    await get().setChoice(next);
  },
}));
