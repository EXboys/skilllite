import { useRef, useEffect, useMemo, type ReactNode } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { MessageBubble } from "./MessageBubble";
import { SystemTimelineGroup } from "./SystemTimelineGroup";
import {
  indexOfLastTimelineSegment,
  partitionChatMessages,
  timelineGroupHasFilePreviewResult,
  timelineGroupNeedsUserAction,
  type ChatSegment,
} from "../../utils/chatNoise";
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

/** 内部步骤默认展开：待操作 / 本轮仍在跑 / 或「最后一段」时间线里含 read_file·list_directory 成功结果（历史段保持折叠） */
function timelineDefaultExpandedState(
  segments: ChatSegment[],
  segmentIndex: number,
  allMessages: ChatMessage[],
  group: ChatMessage[],
  loading: boolean
): boolean {
  if (timelineDefaultExpanded(allMessages, group, loading)) return true;
  if (timelineGroupNeedsUserAction(group)) return true;
  if (indexOfLastTimelineSegment(segments) !== segmentIndex) return false;
  return timelineGroupHasFilePreviewResult(group);
}

interface MessageListProps {
  messages: ChatMessage[];
  loading: boolean;
  workspace: string;
  onConfirm: (id: string, approved: boolean) => void;
  onClarify?: (id: string, action: string, hint?: string) => void;
  onEvolutionAction?: (id: string, option: string) => void;
  /** Rendered after the last message inside the scroll area (e.g. follow-up suggestions). */
  tailSlot?: ReactNode;
  /** When this string changes to a non-empty value, scroll the list end into view. */
  tailScrollSignal?: string;
}

export function MessageList({
  messages,
  loading,
  workspace,
  onConfirm,
  onClarify,
  onEvolutionAction,
  tailSlot,
  tailScrollSignal,
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

  useEffect(() => {
    if (!tailScrollSignal) return;
    requestAnimationFrame(() => {
      messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    });
  }, [tailScrollSignal]);

  const showLoadingIndicator =
    loading && (messages.length === 0 || messages[messages.length - 1]?.type === "user");

  const renderSegment = (index: number) => {
    const seg = segments[index];
    if (seg.kind === "single") {
      return (
        <div className="pb-4">
          <MessageBubble
            message={seg.message}
            workspace={workspace}
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
          workspace={workspace}
          defaultExpanded={timelineDefaultExpandedState(
            segments,
            index,
            messages,
            seg.messages,
            loading
          )}
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
        {segments.map((seg, segmentIndex) =>
          seg.kind === "single" ? (
            <div key={seg.message.id}>
              <MessageBubble
                message={seg.message}
                workspace={workspace}
                onConfirm={onConfirm}
                onClarify={onClarify}
                onEvolutionAction={onEvolutionAction}
              />
            </div>
          ) : (
            <SystemTimelineGroup
              key={seg.messages.map((m) => m.id).join("|")}
              messages={seg.messages}
              workspace={workspace}
              defaultExpanded={timelineDefaultExpandedState(
                segments,
                segmentIndex,
                messages,
                seg.messages,
                loading
              )}
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
        {tailSlot}
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
      {tailSlot}
      <div ref={messagesEndRef} />
    </div>
  );
}
