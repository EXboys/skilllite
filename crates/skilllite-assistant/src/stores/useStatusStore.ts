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
  type: "tool_call" | "tool_result" | "plan" | "progress" | "error";
  name?: string;
  text: string;
  isError?: boolean;
}

interface StatusState {
  tasks: TaskItem[];
  logEntries: LogEntry[];
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
  setRecentData: (data: {
    memoryFiles?: string[];
    outputFiles?: string[];
    plan?: { task: string; steps: { id: number; description: string; completed: boolean }[] };
  }) => void;
  clearAll: () => void;
}

const now = () => new Date().toLocaleTimeString("en-US", { hour12: false });

export const useStatusStore = create<StatusState>()(
  persist(
    (set) => ({
      tasks: [],
      logEntries: [],
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
        set((s) => ({
          tasks: s.tasks.map((t) =>
            t.id === taskId ? { ...t, completed } : t
          ),
        })),

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
          tasks:
            data.plan && data.plan.steps.length > 0
              ? data.plan.steps.map((step) => ({
                  id: step.id,
                  description: step.description,
                  completed: step.completed,
                }))
              : s.tasks,
        })),

      clearAll: () =>
        set({ tasks: [], logEntries: [], memoryHints: [], memoryFiles: [], outputFiles: [], latestOutput: "" }),
    }),
    {
      name: "skilllite-assistant-status",
      partialize: (s) => ({
        tasks: s.tasks,
        logEntries: s.logEntries,
        memoryHints: s.memoryHints,
      }),
    }
  )
);
