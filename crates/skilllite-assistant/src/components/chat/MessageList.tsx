import { useRef, useEffect } from "react";
import { MessageBubble } from "./MessageBubble";
import type { ChatMessage } from "../../types/chat";

interface MessageListProps {
  messages: ChatMessage[];
  loading: boolean;
  onConfirm: (id: string, approved: boolean) => void;
}

export function MessageList({ messages, loading, onConfirm }: MessageListProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);

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
      {messages.map((m) => (
        <div key={m.id}>
          <MessageBubble message={m} onConfirm={onConfirm} />
        </div>
      ))}
      {showLoadingIndicator && (
        <div className="flex justify-start">
          <div className="rounded-lg px-4 py-2.5 bg-white dark:bg-paper-dark border border-border dark:border-border-dark">
            <span className="inline-block w-2 h-4 bg-accent/60 animate-pulse rounded-sm" />
          </div>
        </div>
      )}
      <div ref={messagesEndRef} />
    </div>
  );
}
