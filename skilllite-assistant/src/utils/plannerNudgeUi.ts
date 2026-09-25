/**
 * Detects task-planner "nudge" blocks echoed into chat (assistant or rare user paste)
 * and splits them from the user-visible answer. Markers align with
 * `TaskPlanner::build_nudge_message` and related agent_loop user injections.
 */

const INLINE_START_MARKERS = [
  "There are still pending tasks.",
  "Pending tasks still exist.",
  "CRITICAL: You just described",
  "Updated task list:\n",
];

/** Split after a blank line before these phrases (continuation / echoed nudge). */
const BLOCK_MARKERS = [
  "\n\nThere are still pending tasks.",
  "\n\nPending tasks still exist.",
  "\n\nCRITICAL: You just described",
  "\n\nUpdated task list:\n",
  "\n\nUpdated task list:",
  "\n\n⚠️ After completing this task, call `complete_task`",
];

function earliestBlockIndex(content: string): number {
  let best = -1;
  for (const m of BLOCK_MARKERS) {
    const i = content.indexOf(m);
    if (i >= 0 && (best < 0 || i < best)) {
      best = i;
    }
  }
  return best;
}

/** Optional one-line hint from "Current task: Task N - …" inside boilerplate. */
export function plannerNudgeCurrentTaskHint(boilerplate: string): string | null {
  const m = boilerplate.match(/Current task:\s*Task\s+(\d+)\s*-\s*([^\n]+)/);
  if (!m) return null;
  const rest = m[2].trim();
  if (!rest) return `Task ${m[1]}`;
  const short = rest.length > 52 ? `${rest.slice(0, 52)}…` : rest;
  return `Task ${m[1]} · ${short}`;
}

export function splitPlannerBoilerplate(content: string): {
  main: string;
  boilerplate: string | null;
} {
  const trimmed = content;
  if (!trimmed) {
    return { main: "", boilerplate: null };
  }

  for (const start of INLINE_START_MARKERS) {
    if (trimmed.startsWith(start)) {
      return { main: "", boilerplate: trimmed };
    }
  }

  const idx = earliestBlockIndex(trimmed);
  if (idx < 0) {
    return { main: trimmed, boilerplate: null };
  }

  const main = trimmed.slice(0, idx).trimEnd();
  const boilerplate = trimmed.slice(idx).trimStart();
  if (!boilerplate) {
    return { main: trimmed, boilerplate: null };
  }
  return { main, boilerplate };
}
