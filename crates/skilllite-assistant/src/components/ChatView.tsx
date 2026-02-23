import { useState, useRef, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useStatusStore } from "../stores/useStatusStore";

interface TaskItem {
  id: number;
  description: string;
  tool_hint?: string;
  completed?: boolean;
}

type ChatMessage =
  | { id: string; type: "user"; content: string }
  | { id: string; type: "assistant"; content: string; streaming?: boolean }
  | { id: string; type: "plan"; tasks: TaskItem[] }
  | { id: string; type: "tool_call"; name: string; args: string }
  | { id: string; type: "tool_result"; name: string; result: string; isError: boolean }
  | { id: string; type: "confirmation"; prompt: string; resolved?: boolean; approved?: boolean };

interface StreamEvent {
  event: string;
  data: Record<string, unknown>;
}

const markdownComponents = {
  p: ({ children }: { children?: React.ReactNode }) => (
    <p className="mb-2 last:mb-0">{children}</p>
  ),
  ul: ({ children }: { children?: React.ReactNode }) => (
    <ul className="list-disc list-inside mb-2 space-y-0.5">{children}</ul>
  ),
  ol: ({ children }: { children?: React.ReactNode }) => (
    <ol className="list-decimal list-inside mb-2 space-y-0.5">{children}</ol>
  ),
  li: ({ children }: { children?: React.ReactNode }) => (
    <li className="ml-2">{children}</li>
  ),
  code: ({
    className,
    children,
  }: {
    className?: string;
    children?: React.ReactNode;
  }) => {
    const isInline = !className;
    return isInline ? (
      <code className="px-1.5 py-0.5 rounded bg-black/10 dark:bg-white/10 font-mono text-sm">
        {children}
      </code>
    ) : (
      <code className={`block p-3 rounded-lg text-sm overflow-x-auto ${className ?? ""}`}>
        {children}
      </code>
    );
  },
  pre: ({ children }: { children?: React.ReactNode }) => (
    <pre className="mb-2 overflow-x-auto rounded-lg bg-black/5 dark:bg-white/5 p-3 text-sm">
      {children}
    </pre>
  ),
  a: ({
    href,
    children,
  }: {
    href?: string;
    children?: React.ReactNode;
  }) => (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="underline hover:opacity-80"
    >
      {children}
    </a>
  ),
  h1: ({ children }: { children?: React.ReactNode }) => (
    <h1 className="text-lg font-bold mb-2 mt-3 first:mt-0">{children}</h1>
  ),
  h2: ({ children }: { children?: React.ReactNode }) => (
    <h2 className="text-base font-bold mb-1.5 mt-2 first:mt-0">{children}</h2>
  ),
  h3: ({ children }: { children?: React.ReactNode }) => (
    <h3 className="text-sm font-bold mb-1 mt-1.5 first:mt-0">{children}</h3>
  ),
  blockquote: ({ children }: { children?: React.ReactNode }) => (
    <blockquote className="border-l-4 border-gray-300 dark:border-gray-600 pl-3 my-2 italic text-gray-600 dark:text-gray-400">
      {children}
    </blockquote>
  ),
  table: ({ children }: { children?: React.ReactNode }) => (
    <div className="overflow-x-auto mb-2">
      <table className="min-w-full border-collapse border border-gray-200 dark:border-gray-600">
        {children}
      </table>
    </div>
  ),
  th: ({ children }: { children?: React.ReactNode }) => (
    <th className="border border-gray-200 dark:border-gray-600 px-2 py-1 text-left font-medium bg-gray-100 dark:bg-gray-700">
      {children}
    </th>
  ),
  td: ({ children }: { children?: React.ReactNode }) => (
    <td className="border border-gray-200 dark:border-gray-600 px-2 py-1">
      {children}
    </td>
  ),
  strong: ({ children }: { children?: React.ReactNode }) => (
    <strong className="font-semibold">{children}</strong>
  ),
};

function MarkdownContent({ content, className }: { content: string; className?: string }) {
  return (
    <div className={`markdown-content text-sm leading-relaxed break-words ${className ?? ""}`}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
        {content}
      </ReactMarkdown>
    </div>
  );
}

