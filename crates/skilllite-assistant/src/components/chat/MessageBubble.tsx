import { memo } from "react";
import { MarkdownContent } from "../shared/MarkdownContent";
import type { ChatMessage } from "../../types/chat";

interface MessageBubbleProps {
  message: ChatMessage;
  onConfirm?: (id: string, approved: boolean) => void;
  onClarify?: (id: string, action: string, hint?: string) => void;
}

/** Shared chat bubble chrome: width cap, radius, border, shadow, type scale */
const bubbleShell =
  "max-w-[min(85%,36rem)] rounded-2xl border text-sm leading-relaxed shadow-sm shadow-ink/[0.06] dark:shadow-none";

const bubbleUser =
  `${bubbleShell} ml-8 px-4 py-2.5 bg-accent-light dark:bg-accent-light-dark/90 border-accent/25 dark:border-accent/35 text-ink dark:text-ink-dark [&_a]:text-accent dark:[&_a]:text-blue-300 [&_a]:underline`;

const bubbleAssistant =
  `${bubbleShell} mr-4 px-4 py-2.5 bg-white dark:bg-paper-dark border-border dark:border-border-dark text-ink dark:text-ink-dark [&_a]:text-accent dark:[&_a]:text-blue-300`;

const bubbleMuted =
  `${bubbleShell} mr-4 px-4 py-3 bg-ink/[0.03] dark:bg-white/[0.05] border-border dark:border-border-dark text-ink dark:text-ink-dark`;

function MessageBubbleInner({ message, onConfirm, onClarify }: MessageBubbleProps) {
  if (message.type === "user") {
    return (
      <div className="flex justify-end">
        <div className={bubbleUser}>
          <MarkdownContent content={message.content} />
        </div>
      </div>
    );
  }

  if (message.type === "assistant") {
    return (
      <div className="flex justify-start">
        <div className={bubbleAssistant}>
          <MarkdownContent content={message.content} />
          {message.streaming && (
            <span className="inline-block w-2 h-4 ml-1 bg-accent animate-pulse align-middle rounded-sm" />
          )}
        </div>
      </div>
    );
  }

  if (message.type === "plan") {
    return (
      <div className="flex justify-start">
        <div className={bubbleMuted}>
          <div className="text-xs font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute mb-2">
            任务计划
          </div>
          <ul className="space-y-1.5 text-sm text-ink dark:text-ink-dark-mute">
            {message.tasks.map((t) => (
              <li key={t.id} className="flex items-start gap-2">
                <span className="shrink-0 mt-0.5">{t.completed ? "✓" : "○"}</span>
                <span>{t.description}</span>
                {t.tool_hint && (
                  <span className="text-ink-mute dark:text-ink-dark-mute shrink-0 text-xs">
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

  if (message.type === "tool_call") {
    return (
      <div className="flex justify-start">
        <div className={bubbleMuted}>
          <div className="text-xs font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute mb-1.5">
            工具调用
          </div>
          <div className="text-sm font-mono text-ink-mute dark:text-ink-dark-mute">
            <span className="font-medium text-ink dark:text-ink-dark">→ {message.name}</span>
            {message.args && (
              <pre className="mt-1 text-xs overflow-x-auto whitespace-pre-wrap break-words text-ink-mute dark:text-ink-dark-mute">
                {message.args.length > 200 ? message.args.slice(0, 200) + "…" : message.args}
              </pre>
            )}
          </div>
        </div>
      </div>
    );
  }

  if (message.type === "tool_result") {
    return (
      <div className="flex justify-start">
        <div
          className={`${bubbleShell} mr-4 px-4 py-3 ${
            message.isError
              ? "bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800/50"
              : "bg-ink/[0.03] dark:bg-white/[0.05] border-border dark:border-border-dark"
          }`}
        >
          <div className="text-xs font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute mb-1.5">
            工具结果
          </div>
          <div className="text-sm font-mono">
            <span className="font-medium text-ink dark:text-ink-dark">
              {message.isError ? "✗ " : "✓ "}
              {message.name}
            </span>
            <pre className="mt-1 text-xs overflow-x-auto whitespace-pre-wrap break-words max-h-40 overflow-y-auto text-ink dark:text-ink-dark-mute">
              {message.result.length > 500 ? message.result.slice(0, 500) + "…" : message.result}
            </pre>
          </div>
        </div>
      </div>
    );
  }

  if (message.type === "confirmation") {
    return (
      <div className="flex justify-start">
        <div
          className={`${bubbleShell} mr-4 px-4 py-3 bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-800/50`}
        >
          <div className="text-xs font-semibold uppercase tracking-wide text-amber-800 dark:text-amber-200 mb-2">
            执行确认
          </div>
          <pre className="whitespace-pre-wrap text-sm text-ink dark:text-ink-dark-mute mb-4 max-h-48 overflow-y-auto">
            {message.prompt}
          </pre>
          {message.resolved ? (
            <div className="text-sm text-ink-mute dark:text-ink-dark-mute">
              {message.approved ? "✓ 已允许" : "✗ 已拒绝"}
            </div>
          ) : (
            onConfirm && (
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => onConfirm(message.id, false)}
                  className="px-3 py-1.5 text-sm rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark hover:bg-gray-100 dark:hover:bg-white/5"
                >
                  拒绝
                </button>
                <button
                  type="button"
                  onClick={() => onConfirm(message.id, true)}
                  className="px-3 py-1.5 text-sm rounded-md bg-accent text-white font-medium hover:bg-accent-hover"
                >
                  允许
                </button>
              </div>
            )
          )}
        </div>
      </div>
    );
  }

  if (message.type === "clarification") {
    return (
      <div className="flex justify-start">
        <div
          className={`${bubbleShell} mr-4 px-4 py-3 bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800/50`}
        >
          <div className="text-xs font-semibold uppercase tracking-wide text-blue-800 dark:text-blue-200 mb-1.5">
            需要你的确认
          </div>
          <p className="text-sm text-ink dark:text-ink-dark-mute mb-3">
            {message.message}
          </p>
          {message.resolved ? (
            <div className="text-sm text-ink-mute dark:text-ink-dark-mute">
              {message.selectedOption === "stop"
                ? "✗ 已停止"
                : `✓ ${message.selectedOption ?? "已继续"}`}
            </div>
          ) : (
            onClarify && (
              <div className="flex flex-wrap gap-2">
                {message.suggestions.map((s) => (
                  <button
                    key={s}
                    type="button"
                    onClick={() => onClarify(message.id, "continue", s)}
                    className="px-3 py-1.5 text-sm rounded-md bg-accent text-white font-medium hover:bg-accent-hover transition-colors"
                  >
                    {s}
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => onClarify(message.id, "stop")}
                  className="px-3 py-1.5 text-sm rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark hover:bg-gray-100 dark:hover:bg-white/5 transition-colors"
                >
                  停止
                </button>
              </div>
            )
          )}
        </div>
      </div>
    );
  }

  return null;
}

export const MessageBubble = memo(MessageBubbleInner);
