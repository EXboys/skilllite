/** 与 SessionSidebar 中技能运行时探测同步：执行技能等可能写入缓存后触发重新拉取 */
export const RUNTIME_STATUS_REFRESH_EVENT = "skilllite:runtime-status-refresh";

export function notifyRuntimeStatusMayHaveChanged(): void {
  window.dispatchEvent(new CustomEvent(RUNTIME_STATUS_REFRESH_EVENT));
}
