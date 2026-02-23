import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStatusStore } from "../stores/useStatusStore";

interface RecentDataResponse {
  memory_files: string[];
  output_files: string[];
  plan: {
    task: string;
    steps: { id: number; description: string; completed: boolean }[];
  } | null;
}

function parseRecentData(data: RecentDataResponse) {
  return {
    memoryFiles: data.memory_files ?? [],
    outputFiles: data.output_files ?? [],
    plan: data.plan
      ? {
          task: data.plan.task,
          steps: data.plan.steps.map((s) => ({
            id: s.id,
            description: s.description,
            completed: s.completed,
          })),
        }
      : undefined,
  };
}

/** Hook to load and refresh recent data (memory files, output files, plan) from skilllite. */
export function useRecentData() {
  const setRecentData = useStatusStore((s) => s.setRecentData);

  const refreshRecentData = useCallback(() => {
    invoke<RecentDataResponse>("skilllite_load_recent")
      .then((data) => {
        setRecentData(parseRecentData(data));
      })
      .catch((err) => {
        console.error("[skilllite-assistant] skilllite_load_recent failed:", err);
      });
  }, [setRecentData]);

  return { refreshRecentData };
}
