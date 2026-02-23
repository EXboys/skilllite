import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useShallow } from "zustand/react/shallow";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useStatusStore } from "../stores/useStatusStore";
import { useChatEvents } from "../hooks/useChatEvents";
import { useRecentData } from "../hooks/useRecentData";
import { MessageList } from "./chat/MessageList";
import { ChatInput } from "./chat/ChatInput";
import type { ChatMessage } from "../types/chat";

export default function ChatView() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [isClearing, setIsClearing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { settings } = useSettingsStore();
  const { refreshRecentData } = useRecentData();
  const statusActions = useStatusStore(
    useShallow((s) => ({
      addTaskPlan: s.addTaskPlan,
      updateTaskProgress: s.updateTaskProgress,
      addLog: s.addLog,
      addMemoryHint: s.addMemoryHint,
      clearPlan: s.clearPlan,
      setLatestOutput: s.setLatestOutput,
      clearAll: s.clearAll,
    }))
  );

  useChatEvents({
    setMessages,
    setLoading,
    setError,
    addTaskPlan: statusActions.addTaskPlan,
    updateTaskProgress: statusActions.updateTaskProgress,
    addLog: statusActions.addLog,
    addMemoryHint: statusActions.addMemoryHint,
    setLatestOutput: statusActions.setLatestOutput,
    onTurnComplete: refreshRecentData,
  });

  useEffect(() => {
    invoke<Array<{ id: string; role: string; content: string }>>("skilllite_load_transcript", {
      session_key: "default",
    })
      .then((entries) => {
        if (entries.length > 0) {
          const msgs: ChatMessage[] = entries.map((e) => ({
            id: e.id,
            type: e.role === "user" ? "user" : "assistant",
            content: e.content,
          })) as ChatMessage[];
          setMessages(msgs);
        }
      })
      .catch((err) => {
        console.error("[skilllite-assistant] skilllite_load_transcript failed:", err);
      });
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 1500);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const handleConfirm = async (id: string, approved: boolean) => {
    await invoke("skilllite_confirm", { approved });
    setMessages((prev) =>
      prev.map((m) =>
        m.type === "confirmation" && m.id === id
          ? { ...m, resolved: true, approved }
          : m
      )
    );
  };

  const handleClear = useCallback(async () => {
    if (loading || isClearing) return;
    setIsClearing(true);
    setNotice("正在清空对话...");
    try {
      await invoke("skilllite_clear_transcript", {
        session_key: "default",
        workspace: settings.workspace || ".",
      });
      setMessages([]);
      setError(null);
      statusActions.clearAll();
      refreshRecentData();
      setNotice("已清空对话");
    } catch (err) {
      console.error("[skilllite-assistant] skilllite_clear_transcript failed:", err);
      setError(err instanceof Error ? err.message : String(err));
      setNotice(null);
    } finally {
      setIsClearing(false);
    }
  }, [loading, isClearing, settings.workspace, statusActions.clearAll, refreshRecentData]);

  const handleStop = useCallback(async () => {
    try {
      await invoke("skilllite_stop");
      setLoading(false);
      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (last?.type === "assistant" && last?.streaming) {
          const content = last.content ? `${last.content}\n\n[已中止]` : "[已中止]";
          statusActions.setLatestOutput(content);
          return [...prev.slice(0, -1), { ...last, content, streaming: false }];
        }
        return prev;
      });
      refreshRecentData();
    } catch {
      setLoading(false);
    }
  }, [refreshRecentData, statusActions.setLatestOutput]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading || isClearing) return;

    // Slash commands: /new or /reset to clear chat (like OpenClaw)
    if (text === "/new" || text === "/reset") {
      setInput("");
      await handleClear();
      return;
    }

    setInput("");
    setError(null);
    statusActions.clearPlan();
    statusActions.setLatestOutput("");
    setMessages((prev) => [
      ...prev,
      { id: crypto.randomUUID(), type: "user", content: text },
    ]);
    setLoading(true);

    const config =
      settings.apiKey || settings.model !== "gpt-4o" || settings.workspace !== "." || settings.apiBase
        ? {
            api_key: settings.apiKey || undefined,
            model: settings.model || undefined,
            workspace: settings.workspace || undefined,
            api_base: settings.apiBase || undefined,
          }
        : undefined;

    try {
      await invoke("skilllite_chat_stream", {
        message: text,
        workspace: settings.workspace || ".",
        config,
      });
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setError(errMsg);
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "assistant",
          content: `Request failed: ${errMsg}`,
        },
      ]);
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col h-full bg-surface dark:bg-surface-dark">
      <div className="flex justify-end items-center gap-2 py-1.5 px-3 border-b border-border dark:border-border-dark shrink-0">
        {loading && (
          <button
            type="button"
            onClick={handleStop}
            className="text-xs text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 px-2 py-1 rounded hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors font-medium"
            aria-label="Stop"
            title="停止当前任务"
          >
            停止
          </button>
        )}
        <button
          type="button"
          onClick={handleClear}
          disabled={loading || isClearing}
          className={`text-xs px-2 py-1 rounded transition-colors inline-flex items-center gap-1.5 ${
            isClearing
              ? "text-accent bg-accent/10 dark:bg-accent/20"
              : "text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent hover:bg-ink/5 dark:hover:bg-white/5"
          } disabled:opacity-50`}
          aria-label="Clear chat"
          title={isClearing ? "正在清空对话" : "清空对话"}
        >
          {isClearing && (
            <svg
              className="w-3 h-3 animate-spin"
              viewBox="0 0 24 24"
              fill="none"
              aria-hidden="true"
            >
              <circle
                cx="12"
                cy="12"
                r="9"
                className="opacity-25"
                stroke="currentColor"
                strokeWidth="3"
              />
              <path
                d="M21 12a9 9 0 0 0-9-9"
                className="opacity-100"
                stroke="currentColor"
                strokeWidth="3"
                strokeLinecap="round"
              />
            </svg>
          )}
          {isClearing ? "正在清空" : "清空对话"}
        </button>
      </div>
      {isClearing && (
        <div className="mx-3 mt-2 px-3 py-2 rounded-md border border-accent/30 bg-accent/10 dark:bg-accent/20 text-accent text-xs animate-pulse">
          正在清空会话并整理历史，请稍候...
        </div>
      )}
      <MessageList
        messages={messages}
        loading={loading}
        onConfirm={handleConfirm}
      />

      {error && (
        <div className="px-4 py-2.5 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 text-sm border-t border-red-100 dark:border-red-900/40">
          {error}
        </div>
      )}
      {!error && notice && (
        <div className="px-4 py-2.5 bg-ink/5 dark:bg-white/5 text-ink-mute dark:text-ink-dark-mute text-sm border-t border-border dark:border-border-dark">
          {notice}
        </div>
      )}

      <ChatInput
        value={input}
        onChange={setInput}
        onSend={handleSend}
        onStop={handleStop}
        disabled={loading || isClearing}
        loading={loading}
      />
    </div>
  );
}
