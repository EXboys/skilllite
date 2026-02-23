import { memo } from "react";
import { MarkdownContent } from "../shared/MarkdownContent";
import type { ChatMessage } from "../../types/chat";

interface MessageBubbleProps {
  message: ChatMessage;
  onConfirm?: (id: string, approved: boolean) => void;
}

function MessageBubbleInner({ message, onConfirm }: MessageBubbleProps) {
  if (message.type === "user") {
    return (
      <div className="flex justify-end">
        <div className="max-w-[80%] rounded-lg px-4 py-2.5 bg-gray-200 dark:bg-gray-700 text-ink dark:text-ink-dark [&_a]:text-accent [&_a]:underline [&_code]:bg-black/10 dark:[&_code]:bg-white/10 [&_code]:px-1">
          <MarkdownContent content={message.content} />
        </div>
      </div>
    );
  }

  if (message.type === "assistant") {
    return (
      <div className="flex justify-start">
        <div className="max-w-[80%] rounded-lg px-4 py-2.5 bg-white dark:bg-paper-dark text-ink dark:text-ink-dark border border-border dark:border-border-dark">
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
        <div className="max-w-[85%] rounded-lg px-4 py-3 bg-gray-50 dark:bg-gray-800/50 border border-border dark:border-border-dark">
          <div className="text-sm font-medium text-ink dark:text-ink-dark-mute mb-2">任务计划</div>
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
        <div className="max-w-[85%] rounded-lg px-4 py-2 bg-gray-50 dark:bg-gray-800/50 border border-border dark:border-border-dark">
          <div className="text-sm font-mono text-ink-mute dark:text-ink-dark-mute">
            <span className="font-medium">→ {message.name}</span>
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
          className={`max-w-[85%] rounded-lg px-4 py-2 border ${
            message.isError
              ? "bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800/50"
              : "bg-gray-50 dark:bg-gray-800/50 border-border dark:border-border-dark"
          }`}
        >
          <div className="text-sm font-mono">
            <span className="font-medium">{message.isError ? "✗ " : "✓ "}{message.name}</span>
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
        <div className="max-w-[85%] rounded-lg px-4 py-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800/50">
          <div className="text-sm font-semibold text-amber-800 dark:text-amber-200 mb-2">执行确认</div>
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

  return null;
}

export const MessageBubble = memo(MessageBubbleInner);
