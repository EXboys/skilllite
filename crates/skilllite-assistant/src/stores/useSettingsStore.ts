import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface Settings {
  apiKey: string;
  model: string;
  workspace: string;
  apiBase: string;
  /** 是否已完成首次启动引导；仅当明确为 false 时显示 Onboarding（新安装为 false，旧数据无此字段视为已完成） */
  onboardingCompleted?: boolean;
}

const defaultSettings: Settings = {
  apiKey: "",
  model: "gpt-4o",
  workspace: ".",
  apiBase: "",
  onboardingCompleted: false,
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
