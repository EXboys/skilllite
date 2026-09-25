import type { ChatMessage } from "../types/chat";

/** 不在主对话流展示的工具（仍写入侧栏/详情日志） */
const HIDDEN_TOOL_NAMES = new Set(["complete_task", "update_task_plan"]);

export function isChatHiddenToolName(name: string): boolean {
  return HIDDEN_TOOL_NAMES.has(name);
}

/**
 * 折叠进「内部步骤」时间线：
 * - plan / tool_call：计划与调用
 * - tool_result：工具执行结果（与调用同一视觉分组，避免主对话流被工具输出刷屏）
 * - confirmation / clarification：与当次工具同一轮产生的确认（落盘后应与 tool_call 相邻；单独展示会跑到折叠区「上面」，观感像顺序颠倒）
 */
export function isTechnicalTimelineMessage(m: ChatMessage): boolean {
  return (
    m.type === "plan" ||
    m.type === "tool_call" ||
    m.type === "tool_result" ||
    m.type === "confirmation" ||
    m.type === "clarification"
  );
}

/** 时间线内是否有尚未处理的确认/澄清（需要用户在折叠区内点击按钮） */
export function timelineGroupNeedsUserAction(messages: ChatMessage[]): boolean {
  return messages.some((m) => {
    if (m.type === "confirmation") {
      return !m.resolved;
    }
    if (m.type === "clarification") {
      return !m.resolved;
    }
    return false;
  });
}

/** 对话内有专用预览 UI 的文件类工具结果（与 indexOfLastTimelineSegment 组合使用，只对最后一段时间线默认展开）。 */
const FILE_PREVIEW_TOOL_RESULTS = new Set(["read_file", "list_directory"]);

function normalizeToolName(name: string): string {
  return name.replace(/-/g, "_").trim().toLowerCase();
}

export function timelineGroupHasFilePreviewResult(messages: ChatMessage[]): boolean {
  return messages.some(
    (m) =>
      m.type === "tool_result" &&
      !m.isError &&
      FILE_PREVIEW_TOOL_RESULTS.has(normalizeToolName(m.name)),
  );
}

export type ChatSegment =
  | { kind: "single"; message: ChatMessage }
  | { kind: "timeline"; messages: ChatMessage[] };

/** `partitionChatMessages` 结果中，最后一个 `timeline` 片段的下标；无则 -1 */
export function indexOfLastTimelineSegment(segments: ChatSegment[]): number {
  let idx = -1;
  for (let i = 0; i < segments.length; i++) {
    if (segments[i].kind === "timeline") idx = i;
  }
  return idx;
}

/** 将消息切成「单条」与「内部步骤」时间线（plan + tool_call + tool_result + 确认/澄清）；用户消息与助手正文等为单条。 */
export function partitionChatMessages(messages: ChatMessage[]): ChatSegment[] {
  const out: ChatSegment[] = [];
  let buf: ChatMessage[] = [];

  const flushBuf = () => {
    if (buf.length === 0) return;
    out.push({ kind: "timeline", messages: [...buf] });
    buf = [];
  };

  for (const m of messages) {
    if (isTechnicalTimelineMessage(m)) {
      buf.push(m);
    } else {
      flushBuf();
      out.push({ kind: "single", message: m });
    }
  }
  flushBuf();
  return out;
}

export function summarizeTimelineGroup(items: ChatMessage[]): string {
  const plan = items.find((i): i is Extract<ChatMessage, { type: "plan" }> => i.type === "plan");
  if (plan?.tasks?.[0]?.description) {
    const d = plan.tasks[0].description;
    return d.length > 36 ? `${d.slice(0, 36)}…` : d;
  }
  const call = items.find((i): i is Extract<ChatMessage, { type: "tool_call" }> => i.type === "tool_call");
  if (call?.name) return `工具：${call.name}`;
  const tr = items.find((i): i is Extract<ChatMessage, { type: "tool_result" }> => i.type === "tool_result");
  if (tr?.name) return `工具：${tr.name}`;
  return `${items.length} 条步骤`;
}
