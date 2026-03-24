import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Provider = "api" | "ollama";

/** 沙箱安全等级：1=无沙箱, 2=基础隔离, 3=完全沙箱(默认) */
export type SandboxLevel = 1 | 2 | 3;

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
  /** 沙箱安全等级 1/2/3，默认 3（完全沙箱） */
  sandboxLevel: SandboxLevel;
  /** 是否启用 Swarm P2P 网络 */
  swarmEnabled: boolean;
  /** Swarm 节点 URL，启用时生效 */
  swarmUrl: string;
  /** 会话侧边栏是否折叠 */
  sessionPanelCollapsed?: boolean;
}

const defaultSettings: Settings = {
  provider: "api",
  apiKey: "",
  model: "gpt-4o",
  workspace: ".",
  apiBase: "",
  onboardingCompleted: false,
  showStarterPrompts: false,
  sandboxLevel: 3,
  swarmEnabled: false,
  swarmUrl: "",
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
