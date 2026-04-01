import { useEffect, useState } from "react";
import { MessageBubble } from "./MessageBubble";
import { summarizeTimelineGroup } from "../../utils/chatNoise";
import type { ChatMessage } from "../../types/chat";

interface SystemTimelineGroupProps {
  messages: ChatMessage[];
  /** 为 true 时默认展开（例如本轮仍在进行且尚未出现助手回复） */
  defaultExpanded: boolean;
  onConfirm: (id: string, approved: boolean) => void;
  onClarify?: (id: string, action: string, hint?: string) => void;
  onEvolutionAction?: (id: string, option: string) => void;
}

export function SystemTimelineGroup({
  messages,
  defaultExpanded,
  onConfirm,
  onClarify,
  onEvolutionAction,
}: SystemTimelineGroupProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  useEffect(() => {
    if (!defaultExpanded) setExpanded(false);
  }, [defaultExpanded]);

  const summary = summarizeTimelineGroup(messages);
  const n = messages.length;

  return (
    <div className="flex justify-start">
      <div className="max-w-[min(85%,36rem)] mr-4 w-full rounded-2xl border border-border/80 dark:border-border-dark/80 bg-ink/[0.02] dark:bg-white/[0.04] shadow-sm shadow-ink/[0.04] dark:shadow-none overflow-hidden">
        <button
          type="button"
          onClick={() => setExpanded((e) => !e)}
          className="flex w-full items-center gap-2 px-3.5 py-2.5 text-left text-sm text-ink-mute dark:text-ink-dark-mute hover:bg-ink/[0.04] dark:hover:bg-white/[0.06] transition-colors"
          aria-expanded={expanded}
        >
          <span className="text-ink/50 dark:text-ink-dark-mute/80 shrink-0" aria-hidden>
            {expanded ? "▼" : "▶"}
          </span>
          <span className="font-medium text-ink dark:text-ink-dark">内部步骤</span>
          <span className="text-xs tabular-nums opacity-80">· {n} 条</span>
          {!expanded && (
            <span className="truncate text-xs opacity-75 min-w-0" title={summary}>
              — {summary}
            </span>
          )}
        </button>
        {expanded && (
          <div className="px-3 pb-3 pt-0 space-y-3 border-t border-border/60 dark:border-border-dark/60">
            {messages.map((m) => (
              <div key={m.id} className="first:pt-3">
                <MessageBubble
                  message={m}
                  onConfirm={onConfirm}
                  onClarify={onClarify}
                  onEvolutionAction={onEvolutionAction}
                />
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
