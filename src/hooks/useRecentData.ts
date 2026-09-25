import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStatusStore } from "../stores/useStatusStore";
import { useSettingsStore } from "../stores/useSettingsStore";

interface RecentDataResponse {
  memory_files: string[];
  output_files: string[];
  log_files: string[];
  plan: {
    task: string;
    steps: { id: number; description: string; completed: boolean }[];
  } | null;
}

function parseRecentData(data: RecentDataResponse) {
  return {
    memoryFiles: data.memory_files ?? [],
    outputFiles: data.output_files ?? [],
    logFiles: data.log_files ?? [],
  };
}

/** Hook to load and refresh recent file lists from skilllite（不再把磁盘上的 plan 写进 store，避免启动即出现旧计划条）. */
export function useRecentData() {
  const setRecentData = useStatusStore((s) => s.setRecentData);

  const refreshRecentData = useCallback(() => {
    const workspace =
      useSettingsStore.getState().settings.workspace?.trim() || ".";
    invoke<RecentDataResponse>("skilllite_load_recent", { workspace })
      .then((data) => {
        setRecentData(parseRecentData(data));
      })
      .catch((err) => {
        console.error("[skilllite-assistant] skilllite_load_recent failed:", err);
      });
  }, [setRecentData]);

  return { refreshRecentData };
}
