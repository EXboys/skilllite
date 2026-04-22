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
        className="w-full flex items-center justify-between px-2 py-2 text-left text-xs font-mono bg-white/60 dark:bg-paper-dark/60 hover:bg-ink/5 dark:hover:bg-white/5"
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
                className="w-full rounded border border-border dark:border-border-dark bg-white dark:bg-paper-dark px-2 py-1.5 text-[11px] font-mono text-ink dark:text-ink-dark"
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
  "bg-white dark:bg-paper-dark shadow-sm pl-3 pr-9 py-2 text-[11px] font-mono " +
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

function passiveTryWord(on: boolean, t: (k: string) => string): string {
  return on ? t("evolution.diagnostics.tryYes") : t("evolution.diagnostics.tryNo");
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
  const scheduleHint =
    s.mode_key === "disabled"
      ? "已禁用，后台不会自动进化"
      : `${formatInterval(s.interval_secs)} 检查一次；近期加权（窗口 ${s.signal_window ?? 10}）≥ ${s.weighted_trigger_min ?? 3}（当前 ${s.weighted_signal_sum ?? 0}）或未处理 ≥ ${s.decision_threshold} 条也会触发`;

  return (
    <section className={sectionCls}>
      <div className="flex shrink-0 items-center justify-between gap-2 mb-2 min-w-0">
        <button
          type="button"
          onClick={onOpenDetail}
          className="flex-1 min-w-0 text-left font-medium text-ink dark:text-ink-dark group hover:text-accent dark:hover:text-accent"
        >
          <EvolutionBrandTitle />
          <span className="text-xs font-normal text-ink-mute group-hover:text-accent dark:text-ink-dark-mute dark:group-hover:text-accent inline-flex items-center gap-0.5 ml-0.5 transition-colors">
            {t("evolution.summary.openDetail")}
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M9 18l6-6-6-6" />
            </svg>
          </span>
        </button>
        <button
          type="button"
          onClick={() => void refresh()}
          disabled={loading}
          className="p-1.5 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 shrink-0"
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
        className={`max-w-full min-w-0 cursor-pointer rounded-lg border border-border/60 dark:border-border-dark/60 border-l-[3px] border-l-accent/55 dark:border-l-accent/45 bg-gray-50/50 dark:bg-surface-dark/50 px-2.5 py-2 text-xs text-ink dark:text-ink-dark space-y-1.5 break-words shadow-sm shadow-accent/[0.07] dark:shadow-accent/[0.12] flex flex-col ${metricsClassName ?? ""}`}
        onClick={onOpenDetail}
        role="button"
        onKeyDown={(e) => e.key === "Enter" && onOpenDetail()}
        tabIndex={0}
      >
        {s.db_error && (
          <p className="break-words text-amber-700 dark:text-amber-400">{s.db_error}</p>
        )}
        <div className="flex min-w-0 justify-between gap-2">
          <span className="shrink-0 text-ink-mute dark:text-ink-dark-mute">模式</span>
          <span className="min-w-0 truncate text-right font-medium">{s.mode_label}</span>
        </div>
        <p className="text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute break-words">{scheduleHint}</p>
        <div className="flex min-w-0 justify-between gap-2">
          <span className="shrink-0 text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.profile")}</span>
          <span className="min-w-0 truncate text-right text-[11px]">{profileLabel}</span>
        </div>
        <div className="flex min-w-0 justify-between gap-2">
          <span className="shrink-0 text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.cooldown")}</span>
          <span className="min-w-0 tabular-nums text-right text-[11px]">
            {s.evo_cooldown_hours != null && Number.isFinite(s.evo_cooldown_hours)
              ? `${s.evo_cooldown_hours} h`
              : "—"}
          </span>
        </div>
        <div className="flex min-w-0 justify-between gap-2">
          <span className="shrink-0 text-ink-mute dark:text-ink-dark-mute">未进化决策</span>
          <span className="min-w-0 truncate text-right">{s.unprocessed_decisions}</span>
        </div>
        <div className="flex min-w-0 justify-between gap-2">
          <span className="shrink-0 text-ink-mute dark:text-ink-dark-mute">上次进化运行</span>
          <span className="min-w-0 truncate text-right">
            {s.last_run_ts ? formatTs(s.last_run_ts) : "—"}
          </span>
        </div>
        {s.judgement_label && (
          <div className="min-w-0 border-t border-border/40 pt-1 dark:border-border-dark/40">
            <span className="text-ink-mute dark:text-ink-dark-mute">审核判断 </span>
            <span className="break-words font-medium">{s.judgement_label}</span>
            {s.judgement_reason && (
              <p className="mt-0.5 line-clamp-2 break-words text-[11px] text-ink-mute dark:text-ink-dark-mute">
                {s.judgement_reason}
              </p>
            )}
          </div>
        )}
        <div className="flex min-w-0 justify-between gap-2 pt-0.5">
          <span className="shrink-0 text-ink-mute dark:text-ink-dark-mute">待确认技能</span>
          <span
            className={`min-w-0 shrink-0 text-right ${s.pending_skill_count > 0 ? "font-semibold text-accent" : ""}`}
          >
            {s.pending_skill_count}
          </span>
        </div>
        <p
          className="min-w-0 truncate text-[10px] text-ink-mute/80 dark:text-ink-dark-mute/80"
          title={workspace}
        >
          工作区: {workspace}
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
    <div className="rounded-lg border border-border dark:border-border-dark bg-white/60 dark:bg-paper-dark/60 p-3 space-y-2">
      <div className="flex items-center justify-between gap-2 flex-wrap">
        <span className="text-sm font-semibold text-ink dark:text-ink-dark">{skill.name}</span>
        {skill.needs_review && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-100 dark:bg-amber-900/40 text-amber-900 dark:text-amber-200">
            建议人工过目
          </span>
        )}
      </div>
      <button
        type="button"
        onClick={() => void loadFull()}
        className="text-xs text-accent hover:underline"
        disabled={loadingFull}
      >
        {loadingFull ? "加载全文…" : expanded ? "收起全文" : "查看 / 展开全文"}
      </button>
      <div
        className={`prose prose-sm max-w-none dark:prose-invert [&_pre]:text-xs [&_code]:text-xs overflow-y-auto border border-border/50 dark:border-border-dark/50 rounded-md p-2 bg-gray-50/80 dark:bg-surface-dark/50 ${
          showShort ? "max-h-48" : "max-h-[min(70vh,520px)]"
        }`}
      >
        {displayMd ? (
          <MarkdownContent content={displayMd} />
        ) : (
          <p className="text-xs text-ink-mute">（无 SKILL.md 内容）</p>
        )}
      </div>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => void confirm()}
          disabled={acting !== null}
          className="px-3 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:opacity-90 disabled:opacity-50"
        >
          {acting === "confirm" ? "处理中…" : "确认加入"}
        </button>
        <button
          type="button"
          onClick={() => void reject()}
          disabled={acting !== null}
          className="px-3 py-1.5 rounded-lg border border-border dark:border-border-dark text-xs text-ink dark:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50"
        >
          {acting === "reject" ? "处理中…" : "拒绝"}
        </button>
      </div>
      {msg && <p className="text-xs text-ink-mute dark:text-ink-dark-mute">{msg}</p>}
    </div>
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

  const tabBtnClass = (active: boolean, emphasize?: boolean) =>
    `flex-1 min-w-0 py-2 px-1.5 text-xs font-medium rounded-t-md border-b-2 transition-colors ${
      active
        ? emphasize
          ? "border-accent text-accent dark:text-accent font-semibold"
          : "border-accent text-ink dark:text-ink-dark"
        : "border-transparent text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
    }`;

  const pendingCount = pending.length;
  const hasJudgement = Boolean(s?.judgement_label);

  return (
    <div className="space-y-4 p-1">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div
          role="tablist"
          aria-label={t("evolution.detail.tabListAria")}
          className="flex flex-1 min-w-0 gap-0 border-b border-border/80 dark:border-border-dark/80"
        >
          <button
            type="button"
            role="tab"
            id="evolution-detail-tab-run"
            aria-controls="evolution-detail-panel-run"
            aria-selected={detailTab === "run"}
            tabIndex={detailTab === "run" ? 0 : -1}
            onClick={() => setDetailTab("run")}
            className={tabBtnClass(detailTab === "run")}
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
            className={tabBtnClass(detailTab === "review")}
          >
            <span className="inline-flex items-center justify-center gap-1">
              {t("evolution.detail.tabReview")}
              {(pendingCount > 0 || hasJudgement) && (
                <span className="tabular-nums rounded-full bg-accent/15 dark:bg-accent/25 px-1.5 py-px text-[10px] font-semibold text-accent">
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
            className={tabBtnClass(detailTab === "changes", true)}
          >
            <span className="inline-flex items-center justify-center gap-1">
              {t("evolution.detail.tabChanges")}
              {evolvedPromptWithChangeCount > 0 && (
                <span
                  className="tabular-nums rounded-full bg-emerald-500/20 dark:bg-emerald-400/15 px-1.5 py-px text-[10px] font-semibold text-emerald-800 dark:text-emerald-300"
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
          className="text-xs text-accent hover:underline disabled:opacity-50 shrink-0"
        >
          {loading ? t("evolution.detail.refreshing") : t("evolution.detail.refreshStatus")}
        </button>
      </div>

      {s?.db_error && (
        <p className="text-sm text-amber-700 dark:text-amber-400">{s.db_error}</p>
      )}

      {detailTab === "run" && (
        <div
          role="tabpanel"
          id="evolution-detail-panel-run"
          aria-labelledby="evolution-detail-tab-run"
          className="space-y-6"
        >
          {s && (
            <>
            <section className="space-y-2">
              <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">调度与配置</h2>
              <ul className="text-xs text-ink dark:text-ink-dark space-y-1.5 bg-gray-50/80 dark:bg-surface-dark/50 rounded-lg p-3 border border-border/50 dark:border-border-dark/50">
                <li>
                  <span className="text-ink-mute dark:text-ink-dark-mute">模式：</span>
                  {s.mode_label}
                </li>
                <li>
                  <span className="text-ink-mute dark:text-ink-dark-mute">周期触发：</span>
                  {s.mode_key === "disabled" ? "—" : formatInterval(s.interval_secs)}
                </li>
                <li>
                  <span className="text-ink-mute dark:text-ink-dark-mute">加权触发：</span>
                  窗口 {s.signal_window ?? 10} 条内加权和需 ≥ {s.weighted_trigger_min ?? 3}（当前{" "}
                  {s.weighted_signal_sum ?? 0}）
                </li>
                <li>
                  <span className="text-ink-mute dark:text-ink-dark-mute">未处理条数（OR）：</span>
                  ≥ {s.decision_threshold} 条即触发（当前 {s.unprocessed_decisions}）
                </li>
                <li>
                  <span className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.profile")}（生效）：</span>
                  {(s.evo_profile_key ?? "default") === "demo"
                    ? t("evolution.profile.demo")
                    : (s.evo_profile_key ?? "default") === "conservative"
                      ? t("evolution.profile.conservative")
                      : t("evolution.profile.default")}
                </li>
                <li>
                  <span className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.summary.cooldown")}（生效）：</span>
                  {s.evo_cooldown_hours != null && Number.isFinite(s.evo_cooldown_hours)
                    ? `${s.evo_cooldown_hours} h`
                    : "—"}
                </li>
                <li>
                  <span className="text-ink-mute dark:text-ink-dark-mute">
                    {t("evolution.summary.lastEvolutionAttempt")}
                  </span>
                  {s.last_run_ts ? formatTs(s.last_run_ts) : "暂无记录"}
                </li>
                <li className="text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                  {t("evolution.adjustInSettingsHint")}
                </li>
                <li className="text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                  {t("evolution.detailEnvHint")}
                </li>
              </ul>
            </section>
            {(s.a9 || s.passive) && (
              <section className="space-y-1.5 rounded-lg border border-border/50 dark:border-border-dark/50 bg-gray-50/80 dark:bg-surface-dark/50 p-3">
                <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">
                  {t("evolution.diagnostics.titleShort")}
                </h2>
                {s.last_material_run_ts != null && s.last_material_run_ts !== "" && (
                  <p className="text-xs text-ink dark:text-ink-dark tabular-nums">
                    <span className="text-ink-mute dark:text-ink-dark-mute">
                      {t("evolution.diagnostics.kMaterial")}
                    </span>
                    {formatTs(s.last_material_run_ts)}
                  </p>
                )}
                {s.a9 && (
                  <p className="text-xs text-ink dark:text-ink-dark leading-snug tabular-nums">
                    <span className="font-semibold text-accent">A9</span>{" "}
                    {s.a9.growth_tick_would_be_due
                      ? t("evolution.diagnostics.a9Fire")
                      : t("evolution.diagnostics.a9Idle")}
                    {s.a9.min_run_gap_secs > 0 ? (
                      <>
                        {" · "}
                        <span
                          className={
                            s.a9.min_run_gap_blocked
                              ? "text-amber-700 dark:text-amber-400"
                              : "text-ink-mute dark:text-ink-dark-mute"
                          }
                        >
                          {t("evolution.diagnostics.gapSeg", {
                            secs: s.a9.min_run_gap_secs,
                            st: dxMark(!s.a9.min_run_gap_blocked),
                          })}
                        </span>
                      </>
                    ) : null}
                    {" · "}
                    Σ{s.a9.weighted_signal_sum}/{s.a9.weighted_trigger_min} · {s.a9.raw_unprocessed_decisions}/
                    {s.a9.raw_unprocessed_threshold}
                    {" · "}
                    {t("evolution.diagnostics.lblSweep")}
                    {dxMark(s.a9.arm_sweep)}
                    {" · "}
                    {t("evolution.diagnostics.lblPeriodic")}
                    {dxMark(s.a9.arm_periodic)} {formatDurationSecs(s.a9.periodic_elapsed_secs)}/
                    {formatDurationSecs(s.a9.interval_secs)}
                    {s.a9.growth_tick_would_be_due && s.a9.periodic_only ? (
                      <span className="text-ink-mute dark:text-ink-dark-mute">
                        {" "}
                        {t("evolution.diagnostics.badgePeriodicOnly")}
                      </span>
                    ) : null}
                  </p>
                )}
                {s.passive && (
                  <div className="text-xs text-ink dark:text-ink-dark leading-relaxed space-y-1">
                    <p className="font-semibold text-accent">
                      {t("evolution.diagnostics.kPassive")}
                    </p>
                    <p className="text-ink-mute dark:text-ink-dark-mute">
                      {t("evolution.diagnostics.passiveDailyPlain", {
                        n: s.passive.daily_runs_today,
                        cap: s.passive.daily_cap,
                        hint: s.passive.daily_cap_blocked
                          ? t("evolution.diagnostics.hintDailyBlocked")
                          : t("evolution.diagnostics.hintDailyOk"),
                      })}
                    </p>
                    <p className="text-ink-mute dark:text-ink-dark-mute">
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
                    <p className="text-ink-mute dark:text-ink-dark-mute">
                      {t("evolution.diagnostics.passiveStatsPlain", {
                        days: s.passive.recent_days,
                        m: s.passive.meaningful,
                        f: s.passive.failures,
                        rp: s.passive.repeated_patterns,
                      })}
                    </p>
                    <p className="text-ink dark:text-ink-dark">
                      {t("evolution.diagnostics.passiveArmsPlain", {
                        prompts: passiveTryWord(s.passive.arm_prompts, t),
                        memory: passiveTryWord(s.passive.arm_memory, t),
                        skills: passiveTryWord(s.passive.arm_skills, t),
                        skillExtra: passiveSkillExtra(s.passive, t),
                      })}
                    </p>
                  </div>
                )}
                {typeof s.would_have_evolution_proposals === "boolean" && (
                  <p className="text-xs text-ink dark:text-ink-dark leading-snug">
                    <span className="font-semibold text-accent">
                      {t("evolution.diagnostics.kProposals")}
                    </span>{" "}
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
                <div className="mt-2 rounded-md bg-ink/[0.04] dark:bg-white/[0.06] px-2.5 py-2">
                  <p className="text-[11px] font-semibold text-ink dark:text-ink-dark">
                    {t("evolution.diagnostics.resultHeading")}
                  </p>
                  <p className="text-xs text-ink dark:text-ink-dark leading-relaxed mt-0.5">
                    {t(evolutionBeginnerInsightKey(s))}
                  </p>
                </div>
              </section>
            )}
            </>
          )}

      <section className="space-y-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">能力进化队列与执行</h2>
          <button
            type="button"
            onClick={() => void loadBacklog()}
            className="text-xs text-accent hover:underline"
          >
            刷新队列
          </button>
        </div>
        {backlogLoading ? (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
        ) : backlog.length === 0 ? (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">
            暂无待处理队列项。已执行且验收已结束（met / not_met）的记录不在此列表展示；仍在验收窗口（pending_validation）的仍会显示。
          </p>
        ) : (
          <div className="space-y-2">
            {backlog.map((row) => (
              <div
                key={row.proposal_id}
                className="rounded-lg border border-border/60 dark:border-border-dark/60 bg-gray-50/60 dark:bg-surface-dark/50 p-3 text-xs"
              >
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <span className="font-mono text-ink dark:text-ink-dark">{row.proposal_id}</span>
                  <span className="text-ink-mute dark:text-ink-dark-mute">[{row.source}]</span>
                  <span
                    className="px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-900/40 text-purple-800 dark:text-purple-300"
                    title={row.status}
                  >
                    {evolutionBacklogStatusLabel(row.status)}
                  </span>
                  <span
                    className="px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-800 dark:text-blue-300"
                    title={row.acceptance_status}
                  >
                    {evolutionAcceptanceStatusLabel(row.acceptance_status)}
                  </span>
                  <span className="text-ink-mute dark:text-ink-dark-mute">
                    risk={row.risk_level} ROI={row.roi_score.toFixed(2)}
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
                    className="ml-auto px-2 py-0.5 rounded border border-border dark:border-border-dark text-ink dark:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50"
                    title="对此提案强制触发一轮进化（后端会设置 SKILLLITE_EVO_FORCE_PROPOSAL_ID 并 force 执行该 proposal）"
                  >
                    {triggeringProposalId === row.proposal_id ? "触发中…" : "立即执行"}
                  </button>
                </div>
                <div className="mt-1 text-ink-mute dark:text-ink-dark-mute">
                  更新: {formatTs(row.updated_at)}
                </div>
                {row.note && (
                  <p className="mt-1 whitespace-pre-wrap text-ink-mute dark:text-ink-dark-mute">
                    {(() => {
                      const shown = evolutionBacklogNoteForDisplay(
                        row.status,
                        row.acceptance_status,
                        row.note
                      );
                      return shown.length > 280
                        ? `${shown.slice(0, 280)}…`
                        : shown;
                    })()}
                  </p>
                )}
                {triggerResultByProposal[row.proposal_id] && (
                  <p className="mt-1 whitespace-pre-wrap text-ink-mute dark:text-ink-dark-mute">
                    {triggerResultByProposal[row.proposal_id]}
                  </p>
                )}
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="space-y-2">
        <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">
          {t("evolution.log.sectionRecent")}
        </h2>
        {!s?.recent_events.length ? (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">
            {t("evolution.log.noEvents")}
          </p>
        ) : (
          <ul className="space-y-2 text-xs">
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
                  className="border-b border-border/40 dark:border-border-dark/40 pb-2 last:border-0"
                >
                  <div className="flex items-start gap-2">
                    <span className="shrink-0 w-4 text-center pt-0.5">{eventIcon(e.event_type)}</span>
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
                          className="w-full rounded-lg text-left px-1 py-0.5 -mx-1 transition-colors cursor-pointer hover:bg-ink/[0.06] dark:hover:bg-white/[0.06] focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
                        >
                          <div className="text-ink-mute dark:text-ink-dark-mute font-mono text-[11px]">
                            {formatTs(e.ts)}
                            <span className="ml-1.5 text-[10px] text-accent font-sans font-medium">
                              {rowPrimary === "memory"
                                ? t("evolution.log.rowOpenMemoryBadge")
                                : rowPrimary === "review"
                                  ? t("evolution.log.rowOpenReviewBadge")
                                  : t("evolution.log.rowOpenDiffBadge")}
                            </span>
                          </div>
                          <div className="font-medium text-ink dark:text-ink-dark" title={e.event_type}>
                            {evolutionLogEventTypeLabel(e.event_type, locale)}
                          </div>
                          {targetShown && (
                            <div className="text-ink-mute dark:text-ink-dark-mute truncate">
                              {targetShown}
                            </div>
                          )}
                          <div className="text-[10px] font-mono text-ink-mute/90 dark:text-ink-dark-mute/90 mt-0.5">
                            txn: {txnTrim}
                          </div>
                          {reasonShown && (
                            <p className={reasonRowClass}>
                              {reasonShown.length > 280 ? `${reasonShown.slice(0, 280)}…` : reasonShown}
                            </p>
                          )}
                        </button>
                      ) : (
                        <div className="w-full px-1 py-0.5 -mx-1">
                          <div className="text-ink-mute dark:text-ink-dark-mute font-mono text-[11px]">
                            {formatTs(e.ts)}
                          </div>
                          <div className="font-medium text-ink dark:text-ink-dark" title={e.event_type}>
                            {evolutionLogEventTypeLabel(e.event_type, locale)}
                          </div>
                          {targetShown && (
                            <div className="text-ink-mute dark:text-ink-dark-mute truncate">
                              {targetShown}
                            </div>
                          )}
                          {txnTrim ? (
                            <div className="text-[10px] font-mono text-ink-mute/90 dark:text-ink-dark-mute/90 mt-0.5">
                              txn: {txnTrim}
                            </div>
                          ) : null}
                          {reasonShown && (
                            <p className={reasonRowClass}>
                              {reasonShown.length > 280 ? `${reasonShown.slice(0, 280)}…` : reasonShown}
                            </p>
                          )}
                        </div>
                      )}
                      {showMemoryLink ? (
                        <button
                          type="button"
                          onClick={() => void openDetailWindow("mem")}
                          className="mt-1.5 ml-1 text-[11px] text-accent hover:underline font-medium"
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
      </section>
        </div>
      )}

      {detailTab === "review" && (
        <div
          role="tabpanel"
          id="evolution-detail-panel-review"
          aria-labelledby="evolution-detail-tab-review"
          className="space-y-6"
        >
          <section className="space-y-2">
            <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">系统审核判断</h2>
            {s?.judgement_label ? (
              <div className="rounded-lg border border-border dark:border-border-dark p-3 text-sm">
                <p className="font-medium text-ink dark:text-ink-dark">{s.judgement_label}</p>
                {s.judgement_reason && (
                  <p className="text-xs text-ink-mute dark:text-ink-dark-mute mt-2 whitespace-pre-wrap">
                    {s.judgement_reason}
                  </p>
                )}
              </div>
            ) : (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">
                暂无系统审核结论（最近一次进化判断未记录或为空）。
              </p>
            )}
          </section>

          <section className="space-y-3">
            <div className="flex items-center justify-between gap-2">
              <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">待确认技能（人工审核）</h2>
              <button
                type="button"
                onClick={() => void loadPending()}
                className="text-xs text-accent hover:underline"
              >
                刷新列表
              </button>
            </div>
            {pendingLoading ? (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
            ) : pending.length === 0 ? (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">
                暂无待确认技能。进化生成的新技能会出现在 .skills/_evolved/_pending/。
              </p>
            ) : (
              <div className="space-y-4">
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
          </section>
        </div>
      )}

      {detailTab === "changes" && (
        <div
          role="tabpanel"
          id="evolution-detail-panel-changes"
          aria-labelledby="evolution-detail-tab-changes"
          className="space-y-3"
        >
          <div className="flex items-center justify-between gap-2">
            <h2 className="text-sm font-semibold text-ink dark:text-ink-dark flex items-center gap-2 min-w-0">
              <span
                className="h-5 w-1 shrink-0 rounded-full bg-gradient-to-b from-accent to-accent/55"
                aria-hidden
              />
              <span className="min-w-0 leading-snug">
                <span className="text-accent font-bold">{t("evolution.diff.sectionTitleEvolution")}</span>
                <span>{t("evolution.diff.sectionTitleRest")}</span>
              </span>
            </h2>
            <button
              type="button"
              onClick={() => void loadDiffs()}
              className="text-xs text-accent hover:underline shrink-0"
            >
              {t("status.refresh")}
            </button>
          </div>
          {diffsLoading ? (
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
          ) : (
            <>
              {diffs.length === 0 ? (
                <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">
                  {t("evolution.diff.emptyHint")}
                </p>
              ) : (
                <div className="space-y-3">
                  <p className="text-xs text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                    {t("evolution.diff.legend")}
                  </p>
                  {sortedDiffs.map((d) => (
                    <div
                      key={d.filename}
                      className={`rounded-lg border text-xs overflow-hidden ${
                        evolutionPromptHasNetChange(d)
                          ? "bg-green-50/60 dark:bg-green-900/15 border-green-400/70 dark:border-green-600/50 ring-1 ring-accent/20 dark:ring-accent/25 shadow-sm shadow-emerald-500/10 dark:shadow-emerald-400/5"
                          : "bg-gray-50/50 dark:bg-surface-dark/40 border-border/50 dark:border-border-dark/50"
                      }`}
                    >
                      <div
                        className={`flex items-center justify-between gap-2 px-3 py-2 border-b border-border/30 dark:border-border-dark/30 ${
                          evolutionPromptHasNetChange(d)
                            ? "bg-gradient-to-r from-accent/[0.08] to-transparent dark:from-accent/[0.12]"
                            : "bg-gray-100/50 dark:bg-surface-dark/30"
                        }`}
                      >
                        <span className="font-mono font-medium text-ink dark:text-ink-dark min-w-0">
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
                                className="px-2 py-0.5 rounded-md text-[10px] font-medium bg-ink/[0.06] dark:bg-white/[0.08] text-ink-mute dark:text-ink-dark-mute border border-border/60 dark:border-border-dark/50"
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
  );
}
