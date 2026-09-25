import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "../stores/useSettingsStore";

/**
 * 首次启动或仍为「.」/ 空时，落为 `Documents/SkillLite`（并创建目录），避免 GUI 进程 cwd 为 `/` 导致无权限。
 * 主窗口与详情窗口均需挂载，以保证独立窗口也能拿到可写绝对路径。
 */
export function useEnsureDefaultWorkspace() {
  const workspace = useSettingsStore((s) => s.settings.workspace);
  const setSettings = useSettingsStore((s) => s.setSettings);

  useEffect(() => {
    const ws = workspace?.trim() ?? "";
    if (ws !== "" && ws !== ".") return;
    if (!(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__) return;
    let cancelled = false;
    void invoke<string>("skilllite_default_workspace")
      .then((path) => {
        if (cancelled || !path) return;
        setSettings({ workspace: path });
      })
      .catch((e) => {
        console.error("[skilllite] default workspace:", e);
      });
    return () => {
      cancelled = true;
    };
  }, [workspace, setSettings]);
}
