import type { ChatMessage } from "../types/chat";

const MAX_ASSISTANT_CHARS = 2000;
const MAX_TOTAL_CHARS = 12000;

/**
 * 将当前消息列表压成 User/Assistant 文本，供会话结束时生成「猜你想问」。
 */
export function serializeChatMessagesForFollowup(messages: ChatMessage[]): string {
  const parts: string[] = [];
  for (const m of messages) {
    if (m.type === "user") {
      const c = m.content.trim();
      if (c) parts.push(`User: ${c}`);
    } else if (m.type === "assistant" && !m.streaming) {
      const c = m.content.trim();
      if (c) {
        const slice =
          c.length > MAX_ASSISTANT_CHARS
            ? `${c.slice(0, MAX_ASSISTANT_CHARS)}…`
            : c;
        parts.push(`Assistant: ${slice}`);
      }
    }
  }
  let out = parts.join("\n\n");
  if (out.length > MAX_TOTAL_CHARS) {
    out = out.slice(-MAX_TOTAL_CHARS);
  }
  return out;
}
