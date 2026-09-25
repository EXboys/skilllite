/**
 * Strip provider-specific reasoning XML from assistant markdown before display.
 * Mirrors `skilllite_evolution::sanitize_visible_llm_text` (keep in sync on tag changes).
 */
function stripPairedBlocks(input: string, open: string, close: string): string {
  let out = input;
  for (;;) {
    const start = out.indexOf(open);
    if (start < 0) break;
    const afterOpen = start + open.length;
    const end = out.indexOf(close, afterOpen);
    if (end < 0) {
      out = out.slice(0, start);
      break;
    }
    const endFull = end + close.length;
    out = out.slice(0, start) + out.slice(endFull);
  }
  return out;
}

/** Same semantics as `skilllite_evolution::strip_think_blocks` (subset sufficient for chat UI). */
function stripThinkBlocks(content: string): string {
  const openingTags = [
    "<think>",
    "<think\n",
    "<thinking>",
    "<thinking\n",
    "<reasoning>",
    "<reasoning\n",
  ];
  const closings = [
    "</think>",
    "</Redacted_Thinking>",
    "</thinking>",
    "</Thinking>",
    "</reasoning>",
    "</Reasoning>",
  ];
  let bestEnd = -1;
  for (const tag of closings) {
    const pos = content.lastIndexOf(tag);
    if (pos < 0) continue;
    const end = pos + tag.length;
    if (end > bestEnd) bestEnd = end;
  }
  if (bestEnd >= 0) {
    const after = content.slice(bestEnd).trim();
    if (after.length > 0) return after;
  }
  for (const tag of openingTags) {
    const pos = content.indexOf(tag);
    if (pos >= 0) {
      const before = content.slice(0, pos).trim();
      if (before.length > 0) return before;
    }
  }
  return content;
}

export function sanitizeLlmVisibleChatText(content: string): string {
  let s = content.trim();
  s = stripPairedBlocks(s, "<think>", "</think>");
  s = stripPairedBlocks(s, "<Redacted_Thinking>", "</Redacted_Thinking>");
  s = stripPairedBlocks(s, "<thinking>", "</thinking>");
  s = stripPairedBlocks(s, "<Thinking>", "</Thinking>");
  s = stripPairedBlocks(s, "<reasoning>", "</reasoning>");
  s = stripPairedBlocks(s, "<Reasoning>", "</Reasoning>");
  for (const tag of [
    "</think>",
    "</Redacted_Thinking>",
    "</thinking>",
    "</Thinking>",
    "</reasoning>",
    "</Reasoning>",
  ]) {
    while (s.includes(tag)) {
      s = s.split(tag).join("");
    }
  }
  return stripThinkBlocks(s).trim();
}