export default function ChatView() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const { settings } = useSettingsStore();
  const {
    addTaskPlan,
    updateTaskProgress,
    addLog,
    addMemoryHint,
    clearPlan,
    setLatestOutput,
  } = useStatusStore();

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

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
      .catch(() => {});
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
  }, []);

  useEffect(() => {
    const unlisten = listen<StreamEvent>("skilllite-event", (ev) => {
      const { event, data } = ev.payload;
      if (event === "text_chunk") {
        const text = (data?.text as string) ?? "";
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last?.type === "assistant" && last?.streaming) {
            const newContent = last.content + text;
            setLatestOutput(newContent);
            return [
              ...prev.slice(0, -1),
              { ...last, content: newContent },
            ];
          }
          setLatestOutput(text);
          return [
            ...prev,
            {
              id: crypto.randomUUID(),
              type: "assistant",
              content: text,
              streaming: true,
            },
          ];
        });
        setLoading(false);
      } else if (event === "text") {
        const text = (data?.text as string) ?? "";
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
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last?.type === "assistant" && last?.streaming) {
            setLatestOutput(last.content);
            return [...prev.slice(0, -1), { ...last, streaming: false }];
          }
          return prev;
        });
        setLoading(false);
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
        addLog({ type: "error", text: msg, isError: true });
      } else if (event === "task_plan") {
        const tasks = (data?.tasks as Array<{ id?: number; description?: string; tool_hint?: string; completed?: boolean }>) ?? [];
        const taskItems = tasks.map((t, i) => ({
          id: t.id ?? i + 1,
          description: t.description ?? "",
          tool_hint: t.tool_hint,
          completed: (t.completed ?? false) as boolean,
        }));
        addTaskPlan(taskItems);
        addLog({ type: "plan", text: `计划 ${tasks.length} 个任务` });
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
          type: "tool_call",
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
          type: "tool_result",
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
  }, [addTaskPlan, updateTaskProgress, addLog, addMemoryHint, setLatestOutput]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading) return;

    setInput("");
    setError(null);
    clearPlan();
    setLatestOutput("");
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

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const renderMessage = (m: ChatMessage) => {
    if (m.type === "user") {
      return (
        <div key={m.id} className="flex justify-end">
          <div className="max-w-[80%] rounded-lg px-4 py-2 bg-blue-500 text-white [&_a]:text-blue-100 [&_a]:underline [&_code]:bg-white/20 [&_code]:px-1">
            <MarkdownContent content={m.content} className="text-white [&_*]:text-inherit" />
          </div>
        </div>
      );
    }
    if (m.type === "assistant") {
      return (
        <div key={m.id} className="flex justify-start">
          <div className="max-w-[80%] rounded-lg px-4 py-2 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 border border-gray-200 dark:border-gray-700">
            <MarkdownContent content={m.content} />
            {m.streaming && (
              <span className="inline-block w-2 h-4 ml-1 bg-current animate-pulse align-middle" />
            )}
          </div>
        </div>
      );
    }
    if (m.type === "plan") {
      return (
        <div key={m.id} className="flex justify-start">
          <div className="max-w-[85%] rounded-lg px-4 py-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800">
            <div className="text-sm font-medium text-amber-800 dark:text-amber-200 mb-2">
              任务计划
            </div>
            <ul className="space-y-1.5 text-sm text-gray-700 dark:text-gray-300">
              {m.tasks.map((t) => (
                <li key={t.id} className="flex items-start gap-2">
                  <span className="shrink-0 mt-0.5">
                    {t.completed ? "✓" : "○"}
                  </span>
                  <span>{t.description}</span>
                  {t.tool_hint && (
                    <span className="text-amber-600 dark:text-amber-400 shrink-0">
                      [{t.tool_hint}]
                    </span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        </div>
      );
    }
    if (m.type === "tool_call") {
      return (
        <div key={m.id} className="flex justify-start">
          <div className="max-w-[85%] rounded-lg px-4 py-2 bg-sky-50 dark:bg-sky-900/20 border border-sky-200 dark:border-sky-800">
            <div className="text-sm font-mono text-sky-700 dark:text-sky-300">
              <span className="font-medium">→ {m.name}</span>
              {m.args && (
                <pre className="mt-1 text-xs overflow-x-auto whitespace-pre-wrap break-words text-gray-600 dark:text-gray-400">
                  {m.args.length > 200 ? m.args.slice(0, 200) + "…" : m.args}
                </pre>
              )}
            </div>
          </div>
        </div>
      );
    }
    if (m.type === "tool_result") {
      return (
        <div key={m.id} className="flex justify-start">
          <div
            className={`max-w-[85%] rounded-lg px-4 py-2 border ${
              m.isError
                ? "bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800"
                : "bg-emerald-50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-800"
            }`}
          >
            <div className="text-sm font-mono">
              <span className="font-medium">
                {m.isError ? "✗ " : "✓ "}
                {m.name}
              </span>
              <pre className="mt-1 text-xs overflow-x-auto whitespace-pre-wrap break-words max-h-40 overflow-y-auto text-gray-700 dark:text-gray-300">
                {m.result.length > 500 ? m.result.slice(0, 500) + "…" : m.result}
              </pre>
            </div>
          </div>
        </div>
      );
    }
    if (m.type === "confirmation") {
      return (
        <div key={m.id} className="flex justify-start">
          <div className="max-w-[85%] rounded-lg px-4 py-3 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800">
            <div className="text-sm font-medium text-orange-800 dark:text-orange-200 mb-2">
              执行确认
            </div>
            <pre className="whitespace-pre-wrap text-sm text-gray-700 dark:text-gray-300 mb-4 max-h-48 overflow-y-auto">
              {m.prompt}
            </pre>
            {m.resolved ? (
              <div className="text-sm text-gray-600 dark:text-gray-400">
                {m.approved ? "✓ 已允许" : "✗ 已拒绝"}
              </div>
            ) : (
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => handleConfirm(m.id, false)}
                  className="px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
                >
                  拒绝
                </button>
                <button
                  type="button"
                  onClick={() => handleConfirm(m.id, true)}
                  className="px-3 py-1.5 text-sm rounded-lg bg-orange-500 text-white font-medium hover:bg-orange-600"
                >
                  允许
                </button>
              </div>
            )}
          </div>
        </div>
      );
    }
    return null;
  };

  return (
    <div className="flex flex-col h-full bg-gray-50 dark:bg-gray-900">
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.map(renderMessage)}
        {loading &&
          (messages.length === 0 || messages[messages.length - 1]?.type === "user") && (
            <div className="flex justify-start">
              <div className="rounded-lg px-4 py-2 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700">
                <span className="inline-block w-2 h-4 bg-gray-400 animate-pulse" />
              </div>
            </div>
          )}
        <div ref={messagesEndRef} />
      </div>

      {error && (
        <div className="px-4 py-2 bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 text-sm">
          {error}
        </div>
      )}

      <div className="p-4 border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800">
        <div className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a message..."
            disabled={loading}
            className="flex-1 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-4 py-2 text-gray-900 dark:text-gray-100 placeholder-gray-500 focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:opacity-50"
          />
          <button
            type="button"
            onClick={handleSend}
            disabled={loading || !input.trim()}
            className="px-4 py-2 rounded-lg bg-blue-500 text-white font-medium hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}
