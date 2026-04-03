import { translate, type Locale } from "../i18n/translate";

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

/** Backlog `status` column — human label for evolution queue UI (CN). */
export function evolutionBacklogStatusLabel(status: string): string {
  const s = status.trim().toLowerCase();
  switch (s) {
    case "queued":
      return "待放行";
    case "executing":
      return "执行中";
    case "executed":
      return "已执行";
    case "policy_denied":
      return "策略拒绝";
    case "shadow_approved":
      return "已预准（待跑）";
    default:
      return status.trim() || "—";
  }
}

/** Backlog `acceptance_status` column — human label (CN). */
export function evolutionAcceptanceStatusLabel(acceptanceStatus: string): string {
  const a = acceptanceStatus.trim().toLowerCase();
  switch (a) {
    case "pending":
      return "等待执行";
    case "pending_validation":
      return "验收观察期";
    case "met":
      return "验收通过";
    case "not_met":
      return "未达验收";
    case "rejected":
      return "已拒绝";
    default:
      return acceptanceStatus.trim() || "—";
  }
}

/** `evolution_log.type` → localized label; falls back to raw `eventType`. */
export function evolutionLogEventTypeLabel(eventType: string, locale: Locale): string {
  const key = `evolution.log.type.${eventType}`;
  const out = translate(key, undefined, locale);
  return out === key ? eventType : out;
}

/** One line for target_id (e.g. proposal id or "run"). */
export function evolutionLogTargetLine(
  targetId: string | null | undefined,
  locale: Locale
): string | null {
  if (targetId == null || !String(targetId).trim()) return null;
  return translate("evolution.log.targetWithId", { id: String(targetId).trim() }, locale);
}

/**
 * Map known English `reason` strings from the Rust engine to localized copy.
 * Unknown text is returned unchanged.
 */
export function evolutionLogReasonForDisplay(
  reason: string | null | undefined,
  locale: Locale
): string | undefined {
  if (reason == null || !reason.trim()) return undefined;
  const r = reason.trim();

  if (
    r ===
    "NoScope: no proposals built (thresholds, cooldown, evolution mode, or daily cap)"
  ) {
    return translate("evolution.log.reason.noScopeNoProposals", undefined, locale);
  }
  if (r === "SkippedBusy: another evolution run held the global mutex") {
    return translate("evolution.log.reason.skippedBusy", undefined, locale);
  }
  if (r === "NoScope: evolution coordinator mutex busy; retry later") {
    return translate("evolution.log.reason.noScopeCoordinatorBusy", undefined, locale);
  }
  if (r.startsWith("Error: ")) {
    const detail = r.slice("Error: ".length);
    return translate("evolution.log.reason.errorWithDetail", { detail }, locale);
  }

  const queued = r.match(
    /^Proposal\s+(\S+)\s+\((\w+)\)\s+queued;\s*waiting execution gate$/
  );
  if (queued) {
    return translate(
      "evolution.log.reason.proposalQueued",
      { id: queued[1], source: queued[2] },
      locale
    );
  }
  const denied = r.match(
    /^Proposal\s+(\S+)\s+\((\w+)\)\s+denied by policy runtime$/
  );
  if (denied) {
    return translate(
      "evolution.log.reason.proposalDenied",
      { id: denied[1], source: denied[2] },
      locale
    );
  }

  return r;
}
