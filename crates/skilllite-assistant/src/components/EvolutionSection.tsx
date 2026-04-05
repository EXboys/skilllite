import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
  prependNoMaterialHelpIfNeeded,
} from "../utils/evolutionDisplay";
import {
  openDetailWindow,
  parseDetailWorkspaceFromUrl,
  type EvolutionDetailTab,
} from "../utils/detailWindow";
import { buildAssistantBridgeConfig } from "../utils/buildAssistantBridgeConfig";
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
  judgement_label: string | null;
  judgement_reason: string | null;
  recent_events: EvolutionLogEntryDto[];
  pending_skill_count: number;
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

export interface EvolutionSnapshotTxnDto {
  txn_id: string;
  modified_unix: number;
}

/** Matches `skilllite_bridge::PROMPT_VERSION_CURRENT` */
const PROMPT_VERSION_CURRENT = "__current__";

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
  const [compareMode, setCompareMode] = useState(true);
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

  const selectCls =
    "min-w-0 flex-1 rounded border border-border/60 dark:border-border-dark/60 bg-white/80 dark:bg-paper-dark/80 px-2 py-1.5 text-[11px] font-mono text-ink dark:text-ink-dark";

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
        <div className="flex flex-col gap-2 sm:flex-row sm:items-end">
          <label className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="text-[10px] text-ink-mute dark:text-ink-dark-mute">{t("evolution.diff.versionLeft")}</span>
            <select
              className={selectCls}
              value={leftRef}
              onChange={(e) => setLeftRef(e.target.value)}
              aria-label={t("evolution.diff.versionLeft")}
            >
              <option value={PROMPT_VERSION_CURRENT}>{t("evolution.diff.currentWorkspace")}</option>
              {txnOptions}
            </select>
          </label>
          <label className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="text-[10px] text-ink-mute dark:text-ink-dark-mute">{t("evolution.diff.versionRight")}</span>
            <select
              className={selectCls}
              value={rightRef}
              onChange={(e) => setRightRef(e.target.value)}
              aria-label={t("evolution.diff.versionRight")}
            >
              <option value={PROMPT_VERSION_CURRENT}>{t("evolution.diff.currentWorkspace")}</option>
              {txnOptions}
            </select>
          </label>
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
        <>
          <div className="flex flex-wrap gap-1.5">
            <button
              type="button"
              onClick={() => setCompareMode(true)}
              className={`px-2 py-0.5 rounded text-[10px] border transition-colors ${
                compareMode
                  ? "border-accent/50 bg-accent/10 text-accent dark:text-blue-300"
                  : "border-border/50 dark:border-border-dark/50 text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
              }`}
            >
              {t("evolution.diff.toggleCompare")}
            </button>
            <button
              type="button"
              onClick={() => setCompareMode(false)}
              className={`px-2 py-0.5 rounded text-[10px] border transition-colors ${
                !compareMode
                  ? "border-accent/50 bg-accent/10 text-accent dark:text-blue-300"
                  : "border-border/50 dark:border-border-dark/50 text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
              }`}
            >
              {t("evolution.diff.togglePlain")}
            </button>
          </div>
          {compareMode ? (
            <PromptDiffView original={leftText} current={rightText} />
          ) : (
            <div className="grid gap-2 md:grid-cols-2">
              <pre className="max-h-64 overflow-y-auto whitespace-pre-wrap break-words rounded border border-border/40 dark:border-border-dark/40 bg-gray-50/80 dark:bg-surface-dark/50 p-2 font-mono text-[11px] text-ink-mute dark:text-ink-dark-mute">
                {leftText || "（空）"}
              </pre>
              <pre className="max-h-64 overflow-y-auto whitespace-pre-wrap break-words rounded border border-border/40 dark:border-border-dark/40 bg-gray-50/80 dark:bg-surface-dark/50 p-2 font-mono text-[11px] text-ink-mute dark:text-ink-dark-mute">
                {rightText || "（空）"}
              </pre>
            </div>
          )}
        </>
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

  const bridgeConfig = useMemo(() => buildAssistantBridgeConfig(settings), [settings]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await invoke<EvolutionStatusPayload>("skilllite_load_evolution_status", {
        workspace,
        config: bridgeConfig,
      });
      setStatus(s);
    } catch (e) {
      setError(String(e));
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, [workspace, bridgeConfig]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { status, loading, error, refresh, workspace };
}

