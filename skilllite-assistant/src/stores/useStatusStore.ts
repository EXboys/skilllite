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
    | "error"
    | "llm_usage";
  name?: string;
  text: string;
  isError?: boolean;
}

/** Local calendar month key and cumulative API-reported tokens (from agent `done` payloads). */
export interface LlmUsageMonthTotals {
  monthKey: string;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

function monthKeyLocal(d = new Date()): string {
  const y = d.getFullYear();
  const m = d.getMonth() + 1;
  return `${y}-${m.toString().padStart(2, "0")}`;
}

function initialLlmUsageMonth(): LlmUsageMonthTotals {
  return {
    monthKey: monthKeyLocal(),
    prompt_tokens: 0,
    completion_tokens: 0,
    total_tokens: 0,
  };
}

function parseNonNegInt(v: unknown): number | null {
  if (typeof v === "number" && Number.isFinite(v) && v >= 0) {
    return Math.floor(v);
  }
  if (typeof v === "string" && v.trim() !== "") {
    const n = Number(v);
    if (Number.isFinite(n) && n >= 0) return Math.floor(n);
  }
  return null;
}

interface StatusState {
  tasks: TaskItem[];
  logEntries: LogEntry[];
  logFiles: string[];
  memoryHints: string[];
  memoryFiles: string[];
  outputFiles: string[];
  latestOutput: string;
  /** Cumulative LLM token usage for the current local calendar month (persisted). */
  llmUsageMonth: LlmUsageMonthTotals;
  clearPlan: () => void;
  setLatestOutput: (text: string) => void;
  addTaskPlan: (tasks: TaskItem[]) => void;
  updateTaskProgress: (taskId: number, completed: boolean) => void;
  addLog: (entry: Omit<LogEntry, "id" | "time">) => void;
  addMemoryHint: (hint: string) => void;
  /** Merge one agent-turn `llm_usage` object (from `done` event) into the active month. */
  addLlmUsageFromTurn: (raw: unknown) => void;
  /** If the calendar month changed since last persist, reset counters (no chat yet this month). */
  rollLlmUsageMonthIfNeeded: () => void;
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
      llmUsageMonth: initialLlmUsageMonth(),

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

      addLlmUsageFromTurn: (raw) =>
        set((s) => {
          const o = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : null;
          if (!o) return s;
          const p = parseNonNegInt(o.prompt_tokens);
          const c = parseNonNegInt(o.completion_tokens);
          const t = parseNonNegInt(o.total_tokens);
          if (p === null || c === null || t === null) return s;
          const mk = monthKeyLocal();
          let base = s.llmUsageMonth;
          if (base.monthKey !== mk) {
            base = {
              monthKey: mk,
              prompt_tokens: 0,
              completion_tokens: 0,
              total_tokens: 0,
            };
          }
          return {
            llmUsageMonth: {
              monthKey: mk,
              prompt_tokens: base.prompt_tokens + p,
              completion_tokens: base.completion_tokens + c,
              total_tokens: base.total_tokens + t,
            },
          };
        }),

      rollLlmUsageMonthIfNeeded: () =>
        set((s) => {
          const mk = monthKeyLocal();
          if (s.llmUsageMonth.monthKey === mk) return s;
          return {
            llmUsageMonth: {
              monthKey: mk,
              prompt_tokens: 0,
              completion_tokens: 0,
              total_tokens: 0,
            },
          };
        }),

      setRecentData: (data) =>
        set((s) => ({
          memoryFiles: data.memoryFiles ?? s.memoryFiles,
          outputFiles: data.outputFiles ?? s.outputFiles,
          logFiles: data.logFiles ?? s.logFiles,
        })),

      clearAll: () =>
        set({
          tasks: [],
          logEntries: [],
          logFiles: [],
          memoryHints: [],
          memoryFiles: [],
          outputFiles: [],
          latestOutput: "",
        }),
    }),
    {
      name: STATUS_STORE_PERSIST_KEY,
      partialize: (s) => ({
        logEntries: s.logEntries,
        memoryHints: s.memoryHints,
        llmUsageMonth: s.llmUsageMonth,
      }),
      merge: (persistedState, currentState) => {
        const p = persistedState as Partial<StatusState>;
        return {
          ...currentState,
          ...p,
          tasks: [],
          llmUsageMonth: p.llmUsageMonth ?? currentState.llmUsageMonth,
        };
      },
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
