import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useSettingsStore } from "../stores/useSettingsStore";

export type DetailWindowModule = "plan" | "mem" | "log" | "output" | "evolution";

/** 从 hash 解析模块，如 #detail/plan 或 #detail/plan?w=... → "plan"（忽略 query，避免匹配失败） */
export function parseDetailModuleFromHash(): DetailWindowModule | null {
  const hash = window.location.hash.replace(/^#/, "");
  const pathPart = hash.split("?")[0];
  const m = pathPart.match(/^detail\/(plan|mem|log|output|evolution)$/);
  return (m?.[1] as DetailWindowModule) ?? null;
}

/**
 * 详情窗口 URL 中由主窗口传入的工作区路径。
 * 独立 WebView 往往与主窗口不共享 zustand 持久化，须靠 URL 对齐 workspace。
 */
export function parseDetailWorkspaceFromUrl(): string | null {
  const hash = window.location.hash.replace(/^#/, "");
  const q = hash.split("?")[1];
  if (!q) return null;
  const w = new URLSearchParams(q).get("w");
  return w != null && w.trim() !== "" ? w : null;
}

const MODULE_TITLES: Record<DetailWindowModule, string> = {
  plan: "任务计划",
  mem: "记忆",
  log: "执行日志",
  output: "输出",
  evolution: "自进化与审核",
};

/** 打开全新窗口显示详情，紧贴当前窗口右侧 */
export async function openDetailWindow(module: DetailWindowModule) {
  const mainWindow = getCurrentWindow();
  const [pos, size] = await Promise.all([
    mainWindow.outerPosition(),
    mainWindow.outerSize(),
  ]);
  const x = pos.x + size.width;
  const y = pos.y;

  const label = `detail-${module}`;
  const base = `${window.location.origin}${window.location.pathname || "/"}`.replace(/\/$/, "");
  const workspace = useSettingsStore.getState().settings.workspace || ".";
  const qs = new URLSearchParams({ w: workspace }).toString();
  const url = `${base}#detail/${module}?${qs}`;
  const wide = module === "evolution";
  new WebviewWindow(label, {
    url,
    title: MODULE_TITLES[module],
    x: Math.round(x),
    y: Math.round(y),
    width: wide ? 520 : 420,
    height: wide ? 640 : 560,
    resizable: true,
  });
}