/** 右侧面板摘要 */
export function EvolutionStatusSummary({ onOpenDetail }: { onOpenDetail: () => void }) {
  const { t } = useI18n();
  const { status, loading, error, refresh, workspace } = useEvolutionStatus();

  if (loading && !status) {
    return (
      <section className="mb-4 min-w-0">
        <div className="flex items-center justify-between mb-2">
          <span className="font-medium text-ink dark:text-ink-dark">自进化</span>
        </div>
        <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
      </section>
    );
  }

  if (error && !status) {
    return (
      <section className="mb-4 min-w-0">
        <div className="flex items-center justify-between mb-2">
          <span className="font-medium text-ink dark:text-ink-dark">自进化</span>
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
    <section className="mb-4 min-w-0">
      <div className="flex items-center justify-between gap-2 mb-2 min-w-0">
        <button
          type="button"
          onClick={onOpenDetail}
          className="flex-1 min-w-0 text-left font-medium text-ink dark:text-ink-dark group hover:text-accent dark:hover:text-accent"
        >
          <span>自进化</span>
          <span className="text-xs font-normal text-ink-mute group-hover:text-accent dark:text-ink-dark-mute dark:group-hover:text-accent inline-flex items-center gap-0.5 ml-0.5 transition-colors">
            详情与审核
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
          title="刷新"
          aria-label="刷新进化状态"
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
        className="max-w-full min-w-0 cursor-pointer rounded-lg border border-border/60 dark:border-border-dark/60 bg-gray-50/50 dark:bg-surface-dark/50 px-2.5 py-2 text-xs text-ink dark:text-ink-dark space-y-1.5 break-words"
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

  const tabBtnClass = (active: boolean) =>
    `flex-1 min-w-0 py-2 px-1.5 text-xs font-medium rounded-t-md border-b-2 transition-colors ${
      active
        ? "border-accent text-ink dark:text-ink-dark"
        : "border-transparent text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
    }`;

  const pendingCount = pending.length;
  const hasJudgement = Boolean(s?.judgement_label);

  return (
    <div className="space-y-4 p-1">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div
          role="tablist"
          aria-label="自进化详情分类"
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
            运行与队列
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
              审核
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
            className={tabBtnClass(detailTab === "changes")}
          >
            变更对比
          </button>
        </div>
        <button
          type="button"
          onClick={() => void refresh()}
          disabled={loading}
          className="text-xs text-accent hover:underline disabled:opacity-50 shrink-0"
        >
          {loading ? "刷新中…" : "刷新状态"}
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
                  <span className="text-ink-mute dark:text-ink-dark-mute">上次 evolution_run：</span>
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
                        const config = buildAssistantBridgeConfig(settings);
                        const out = await invoke<string>(
                          "skilllite_trigger_evolution_run",
                          {
                            workspace,
                            proposalId: row.proposal_id,
                            config,
                          }
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
              const targetShown = evolutionLogTargetLine(e.target_id, locale);
              const txnTrim = e.txn_id?.trim() ?? "";
              const rowOpensDiff =
                e.event_type === "evolution_run" && txnTrim.length > 0;
              const showMemoryLink = reasonMentionsMemoryKnowledge(e.reason);
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
                            setDetailTab("changes");
                            setCompareFocusTxn(txnTrim);
                          }}
                          title={t("evolution.log.rowOpenDiffTitle")}
                          className="w-full rounded-lg text-left px-1 py-0.5 -mx-1 transition-colors cursor-pointer hover:bg-ink/[0.06] dark:hover:bg-white/[0.06] focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
                        >
                          <div className="text-ink-mute dark:text-ink-dark-mute font-mono text-[11px]">
                            {formatTs(e.ts)}
                            <span className="ml-1.5 text-[10px] text-accent font-sans font-medium">
                              {t("evolution.log.rowOpenDiffBadge")}
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
                            <p className="text-ink-mute dark:text-ink-dark-mute mt-0.5 whitespace-pre-wrap break-words">
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
                            <p className="text-ink-mute dark:text-ink-dark-mute mt-0.5 whitespace-pre-wrap break-words">
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
            <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">进化变更对比</h2>
            <button
              type="button"
              onClick={() => void loadDiffs()}
              className="text-xs text-accent hover:underline"
            >
              刷新
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
                  {diffs.map((d) => (
                    <div
                      key={d.filename}
                      className={`rounded-lg border text-xs overflow-hidden ${
                        d.evolved
                          ? "bg-green-50/50 dark:bg-green-900/10 border-green-300/60 dark:border-green-700/40"
                          : "bg-gray-50/50 dark:bg-surface-dark/40 border-border/50 dark:border-border-dark/50"
                      }`}
                    >
                      <div className="flex items-center justify-between gap-2 px-3 py-2 border-b border-border/30 dark:border-border-dark/30 bg-gray-100/50 dark:bg-surface-dark/30">
                        <span className="font-mono font-medium text-ink dark:text-ink-dark">
                          {d.filename}
                        </span>
                        {d.evolved ? (
                          <span className="px-1.5 py-0.5 rounded text-[10px] bg-green-100 dark:bg-green-900/50 text-green-700 dark:text-green-400 border border-green-300/50 dark:border-green-600/50 shrink-0">
                            ✨ 进化
                          </span>
                        ) : null}
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
