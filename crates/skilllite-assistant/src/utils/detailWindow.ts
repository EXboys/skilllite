import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

export type DetailWindowModule = "plan" | "mem" | "log" | "output" | "evolution";

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
  const url = `${base}#detail/${module}`;
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
