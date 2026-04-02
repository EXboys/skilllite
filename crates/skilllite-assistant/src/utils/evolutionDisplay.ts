/**
 * User-facing copy for evolution backlog / proposal status.
 * Backend may set acceptance_status=not_met with note "Executed with no material changes"
 * when a run completed but produced no file changes — this is easy to misread as "进化没跑".
 */

/** One-line summary for compact UI (e.g. chat progress). */
export const EVOLUTION_NO_SKILL_HEADLINE = "已执行，本轮未生成可落盘技能";
const PATTERN_EXPLAIN_LINE =
  "当前技能进化依赖决策库中的重复模式，单次缺口不会自动合成技能。";

const NO_MATERIAL_NOTE_RE = /no\s+material\s+changes/i;

export function isEvolutionNoMaterialChanges(
  status: string,
  acceptanceStatus: string,
  note: string | null | undefined
): boolean {
  const s = status.trim().toLowerCase();
  const a = acceptanceStatus.trim().toLowerCase();
  const n = (note ?? "").trim();
  return s === "executed" && a === "not_met" && NO_MATERIAL_NOTE_RE.test(n);
}

/** Short label for status row (e.g. bubble "进度" line). */
export function evolutionStatusHeadline(
  status: string,
  acceptanceStatus: string,
  note: string | null | undefined
): string {
  if (isEvolutionNoMaterialChanges(status, acceptanceStatus, note)) {
    return EVOLUTION_NO_SKILL_HEADLINE;
  }
  return `${status.trim()}/${acceptanceStatus.trim()}`;
}

/** Note / detail under headline; null means hide. */
export function evolutionNoteForDisplay(
  status: string,
  acceptanceStatus: string,
  note: string | null | undefined
): string | null {
  if (isEvolutionNoMaterialChanges(status, acceptanceStatus, note)) {
    return PATTERN_EXPLAIN_LINE;
  }
  const n = (note ?? "").trim();
  return n.length > 0 ? n : null;
}

/** Format backlog row note column (may replace raw English system note). */
export function evolutionBacklogNoteForDisplay(
  status: string,
  acceptanceStatus: string,
  note: string
): string {
  if (isEvolutionNoMaterialChanges(status, acceptanceStatus, note)) {
    return `${EVOLUTION_NO_SKILL_HEADLINE}。\n${PATTERN_EXPLAIN_LINE}`;
  }
  return note;
}

/** If backend output mentions no material changes, prepend friendly explanation. */
export function prependNoMaterialHelpIfNeeded(text: string): string {
  if (!NO_MATERIAL_NOTE_RE.test(text)) {
    return text;
  }
  return `${EVOLUTION_NO_SKILL_HEADLINE}。\n${PATTERN_EXPLAIN_LINE}\n\n${text}`;
}
