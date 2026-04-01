import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ChatMessage, StreamEventPayload } from "../types/chat";
import type { LogEntry } from "../stores/useStatusStore";
import { isChatHiddenToolName } from "../utils/chatNoise";
import { humanizeApiError } from "../utils/humanizeApiError";

const STREAM_THROTTLE_MS = 80;
const EVOLUTION_OPTIONS = [
  "重试当前方案",
  "切换数据源/参数",
  "稍后由定时任务处理",
  "【授权进化能力】",
];

function parseToolOutcome(
  result: string,
  isError: boolean
): "failure" | "partial_success" | null {
  if (isError) return "failure";
  const text = result.trim();
  if (!text || (text[0] !== "{" && text[0] !== "[")) return null;
  try {
    const parsed = JSON.parse(text) as Record<string, unknown>;
    if (parsed.success === false) return "failure";
    if (parsed.partial_success === true) return "partial_success";
    return null;
  } catch {
    return null;
  }
}

interface UseChatEventsParams {
  sessionKey: string;
  setMessages: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
  setLoading: React.Dispatch<React.SetStateAction<boolean>>;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  addTaskPlan: (tasks: Array<{ id: number; description: string; tool_hint?: string; completed?: boolean }>) => void;
  updateTaskProgress: (taskId: number, completed: boolean) => void;
  addLog: (entry: Omit<LogEntry, "id" | "time">) => void;
  addMemoryHint: (hint: string) => void;
  setLatestOutput: (text: string) => void;
  /** 一轮 assistant 输出结束或出错时清空顶部「任务计划」条（历史里的 plan 消息仍保留） */
  clearPlan?: () => void;
  onTurnComplete?: () => void;
}

