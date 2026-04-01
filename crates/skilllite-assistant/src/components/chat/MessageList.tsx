import { useRef, useEffect, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { MessageBubble } from "./MessageBubble";
import { SystemTimelineGroup } from "./SystemTimelineGroup";
import { partitionChatMessages } from "../../utils/chatNoise";
import type { ChatMessage } from "../../types/chat";

const USE_VIRTUAL_THRESHOLD = 48;

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
  onEvolutionAction?: (id: string, option: string) => void;
}

export function MessageList({
  messages,
  loading,
  onConfirm,
  onClarify,
  onEvolutionAction,
}: MessageListProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const parentRef = useRef<HTMLDivElement>(null);
  const segments = useMemo(() => partitionChatMessages(messages), [messages]);

  const useVirtual = segments.length >= USE_VIRTUAL_THRESHOLD;

  const virtualizer = useVirtualizer({
    count: segments.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 96,
    overscan: 8,
    enabled: useVirtual,
  });

  const lastMsg = messages.length > 0 ? messages[messages.length - 1] : null;
  const lastContentLen =
    lastMsg && "content" in lastMsg ? (lastMsg as { content: string }).content.length : 0;

  useEffect(() => {
    if (useVirtual) {
      if (segments.length > 0) {
        requestAnimationFrame(() => {
          virtualizer.scrollToIndex(segments.length - 1, { align: "end" });
        });
      }
      return;
    }
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    // virtualizer intentionally omitted: identity changes would retrigger every frame
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messages.length, lastMsg?.id, lastContentLen, segments.length, useVirtual]);

  const showLoadingIndicator =
    loading && (messages.length === 0 || messages[messages.length - 1]?.type === "user");

  const renderSegment = (index: number) => {
    const seg = segments[index];
    if (seg.kind === "single") {
      return (
        <div className="pb-4">
          <MessageBubble
            message={seg.message}
            onConfirm={onConfirm}
            onClarify={onClarify}
            onEvolutionAction={onEvolutionAction}
          />
        </div>
      );
    }
    return (
      <div className="pb-4">
        <SystemTimelineGroup
          key={seg.messages.map((m) => m.id).join("|")}
          messages={seg.messages}
          defaultExpanded={timelineDefaultExpanded(messages, seg.messages, loading)}
          onConfirm={onConfirm}
          onClarify={onClarify}
          onEvolutionAction={onEvolutionAction}
        />
      </div>
    );
  };

  if (!useVirtual) {
    return (
      <div ref={parentRef} className="flex-1 overflow-y-auto p-5 space-y-4">
        {segments.map((seg) =>
          seg.kind === "single" ? (
            <div key={seg.message.id}>
              <MessageBubble
                message={seg.message}
                onConfirm={onConfirm}
                onClarify={onClarify}
                onEvolutionAction={onEvolutionAction}
              />
            </div>
          ) : (
            <SystemTimelineGroup
              key={seg.messages.map((m) => m.id).join("|")}
              messages={seg.messages}
              defaultExpanded={timelineDefaultExpanded(messages, seg.messages, loading)}
              onConfirm={onConfirm}
              onClarify={onClarify}
              onEvolutionAction={onEvolutionAction}
            />
          )
        )}
        {showLoadingIndicator && (
          <div className="flex justify-start">
            <div className="max-w-[min(85%,36rem)] mr-4 rounded-2xl px-4 py-2.5 bg-white dark:bg-paper-dark border border-border dark:border-border-dark shadow-sm shadow-ink/[0.06] dark:shadow-none">
              <span
                className="inline-block w-2 h-4 bg-accent/60 animate-pulse rounded-sm"
                aria-hidden
              />
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>
    );
  }

  return (
    <div ref={parentRef} className="flex-1 overflow-y-auto p-5">
      <div
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          width: "100%",
          position: "relative",
        }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => (
          <div
            key={virtualRow.key}
            data-index={virtualRow.index}
            ref={virtualizer.measureElement}
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              width: "100%",
              transform: `translateY(${virtualRow.start}px)`,
            }}
          >
            {renderSegment(virtualRow.index)}
          </div>
        ))}
      </div>
      {showLoadingIndicator && (
        <div className="flex justify-start pt-2">
          <div className="max-w-[min(85%,36rem)] mr-4 rounded-2xl px-4 py-2.5 bg-white dark:bg-paper-dark border border-border dark:border-border-dark shadow-sm shadow-ink/[0.06] dark:shadow-none">
            <span
              className="inline-block w-2 h-4 bg-accent/60 animate-pulse rounded-sm"
              aria-hidden
            />
          </div>
        </div>
      )}
      <div ref={messagesEndRef} />
    </div>
  );
}
