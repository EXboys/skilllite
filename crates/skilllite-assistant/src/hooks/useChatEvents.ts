import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ChatMessage, StreamEventPayload, TurnLlmUsage } from "../types/chat";
import type { LogEntry } from "../stores/useStatusStore";
import { useStatusStore } from "../stores/useStatusStore";
import { isChatHiddenToolName } from "../utils/chatNoise";
import { tryParseReadFilePathFromToolArgs } from "../utils/readFileToolMeta";
import { humanizeApiError } from "../utils/humanizeApiError";
import { sanitizeLlmVisibleChatText } from "../utils/sanitizeLlmVisibleChatText";
import { translate } from "../i18n";

/** 工具结果文本里这种结构（complete_task ack）不能当作给用户看的回退正文。 */
function isCompleteTaskAckJson(text: string): boolean {
  const t = text.trim();
  if (!t.startsWith("{")) return false;
  try {
    const obj = JSON.parse(t) as Record<string, unknown>;
    return (
      typeof obj.task_id !== "undefined" &&
      typeof obj.completion_type !== "undefined" &&
      typeof obj.success === "boolean"
    );
  } catch {
    return false;
  }
}

/** 在最近若干条消息里挑一条「主答案」级的工具结果文本（按 helpers.rs 的策略简化）。 */
function pickTrailingToolFallback(messages: ChatMessage[]): string | null {
  const MIN_CHARS = 80;
  const MAX_BYTES = 12 * 1024;
  let candidate: string | null = null;
  for (let i = messages.length - 1; i >= 0; i--) {
    const m = messages[i];
    if (m.type === "user" || m.type === "assistant") break;
    if (m.type !== "tool_result") continue;
    if (m.isError) continue;
    const body = (m.result || "").trim();
    if (!body || isCompleteTaskAckJson(body)) continue;
    if (body.length >= MIN_CHARS) {
      return body.length > MAX_BYTES ? body.slice(0, MAX_BYTES) : body;
    }
    if (candidate == null) candidate = body;
  }
  if (candidate && candidate.length > MAX_BYTES) {
    return candidate.slice(0, MAX_BYTES);
  }
  return candidate;
}

function nonNegInt(v: unknown): number | null {
  const x = typeof v === "number" ? v : Number(v);
  if (!Number.isFinite(x) || x < 0) return null;
  return Math.floor(x);
}

const STREAM_THROTTLE_MS = 80;