export function useChatEvents({
  sessionKey,
  setMessages,
  setLoading,
  setError,
  addTaskPlan,
  updateTaskProgress,
  addLog,
  addMemoryHint,
  setLatestOutput,
  clearPlan,
  onTurnComplete,
}: UseChatEventsParams) {
  useEffect(() => {
    let dead = false;

    const unlistenConfirm = listen<{ prompt: string; session_key?: string }>(
      "skilllite-confirmation-request",
      (ev) => {
        if (dead) return;
        const sk = ev.payload.session_key;
        if (sk == null || sk !== sessionKey) return;
        const prompt = ev.payload.prompt ?? "";
        setMessages((prev) => [
          ...prev,
          { id: crypto.randomUUID(), type: "confirmation", prompt },
        ]);
      }
    );

    const unlistenClarify = listen<{
      reason: string;
      message: string;
      suggestions: string[];
      session_key?: string;
    }>("skilllite-clarification-request", (ev) => {
      if (dead) return;
      const sk = ev.payload.session_key;
      if (sk == null || sk !== sessionKey) return;
      const { reason, message, suggestions } = ev.payload;
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "clarification" as const,
          reason: reason ?? "",
          message: message ?? "",
          suggestions: suggestions ?? [],
        },
      ]);
      setLoading(false);
    });

    const pendingChunks = { current: "" };
    const lastFlushTime = { current: 0 };
    const flushScheduled = { current: false };
    let flushTimer: ReturnType<typeof setTimeout> | undefined;

    const flushStreaming = (delta: string) => {
      if (dead) return;
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
        flushTimer = setTimeout(() => {
          lastFlushTime.current = Date.now();
          run();
        }, STREAM_THROTTLE_MS - elapsed);
      }
    };

    const unlisten = listen<StreamEventPayload>("skilllite-event", (ev) => {
      if (dead) return;
      const sk = ev.payload.session_key;
      if (sk == null || sk !== sessionKey) return;
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
        clearPlan?.();
        onTurnComplete?.();
      } else if (event === "error") {
        const raw = (data?.message as string) ?? "Unknown error";
        const friendly = humanizeApiError(raw);
        setLatestOutput(friendly);
        setMessages((prev) => [
          ...prev,
          { id: crypto.randomUUID(), type: "assistant", content: friendly },
        ]);
        setError(friendly);
        setLoading(false);
        addLog({ type: "error" as const, text: raw, isError: true });
        clearPlan?.();
        onTurnComplete?.();
      } else if (event === "protocol_warning") {
        const msg = (data?.message as string) ?? "检测到 agent-rpc 协议流异常，正在自动恢复";
        const totalInvalid = (data?.total_invalid_lines as number) ?? 0;
        const preview = (data?.line_preview as string) ?? "";
        addLog({
          type: "warning" as const,
          name: "agent-rpc",
          text: totalInvalid > 0
            ? `${msg}（累计异常 ${totalInvalid} 行）${preview ? `；样例：${preview}` : ""}`
            : msg,
        });
      } else if (event === "protocol_recovered") {
        const msg = (data?.message as string) ?? "agent-rpc 协议流已自动恢复";
        const recoveredLines = (data?.recovered_lines as number) ?? 0;
        addLog({
          type: "warning" as const,
          name: "agent-rpc",
          text: recoveredLines > 0
            ? `${msg}（本轮跳过 ${recoveredLines} 行异常输出）`
            : msg,
        });
      } else if (event === "task_plan") {
        const tasks = (data?.tasks as Array<{
          id?: number; description?: string; tool_hint?: string; completed?: boolean;
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
          type: "tool_call" as const, name,
          text: args.length > 300 ? args.slice(0, 300) + "…" : args,
        });
        if (["memory_write", "memory_search", "memory_list"].includes(name)) {
          addMemoryHint(`${name}: ${args.slice(0, 40)}…`);
        }
        if (!isChatHiddenToolName(name)) {
          setMessages((prev) => [
            ...prev,
            { id: crypto.randomUUID(), type: "tool_call", name, args },
          ]);
        }
      } else if (event === "tool_result") {
        const name = (data?.name as string) ?? "";
        const isErr = (data?.is_error as boolean) ?? false;
        const result = (data?.result as string) ?? "";
        const outcome = parseToolOutcome(result, isErr);
        addLog({
          type: "tool_result" as const, name,
          text: result.length > 1200 ? result.slice(0, 1200) + "…" : result,
          isError: isErr,
        });
        if (!isChatHiddenToolName(name)) {
          setMessages((prev) => [
            ...prev,
            { id: crypto.randomUUID(), type: "tool_result", name, result, isError: isErr },
          ]);
          if (outcome !== null) {
            setMessages((prev) => {
              const exists = prev.some(
                (m) =>
                  m.type === "evolution_options" &&
                  !m.resolved &&
                  m.toolName === name &&
                  m.outcome === outcome
              );
              if (exists) return prev;
              const message =
                outcome === "failure"
                  ? `工具「${name}」执行失败。你可以选择下一步处理方式，或直接授权补齐能力。`
                  : `工具「${name}」只部分满足了需求（partial_success）。你可以选择下一步处理方式，或直接授权补齐能力。`;
              return [
                ...prev,
                {
                  id: crypto.randomUUID(),
                  type: "evolution_options",
                  toolName: name,
                  outcome,
                  message,
                  options: EVOLUTION_OPTIONS,
                },
              ];
            });
          }
        }
      } else if (event === "command_started") {
        const command = (data?.command as string) ?? "";
        if (!command) return;
        addLog({ type: "command_started" as const, name: "run_command", text: command.length > 240 ? command.slice(0, 240) + "…" : command });
      } else if (event === "command_output") {
        const stream = (data?.stream as string) ?? "stdout";
        const chunk = (data?.chunk as string) ?? "";
        if (!chunk) return;
        addLog({ type: "command_output" as const, name: stream, text: chunk.length > 1200 ? chunk.slice(0, 1200) + "…" : chunk, isError: stream === "stderr" });
      } else if (event === "command_finished") {
        const success = (data?.success as boolean) ?? false;
        const exitCode = (data?.exit_code as number) ?? -1;
        const durationMs = (data?.duration_ms as number) ?? 0;
        addLog({ type: "command_finished" as const, name: "run_command", text: `exit ${exitCode} in ${durationMs}ms`, isError: !success });
      } else if (event === "preview_started") {
        const path = (data?.path as string) ?? "";
        const port = (data?.port as number) ?? 0;
        addLog({ type: "preview_started" as const, name: "preview_server", text: `${path || "preview"} on port ${port}` });
      } else if (event === "preview_ready") {
        const url = (data?.url as string) ?? "";
        addLog({ type: "preview_ready" as const, name: "preview_server", text: url });
      } else if (event === "preview_failed") {
        const message = (data?.message as string) ?? "";
        addLog({ type: "preview_failed" as const, name: "preview_server", text: message, isError: true });
      } else if (event === "preview_stopped") {
        const reason = (data?.reason as string) ?? "";
        addLog({ type: "preview_stopped" as const, name: "preview_server", text: reason || "stopped" });
      } else if (event === "swarm_started") {
        const description = (data?.description as string) ?? "";
        addLog({ type: "swarm_started" as const, name: "delegate_to_swarm", text: description.length > 240 ? description.slice(0, 240) + "…" : description });
      } else if (event === "swarm_progress") {
        const status = (data?.status as string) ?? "";
        addLog({ type: "swarm_progress" as const, name: "delegate_to_swarm", text: status });
      } else if (event === "swarm_finished") {
        const summary = (data?.summary as string) ?? "";
        addLog({ type: "swarm_finished" as const, name: "delegate_to_swarm", text: summary.length > 240 ? summary.slice(0, 240) + "…" : summary });
      } else if (event === "swarm_failed") {
        const message = (data?.message as string) ?? "";
        addLog({ type: "swarm_failed" as const, name: "delegate_to_swarm", text: message.length > 240 ? message.slice(0, 240) + "…" : message, isError: true });
      }
    });

    return () => {
      dead = true;
      unlistenConfirm.then((fn) => fn());
      unlistenClarify.then((fn) => fn());
      unlisten.then((fn) => fn());
      pendingChunks.current = "";
      flushScheduled.current = false;
      if (flushTimer !== undefined) clearTimeout(flushTimer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionKey]);
}
