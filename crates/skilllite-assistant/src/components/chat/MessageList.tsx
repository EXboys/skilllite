import { useRef, useEffect, useMemo } from "react";
import { MessageBubble } from "./MessageBubble";
import { SystemTimelineGroup } from "./SystemTimelineGroup";
import { partitionChatMessages } from "../../utils/chatNoise";
import type { ChatMessage } from "../../types/chat";

function timelineDefaultExpanded(
  all: ChatMessage[],
  group: ChatMessage[],
  loading: boolean
): boolean {
  if (!loading || group.length === 0) return false;
  const last = all[all.length - 1];
  const gl = group[group.length - 1];
  return last?.id === gl?.id;
}

interface MessageListProps {
  messages: ChatMessage[];
  loading: boolean;
  onConfirm: (id: string, approved: boolean) => void;
  onClarify?: (id: string, action: string, hint?: string) => void;
}

export function MessageList({ messages, loading, onConfirm, onClarify }: MessageListProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const segments = useMemo(() => partitionChatMessages(messages), [messages]);

  const lastMsg = messages.length > 0 ? messages[messages.length - 1] : null;
  const lastContentLen =
    lastMsg && "content" in lastMsg ? (lastMsg as { content: string }).content.length : 0;
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length, lastMsg?.id, lastContentLen]);

  const showLoadingIndicator =
    loading && (messages.length === 0 || messages[messages.length - 1]?.type === "user");

  return (
    <div className="flex-1 overflow-y-auto p-5 space-y-4">
      {segments.map((seg) =>
        seg.kind === "single" ? (
          <div key={seg.message.id}>
            <MessageBubble message={seg.message} onConfirm={onConfirm} onClarify={onClarify} />
          </div>
        ) : (
          <SystemTimelineGroup
            key={seg.messages.map((m) => m.id).join("|")}
            messages={seg.messages}
            defaultExpanded={timelineDefaultExpanded(messages, seg.messages, loading)}
            onConfirm={onConfirm}
            onClarify={onClarify}
          />
        )
      )}
      {showLoadingIndicator && (
        <div className="flex justify-start">
          <div className="max-w-[min(85%,36rem)] mr-4 rounded-2xl px-4 py-2.5 bg-white dark:bg-paper-dark border border-border dark:border-border-dark shadow-sm shadow-ink/[0.06] dark:shadow-none">
            <span className="inline-block w-2 h-4 bg-accent/60 animate-pulse rounded-sm" aria-hidden />
          </div>
        </div>
      )}
      <div ref={messagesEndRef} />
    </div>
  );
}