/** 在 React 提交本轮 `setMessages` 之后再跑回调，避免依赖 ref 时出现「猜你想问」早于正文。 */
function afterReactPaint(fn: () => void) {
  requestAnimationFrame(() => {
    requestAnimationFrame(fn);
  });
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
    const readFilePathByToolCallId = new Map<string, string>();

    const unlistenConfirm = listen<{
      prompt: string;
      risk_tier?: string;
      session_key?: string;
    }>("skilllite-confirmation-request", (ev) => {
      if (dead) return;
      const sk = ev.payload.session_key;
      if (sk == null || sk !== sessionKey) return;
      const prompt = ev.payload.prompt ?? "";
      const raw = ev.payload.risk_tier;
      const riskTier: "low" | "confirm_required" | undefined =
        raw === "low" || raw === "confirm_required" ? raw : undefined;
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "confirmation",
          prompt,
          ...(riskTier != null ? { riskTier } : {}),
        },
      ]);
    });

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
        if (text.trim() === "") return;
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
          // 避免 agent-rpc 连续两次下发相同全文时重复追加一条 assistant
          if (
            last?.type === "assistant" &&
            !last.streaming &&
            last.content === text
          ) {
            return prev;
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
        const usageRaw = data?.llm_usage;
        let turnUsage: TurnLlmUsage | null = null;
        if (usageRaw && typeof usageRaw === "object") {
          const u = usageRaw as Record<string, unknown>;
          const p = nonNegInt(u.prompt_tokens);
          const c = nonNegInt(u.completion_tokens);
          const t = nonNegInt(u.total_tokens);
          if (p !== null && c !== null && t !== null) {
            useStatusStore.getState().addLlmUsageFromTurn(usageRaw);
            turnUsage = {
              prompt_tokens: p,
              completion_tokens: c,
              total_tokens: t,
            };
          }
        }
        setMessages((prev) => {
          let next: ChatMessage[];
          const last = prev[prev.length - 1];
          if (last?.type === "assistant" && last?.streaming) {
            const finalContent = last.content + remainder;
            setLatestOutput(finalContent);
            next = [
              ...prev.slice(0, -1),
              { ...last, content: finalContent, streaming: false },
            ];
          } else {
            next = [...prev];
          }
          /**
           * 桌面兜底：如果本轮结束时最后一条 assistant **可见正文为空**（常见于
           * MiniMax / 推理模型把答案塞进 `reasoning_*` 而 `content` 为空，或被
           * `sanitizeLlmVisibleChatText` 清光），但前面有大段 `tool_result`，
           * 用工具结果填进去，避免「只有内部步骤 + token 行」的空回复观感。
           * Rust 引擎 `emit_assistant_visible` 已会做同样的回退；这里是 UI 侧的
           * 第二道防线，确保旧引擎也不会出现空气泡。
           */
          const tail = next[next.length - 1];
          const tailEmpty =
            tail?.type === "assistant" &&
            !tail.streaming &&
            sanitizeLlmVisibleChatText(tail.content || "").trim().length === 0;
          if (tailEmpty) {
            const fallback = pickTrailingToolFallback(next.slice(0, -1));
            if (fallback) {
              setLatestOutput(fallback);
              next = [
                ...next.slice(0, -1),
                { ...tail, content: fallback },
              ];
            } else if (next.length >= 2) {
              const prevTool = next[next.length - 2];
              if (prevTool?.type === "tool_result") {
                next = next.slice(0, -1);
              }
            }
          }
          if (turnUsage != null) {
            for (let i = next.length - 1; i >= 0; i--) {
              const m = next[i];
              if (m.type === "assistant") {
                next = [
                  ...next.slice(0, i),
                  { ...m, turnLlmUsage: turnUsage },
                  ...next.slice(i + 1),
                ];
                break;
              }
            }
          }
          return next;
        });
        setLoading(false);
        clearPlan?.();
        afterReactPaint(() => onTurnComplete?.());
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
        afterReactPaint(() => onTurnComplete?.());
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
        const toolCallIdRaw = data?.tool_call_id;
        const toolCallId =
          typeof toolCallIdRaw === "string" && toolCallIdRaw.trim().length > 0
            ? toolCallIdRaw.trim()
            : undefined;
        addLog({
          type: "tool_call" as const, name,
          text: args.length > 300 ? args.slice(0, 300) + "…" : args,
        });
        if (["memory_write", "memory_search", "memory_list"].includes(name)) {
          addMemoryHint(`${name}: ${args.slice(0, 40)}…`);
        }
        if (name.replace(/-/g, "_") === "read_file" && toolCallId) {
          const path = tryParseReadFilePathFromToolArgs(args);
          if (path) readFilePathByToolCallId.set(toolCallId, path);
        }
        if (!isChatHiddenToolName(name)) {
          setMessages((prev) => [
            ...prev,
            {
              id: crypto.randomUUID(),
              type: "tool_call",
              name,
              args,
              ...(toolCallId ? { toolCallId } : {}),
            },
          ]);
        }
      } else if (event === "tool_result") {
        const name = (data?.name as string) ?? "";
        const isErr = (data?.is_error as boolean) ?? false;
        const result = (data?.result as string) ?? "";
        const toolCallIdRaw = data?.tool_call_id;
        const toolCallId =
          typeof toolCallIdRaw === "string" && toolCallIdRaw.trim().length > 0
            ? toolCallIdRaw.trim()
            : undefined;
        addLog({
          type: "tool_result" as const, name,
          text: result.length > 1200 ? result.slice(0, 1200) + "…" : result,
          isError: isErr,
        });
        if (!isChatHiddenToolName(name)) {
          setMessages((prev) => {
            const norm = (n: string) => n.replace(/-/g, "_").toLowerCase();
            const nn = norm(name);
            if (
              prev.some(
                (m) =>
                  m.type === "tool_result" &&
                  norm(m.name) === nn &&
                  (m.toolCallId ?? "") === (toolCallId ?? "") &&
                  m.result === result &&
                  m.isError === isErr
              )
            ) {
              return prev;
            }
            let sourcePath: string | undefined;
            if (name.replace(/-/g, "_") === "read_file" && toolCallId) {
              sourcePath = readFilePathByToolCallId.get(toolCallId);
            }
            return [
              ...prev,
              {
                id: crypto.randomUUID(),
                type: "tool_result" as const,
                name,
                result,
                isError: isErr,
                ...(toolCallId ? { toolCallId } : {}),
                sourcePath,
              },
            ];
          });
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
      } else if (event === "llm_usage") {
        if ((data?.reported as boolean) === false) {
          addLog({
            type: "llm_usage",
            name: "LLM",
            text: translate("status.llmUsageNotReportedLog"),
          });
          return;
        }
        const p = nonNegInt(data?.prompt_tokens);
        const c = nonNegInt(data?.completion_tokens);
        const t = nonNegInt(data?.total_tokens);
        if (p !== null && c !== null && t !== null) {
          addLog({
            type: "llm_usage",
            name: "LLM",
            text: translate("status.llmUsageLogLine", {
              inTok: p,
              outTok: c,
              totalTok: t,
            }),
          });
        }
      }
    });

    return () => {
      dead = true;
      unlistenConfirm.then((fn) => fn());
      unlistenClarify.then((fn) => fn());
      unlisten.then((fn) => fn());
      pendingChunks.current = "";
      flushScheduled.current = false;
      readFilePathByToolCallId.clear();
      if (flushTimer !== undefined) clearTimeout(flushTimer);
    };
    // onTurnComplete 等回调需随渲染更新，否则会用陈旧闭包
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionKey, onTurnComplete]);
}
