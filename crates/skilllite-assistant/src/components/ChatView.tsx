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
    if (loading) return;
    try {
      await invoke("skilllite_clear_transcript", { session_key: "default" });
      setMessages([]);
      setError(null);
      statusActions.clearAll();
      refreshRecentData();
    } catch (err) {
      console.error("[skilllite-assistant] skilllite_clear_transcript failed:", err);
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [loading, statusActions.clearAll, refreshRecentData]);

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
    if (!text || loading) return;

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
          disabled={loading}
          className="text-xs text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent disabled:opacity-50 px-2 py-1 rounded hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
          aria-label="Clear chat"
          title="清空对话"
        >
          清空对话
        </button>
      </div>
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

      <ChatInput
        value={input}
        onChange={setInput}
        onSend={handleSend}
        onStop={handleStop}
        disabled={loading}
        loading={loading}
      />
    </div>
  );
}
