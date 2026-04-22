import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MarkdownContent } from "./shared/MarkdownContent";
import { PromptDiffView } from "./PromptDiffView";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useUiToastStore } from "../stores/useUiToastStore";
import {
  evolutionAcceptanceStatusLabel,
  evolutionBacklogNoteForDisplay,
  evolutionBacklogStatusLabel,
  evolutionLogEventTypeLabel,
  evolutionLogReasonForDisplay,
  evolutionLogTargetLine,
  isEvolutionRunCompletedNoFileDeltaReason,
  prependNoMaterialHelpIfNeeded,
} from "../utils/evolutionDisplay";
import {
  openDetailWindow,
  parseDetailWorkspaceFromUrl,
  type EvolutionDetailTab,
} from "../utils/detailWindow";
import {
  type StructuredLlmInvokeResult,
  unwrapStructuredLlmInvokeResult,
} from "../utils/llmScenarioFallback";
import { runWithScenarioFallbackNotified } from "../utils/llmScenarioFallbackToast";
import { useI18n } from "../i18n";
import type { Locale } from "../i18n/translate";

export interface EvolutionLogEntryDto {
  ts: string;
  event_type: string;
  target_id: string | null;
  reason: string | null;
  /** evolution_log.version，常为本轮 txn */
  txn_id?: string | null;
}

/** Mirrors `skilllite_evolution::GrowthDueDiagnostics` JSON. */
export interface GrowthDueDiagnostics {
  min_run_gap_secs: number;
  min_run_gap_blocked: boolean;
  seconds_since_last_material_run: number | null;
  weighted_signal_sum: number;
  weighted_trigger_min: number;
  signal_window: number;
  raw_unprocessed_decisions: number;
  raw_unprocessed_threshold: number;
  weighted_arm_met: boolean;
  raw_arm_met: boolean;
  arm_signal: boolean;
  sweep_interval_secs: number;
  arm_sweep: boolean;
  interval_secs: number;
  periodic_anchor_unix: number | null;
  periodic_elapsed_secs: number;
  arm_periodic: boolean;
  growth_tick_would_be_due: boolean;
  periodic_only: boolean;
}

/** Mirrors `skilllite_evolution::PassiveScheduleDiagnostics` JSON. */
export interface PassiveScheduleDiagnostics {
  evolution_disabled: boolean;
  daily_runs_today: number;
  daily_cap: number;
  daily_cap_blocked: boolean;
  hours_since_last_material_run: number | null;
  cooldown_hours: number;
  cooldown_blocked: boolean;
  passive_cooldown_uses_log_types: string;
  meaningful: number;
  failures: number;
  replans: number;
  repeated_patterns: number;
  recent_days: number;
  recent_decision_sample_limit: number;
  arm_prompts: boolean;
  arm_memory: boolean;
  arm_skills: boolean;
  skills_skill_action: string | null;
}

export interface EvolutionStatusPayload {
  mode_key: string;
  mode_label: string;
  interval_secs: number;
  decision_threshold: number;
  /** Weighted sum over latest meaningful unprocessed decisions (A9). */
  weighted_signal_sum: number;
  weighted_trigger_min: number;
  signal_window: number;
  evo_profile_key: string;
  evo_cooldown_hours: number;
  unprocessed_decisions: number;
  last_run_ts: string | null;
  last_material_run_ts?: string | null;
  judgement_label: string | null;
  judgement_reason: string | null;
  recent_events: EvolutionLogEntryDto[];
  pending_skill_count: number;
  a9?: GrowthDueDiagnostics | null;
  passive?: PassiveScheduleDiagnostics | null;
  would_have_evolution_proposals?: boolean;
  empty_proposals_reason?: string | null;
  db_error: string | null;
}

export interface PendingSkillDto {
  name: string;
  needs_review: boolean;
  preview: string;
}

export interface EvolutionFileDiffDto {
  filename: string;
  evolved: boolean;
  content: string;
  original_content: string | null;
}

/** 曾在 changelog 中出现且相对最早快照有文本差异：用于绿框与「本轮已进化」徽标 */
export function evolutionPromptHasNetChange(d: EvolutionFileDiffDto): boolean {
  if (!d.evolved) return false;
  if (d.original_content == null) return false;
  return d.original_content !== d.content;
}

export interface EvolutionSnapshotTxnDto {
  txn_id: string;
  modified_unix: number;
}

/** Matches `skilllite_bridge::PROMPT_VERSION_CURRENT` */
const PROMPT_VERSION_CURRENT = "__current__";

function EvolutionBrandTitle({ className }: { className?: string }) {
  const { t } = useI18n();
  return (
    <span className={className}>
      {t("evolution.brand.lead")}
      <span className="text-accent font-bold">{t("evolution.brand.accent")}</span>
    </span>
  );
}

/** 与 `EVOLUTION_PROMPT_DIFF_FILENAMES` 一致，可安全写入 chat prompts */
const CHAT_PROMPT_EDIT_FILENAMES = [
  "planning.md",
  "execution.md",
  "system.md",
  "examples.md",
  "rules.json",
  "examples.json",
] as const;

