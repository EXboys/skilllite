import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ChatMessage, StreamEventPayload } from "../types/chat";
import type { LogEntry } from "../stores/useStatusStore";

const STREAM_THROTTLE_MS = 80;

interface UseChatEventsParams {
  setMessages: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
  setLoading: React.Dispatch<React.SetStateAction<boolean>>;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  addTaskPlan: (tasks: Array<{ id: number; description: string; tool_hint?: string; completed?: boolean }>) => void;
  updateTaskProgress: (taskId: number, completed: boolean) => void;
  addLog: (entry: Omit<LogEntry, "id" | "time">) => void;
  addMemoryHint: (hint: string) => void;
  setLatestOutput: (text: string) => void;
  /** 当 Agent 本轮完成时调用，用于刷新 output/memory/plan 等数据 */
  onTurnComplete?: () => void;
}

export function useChatEvents({
  setMessages,
  setLoading,
  setError,
  addTaskPlan,
  updateTaskProgress,
  addLog,
  addMemoryHint,
  setLatestOutput,
  onTurnComplete,
}: UseChatEventsParams) {
  useEffect(() => {
    const unlistenConfirm = listen<{ prompt: string }>(
      "skilllite-confirmation-request",
      (ev) => {
        const prompt = ev.payload.prompt ?? "";
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            type: "confirmation",
            prompt,
          },
        ]);
      }
    );
    return () => {
      unlistenConfirm.then((fn) => fn());
    };
  }, [setMessages]);

  useEffect(() => {
    const pendingChunks = { current: "" };
    const lastFlushTime = { current: 0 };
    const flushScheduled = { current: false };

    const flushStreaming = (delta: string) => {
      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (last?.type === "assistant" && last?.streaming) {
          const newContent = last.content + delta;
          setLatestOutput(newContent);
          return [...prev.slice(0, -1), { ...last, content: newContent }];
        }
        setLatestOutput(delta);
        return prev;
      });
    };

    const scheduleFlush = () => {
      if (flushScheduled.current) return;
      flushScheduled.current = true;
      const run = () => {
        const content = pendingChunks.current;
        pendingChunks.current = "";
        flushScheduled.current = false;
        if (content) flushStreaming(content);
      };
      const now = Date.now();
      const elapsed = now - lastFlushTime.current;
      if (elapsed >= STREAM_THROTTLE_MS || lastFlushTime.current === 0) {
        lastFlushTime.current = now;
        run();
      } else {
        setTimeout(() => {
          lastFlushTime.current = Date.now();
          run();
        }, STREAM_THROTTLE_MS - elapsed);
      }
    };

    const unlisten = listen<StreamEventPayload>("skilllite-event", (ev) => {
      const { event, data } = ev.payload;
      if (event === "text_chunk") {
        const text = (data?.text as string) ?? "";
        if (text === "") return;
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last?.type === "assistant" && last?.streaming) {
            pendingChunks.current += text;
            scheduleFlush();
            return prev;
          }
          pendingChunks.current = text;
          scheduleFlush();
          return [
            ...prev,
            {
              id: crypto.randomUUID(),
              type: "assistant",
              content: "",
              streaming: true,
            },
          ];
        });
        setLoading(false);
      } else if (event === "text") {
        const text = (data?.text as string) ?? "";
        pendingChunks.current = "";
        flushScheduled.current = false;
        setLatestOutput(text);
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last?.type === "assistant" && last?.streaming) {
            return [
              ...prev.slice(0, -1),
              { ...last, content: text, streaming: false },
            ];
          }
          return [
            ...prev,
            {
              id: crypto.randomUUID(),
              type: "assistant",
              content: text,
              streaming: false,
            },
          ];
        });
        setLoading(false);
      } else if (event === "done") {
        const remainder = pendingChunks.current;
        pendingChunks.current = "";
        flushScheduled.current = false;
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last?.type === "assistant" && last?.streaming) {
            const finalContent = last.content + remainder;
            setLatestOutput(finalContent);
            return [...prev.slice(0, -1), { ...last, content: finalContent, streaming: false }];
          }
          return prev;
        });
        setLoading(false);
        onTurnComplete?.();
      } else if (event === "error") {
        const msg = (data?.message as string) ?? "Unknown error";
        const errContent = `Error: ${msg}`;
        setLatestOutput(errContent);
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            type: "assistant",
            content: errContent,
          },
        ]);
        setError(msg);
        setLoading(false);
        addLog({ type: "error" as const, text: msg, isError: true });
        onTurnComplete?.();
      } else if (event === "task_plan") {
        const tasks =
          (data?.tasks as Array<{
            id?: number;
            description?: string;
            tool_hint?: string;
            completed?: boolean;
          }>) ?? [];
        const taskItems = tasks.map((t, i) => ({
          id: t.id ?? i + 1,
          description: t.description ?? "",
          tool_hint: t.tool_hint,
          completed: (t.completed ?? false) as boolean,
        }));
        addTaskPlan(taskItems);
        addLog({ type: "plan" as const, text: `计划 ${tasks.length} 个任务` });
        setMessages((prev) => [
          ...prev,
          { id: crypto.randomUUID(), type: "plan", tasks: taskItems },
        ]);
      } else if (event === "task_progress") {
        const taskId = (data?.task_id as number) ?? 0;
        const completed = (data?.completed as boolean) ?? false;
        updateTaskProgress(taskId, completed);
      } else if (event === "tool_call") {
        const name = (data?.name as string) ?? "";
        const args = (data?.arguments as string) ?? "";
        addLog({
          type: "tool_call" as const,
          name,
          text: args.length > 60 ? args.slice(0, 60) + "…" : args,
        });
        if (["memory_write", "memory_search", "memory_list"].includes(name)) {
          addMemoryHint(`${name}: ${args.slice(0, 40)}…`);
        }
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            type: "tool_call",
            name,
            args,
          },
        ]);
      } else if (event === "tool_result") {
        const name = (data?.name as string) ?? "";
        const isErr = (data?.is_error as boolean) ?? false;
        const result = (data?.result as string) ?? "";
        addLog({
          type: "tool_result" as const,
          name,
          text: result.length > 80 ? result.slice(0, 80) + "…" : result,
          isError: isErr,
        });
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            type: "tool_result",
            name,
            result,
            isError: isErr,
          },
        ]);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [
    setMessages,
    setLoading,
    setError,
    addTaskPlan,
    updateTaskProgress,
    addLog,
    addMemoryHint,
    setLatestOutput,
    onTurnComplete,
  ]);
}
