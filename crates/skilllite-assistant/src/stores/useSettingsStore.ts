import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface Settings {
  apiKey: string;
  model: string;
  workspace: string;
  apiBase: string;
}

const defaultSettings: Settings = {
  apiKey: "",
  model: "gpt-4o",
  workspace: ".",
  apiBase: "",
};

export const useSettingsStore = create<{
  settings: Settings;
  setSettings: (s: Partial<Settings>) => void;
  reset: () => void;
}>()(
  persist(
    (set) => ({
      settings: defaultSettings,
      setSettings: (partial) =>
        set((s) => ({
          settings: { ...s.settings, ...partial },
        })),
      reset: () => set({ settings: defaultSettings }),
    }),
    { name: "skilllite-assistant-settings" }
  )
);
