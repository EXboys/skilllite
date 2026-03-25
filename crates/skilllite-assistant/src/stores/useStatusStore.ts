import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface TaskItem {
  id: number;
  description: string;
  tool_hint?: string;
  completed?: boolean;
}

export interface LogEntry {
  id: string;
  time: string;
  type:
    | "tool_call"
    | "tool_result"
    | "command_started"
    | "command_output"
    | "command_finished"
    | "preview_started"
    | "preview_ready"
    | "preview_failed"
    | "preview_stopped"
    | "swarm_started"
    | "swarm_progress"
    | "swarm_finished"
    | "swarm_failed"
    | "plan"
    | "progress"
    | "warning"
    | "error";
  name?: string;
  text: string;
  isError?: boolean;
}

interface StatusState {
  tasks: TaskItem[];
  logEntries: LogEntry[];
  logFiles: string[];
  memoryHints: string[];
  memoryFiles: string[];
  outputFiles: string[];
  latestOutput: string;
  clearPlan: () => void;
  setLatestOutput: (text: string) => void;
  addTaskPlan: (tasks: TaskItem[]) => void;
  updateTaskProgress: (taskId: number, completed: boolean) => void;
  addLog: (entry: Omit<LogEntry, "id" | "time">) => void;
  addMemoryHint: (hint: string) => void;
  /** 仅同步文件列表；计划条只来自当前轮次的流式事件，不从磁盘 / persist 恢复 */
  setRecentData: (data: {
    memoryFiles?: string[];
    outputFiles?: string[];
    logFiles?: string[];
  }) => void;
  clearAll: () => void;
}

const now = () => new Date().toLocaleTimeString("en-US", { hour12: false });

/** 与 persist.name 一致，供详情窗等监听 localStorage 同步 */
export const STATUS_STORE_PERSIST_KEY = "skilllite-assistant-status";

/** 主窗口 tasks 变更时通知其它 WebView（详情窗）从 localStorage 重新 hydrate */
export const STATUS_STORE_BROADCAST = "skilllite-assistant-status-broadcast";

export const useStatusStore = create<StatusState>()(
  persist(
    (set) => ({
      tasks: [],
      logEntries: [],
      logFiles: [],
      memoryHints: [],
      memoryFiles: [],
      outputFiles: [],
      latestOutput: "",

      clearPlan: () => set({ tasks: [] }),

      setLatestOutput: (text) => set({ latestOutput: text }),

      addTaskPlan: (tasks) =>
        set(() => ({
          tasks: tasks.map((t) => ({
            ...t,
            completed: (t as TaskItem & { completed?: boolean }).completed ?? false,
          })),
        })),

      updateTaskProgress: (taskId, completed) =>
        set((s) => {
          const tasks = s.tasks.map((t) =>
            t.id === taskId ? { ...t, completed } : t
          );
          const allDone =
            tasks.length > 0 && tasks.every((t) => t.completed);
          return { tasks: allDone ? [] : tasks };
        }),

      addLog: (entry) =>
        set((s) => ({
          logEntries: [
            ...s.logEntries.slice(-99),
            {
              ...entry,
              id: crypto.randomUUID(),
              time: now(),
            },
          ],
        })),

      addMemoryHint: (hint) =>
        set((s) => ({
          memoryHints: [...s.memoryHints.slice(-19), hint],
        })),

      setRecentData: (data) =>
        set((s) => ({
          memoryFiles: data.memoryFiles ?? s.memoryFiles,
          outputFiles: data.outputFiles ?? s.outputFiles,
          logFiles: data.logFiles ?? s.logFiles,
        })),

      clearAll: () =>
        set({ tasks: [], logEntries: [], logFiles: [], memoryHints: [], memoryFiles: [], outputFiles: [], latestOutput: "" }),
    }),
    {
      name: STATUS_STORE_PERSIST_KEY,
      partialize: (s) => ({
        logEntries: s.logEntries,
        memoryHints: s.memoryHints,
      }),
      merge: (persistedState, currentState) => ({
        ...currentState,
        ...(persistedState as Partial<StatusState>),
        tasks: [],
      }),
    }
  )
);

function broadcastTasksChanged() {
  if (typeof window === "undefined") return;
  try {
    const c = new BroadcastChannel(STATUS_STORE_BROADCAST);
    c.postMessage(null);
    c.close();
  } catch {
    /* ignore */
  }
}

let prevTasksRef = useStatusStore.getState().tasks;
useStatusStore.subscribe((s) => {
  if (s.tasks === prevTasksRef) return;
  prevTasksRef = s.tasks;
  broadcastTasksChanged();
});
