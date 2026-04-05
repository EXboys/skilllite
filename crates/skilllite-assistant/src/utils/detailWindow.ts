import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useSettingsStore } from "../stores/useSettingsStore";

export type DetailWindowModule = "plan" | "mem" | "log" | "output" | "evolution";

/** 自进化详情窗口内的 Tab，由 `#detail/evolution?...&tab=changes` 等指定 */
export type EvolutionDetailTab = "run" | "review" | "changes";

/** 从 hash 的 query 解析 `tab=`（仅 evolution 详情使用） */
export function parseDetailEvolutionTabFromHash(): EvolutionDetailTab | null {
  const hash = window.location.hash.replace(/^#/, "");
  const q = hash.split("?")[1];
  if (!q) return null;
  const tab = new URLSearchParams(q).get("tab");
  if (tab === "run" || tab === "review" || tab === "changes") {
    return tab;
  }
  return null;
}

/** 与后端快照目录名规则一致，避免 query 注入路径 */
function isSafeEvolutionTxnQueryValue(s: string): boolean {
  const t = s.trim();
  if (!t || t.length > 256 || t.includes("..") || t === "__current__") {
    return false;
  }
  return /^[a-zA-Z0-9_.-]+$/.test(t);
}

/** 从 hash 解析 `txn=`，用于变更对比左侧默认选中该快照 */
export function parseDetailEvolutionFocusTxnFromHash(): string | null {
  const hash = window.location.hash.replace(/^#/, "");
  const q = hash.split("?")[1];
  if (!q) return null;
  const raw = new URLSearchParams(q).get("txn");
  if (!raw || !isSafeEvolutionTxnQueryValue(raw)) return null;
  return raw.trim();
}

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
export async function openDetailWindow(
  module: DetailWindowModule,
  options?: { evolutionTab?: EvolutionDetailTab; focusTxn?: string }
) {
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
  const params = new URLSearchParams({ w: workspace });
  if (module === "evolution" && options?.evolutionTab) {
    params.set("tab", options.evolutionTab);
  }
  if (
    module === "evolution" &&
    options?.focusTxn &&
    isSafeEvolutionTxnQueryValue(options.focusTxn)
  ) {
    params.set("txn", options.focusTxn.trim());
  }
  const url = `${base}#detail/${module}?${params.toString()}`;
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
