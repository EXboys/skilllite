import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Provider = "api" | "ollama";

export interface Settings {
  provider: Provider;
  apiKey: string;
  model: string;
  workspace: string;
  apiBase: string;
  /** 是否已完成首次启动引导；仅当明确为 false 时显示 Onboarding（新安装为 false，旧数据无此字段视为已完成） */
  onboardingCompleted?: boolean;
  /** 首次引导完成后，在聊天页展示入门操作卡片。 */
  showStarterPrompts?: boolean;
}

const defaultSettings: Settings = {
  provider: "api",
  apiKey: "",
  model: "gpt-4o",
  workspace: ".",
  apiBase: "",
  onboardingCompleted: false,
  showStarterPrompts: false,
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