function PromptChatFileEditorRow({ filename }: { filename: string }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const [baseline, setBaseline] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const saveOkText = t("evolution.manualEdit.saveOk");

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setNotice(null);
    void (async () => {
      try {
        const text = await invoke<string>("skilllite_read_prompt_version_content", {
          filename,
          versionRef: PROMPT_VERSION_CURRENT,
        });
        if (cancelled) return;
        setDraft(text);
        setBaseline(text);
      } catch (e) {
        if (!cancelled) setNotice(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, filename]);

  const dirty = draft !== baseline;
  const save = async () => {
    setSaving(true);
    setNotice(null);
    try {
      await invoke("skilllite_write_chat_prompt_file", { filename, content: draft });
      setBaseline(draft);
      setNotice(saveOkText);
    } catch (e) {
      setNotice(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="rounded border border-border/50 dark:border-border-dark/50 overflow-hidden">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center justify-between px-2 py-2 text-left text-xs font-mono bg-paper/35 dark:bg-paper-dark/60 hover:bg-ink/5 dark:hover:bg-paper/55/5"
      >
        <span>{filename}</span>
        <span className="text-ink-mute dark:text-ink-dark-mute text-[10px]">{open ? "▼" : "▶"}</span>
      </button>
      {open ? (
        <div className="p-2 border-t border-border/40 dark:border-border-dark/40 space-y-2">
          {loading ? (
            <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">{t("common.loading")}</p>
          ) : (
            <>
              <textarea
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                rows={12}
                className="w-full rounded border border-border dark:border-border-dark bg-paper/55 dark:bg-paper-dark px-2 py-1.5 text-[11px] font-mono text-ink dark:text-ink-dark"
                spellCheck={false}
              />
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  disabled={!dirty || saving}
                  onClick={() => void save()}
                  className="px-2 py-1 rounded bg-accent text-white text-[11px] font-medium disabled:opacity-50"
                >
                  {saving ? "…" : t("evolution.manualEdit.save")}
                </button>
                <button
                  type="button"
                  disabled={!dirty}
                  onClick={() => setDraft(baseline)}
                  className="px-2 py-1 rounded border border-border dark:border-border-dark text-[11px] text-ink dark:text-ink-dark disabled:opacity-50"
                >
                  {t("evolution.manualEdit.revert")}
                </button>
              </div>
            </>
          )}
          {notice ? (
            <p
              className={
                notice === saveOkText
                  ? "text-[11px] text-emerald-600 dark:text-emerald-400"
                  : "text-[11px] text-red-600 dark:text-red-400 break-words"
              }
            >
              {notice}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

function EvolutionPromptManualEditSection({
  workspace,
  defaultOpen,
}: {
  workspace: string;
  defaultOpen: boolean;
}) {
  const { t } = useI18n();
  const ws = workspace.trim() || ".";
  const [sectionOpen, setSectionOpen] = useState(defaultOpen);
  useEffect(() => {
    setSectionOpen(defaultOpen);
  }, [defaultOpen]);
  return (
    <details
      className="rounded-lg border border-border/60 dark:border-border-dark/60 bg-gray-50/50 dark:bg-surface-dark/40 px-3 py-2 mt-3"
      open={sectionOpen}
      onToggle={(e) => setSectionOpen(e.currentTarget.open)}
    >
      <summary className="cursor-pointer text-sm font-medium text-ink dark:text-ink-dark select-none list-none [&::-webkit-details-marker]:hidden flex items-center gap-1.5">
        <span
          className={`text-[10px] opacity-70 transition-transform inline-block ${sectionOpen ? "rotate-90" : ""}`}
          aria-hidden
        >
          ▸
        </span>
        {t("evolution.manualEdit.summary")}
      </summary>
      <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute mt-2 leading-relaxed">
        {t("evolution.manualEdit.intro")}
      </p>
      <button
        type="button"
        className="mt-2 text-xs text-accent hover:underline font-medium"
        onClick={() =>
          void invoke("skilllite_open_directory", { module: "prompts", workspace: ws }).catch(
            (e) =>
              useUiToastStore.getState().show(e instanceof Error ? e.message : String(e), "error")
          )
        }
      >
        {t("evolution.manualEdit.openPromptsDir")}
      </button>
      <div className="mt-3 space-y-2">
        {CHAT_PROMPT_EDIT_FILENAMES.map((fn) => (
          <PromptChatFileEditorRow key={fn} filename={fn} />
        ))}
      </div>
    </details>
  );
}

function formatSnapshotTimeLabel(modifiedUnix: number, locale: Locale): string {
  if (!modifiedUnix || modifiedUnix <= 0) return "";
  try {
    const d = new Date(modifiedUnix * 1000);
    if (Number.isNaN(d.getTime())) return "";
    return locale === "en"
      ? d.toLocaleString("en-US", { dateStyle: "short", timeStyle: "short" })
      : d.toLocaleString("zh-CN", { dateStyle: "short", timeStyle: "short" });
  } catch {
    return "";
  }
}

const PROMPT_VERSION_SELECT_CLS =
  "w-full min-w-0 appearance-none rounded-lg border border-border dark:border-border-dark " +
  "bg-paper/55 dark:bg-paper-dark shadow-sm pl-3 pr-9 py-2 text-[11px] font-mono " +
  "text-ink dark:text-ink-dark transition-[border-color,box-shadow] duration-150 " +
  "hover:border-accent/40 dark:hover:border-accent/35 " +
  "focus:outline-none focus:ring-2 focus:ring-accent/25 focus:border-accent/50 " +
  "disabled:cursor-not-allowed disabled:opacity-60";

function PromptVersionSelectRow({
  label,
  value,
  onChange,
  ariaLabel,
  txnOptions,
  currentOptionLabel,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  ariaLabel: string;
  txnOptions: ReactNode;
  currentOptionLabel: string;
}) {
  return (
    <label className="flex min-w-0 flex-1 flex-col gap-1">
      <span className="text-[10px] font-medium uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
        {label}
      </span>
      <div className="relative min-w-0 group">
        <select
          className={PROMPT_VERSION_SELECT_CLS}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-label={ariaLabel}
        >
          <option value={PROMPT_VERSION_CURRENT}>{currentOptionLabel}</option>
          {txnOptions}
        </select>
        <span
          className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-ink-mute dark:text-ink-dark-mute opacity-55 transition-opacity group-focus-within:opacity-100 group-focus-within:text-accent"
          aria-hidden
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M6 9l6 6 6-6" />
          </svg>
        </span>
      </div>
    </label>
  );
}

function EvolutionPromptVersionCompare({
  filename,
  focusTxn = null,
  txns,
  txnsLoading,
  txnsErr,
}: {
  filename: string;
  /** 若该 txn 在快照中存在，则左侧默认选中它（相对当前） */
  focusTxn?: string | null;
  /** 由父级批量拉取，避免每文件一次 invoke（Strict Mode 下易卡住「加载快照列表」） */
  txns: EvolutionSnapshotTxnDto[];
  txnsLoading: boolean;
  txnsErr: string | null;
}) {
  const { t, locale } = useI18n();
  const localeResolved: Locale = locale === "en" ? "en" : "zh";
  const [leftRef, setLeftRef] = useState(PROMPT_VERSION_CURRENT);
  const [rightRef, setRightRef] = useState(PROMPT_VERSION_CURRENT);
  const [leftText, setLeftText] = useState("");
  const [rightText, setRightText] = useState("");
  const [contentErr, setContentErr] = useState<string | null>(null);
  const [loadingContent, setLoadingContent] = useState(false);
  const contentFetchGen = useRef(0);

  useEffect(() => {
    if (txnsLoading) return;
    const txnTrim = focusTxn?.trim() ?? "";
    const hasFocus =
      txnTrim.length > 0 &&
      txnTrim !== PROMPT_VERSION_CURRENT &&
      txns.some((x) => x.txn_id === txnTrim);
    const oldest =
      txns.length > 0 ? txns[txns.length - 1].txn_id : PROMPT_VERSION_CURRENT;
    setLeftRef(hasFocus ? txnTrim : oldest);
    setRightRef(PROMPT_VERSION_CURRENT);
  }, [txns, txnsLoading, focusTxn]);

  useEffect(() => {
    const id = ++contentFetchGen.current;
    setLoadingContent(true);
    setContentErr(null);
    void (async () => {
      try {
        const [l, r] = await Promise.all([
          invoke<string>("skilllite_read_prompt_version_content", {
            filename,
            versionRef: leftRef,
          }),
          invoke<string>("skilllite_read_prompt_version_content", {
            filename,
            versionRef: rightRef,
          }),
        ]);
        if (contentFetchGen.current !== id) return;
        setLeftText(l);
        setRightText(r);
      } catch (e) {
        if (contentFetchGen.current !== id) return;
        setContentErr(e instanceof Error ? e.message : String(e));
        setLeftText("");
        setRightText("");
      } finally {
        if (contentFetchGen.current === id) setLoadingContent(false);
      }
    })();
  }, [filename, leftRef, rightRef]);

  const txnOptions = txns.map((x) => {
    const time = formatSnapshotTimeLabel(x.modified_unix, localeResolved);
    const label =
      time.length > 0
        ? t("evolution.diff.txnOption", { id: x.txn_id, time })
        : x.txn_id;
    return (
      <option key={x.txn_id} value={x.txn_id}>
        {label}
      </option>
    );
  });

  return (
    <div className="space-y-2 px-3 pb-3 pt-1">
      {txnsLoading ? (
        <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">{t("evolution.diff.loadingVersions")}</p>
      ) : txnsErr ? (
        <p className="text-[11px] text-red-600 dark:text-red-400 break-words">
          {t("evolution.diff.loadVersionsError")}: {txnsErr}
        </p>
      ) : (
        <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
          <PromptVersionSelectRow
            label={t("evolution.diff.versionLeft")}
            value={leftRef}
            onChange={setLeftRef}
            ariaLabel={t("evolution.diff.versionLeft")}
            currentOptionLabel={t("evolution.diff.currentWorkspace")}
            txnOptions={txnOptions}
          />
          <PromptVersionSelectRow
            label={t("evolution.diff.versionRight")}
            value={rightRef}
            onChange={setRightRef}
            ariaLabel={t("evolution.diff.versionRight")}
            currentOptionLabel={t("evolution.diff.currentWorkspace")}
            txnOptions={txnOptions}
          />
        </div>
      )}
      {contentErr && (
        <p className="text-[11px] text-red-600 dark:text-red-400 break-words">
          {t("evolution.diff.loadContentError")}: {contentErr}
        </p>
      )}
      {loadingContent && !contentErr ? (
        <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">{t("evolution.diff.loadingContent")}</p>
      ) : null}
      {!loadingContent && !contentErr && !txnsLoading && !txnsErr ? (
        <PromptDiffView original={leftText} current={rightText} />
      ) : null}
    </div>
  );
}

export interface EvolutionBacklogRowDto {
  proposal_id: string;
  source: string;
  risk_level: string;
  status: string;
  acceptance_status: string;
  roi_score: number;
  updated_at: string;
  note: string;
}

function formatInterval(secs: number): string {
  if (secs >= 3600 && secs % 3600 === 0) {
    return `每 ${secs / 3600} 小时`;
  }
  if (secs % 60 === 0) {
    return `每 ${secs / 60} 分钟`;
  }
  return `每 ${secs} 秒`;
}

/** Compact duration for diagnostics (locale-neutral numbers). */
function formatDurationSecs(secs: number): string {
  if (!Number.isFinite(secs) || secs < 0) return "—";
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return s > 0 ? `${m}m${s}s` : `${m}m`;
  }
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return m > 0 ? `${h}h${m}m` : `${h}h`;
}

function dxMark(on: boolean): string {
  return on ? "✓" : "×";
}

function passiveSkillExtra(p: PassiveScheduleDiagnostics, t: (k: string) => string): string {
  if (!p.arm_skills) return "";
  if (p.skills_skill_action === "generate") {
    return t("evolution.diagnostics.skillExtraGenerate");
  }
  if (p.skills_skill_action === "refine") {
    return t("evolution.diagnostics.skillExtraRefine");
  }
  return "";
}

/** One i18n key for a beginner-facing sentence under 「实际结果」. */
function evolutionBeginnerInsightKey(s: EvolutionStatusPayload): string {
  if (s.mode_key === "disabled") {
    return "evolution.diagnostics.insight.disabled";
  }
  const would = s.would_have_evolution_proposals === true;
  const a9due = s.a9?.growth_tick_would_be_due === true;
  if (would && a9due) {
    return "evolution.diagnostics.insight.wouldRunSoon";
  }
  if (would && !a9due) {
    return "evolution.diagnostics.insight.wouldButA9Idle";
  }
  const r = (s.empty_proposals_reason ?? "").trim();
  if (r.includes("daily evolution cap")) {
    return "evolution.diagnostics.insight.dailyCap";
  }
  if (r.includes("cooldown active")) {
    return "evolution.diagnostics.insight.cooldown";
  }
  if (r.includes("passive and active scopes idle")) {
    return "evolution.diagnostics.insight.idle";
  }
  if (r.includes("evolution disabled")) {
    return "evolution.diagnostics.insight.disabled";
  }
  if (r.length > 0) {
    return "evolution.diagnostics.insight.generic";
  }
  return "evolution.diagnostics.insight.unknown";
}

function formatTs(ts: string): string {
  try {
    const d = new Date(ts);
    if (isNaN(d.getTime())) {
      return ts.length >= 16 ? ts.slice(0, 16).replace("T", " ") : ts;
    }
    const y = d.getFullYear();
    const mo = String(d.getMonth() + 1).padStart(2, "0");
    const da = String(d.getDate()).padStart(2, "0");
    const h = String(d.getHours()).padStart(2, "0");
    const mi = String(d.getMinutes()).padStart(2, "0");
    return `${y}-${mo}-${da} ${h}:${mi}`;
  } catch {
    return ts.length >= 16 ? ts.slice(0, 16).replace("T", " ") : ts;
  }
}

function reasonMentionsMemoryKnowledge(reason: string | null | undefined): boolean {
  if (!reason) return false;
  return /memory knowledge|knowledge update/i.test(reason);
}

function reasonMentionsPromptChanges(reason: string | null | undefined): boolean {
  if (!reason) return false;
  return /\d+\s+prompt changes|prompt changes\b/i.test(reason);
}

function reasonMentionsSkillChanges(reason: string | null | undefined): boolean {
  if (!reason) return false;
  return /\d+\s+skill changes|skill changes\b/i.test(reason);
}

/**
 * 进化对比 Tab 只覆盖 chat/prompts；记忆写入 memory/evolution/、技能在待确认列表。
 * 若日志里仅有 memory/skill 而无 prompt changes，点击行应直达对应入口，避免「可点却无对比」。
 */
function evolutionRunRowPrimaryAction(reason: string | null | undefined): "prompt_diff" | "memory" | "review" {
  const prompt = reasonMentionsPromptChanges(reason);
  if (prompt) return "prompt_diff";
  if (reasonMentionsMemoryKnowledge(reason)) return "memory";
  if (reasonMentionsSkillChanges(reason)) return "review";
  return "prompt_diff";
}

function eventIcon(eventType: string): string {
  switch (eventType) {
    case "rule_added":
      return "✓";
    case "example_added":
      return "📖";
    case "skill_generated":
      return "✨";
    case "skill_pending":
      return "🆕";
    case "skill_refined":
      return "🔧";
    case "skill_confirmed":
      return "✅";
    case "evolution_judgement":
      return "🧭";
    case "evolution_run":
    case "evolution_run_noop":
      return "◆";
    case "auto_rollback":
      return "⚠";
    default:
      if (eventType.includes("rolled_back")) return "↩";
      if (eventType.includes("retired")) return "🗑";
      return "·";
  }
}

function useEvolutionStatus() {
  const { settings } = useSettingsStore();
  /** 详情独立窗口通过 URL ?w= 传入主窗口工作区，避免 WebView 间 localStorage 不同步 */
  const workspaceFromUrl = useMemo(() => parseDetailWorkspaceFromUrl(), []);
  const workspace = workspaceFromUrl ?? (settings.workspace || ".");
  const [status, setStatus] = useState<EvolutionStatusPayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const { result: s } = await runWithScenarioFallbackNotified<EvolutionStatusPayload>(
        settings,
        "evolution",
        (config) =>
          invoke<StructuredLlmInvokeResult<EvolutionStatusPayload>>(
            "skilllite_load_evolution_status",
            {
              workspace,
              config,
            }
          ).then(unwrapStructuredLlmInvokeResult)
      );
      setStatus(s);
    } catch (e) {
      setError(String(e));
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, [workspace, settings]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { status, loading, error, refresh, workspace };
}

/** 右侧面板摘要 */
export function EvolutionStatusSummary({
  onOpenDetail,
  className,
  metricsClassName,
}: {
  onOpenDetail: () => void;
  /** 外层 section，用于右栏占满剩余高度等布局（默认仅 `mb-4`）。 */
  className?: string;
  /** 指标卡片额外 class（如 `flex-1 min-h-*`）。 */
  metricsClassName?: string;
}) {
  const { t } = useI18n();
  const { status, loading, error, refresh, workspace } = useEvolutionStatus();
  const sectionCls = `min-w-0 flex flex-col ${className ?? "mb-4"}`;

  if (loading && !status) {
    return (
      <section className={sectionCls}>
        <div className="flex items-center justify-between mb-2">
          <EvolutionBrandTitle className="font-medium text-ink dark:text-ink-dark" />
        </div>
        <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
      </section>
    );
  }

  if (error && !status) {
    return (
      <section className={sectionCls}>
        <div className="flex items-center justify-between mb-2">
          <EvolutionBrandTitle className="font-medium text-ink dark:text-ink-dark" />
          <button
            type="button"
            onClick={() => void refresh()}
            className="text-xs text-accent hover:underline"
          >
            重试
          </button>
        </div>
        <p className="break-words text-xs text-red-600 dark:text-red-400">{error}</p>
      </section>
    );
  }

  const s = status!;
  const pk = s.evo_profile_key ?? "default";
  const profileLabel =
    pk === "demo"
      ? t("evolution.profile.demo")
      : pk === "conservative"
        ? t("evolution.profile.conservative")
        : t("evolution.profile.default");

  const schedParams = {
    interval: formatInterval(s.interval_secs),
    w: s.weighted_signal_sum ?? 0,
    wmin: s.weighted_trigger_min ?? 3,
    win: s.signal_window ?? 10,
    d: s.unprocessed_decisions,
    thr: s.decision_threshold,
  };
  const scheduleOneLine =
    s.mode_key === "disabled"
      ? t("evolution.summary.scheduleDisabled")
      : t("evolution.summary.scheduleCompact", schedParams);
  const scheduleTitleLong =
    s.mode_key === "disabled"
      ? t("evolution.summary.scheduleDisabled")
      : t("evolution.summary.scheduleTitleLong", schedParams);

  return (
    <section className={sectionCls}>
      <div className="mb-2 flex min-w-0 shrink-0 items-center justify-between gap-2">
        <button
          type="button"
          onClick={onOpenDetail}
          className="group min-w-0 flex-1 text-left text-ink dark:text-ink-dark"
        >
          <div className="font-medium leading-tight group-hover:text-accent dark:group-hover:text-accent">
            <EvolutionBrandTitle />
          </div>
          <div className="mt-0.5 inline-flex items-center gap-1 text-[11px] font-normal text-ink-mute transition-colors group-hover:text-accent dark:text-ink-dark-mute dark:group-hover:text-accent">
            <span>{t("evolution.summary.openDetail")}</span>
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="opacity-70"
              aria-hidden
            >
              <path d="M9 18l6-6-6-6" />
            </svg>
          </div>
        </button>
        <button
          type="button"
          onClick={() => void refresh()}
          disabled={loading}
          className="shrink-0 rounded-lg p-1.5 text-ink-mute hover:bg-ink/5 hover:text-ink disabled:opacity-50 dark:text-ink-dark-mute dark:hover:bg-paper-dark/25 dark:hover:text-ink-dark"
          title={t("evolution.summary.refreshTitle")}
          aria-label={t("evolution.summary.refreshEvolutionAria")}
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={loading ? "animate-spin" : ""}
          >
            <path d="M21 2v6h-6" />
            <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
            <path d="M3 22v-6h6" />
            <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
          </svg>
        </button>
      </div>

      <div
        className={`flex max-w-full min-w-0 cursor-pointer flex-col gap-2.5 rounded-xl border-l-[3px] border-l-accent/50 bg-gradient-to-b from-paper/55 via-paper/25 to-surface/90 py-3 pl-2.5 pr-3 text-xs text-ink shadow-md shadow-black/[0.06] ring-1 ring-border/25 dark:border-l-accent/45 dark:from-paper-dark/35 dark:via-paper-dark/18 dark:to-surface-dark/50 dark:text-ink-dark dark:shadow-black/30 dark:ring-border-dark/30 ${metricsClassName ?? ""}`}
        onClick={onOpenDetail}
        role="button"
        onKeyDown={(e) => e.key === "Enter" && onOpenDetail()}
        tabIndex={0}
      >
        {s.db_error ? (
          <p className="break-words rounded-md bg-amber-500/10 px-2 py-1.5 text-[11px] leading-snug text-amber-900 dark:bg-amber-500/15 dark:text-amber-100">
            {s.db_error}
          </p>
        ) : null}

        <div className="rounded-lg bg-ink/[0.04] px-2.5 py-2 dark:bg-white/[0.06]">
          <p className="truncate text-sm font-semibold tracking-tight text-ink dark:text-ink-dark">{s.mode_label}</p>
          <p
            className="mt-1 line-clamp-2 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute"
            title={s.mode_key === "disabled" ? scheduleTitleLong : `${scheduleOneLine}\n\n${scheduleTitleLong}`}
          >
            {scheduleOneLine}
          </p>
        </div>

        <div className="rounded-lg bg-paper/55 px-2.5 py-2 shadow-[inset_0_1px_0_rgba(0,0,0,0.04)] dark:bg-black/25 dark:shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]">
          <p className="mb-1.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-ink-mute dark:text-ink-dark-mute">
            {t("evolution.summary.snapshotLabel")}
          </p>
          <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-[11px] leading-snug">
            <dt className="shrink-0 text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.profile")}</dt>
            <dd className="min-w-0 truncate font-medium text-ink dark:text-ink-dark">{profileLabel}</dd>
            <dt className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.cooldown")}</dt>
            <dd className="font-mono tabular-nums text-ink dark:text-ink-dark">
              {s.evo_cooldown_hours != null && Number.isFinite(s.evo_cooldown_hours) ? `${s.evo_cooldown_hours} h` : "—"}
            </dd>
            <dt className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.unprocessedShort")}</dt>
            <dd className="font-mono tabular-nums text-ink dark:text-ink-dark">{s.unprocessed_decisions}</dd>
            <dt className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.lastRunShort")}</dt>
            <dd className="truncate font-mono tabular-nums text-ink dark:text-ink-dark">
              {s.last_run_ts ? formatTs(s.last_run_ts) : "—"}
            </dd>
          </dl>
        </div>

        {s.judgement_label ? (
          <div className="rounded-lg bg-gradient-to-br from-accent-light/70 to-accent-light/25 px-2.5 py-2 shadow-sm dark:from-accent-light-dark/35 dark:to-accent-light-dark/10">
            <p className="text-[11px] font-semibold text-ink dark:text-ink-dark">
              <span className="font-normal text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.judgementHeading")} · </span>
              {s.judgement_label}
            </p>
            {s.judgement_reason ? (
              <p className="mt-1 line-clamp-2 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
                {s.judgement_reason}
              </p>
            ) : null}
          </div>
        ) : null}

        <div className="flex min-w-0 items-baseline justify-between gap-2 rounded-md bg-ink/[0.035] px-2 py-1.5 text-[11px] dark:bg-white/[0.05]">
          <span className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.pendingShort")}</span>
          <span
            className={`font-mono tabular-nums ${s.pending_skill_count > 0 ? "font-semibold text-accent" : "font-medium text-ink dark:text-ink-dark"}`}
          >
            {s.pending_skill_count}
          </span>
        </div>

        <p
          className="min-w-0 truncate rounded-md bg-ink/[0.03] px-1.5 py-1 font-mono text-[10px] text-ink-mute dark:bg-white/[0.04] dark:text-ink-dark-mute"
          title={workspace}
        >
          {t("evolution.summary.workspace")}: {workspace}
        </p>
      </div>
    </section>
  );
}

function PendingSkillReviewCard({
  skill,
  workspace,
  onChanged,
}: {
  skill: PendingSkillDto;
  workspace: string;
  onChanged: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [fullMd, setFullMd] = useState<string | null>(null);
  const [loadingFull, setLoadingFull] = useState(false);
  const [acting, setActing] = useState<"confirm" | "reject" | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const loadFull = async () => {
    if (fullMd !== null) {
      setExpanded(!expanded);
      return;
    }
    setLoadingFull(true);
    try {
      const md = await invoke<string>("skilllite_read_pending_skill_md", {
        workspace,
        skillName: skill.name,
      });
      setFullMd(md);
      setExpanded(true);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setLoadingFull(false);
    }
  };

  const confirm = async () => {
    setActing("confirm");
    setMsg(null);
    try {
      await invoke("skilllite_confirm_pending_skill", { workspace, skillName: skill.name });
      setMsg("已加入已确认技能");
      onChanged();
    } catch (e) {
      setMsg(String(e));
    } finally {
      setActing(null);
    }
  };

  const reject = async () => {
    setActing("reject");
    setMsg(null);
    try {
      await invoke("skilllite_reject_pending_skill", { workspace, skillName: skill.name });
      setMsg("已拒绝并删除");
      onChanged();
    } catch (e) {
      setMsg(String(e));
    } finally {
      setActing(null);
    }
  };

  const displayMd = expanded && fullMd !== null ? fullMd : skill.preview;
  const showShort = !expanded || fullMd === null;

  return (
    <div className="overflow-hidden rounded-2xl border border-border/70 bg-paper/50 shadow-sm dark:border-border-dark/55 dark:bg-paper-dark/35">
      <div className="flex flex-wrap items-start justify-between gap-2 border-b border-border/50 bg-surface dark:bg-paper-dark/15 px-4 py-3 dark:border-border-dark dark:bg-black/25">
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
          <span className="truncate text-sm font-semibold text-ink dark:text-ink-dark">{skill.name}</span>
          {skill.needs_review && (
            <span className="shrink-0 rounded-md bg-amber-100 px-2 py-0.5 text-[10px] font-semibold text-amber-900 dark:bg-amber-900/40 dark:text-amber-100">
              建议人工过目
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={() => void loadFull()}
          className="shrink-0 rounded-lg border border-border bg-paper/55 px-2.5 py-1 text-xs font-medium text-accent shadow-sm hover:border-accent/30 disabled:opacity-50 dark:border-border-dark dark:bg-paper-dark/30 dark:hover:border-accent/40"
          disabled={loadingFull}
        >
          {loadingFull ? "加载全文…" : expanded ? "收起全文" : "查看全文"}
        </button>
      </div>
      <div
        className={`prose prose-sm max-w-none dark:prose-invert [&_pre]:text-xs [&_code]:text-xs overflow-y-auto border-border/50 bg-paper/55 px-3 py-3 dark:border-border-dark dark:bg-paper-dark/25 ${
          showShort ? "max-h-40" : "max-h-[min(65vh,480px)]"
        }`}
      >
        {displayMd ? (
          <MarkdownContent content={displayMd} />
        ) : (
          <p className="text-xs text-ink-mute">（无 SKILL.md 内容）</p>
        )}
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border/50 bg-surface/85 px-4 py-3 dark:border-border-dark dark:bg-black/20">
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void confirm()}
            disabled={acting !== null}
            className="rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-white shadow-sm hover:opacity-90 disabled:opacity-50"
          >
            {acting === "confirm" ? "处理中…" : "确认加入"}
          </button>
          <button
            type="button"
            onClick={() => void reject()}
            disabled={acting !== null}
            className="rounded-lg border border-border bg-paper/55 px-3 py-1.5 text-xs font-medium text-ink shadow-sm hover:bg-paper/40 disabled:opacity-50 dark:border-border-dark dark:bg-paper-dark/30 dark:text-ink-dark dark:hover:bg-paper-dark/35"
          >
            {acting === "reject" ? "处理中…" : "拒绝"}
          </button>
        </div>
        {msg ? (
          <p className="max-w-full min-w-0 flex-1 text-right text-xs text-ink-mute dark:text-ink-dark-mute">{msg}</p>
        ) : null}
      </div>
    </div>
  );
}

function EvolutionArmChip({ label, active }: { label: string; active: boolean }) {
  return (
    <span
      className={`inline-flex min-h-[1.375rem] items-center rounded-md px-2 py-0.5 text-[10px] font-semibold ${
        active
          ? "bg-emerald-500/15 text-emerald-900 ring-1 ring-emerald-500/20 dark:bg-emerald-400/12 dark:text-emerald-100 dark:ring-emerald-400/25"
          : "bg-surface text-ink-mute ring-1 ring-border/60 dark:bg-paper-dark/25 dark:text-ink-dark-mute dark:ring-border-dark/60"
      }`}
    >
      {label}
    </span>
  );
}

/** 独立详情窗口：分 tab（运行 / 审核 / 变更）避免单页过长 */
export function EvolutionDetailBody({
  initialTab,
  initialFocusTxn,
}: {
  /** 自 `#detail/evolution?tab=...` 传入，用于从聊天等入口直达某 Tab */
  initialTab?: EvolutionDetailTab;
  /** 自 `#detail/evolution?txn=...` 传入，变更对比默认左侧选中该快照 */
  initialFocusTxn?: string | null;
} = {}) {
  const { t, locale: localeRaw } = useI18n();
  const locale: Locale = localeRaw === "en" ? "en" : "zh";
  const { settings } = useSettingsStore();
  const { status, loading, error, refresh, workspace } = useEvolutionStatus();
  const [detailTab, setDetailTab] = useState<EvolutionDetailTab>(() => initialTab ?? "run");
  const [compareFocusTxn, setCompareFocusTxn] = useState<string | null>(() => {
    const x = initialFocusTxn?.trim();
    return x && x.length > 0 ? x : null;
  });
  const [pending, setPending] = useState<PendingSkillDto[]>([]);
  const [pendingLoading, setPendingLoading] = useState(true);
  const [diffs, setDiffs] = useState<EvolutionFileDiffDto[]>([]);
  const [diffsLoading, setDiffsLoading] = useState(true);
  const evolvedPromptWithChangeCount = useMemo(
    () => diffs.filter(evolutionPromptHasNetChange).length,
    [diffs]
  );
  const sortedDiffs = useMemo(() => {
    const rank = (d: EvolutionFileDiffDto) => {
      if (evolutionPromptHasNetChange(d)) return 0;
      if (d.evolved) return 1;
      return 2;
    };
    return [...diffs].sort((a, b) => {
      const ra = rank(a);
      const rb = rank(b);
      if (ra !== rb) return ra - rb;
      return a.filename.localeCompare(b.filename);
    });
  }, [diffs]);
  const snapFetchGenRef = useRef(0);
  const [snapshotsByFile, setSnapshotsByFile] = useState<Record<
    string,
    EvolutionSnapshotTxnDto[]
  > | null>(null);
  const [snapshotsLoading, setSnapshotsLoading] = useState(false);
  const [snapshotsErr, setSnapshotsErr] = useState<string | null>(null);
  const [backlog, setBacklog] = useState<EvolutionBacklogRowDto[]>([]);
  const [backlogLoading, setBacklogLoading] = useState(true);
  const [triggeringProposalId, setTriggeringProposalId] = useState<string | null>(null);
  const [triggerResultByProposal, setTriggerResultByProposal] = useState<Record<string, string>>({});

  const loadPending = useCallback(async () => {
    setPendingLoading(true);
    try {
      const list = await invoke<PendingSkillDto[]>("skilllite_list_evolution_pending", { workspace });
      setPending(list);
    } catch {
      setPending([]);
    } finally {
      setPendingLoading(false);
    }
  }, [workspace]);

  const loadDiffs = useCallback(async () => {
    setDiffsLoading(true);
    try {
      const list = await invoke<EvolutionFileDiffDto[]>("skilllite_load_evolution_diffs", { workspace });
      setDiffs(list);
    } catch {
      setDiffs([]);
    } finally {
      setDiffsLoading(false);
    }
  }, [workspace]);

  const loadBacklog = useCallback(async () => {
    setBacklogLoading(true);
    try {
      const list = await invoke<EvolutionBacklogRowDto[]>("skilllite_load_evolution_backlog", {
        workspace,
        limit: 40,
      });
      setBacklog(list);
    } catch {
      setBacklog([]);
    } finally {
      setBacklogLoading(false);
    }
  }, [workspace]);

  useEffect(() => {
    void loadPending();
    void loadDiffs();
    void loadBacklog();
  }, [loadPending, loadDiffs, loadBacklog]);

  useEffect(() => {
    if (detailTab !== "changes") {
      return;
    }
    if (diffsLoading) {
      setSnapshotsLoading(true);
      return;
    }
    if (diffs.length === 0) {
      setSnapshotsByFile({});
      setSnapshotsErr(null);
      setSnapshotsLoading(false);
      return;
    }
    const id = ++snapFetchGenRef.current;
    setSnapshotsLoading(true);
    setSnapshotsErr(null);
    const filenames = diffs.map((d) => d.filename);
    void (async () => {
      try {
        const m = await invoke<Record<string, EvolutionSnapshotTxnDto[]>>(
          "skilllite_list_prompt_snapshots_batch",
          { filenames }
        );
        if (snapFetchGenRef.current !== id) return;
        setSnapshotsByFile(m);
      } catch (e) {
        if (snapFetchGenRef.current !== id) return;
        setSnapshotsErr(e instanceof Error ? e.message : String(e));
        setSnapshotsByFile(null);
      } finally {
        if (snapFetchGenRef.current === id) {
          setSnapshotsLoading(false);
        }
      }
    })();
  }, [detailTab, diffs, diffsLoading]);

  const onSkillChanged = useCallback(() => {
    void loadPending();
    void refresh();
    void loadBacklog();
  }, [loadPending, refresh, loadBacklog]);

  if (error && !status) {
    return (
      <div className="p-4">
        <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        <button type="button" className="mt-2 text-sm text-accent" onClick={() => void refresh()}>
          重试
        </button>
      </div>
    );
  }

  const s = status;

  const pendingCount = pending.length;
  const hasJudgement = Boolean(s?.judgement_label);

  const tabPill = (active: boolean, emphasize?: boolean) =>
    `flex-1 min-w-0 rounded-xl px-2.5 py-2.5 text-xs font-medium outline-none transition-all focus-visible:ring-2 focus-visible:ring-accent/40 sm:px-3 ${
      active
        ? `bg-paper/85 text-ink shadow-md ring-1 ring-border/80 backdrop-blur-sm dark:bg-paper-dark/35 dark:text-ink-dark dark:ring-border-dark/60 ${
            emphasize ? "!text-accent dark:!text-accent" : ""
          }`
        : "text-ink-mute hover:bg-paper/40 dark:text-ink-dark-mute dark:hover:bg-paper-dark/20"
    }`;

  /** 与全局 token 一致：有色页面底 + 半透明卡片，避免「一片白」 */
  const evoPage =
    "relative bg-surface dark:bg-surface-dark before:pointer-events-none before:absolute before:inset-0 before:bg-[linear-gradient(185deg,rgba(59,130,246,0.085)_0%,transparent_42%)] before:content-[''] dark:before:bg-[linear-gradient(185deg,rgba(59,130,246,0.11)_0%,transparent_38%)]";
  const evoPanel =
    "overflow-hidden rounded-2xl border border-border/70 bg-paper/40 shadow-sm backdrop-blur-md dark:border-border-dark/55 dark:bg-paper-dark/20 dark:shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]";
  const evoPanelPad = "px-4 py-4 sm:px-5 sm:py-5";
  const detailSectionTitle =
    "text-[11px] font-semibold uppercase tracking-[0.14em] text-ink-mute dark:text-ink-dark-mute";

  return (
    <div className={`mx-auto max-w-5xl min-h-0 ${evoPage}`}>
      <div className="relative z-[1] space-y-4 px-3 py-4 sm:px-6 sm:py-6">
        <nav className="sticky top-0 z-20 flex flex-wrap items-stretch gap-1.5 rounded-2xl border border-border/70 bg-paper/35 p-1 shadow-sm backdrop-blur-md dark:border-border-dark/55 dark:bg-paper-dark/25">
          <div
            role="tablist"
            aria-label={t("evolution.detail.tabListAria")}
            className="flex min-w-0 flex-1 gap-1"
          >
            <button
              type="button"
              role="tab"
              id="evolution-detail-tab-run"
              aria-controls="evolution-detail-panel-run"
              aria-selected={detailTab === "run"}
              tabIndex={detailTab === "run" ? 0 : -1}
              onClick={() => setDetailTab("run")}
              className={tabPill(detailTab === "run")}
            >
              {t("evolution.detail.tabRun")}
            </button>
            <button
              type="button"
              role="tab"
              id="evolution-detail-tab-review"
              aria-controls="evolution-detail-panel-review"
              aria-selected={detailTab === "review"}
              tabIndex={detailTab === "review" ? 0 : -1}
              onClick={() => setDetailTab("review")}
              className={tabPill(detailTab === "review")}
            >
              <span className="inline-flex items-center justify-center gap-1.5">
                {t("evolution.detail.tabReview")}
                {(pendingCount > 0 || hasJudgement) && (
                  <span className="tabular-nums rounded-full bg-accent/15 px-1.5 py-0.5 text-[10px] font-bold text-accent dark:bg-accent/25">
                    {pendingCount > 0 ? pendingCount : "!"}
                  </span>
                )}
              </span>
            </button>
            <button
              type="button"
              role="tab"
              id="evolution-detail-tab-changes"
              aria-controls="evolution-detail-panel-changes"
              aria-selected={detailTab === "changes"}
              tabIndex={detailTab === "changes" ? 0 : -1}
              onClick={() => setDetailTab("changes")}
              className={tabPill(detailTab === "changes", true)}
            >
              <span className="inline-flex items-center justify-center gap-1.5">
                {t("evolution.detail.tabChanges")}
                {evolvedPromptWithChangeCount > 0 && (
                  <span
                    className="tabular-nums rounded-full bg-emerald-500/20 px-1.5 py-0.5 text-[10px] font-bold text-emerald-800 dark:bg-emerald-400/15 dark:text-emerald-200"
                    title={t("evolution.diff.changedFilesTabHint")}
                  >
                    {t("evolution.diff.evolvedFilesCount", { n: evolvedPromptWithChangeCount })}
                  </span>
                )}
              </span>
            </button>
          </div>
          <button
            type="button"
            onClick={() => void refresh()}
            disabled={loading}
            className="shrink-0 self-center rounded-xl px-3 py-2 text-xs font-medium text-accent hover:bg-accent/[0.08] disabled:opacity-50 dark:hover:bg-accent/10"
          >
            {loading ? t("evolution.detail.refreshing") : t("evolution.detail.refreshStatus")}
          </button>
        </nav>

        {s?.db_error && (
          <p className="rounded-2xl border border-amber-200/90 bg-amber-50/95 px-4 py-3 text-sm text-amber-950 shadow-sm dark:border-amber-800/50 dark:bg-amber-950/40 dark:text-amber-100">
            {s.db_error}
          </p>
        )}

      {detailTab === "run" && (
        <div
          role="tabpanel"
          id="evolution-detail-panel-run"
          aria-labelledby="evolution-detail-tab-run"
          className="space-y-4"
        >
          {s && (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4 sm:gap-3">
              <div className="rounded-2xl border border-border/70 bg-paper/45 p-3 shadow-sm dark:border-border-dark/55 dark:bg-paper-dark/35">
                <p className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                  {t("evolution.detail.kpiMode")}
                </p>
                <p className="mt-1 truncate text-sm font-semibold text-ink dark:text-ink-dark">
                  {s.mode_label}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-paper/45 p-3 shadow-sm dark:border-border-dark/55 dark:bg-paper-dark/35">
                <p className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                  {t("evolution.detail.kpiSignal")}
                </p>
                <p className="mt-1 font-mono text-sm font-semibold tabular-nums text-ink dark:text-ink-dark">
                  Σ {s.weighted_signal_sum ?? 0}/{s.weighted_trigger_min ?? 3}
                </p>
                {s.mode_key !== "disabled" ? (
                  <div className="mt-2 h-1 overflow-hidden rounded-full bg-surface dark:bg-paper-dark/25">
                    <div
                      className="h-full rounded-full bg-gradient-to-r from-accent/80 to-accent transition-[width] duration-300"
                      style={{
                        width: `${Math.min(
                          100,
                          ((s.weighted_signal_sum ?? 0) / Math.max(1, s.weighted_trigger_min ?? 3)) * 100
                        )}%`,
                      }}
                    />
                  </div>
                ) : null}
              </div>
              <div className="rounded-2xl border border-border/70 bg-paper/45 p-3 shadow-sm dark:border-border-dark/55 dark:bg-paper-dark/35">
                <p className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                  {t("evolution.detail.kpiBacklog")}
                </p>
                <p className="mt-1 font-mono text-sm font-semibold tabular-nums text-ink dark:text-ink-dark">
                  {s.unprocessed_decisions}/{s.decision_threshold}
                </p>
                {s.mode_key !== "disabled" ? (
                  <div className="mt-2 h-1 overflow-hidden rounded-full bg-surface dark:bg-paper-dark/25">
                    <div
                      className="h-full rounded-full bg-gradient-to-r from-violet-400/90 to-violet-500/80 dark:from-violet-400/70 dark:to-violet-500/60"
                      style={{
                        width: `${Math.min(
                          100,
                          (s.unprocessed_decisions / Math.max(1, s.decision_threshold)) * 100
                        )}%`,
                      }}
                    />
                  </div>
                ) : null}
              </div>
              <div className="rounded-2xl border border-border/70 bg-paper/45 p-3 shadow-sm dark:border-border-dark/55 dark:bg-paper-dark/35">
                <p className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                  {t("evolution.detail.kpiLastRun")}
                </p>
                <p className="mt-1 font-mono text-xs font-semibold tabular-nums leading-snug text-ink dark:text-ink-dark">
                  {s.last_run_ts ? formatTs(s.last_run_ts) : "—"}
                </p>
              </div>
            </div>
          )}

          <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_min(22rem,100%)] lg:items-start lg:gap-5">
            {s && (
              <div className="min-w-0 space-y-4">
                <article className={evoPanel}>
                  <header className="border-b border-border/50 bg-surface dark:bg-paper-dark/15 px-4 py-3 dark:border-border-dark/70 dark:bg-black/20">
                    <h2 className={detailSectionTitle}>{t("evolution.detail.scheduleHeading")}</h2>
                  </header>
                  <div className={`${evoPanelPad} bg-paper/35 dark:bg-transparent`}>
                    <dl className="divide-y divide-border/50 text-[12px] leading-relaxed dark:divide-border-dark/60">
                      <div className="grid grid-cols-[6.5rem_1fr] gap-x-3 py-2.5 first:pt-0 sm:grid-cols-[8rem_1fr]">
                        <dt className="text-ink-mute dark:text-ink-dark-mute">模式</dt>
                        <dd className="min-w-0 font-medium text-ink dark:text-ink-dark">{s.mode_label}</dd>
                      </div>
                      <div className="grid grid-cols-[6.5rem_1fr] gap-x-3 py-2.5 sm:grid-cols-[8rem_1fr]">
                        <dt className="text-ink-mute dark:text-ink-dark-mute">周期触发</dt>
                        <dd className="min-w-0 tabular-nums text-ink dark:text-ink-dark">
                          {s.mode_key === "disabled" ? "—" : formatInterval(s.interval_secs)}
                        </dd>
                      </div>
                      <div className="grid grid-cols-[6.5rem_1fr] gap-x-3 py-2.5 sm:grid-cols-[8rem_1fr]">
                        <dt className="text-ink-mute dark:text-ink-dark-mute">
                          {t("evolution.detail.mergedTriggersLabel")}
                        </dt>
                        <dd className="min-w-0 text-ink dark:text-ink-dark">
                          {s.mode_key === "disabled"
                            ? "—"
                            : t("evolution.detail.mergedTriggersValue", {
                                w: s.weighted_signal_sum ?? 0,
                                wmin: s.weighted_trigger_min ?? 3,
                                win: s.signal_window ?? 10,
                                d: s.unprocessed_decisions,
                                thr: s.decision_threshold,
                              })}
                        </dd>
                      </div>
                      <div className="grid grid-cols-[6.5rem_1fr] gap-x-3 py-2.5 sm:grid-cols-[8rem_1fr]">
                        <dt className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.profile")}</dt>
                        <dd className="min-w-0 text-ink dark:text-ink-dark">
                          {(s.evo_profile_key ?? "default") === "demo"
                            ? t("evolution.profile.demo")
                            : (s.evo_profile_key ?? "default") === "conservative"
                              ? t("evolution.profile.conservative")
                              : t("evolution.profile.default")}
                        </dd>
                      </div>
                      <div className="grid grid-cols-[6.5rem_1fr] gap-x-3 py-2.5 sm:grid-cols-[8rem_1fr]">
                        <dt className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.cooldown")}</dt>
                        <dd className="min-w-0 tabular-nums text-ink dark:text-ink-dark">
                          {s.evo_cooldown_hours != null && Number.isFinite(s.evo_cooldown_hours)
                            ? `${s.evo_cooldown_hours} h`
                            : "—"}
                        </dd>
                      </div>
                      <div className="grid grid-cols-[6.5rem_1fr] gap-x-3 py-2.5 last:pb-0 sm:grid-cols-[8rem_1fr]">
                        <dt className="text-ink-mute dark:text-ink-dark-mute">
                          {t("evolution.summary.lastEvolutionAttempt")}
                        </dt>
                        <dd className="min-w-0 tabular-nums text-ink dark:text-ink-dark">
                          {s.last_run_ts ? formatTs(s.last_run_ts) : "—"}
                        </dd>
                      </div>
                    </dl>
                    <details className="mt-3 rounded-xl border border-border/65 bg-surface/90 text-[11px] text-ink-mute dark:border-border-dark/55 dark:bg-paper-dark/25 dark:text-ink-dark-mute">
                      <summary className="cursor-pointer select-none px-3 py-2 font-medium text-accent list-none hover:bg-paper/30 dark:hover:bg-paper-dark/30 [&::-webkit-details-marker]:hidden">
                        {t("evolution.detail.settingsHintsToggle")}
                      </summary>
                      <div className="space-y-2 border-t border-border/65 px-3 py-2.5 leading-relaxed dark:border-border-dark/55">
                        <p>{t("evolution.adjustInSettingsHint")}</p>
                        <p>{t("evolution.detailEnvHint")}</p>
                      </div>
                    </details>
                  </div>
                </article>

                {(s.a9 || s.passive) && (
                  <article className={evoPanel}>
                    <header className="border-b border-border/50 bg-gradient-to-r from-accent/[0.08] to-transparent px-4 py-3 dark:border-border-dark/70 dark:from-accent/[0.12]">
                      <h2 className={detailSectionTitle}>{t("evolution.diagnostics.titleShort")}</h2>
                    </header>
                    <div className={`${evoPanelPad} space-y-3`}>
                      {s.last_material_run_ts != null && s.last_material_run_ts !== "" && (
                        <p className="text-xs tabular-nums text-ink dark:text-ink-dark">
                          <span className="font-medium text-ink-mute dark:text-ink-dark-mute">
                            {t("evolution.diagnostics.kMaterial")}
                          </span>
                          {formatTs(s.last_material_run_ts)}
                        </p>
                      )}
                      {s.a9 && (
                        <div className="flex flex-wrap items-center gap-2">
                          <span
                            className={`rounded-full px-2.5 py-1 text-[11px] font-semibold ${
                              s.a9.growth_tick_would_be_due
                                ? "bg-emerald-500/15 text-emerald-800 ring-1 ring-emerald-500/25 dark:text-emerald-200"
                                : "bg-surface text-ink-mute ring-1 ring-border/60 dark:bg-paper-dark/25 dark:text-ink-dark-mute dark:ring-border-dark/60"
                            }`}
                          >
                            A9 ·{" "}
                            {s.a9.growth_tick_would_be_due
                              ? t("evolution.diagnostics.a9Fire")
                              : t("evolution.diagnostics.a9Idle")}
                          </span>
                          {s.a9.min_run_gap_secs > 0 ? (
                            <span
                              className={`rounded-full px-2.5 py-1 text-[11px] font-medium ring-1 ${
                                s.a9.min_run_gap_blocked
                                  ? "bg-amber-500/15 text-amber-900 ring-amber-400/30 dark:text-amber-200"
                                  : "bg-surface text-ink-mute ring-border/60 dark:bg-paper-dark/25 dark:text-ink-dark-mute dark:ring-border-dark/60"
                              }`}
                            >
                              {t("evolution.diagnostics.gapSeg", {
                                secs: s.a9.min_run_gap_secs,
                                st: dxMark(!s.a9.min_run_gap_blocked),
                              })}
                            </span>
                          ) : null}
                          <span className="rounded-full bg-surface px-2.5 py-1 font-mono text-[10px] text-ink ring-1 ring-border/60 dark:bg-paper-dark/25 dark:text-ink-dark-mute dark:ring-border-dark/60">
                            Σ{s.a9.weighted_signal_sum}/{s.a9.weighted_trigger_min} · {s.a9.raw_unprocessed_decisions}/
                            {s.a9.raw_unprocessed_threshold}
                          </span>
                          <span className="rounded-full bg-surface px-2 py-1 font-mono text-[10px] text-ink-mute dark:bg-paper-dark/25 dark:text-ink-dark-mute">
                            {t("evolution.diagnostics.lblSweep")}
                            {dxMark(s.a9.arm_sweep)} · {t("evolution.diagnostics.lblPeriodic")}
                            {dxMark(s.a9.arm_periodic)}{" "}
                            {formatDurationSecs(s.a9.periodic_elapsed_secs)}/
                            {formatDurationSecs(s.a9.interval_secs)}
                            {s.a9.growth_tick_would_be_due && s.a9.periodic_only
                              ? ` ${t("evolution.diagnostics.badgePeriodicOnly")}`
                              : ""}
                          </span>
                        </div>
                      )}
                      {s.passive && (
                        <div className="rounded-xl border border-border/65 bg-surface/80 p-3 dark:border-border-dark/55 dark:bg-black/25">
                          <p className="text-[11px] font-semibold text-accent">{t("evolution.diagnostics.kPassive")}</p>
                          <div className="mt-2 grid gap-3 sm:grid-cols-2">
                            <p className="text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
                              {t("evolution.diagnostics.passiveDailyPlain", {
                                n: s.passive.daily_runs_today,
                                cap: s.passive.daily_cap,
                                hint: s.passive.daily_cap_blocked
                                  ? t("evolution.diagnostics.hintDailyBlocked")
                                  : t("evolution.diagnostics.hintDailyOk"),
                              })}
                            </p>
                            <p className="text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
                              {s.passive.hours_since_last_material_run != null
                                ? t("evolution.diagnostics.passiveCoolPlain", {
                                    since: s.passive.hours_since_last_material_run.toFixed(1),
                                    need: s.passive.cooldown_hours,
                                    hint: s.passive.cooldown_blocked
                                      ? t("evolution.diagnostics.hintCoolWait")
                                      : t("evolution.diagnostics.hintCoolOk"),
                                  })
                                : t("evolution.diagnostics.passiveCoolNone")}
                            </p>
                          </div>
                          <p className="mt-2 text-[11px] text-ink-mute dark:text-ink-dark-mute">
                            {t("evolution.diagnostics.passiveStatsOneLine", {
                              days: s.passive.recent_days,
                              m: s.passive.meaningful,
                              f: s.passive.failures,
                              rp: s.passive.repeated_patterns,
                            })}
                          </p>
                          <div className="mt-2 flex flex-wrap gap-1.5">
                            <EvolutionArmChip
                              label={t("evolution.detail.armShortPrompts")}
                              active={s.passive.arm_prompts}
                            />
                            <EvolutionArmChip
                              label={t("evolution.detail.armShortMemory")}
                              active={s.passive.arm_memory}
                            />
                            <EvolutionArmChip
                              label={t("evolution.detail.armShortSkills")}
                              active={s.passive.arm_skills}
                            />
                          </div>
                          {passiveSkillExtra(s.passive, t) ? (
                            <p className="mt-1.5 text-[10px] text-ink-mute dark:text-ink-dark-mute">
                              {passiveSkillExtra(s.passive, t)}
                            </p>
                          ) : null}
                        </div>
                      )}
                      {typeof s.would_have_evolution_proposals === "boolean" && (
                        <p className="text-[12px] leading-snug text-ink dark:text-ink-dark">
                          <span className="font-semibold text-accent">{t("evolution.diagnostics.kProposals")}</span>{" "}
                          {s.would_have_evolution_proposals
                            ? t("evolution.diagnostics.proposalsYes")
                            : t("evolution.diagnostics.proposalsNo")}
                          {!s.would_have_evolution_proposals && s.empty_proposals_reason ? (
                            <span className="text-ink-mute dark:text-ink-dark-mute">
                              {" — "}
                              {evolutionLogReasonForDisplay(s.empty_proposals_reason, locale) ??
                                s.empty_proposals_reason}
                            </span>
                          ) : null}
                        </p>
                      )}
                      <div className="rounded-xl border-l-4 border-accent bg-gradient-to-r from-accent/[0.07] to-transparent px-3 py-2.5 dark:from-accent/[0.12] dark:to-transparent">
                        <p className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                          {t("evolution.diagnostics.resultHeading")}
                        </p>
                        <p className="mt-1 text-sm leading-snug text-ink dark:text-ink-dark">
                          {t(evolutionBeginnerInsightKey(s))}
                        </p>
                      </div>
                    </div>
                  </article>
                )}
              </div>
            )}

            <div className="min-w-0 space-y-4 lg:sticky lg:top-24 lg:self-start">
                <section className={evoPanel}>
                  <div className="flex items-center justify-between gap-2 border-b border-border/50 px-4 py-3 dark:border-border-dark/70">
                    <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">
                      {t("evolution.detail.backlogHeading")}
                    </h2>
                    <button
                      type="button"
                      onClick={() => void loadBacklog()}
                      className="rounded-lg px-2.5 py-1 text-xs font-medium text-accent hover:bg-accent/[0.08] dark:hover:bg-accent/10"
                    >
                      {t("evolution.detail.backlogRefresh")}
                    </button>
                  </div>
                  <div className="space-y-2 p-4">
                    {backlogLoading ? (
                      <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
                    ) : backlog.length === 0 ? (
                      <p className="text-xs italic leading-relaxed text-ink-mute dark:text-ink-dark-mute">
                        {t("evolution.detail.backlogEmptyShort")}
                      </p>
                    ) : (
                      <div className="space-y-2">
                        {backlog.map((row) => (
                          <div
                            key={row.proposal_id}
                            className="rounded-xl border border-border/70 bg-surface/90 p-3 transition-colors hover:border-accent/25 hover:bg-paper dark:border-border-dark/50 dark:bg-paper-dark/25 dark:hover:border-accent/30 dark:hover:bg-paper-dark/40"
                          >
                            <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                              <span className="font-mono text-[11px] font-medium text-ink dark:text-ink-dark">
                                {row.proposal_id}
                              </span>
                              <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute">[{row.source}]</span>
                              <span
                                className="rounded-md bg-violet-100 px-1.5 py-0.5 text-[10px] font-semibold text-violet-800 dark:bg-violet-900/50 dark:text-violet-200"
                                title={row.status}
                              >
                                {evolutionBacklogStatusLabel(row.status)}
                              </span>
                              <span
                                className="rounded-md bg-sky-100 px-1.5 py-0.5 text-[10px] font-semibold text-sky-900 dark:bg-sky-900/45 dark:text-sky-100"
                                title={row.acceptance_status}
                              >
                                {evolutionAcceptanceStatusLabel(row.acceptance_status)}
                              </span>
                              <span className="text-[11px] tabular-nums text-ink-mute dark:text-ink-dark-mute">
                                r{row.risk_level} · ROI {row.roi_score.toFixed(2)}
                              </span>
                              <button
                                type="button"
                                onClick={async () => {
                                  setTriggeringProposalId(row.proposal_id);
                                  setTriggerResultByProposal((prev) => ({
                                    ...prev,
                                    [row.proposal_id]: "触发请求已发送，等待执行结果…",
                                  }));
                                  try {
                                    const { result: out } = await runWithScenarioFallbackNotified<string>(
                                      settings,
                                      "evolution",
                                      (config) =>
                                        invoke<StructuredLlmInvokeResult<string>>(
                                          "skilllite_trigger_evolution_run",
                                          {
                                            workspace,
                                            proposalId: row.proposal_id,
                                            config,
                                          }
                                        ).then(unwrapStructuredLlmInvokeResult)
                                    );
                                    setTriggerResultByProposal((prev) => ({
                                      ...prev,
                                      [row.proposal_id]: `已手动触发：${prependNoMaterialHelpIfNeeded(out)}`,
                                    }));
                                    useUiToastStore.getState().show("已触发一次进化运行", "info");
                                    await loadBacklog();
                                    await refresh();
                                  } catch (e) {
                                    const msg = String(e);
                                    setTriggerResultByProposal((prev) => ({
                                      ...prev,
                                      [row.proposal_id]: `触发失败：${msg}`,
                                    }));
                                    useUiToastStore.getState().show(`触发失败：${msg}`, "error");
                                  } finally {
                                    setTriggeringProposalId(null);
                                  }
                                }}
                                disabled={triggeringProposalId !== null}
                                className="ml-auto rounded-lg border border-border bg-paper/55 px-2 py-1 text-[11px] font-medium text-ink shadow-sm hover:border-accent/40 hover:text-accent disabled:opacity-50 dark:border-border-dark dark:bg-paper-dark/30 dark:text-ink-dark dark:hover:border-accent/40"
                                title="对此提案强制触发一轮进化（后端会设置 SKILLLITE_EVO_FORCE_PROPOSAL_ID 并 force 执行该 proposal）"
                              >
                                {triggeringProposalId === row.proposal_id ? "触发中…" : "立即执行"}
                              </button>
                            </div>
                            <div className="mt-1 font-mono text-[10px] tabular-nums text-ink-mute dark:text-ink-dark-mute">
                              {formatTs(row.updated_at)}
                            </div>
                            {row.note && (
                              <p
                                className="mt-1 line-clamp-3 whitespace-pre-wrap break-words text-[11px] leading-relaxed text-ink-mute dark:text-ink-dark-mute"
                                title={evolutionBacklogNoteForDisplay(
                                  row.status,
                                  row.acceptance_status,
                                  row.note
                                )}
                              >
                                {(() => {
                                  const shown = evolutionBacklogNoteForDisplay(
                                    row.status,
                                    row.acceptance_status,
                                    row.note
                                  );
                                  return shown.length > 220 ? `${shown.slice(0, 220)}…` : shown;
                                })()}
                              </p>
                            )}
                            {triggerResultByProposal[row.proposal_id] && (
                              <p className="mt-1 line-clamp-2 whitespace-pre-wrap text-[10px] text-ink-mute dark:text-ink-dark-mute">
                                {triggerResultByProposal[row.proposal_id]}
                              </p>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                </section>

                <section className={evoPanel}>
                  <div className="border-b border-border/50 px-4 py-3 dark:border-border-dark/70">
                    <h2 className={detailSectionTitle}>{t("evolution.log.sectionRecent")}</h2>
                  </div>
                  <div className="p-3 sm:p-4">
                    {!s?.recent_events.length ? (
                      <p className="text-xs italic text-ink-mute dark:text-ink-dark-mute">
                        {t("evolution.log.noEvents")}
                      </p>
                    ) : (
                      <ul className="max-h-[min(42vh,20rem)] space-y-0 overflow-y-auto overscroll-contain pr-1 sm:pr-2">
            {s.recent_events.map((e, i) => {
              const reasonShown = evolutionLogReasonForDisplay(e.reason ?? null, locale);
              const softenRunNoOutput =
                (e.event_type === "evolution_run" &&
                  isEvolutionRunCompletedNoFileDeltaReason(e.reason ?? null)) ||
                e.event_type === "evolution_run_noop";
              const rawReason = (e.reason ?? "").trim();
              const softenJudgement =
                e.event_type === "evolution_judgement" &&
                (rawReason === "指标无显著变化" || rawReason === "无基线数据，继续观察");
              const reasonRowClass =
                softenRunNoOutput || softenJudgement
                  ? "text-ink-mute/70 dark:text-ink-dark-mute/70 mt-0.5 whitespace-pre-wrap break-words"
                  : "text-ink-mute dark:text-ink-dark-mute mt-0.5 whitespace-pre-wrap break-words";
              const targetShown = evolutionLogTargetLine(e.target_id, locale);
              const txnTrim = e.txn_id?.trim() ?? "";
              const rowOpensDiff =
                (e.event_type === "evolution_run" ||
                  e.event_type === "evolution_run_noop") &&
                txnTrim.length > 0;
              const rowPrimary = evolutionRunRowPrimaryAction(e.reason);
              const showMemoryLink =
                reasonMentionsMemoryKnowledge(e.reason) && rowPrimary !== "memory";
              return (
                <li
                  key={`${e.ts}-${e.event_type}-${i}`}
                  className="mb-2 rounded-xl border border-border/60 bg-surface/80 p-2.5 last:mb-0 dark:border-border-dark/55 dark:bg-paper-dark/25"
                >
                  <div className="flex items-start gap-2">
                    <span className="shrink-0 w-6 text-center text-sm leading-none text-ink-mute dark:text-ink-dark-mute">
                      {eventIcon(e.event_type)}
                    </span>
                    <div className="min-w-0 flex-1">
                      {rowOpensDiff ? (
                        <button
                          type="button"
                          onClick={() => {
                            if (rowPrimary === "memory") {
                              void openDetailWindow("mem");
                              return;
                            }
                            if (rowPrimary === "review") {
                              setDetailTab("review");
                              return;
                            }
                            setDetailTab("changes");
                            setCompareFocusTxn(txnTrim);
                          }}
                          title={
                            rowPrimary === "memory"
                              ? t("evolution.log.rowOpenMemoryTitle")
                              : rowPrimary === "review"
                                ? t("evolution.log.rowOpenReviewTitle")
                                : t("evolution.log.rowOpenDiffTitle")
                          }
                          className="w-full rounded-lg text-left transition-colors hover:bg-paper/55 focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/30 dark:hover:bg-paper-dark/35"
                        >
                          <div className="font-mono text-[10px] text-ink-mute dark:text-ink-dark-mute">
                            {formatTs(e.ts)}
                            <span className="ml-1.5 font-sans text-[10px] font-semibold text-accent">
                              {rowPrimary === "memory"
                                ? t("evolution.log.rowOpenMemoryBadge")
                                : rowPrimary === "review"
                                  ? t("evolution.log.rowOpenReviewBadge")
                                  : t("evolution.log.rowOpenDiffBadge")}
                            </span>
                          </div>
                          <div className="mt-0.5 font-medium text-ink dark:text-ink-dark text-[12px]" title={e.event_type}>
                            {evolutionLogEventTypeLabel(e.event_type, locale)}
                          </div>
                          {targetShown && (
                            <div className="truncate text-[11px] text-ink-mute dark:text-ink-dark-mute">
                              {targetShown}
                            </div>
                          )}
                          <div className="mt-0.5 font-mono text-[10px] text-ink-mute dark:text-ink-dark-mute">
                            txn: {txnTrim}
                          </div>
                          {reasonShown && (
                            <p className={`${reasonRowClass} mt-1 text-[11px] text-ink-mute dark:text-ink-dark-mute`}>
                              {reasonShown.length > 200 ? `${reasonShown.slice(0, 200)}…` : reasonShown}
                            </p>
                          )}
                        </button>
                      ) : (
                        <div className="w-full">
                          <div className="font-mono text-[10px] text-ink-mute dark:text-ink-dark-mute">
                            {formatTs(e.ts)}
                          </div>
                          <div className="mt-0.5 font-medium text-ink dark:text-ink-dark text-[12px]" title={e.event_type}>
                            {evolutionLogEventTypeLabel(e.event_type, locale)}
                          </div>
                          {targetShown && (
                            <div className="truncate text-[11px] text-ink-mute dark:text-ink-dark-mute">
                              {targetShown}
                            </div>
                          )}
                          {txnTrim ? (
                            <div className="mt-0.5 font-mono text-[10px] text-ink-mute dark:text-ink-dark-mute">
                              txn: {txnTrim}
                            </div>
                          ) : null}
                          {reasonShown && (
                            <p className={`${reasonRowClass} mt-1 text-[11px] text-ink-mute dark:text-ink-dark-mute`}>
                              {reasonShown.length > 200 ? `${reasonShown.slice(0, 200)}…` : reasonShown}
                            </p>
                          )}
                        </div>
                      )}
                      {showMemoryLink ? (
                        <button
                          type="button"
                          onClick={() => void openDetailWindow("mem")}
                          className="mt-1.5 text-[11px] font-medium text-accent hover:underline"
                        >
                          {t("evolution.log.openMemoryPanel")}
                        </button>
                      ) : null}
                    </div>
                  </div>
                </li>
              );
            })}
                      </ul>
                    )}
                  </div>
                </section>
              </div>
            </div>
        </div>
      )}

      {detailTab === "review" && (
        <div
          role="tabpanel"
          id="evolution-detail-panel-review"
          aria-labelledby="evolution-detail-tab-review"
          className={`${evoPanel} divide-y divide-border/50 dark:divide-border-dark/60`}
        >
          <div
            className={`${evoPanelPad} bg-gradient-to-br from-accent-light/70 via-surface to-paper/40 dark:from-accent-light-dark/25 dark:via-surface-dark dark:to-paper-dark/25`}
          >
            <h2 className={detailSectionTitle}>{t("evolution.detail.reviewSystemHeading")}</h2>
            {s?.judgement_label ? (
              <div className="mt-3 rounded-2xl border border-violet-200/70 bg-paper/50 p-4 shadow-sm dark:border-violet-800/40 dark:bg-paper-dark/30">
                <p className="text-base font-semibold leading-snug text-ink dark:text-ink-dark">
                  {s.judgement_label}
                </p>
                {s.judgement_reason && (
                  <p className="mt-2 line-clamp-5 whitespace-pre-wrap text-sm leading-relaxed text-ink-mute dark:text-ink-dark-mute">
                    {s.judgement_reason}
                  </p>
                )}
              </div>
            ) : (
              <p className="mt-2 text-sm italic leading-relaxed text-ink-mute dark:text-ink-dark-mute">
                {t("evolution.detail.reviewNoJudgement")}
              </p>
            )}
          </div>
          <div className={`${evoPanelPad} space-y-3`}>
            <div className="flex items-center justify-between gap-2 border-b border-border/50 pb-2 dark:border-border-dark/70">
              <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">
                {t("evolution.detail.reviewPendingHeading")}
              </h2>
              <button
                type="button"
                onClick={() => void loadPending()}
                className="rounded-lg px-2.5 py-1 text-xs font-medium text-accent hover:bg-accent/[0.08] dark:hover:bg-accent/10"
              >
                {t("evolution.detail.reviewPendingRefresh")}
              </button>
            </div>
            {pendingLoading ? (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
            ) : pending.length === 0 ? (
              <p className="text-sm italic leading-relaxed text-ink-mute dark:text-ink-dark-mute">
                {t("evolution.detail.reviewPendingEmpty")}
              </p>
            ) : (
              <div className="space-y-3">
                {pending.map((p) => (
                  <PendingSkillReviewCard
                    key={p.name}
                    skill={p}
                    workspace={workspace}
                    onChanged={onSkillChanged}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {detailTab === "changes" && (
        <div
          role="tabpanel"
          id="evolution-detail-panel-changes"
          aria-labelledby="evolution-detail-tab-changes"
          className={`${evoPanel} ${evoPanelPad} space-y-4`}
        >
          <div className="flex items-center justify-between gap-2 border-b border-border/50 pb-3 dark:border-border-dark/70">
            <h2 className="flex min-w-0 items-center gap-2 text-sm font-semibold text-ink dark:text-ink-dark">
              <span
                className="h-6 w-1 shrink-0 rounded-full bg-gradient-to-b from-accent to-violet-500/80"
                aria-hidden
              />
              <span className="min-w-0 leading-snug">
                <span className="text-accent">{t("evolution.diff.sectionTitleEvolution")}</span>
                <span className="text-ink dark:text-ink-dark">{t("evolution.diff.sectionTitleRest")}</span>
              </span>
            </h2>
            <button
              type="button"
              onClick={() => void loadDiffs()}
              className="shrink-0 rounded-lg px-2.5 py-1 text-xs font-medium text-accent hover:bg-accent/[0.08] dark:hover:bg-accent/10"
            >
              {t("status.refresh")}
            </button>
          </div>
          {diffsLoading ? (
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
          ) : (
            <>
              {diffs.length === 0 ? (
                <p className="text-sm italic leading-relaxed text-ink-mute dark:text-ink-dark-mute">
                  {t("evolution.diff.emptyHint")}
                </p>
              ) : (
                <div className="space-y-3">
                  <details className="rounded-xl border border-border/65 bg-surface/90 text-sm text-ink-mute dark:border-border-dark/55 dark:bg-paper-dark/25 dark:text-ink-dark-mute">
                    <summary className="cursor-pointer select-none px-3 py-2 font-medium text-accent list-none hover:bg-paper/30 dark:hover:bg-paper-dark/30 [&::-webkit-details-marker]:hidden">
                      {t("evolution.diff.legendToggle")}
                    </summary>
                    <p className="border-t border-border/65 px-3 py-2.5 text-xs leading-relaxed dark:border-border-dark/55">
                      {t("evolution.diff.legend")}
                    </p>
                  </details>
                  {sortedDiffs.map((d) => (
                    <div
                      key={d.filename}
                      className={`overflow-hidden rounded-xl border text-[12px] ${
                        evolutionPromptHasNetChange(d)
                          ? "border-emerald-300/80 bg-gradient-to-b from-emerald-50/90 to-surface shadow-sm dark:border-emerald-700/50 dark:from-emerald-950/35 dark:to-paper-dark/35"
                          : "border-border/65 bg-surface/80 dark:border-border-dark/50 dark:bg-paper-dark/25"
                      }`}
                    >
                      <div
                        className={`flex items-center justify-between gap-2 border-b border-border/60 px-3 py-2.5 dark:border-border-dark/70 ${
                          evolutionPromptHasNetChange(d)
                            ? "bg-emerald-500/[0.06] dark:bg-emerald-500/[0.08]"
                            : "bg-paper/35 dark:bg-paper-dark/35"
                        }`}
                      >
                        <span className="min-w-0 font-mono font-medium text-ink dark:text-ink-dark">
                          {d.filename}
                        </span>
                        <div className="flex flex-col items-end gap-0.5 shrink-0">
                          {evolutionPromptHasNetChange(d) ? (
                            <span className="px-2 py-0.5 rounded-md text-[10px] font-semibold tracking-wide bg-emerald-100/95 dark:bg-emerald-900/55 text-emerald-900 dark:text-emerald-200 border border-emerald-400/55 dark:border-emerald-500/45 ring-1 ring-emerald-500/25">
                              {t("evolution.diff.evolvedBadge")}
                            </span>
                          ) : d.evolved ? (
                            d.original_content != null ? (
                              <span
                                className="px-2 py-0.5 rounded-md text-[10px] font-medium bg-ink/[0.06] dark:bg-paper/55/[0.08] text-ink-mute dark:text-ink-dark-mute border border-border/60 dark:border-border-dark/50"
                                title={t("evolution.diff.evolvedNoChangeHint")}
                              >
                                {t("evolution.diff.evolvedNoChangeBadge")}
                              </span>
                            ) : (
                              <span
                                className="px-2 py-0.5 rounded-md text-[10px] font-medium bg-amber-50/90 dark:bg-amber-900/25 text-amber-900 dark:text-amber-200/90 border border-amber-200/70 dark:border-amber-700/45"
                                title={t("evolution.diff.evolvedNoSnapshotHint")}
                              >
                                {t("evolution.diff.evolvedNoSnapshotBadge")}
                              </span>
                            )
                          ) : null}
                        </div>
                      </div>
                      <EvolutionPromptVersionCompare
                        filename={d.filename}
                        focusTxn={compareFocusTxn}
                        txns={snapshotsByFile?.[d.filename] ?? []}
                        txnsLoading={snapshotsLoading}
                        txnsErr={snapshotsErr}
                      />
                    </div>
                  ))}
                </div>
              )}
              <EvolutionPromptManualEditSection
                workspace={workspace}
                defaultOpen={diffs.length === 0}
              />
            </>
          )}
        </div>
      )}
      </div>
    </div>
  );
}
