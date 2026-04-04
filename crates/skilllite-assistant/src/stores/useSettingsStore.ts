import { create } from "zustand";
import { persist } from "zustand/middleware";

/** 界面语言：中文默认，可切换英文 */
export type UiLocale = "zh" | "en";

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
  /**
   * 覆盖 SKILLLITE_MAX_ITERATIONS（Agent 外层循环上限）。未设置时沿用环境变量 / 默认 50。
   */
  maxIterations?: number;
  /**
   * 覆盖 SKILLLITE_MAX_TOOL_CALLS_PER_TASK（单任务内工具调用深度等）。未设置时沿用环境变量 / 默认 15。
   */
  maxToolCallsPerTask?: number;
  /** 会话侧边栏是否折叠 */
  sessionPanelCollapsed?: boolean;
  /** 界面语言 */
  locale?: UiLocale;
  /**
   * 自动允许「执行确认」（工具执行前弹窗）；默认关闭以降低误操作风险。
   * 持久化在 localStorage，与 `SETTINGS_STORE_PERSIST_KEY` 一致。
   */
  autoApproveToolConfirmations?: boolean;
  /**
   * 覆盖 `SKILLLITE_EVOLUTION_INTERVAL_SECS`（Life Pulse 周期与状态展示）。不设则沿用工作区已合并配置 / 默认 600。
   */
  evolutionIntervalSecs?: number;
  /**
   * 覆盖 `SKILLLITE_EVOLUTION_DECISION_THRESHOLD`。不设则沿用工作区已合并配置 / 默认 10。
   */
  evolutionDecisionThreshold?: number;
  /**
   * 覆盖 `SKILLLITE_EVO_PROFILE`：`demo` | `conservative`。不设则跟随工作区已合并配置。
   */
  evoProfile?: "demo" | "conservative";
  /**
   * 覆盖 `SKILLLITE_EVO_COOLDOWN_HOURS`（被动提案冷却，小时）。不设则沿用工作区已合并配置 / 内置默认。
   */
  evoCooldownHours?: number;
}

/** localStorage 键名；详情窗口与主窗口同步设置时需与此一致 */
export const SETTINGS_STORE_PERSIST_KEY = "skilllite-assistant-settings";

const defaultSettings: Settings = {
  provider: "api",
  apiKey: "",
  model: "gpt-4o",
  workspace: ".",
  apiBase: "",
  locale: "zh",
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
    { name: SETTINGS_STORE_PERSIST_KEY }
  )
);
