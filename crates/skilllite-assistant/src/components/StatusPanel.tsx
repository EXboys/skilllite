import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import { useStatusStore, type LogEntry } from "../stores/useStatusStore";
import { useSettingsStore } from "../stores/useSettingsStore";
import {
  groupMemoryEntriesByTopDir,
  groupMemoryFiles,
  memoryPathUnderTopGroup,
  sortedMemoryGroupKeys,
} from "../utils/fileUtils";
import { openDetailWindow } from "../utils/detailWindow";
import { EvolutionStatusSummary } from "./EvolutionSection";
import { useLifePulse, type LifePulseActivity } from "../hooks/useLifePulse";
import { translate, useI18n } from "../i18n";
import { runWithScenarioFallback } from "../utils/llmScenarioFallback";

interface MemoryEntryData {
  path: string;
  title: string;
  summary: string;
  updated_at: string;
}

const PREVIEW_LIMIT = 3;

function LogList({ entries, limit }: { entries: LogEntry[]; limit?: number }) {
  const { t } = useI18n();
  if (entries.length === 0) {
    return (
      <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">{t("status.noLogs")}</p>
    );
  }
  const reversed = entries.slice().reverse();
  const show = limit ? reversed.slice(0, limit) : reversed;
  return (
    <ul className="space-y-1">
      {show.map((e) => (
        <li
          key={e.id}
          className={`min-w-0 max-w-full text-xs font-mono ${
            e.isError ? "text-red-600 dark:text-red-400" : "text-ink-mute dark:text-ink-dark-mute"
          }`}
        >
          <span className="text-ink-mute/80 dark:text-ink-dark-mute/80">[{e.time}]</span>{" "}
          {e.type === "tool_call" && "→"}
          {e.type === "command_started" && "▶"}
          {e.type === "tool_result" && (e.isError ? "✗" : "✓")}
          {e.type === "command_output" && (e.isError ? "!" : "│")}
          {e.type === "command_finished" && (e.isError ? "✗" : "■")}
          {e.type === "preview_started" && "▶"}
          {e.type === "preview_ready" && "■"}
          {e.type === "preview_failed" && "✗"}
          {e.type === "preview_stopped" && "■"}
          {e.type === "swarm_started" && "▶"}
          {e.type === "swarm_progress" && "…"}
          {e.type === "swarm_finished" && "■"}
          {e.type === "swarm_failed" && "✗"}
          {e.type === "llm_usage" && "∑"}
          {e.name && <span className="font-medium">{e.name}: </span>}
          <span className="block min-w-0 break-words text-left line-clamp-4">{e.text}</span>
        </li>
      ))}
    </ul>
  );
}

function LogFilePreview({ files, entries, limit = 3 }: { files: string[]; entries: LogEntry[]; limit?: number }) {
  const { t } = useI18n();
  const hasFiles = files.length > 0;
  const hasEntries = entries.length > 0;
  if (!hasFiles && !hasEntries) {
    return (
      <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">{t("status.noLogs")}</p>
    );
  }
  const show = limit ? files.slice(0, limit) : files;
  return (
    <div className="min-w-0 space-y-1">
      {hasFiles && (
        <ul className="min-w-0 space-y-0.5">
          {show.map((f, i) => (
            <li
              key={i}
              className="flex min-w-0 max-w-full items-center gap-1 truncate text-xs text-ink-mute dark:text-ink-dark-mute"
            >
              <span className="shrink-0">📄</span>
              <span className="min-w-0 truncate">{f}</span>
            </li>
          ))}
          {files.length > limit && (
            <li className="text-xs text-ink-mute dark:text-ink-dark-mute truncate pt-1">
              {t("status.moreFiles", { n: files.length - limit })}
            </li>
          )}
        </ul>
      )}
      {hasEntries && (
        <div className={hasFiles ? "pt-2 border-t border-border/50 dark:border-border-dark/50" : ""}>
          {hasFiles && (
            <div className="text-[10px] font-medium uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute mb-1">
              {t("detail.liveLog")}
            </div>
          )}
          <LogList entries={entries} limit={limit} />
        </div>
      )}
    </div>
  );
}

function OutputPreview({ files, limit = 3 }: { files: string[]; limit?: number }) {
  const { t } = useI18n();
  if (files.length === 0) {
    return (
      <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">{t("status.noOutput")}</p>
    );
  }
  const show = limit ? files.slice(0, limit) : files;
  return (
    <ul className="min-w-0 space-y-0.5">
      {show.map((f, i) => (
        <li
          key={i}
          className="flex min-w-0 max-w-full items-center gap-1 truncate text-xs text-ink-mute dark:text-ink-dark-mute"
        >
          <span className="shrink-0">📄</span>
          <span className="min-w-0 truncate">{f.split("/").pop() ?? f}</span>
        </li>
      ))}
      {files.length > limit && (
        <li className="text-xs text-ink-mute dark:text-ink-dark-mute truncate pt-1">
          {t("status.moreFiles", { n: files.length - limit })}
        </li>
      )}
    </ul>
  );
}

function MemoryPreview({ files, hints, limit }: { files: string[]; hints: string[]; limit?: number }) {
  const { t } = useI18n();
  const [summaries, setSummaries] = useState<MemoryEntryData[]>([]);

  useEffect(() => {
    invoke<MemoryEntryData[]>("skilllite_load_memory_summaries")
      .then(setSummaries)
      .catch(() => setSummaries([]));
  }, [files.length]);

  const hasSummaries = summaries.length > 0;
  const hasFiles = files.length > 0;
  const hasHints = hints.length > 0;

  if (!hasSummaries && !hasFiles && !hasHints) {
    return (
      <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">{t("status.noMemory")}</p>
    );
  }

  if (hasSummaries) {
    const lim = limit ?? summaries.length;
    const groupedAll = groupMemoryEntriesByTopDir(summaries);
    const keysAll = sortedMemoryGroupKeys(groupedAll);
    const showEntries: MemoryEntryData[] = [];
    outer: for (const k of keysAll) {
      for (const e of groupedAll[k]!) {
        showEntries.push(e);
        if (showEntries.length >= lim) break outer;
      }
    }
    const showGrouped = groupMemoryEntriesByTopDir(showEntries);
    const showKeys = sortedMemoryGroupKeys(showGrouped);
    const shownCount = showEntries.length;

    return (
      <div className="min-w-0 space-y-2">
        {showKeys.map((group) => (
          <div key={group} className="min-w-0 space-y-1">
            {group !== "." && (
              <div className="text-[10px] font-medium uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute truncate">
                {group}/
              </div>
            )}
            <div className="min-w-0 space-y-1.5">
              {showGrouped[group]!.map((entry) => {
                const subPath = memoryPathUnderTopGroup(entry.path);
                return (
                <div
                  key={entry.path}
                  className="max-w-full min-w-0 rounded-md border border-border/50 dark:border-border-dark/50 bg-white/50 px-2.5 py-1.5 dark:bg-white/[0.02]"
                  title={entry.path}
                >
                  <div className="text-xs font-medium text-ink dark:text-ink-dark truncate">{entry.title}</div>
                  {subPath.includes("/") && (
                    <div className="text-[10px] text-ink-mute/90 dark:text-ink-dark-mute/90 truncate mt-0.5 font-mono">
                      {subPath}
                    </div>
                  )}
                  {entry.summary && (
                    <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute mt-0.5 line-clamp-2 break-words">
                      {entry.summary}
                    </p>
                  )}
                </div>
                );
              })}
            </div>
          </div>
        ))}
        {summaries.length > shownCount && (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute">
            {t("status.memoryMoreEntries", {
              n: summaries.length - shownCount,
            })}
          </p>
        )}
        {hasHints && limit && (
          <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute pt-0.5">
            {t("status.recentHintsCount", { n: hints.length })}
          </p>
        )}
      </div>
    );
  }

  const groups = groupMemoryFiles(files);
  const keys = sortedMemoryGroupKeys(groups);
  const lim = limit ?? Number.POSITIVE_INFINITY;
  let remaining = lim;
  return (
    <div className="min-w-0 space-y-2">
      {keys.map((group) => {
        const paths = groups[group]!;
        const showPaths = limit ? paths.slice(0, Math.max(0, remaining)) : paths;
        if (limit) remaining -= showPaths.length;
        if (showPaths.length === 0) return null;
        return (
          <div key={group} className="min-w-0 space-y-0.5">
            {group !== "." && (
              <div className="text-[10px] font-medium uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute truncate">
                {group}/
              </div>
            )}
            <ul className="min-w-0 space-y-0.5">
              {showPaths.map((f) => {
                const subPath = memoryPathUnderTopGroup(f);
                return (
                <li
                  key={f}
                  className="flex min-w-0 max-w-full flex-col gap-0.5 text-xs text-ink-mute dark:text-ink-dark-mute"
                  title={f}
                >
                  <span className="flex min-w-0 items-center gap-1 truncate">
                    <span className="shrink-0">📄</span>
                    <span className="min-w-0 truncate">{f.split("/").pop() ?? f}</span>
                  </span>
                  {subPath.includes("/") && (
                    <span className="min-w-0 truncate pl-5 text-[10px] font-mono opacity-90">{subPath}</span>
                  )}
                </li>
                );
              })}
            </ul>
          </div>
        );
      })}
      {limit && files.length > limit && (
        <p className="text-xs text-ink-mute dark:text-ink-dark-mute pt-0.5">
          {t("status.moreFiles", { n: files.length - limit })}
        </p>
      )}
      {hasHints && limit && (
        <p className="text-xs text-ink-mute dark:text-ink-dark-mute truncate pt-1">
          {t("status.recentHintsCount", { n: hints.length })}
        </p>
      )}
    </div>
  );
}

/** 摘要区块，点击打开全新窗口 */
function SummarySection({
  title,
  onClickMore,
  onOpenDir,
  hasMore,
  children,
}: {
  title: string;
  onClickMore: () => void;
  onOpenDir?: () => void;
  hasMore: boolean;
  children: React.ReactNode;
}) {
  const { t } = useI18n();
  return (
    <section className="mb-4 min-w-0">
      <div className="flex items-center justify-between gap-2 mb-2 min-w-0">
        <button
          type="button"
          onClick={onClickMore}
          className="flex-1 min-w-0 text-left font-medium text-ink dark:text-ink-dark group hover:text-accent dark:hover:text-accent"
        >
          <span>{title}</span>
          {hasMore && (
            <span className="text-xs font-normal text-ink-mute group-hover:text-accent dark:text-ink-dark-mute dark:group-hover:text-accent inline-flex items-center gap-0.5 ml-0.5 transition-colors">
              {t("status.more")}
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M9 18l6-6-6-6" />
              </svg>
            </span>
          )}
        </button>
        {onOpenDir && (
          <button
            type="button"
            onClick={(e) => { e.stopPropagation(); onOpenDir(); }}
            className="p-1.5 shrink-0 text-ink-mute hover:text-ink dark:hover:text-ink-dark rounded hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            aria-label={t("status.openFolderAria")}
            title={t("status.openInFmTitle")}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
            </svg>
          </button>
        )}
      </div>
      <div
        onClick={hasMore ? onClickMore : undefined}
        role={hasMore ? "button" : undefined}
        className={hasMore ? "min-w-0 cursor-pointer" : "min-w-0"}
      >
        {children}
      </div>
    </section>
  );
}

type StatusPanelTab = "evolution" | "archive";

function formatPulseTime(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts * 1000);
  return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
}

/** Compact pulse dot + label for the top header bar. */
export function LifePulseBadge() {
  const { t } = useI18n();
  const { status, activities, chatting, toggle, setWorkspace } = useLifePulse();
  const { settings } = useSettingsStore();

  useEffect(() => {
    const ws = settings.workspace?.trim() || ".";
    void setWorkspace(ws);
    void runWithScenarioFallback<unknown>(
      settings,
      "lifePulse",
      (config) =>
        invoke("skilllite_life_pulse_set_llm_overrides", { config })
    ).catch((e) => {
      console.warn("[skilllite-assistant] life pulse LLM sync failed:", e);
    });
  }, [
    settings.apiKey,
    settings.apiBase,
    settings.model,
    settings.workspace,
    settings.locale,
    settings.sandboxLevel,
    settings.swarmEnabled,
    settings.swarmUrl,
    settings.maxIterations,
    settings.maxToolCallsPerTask,
    settings.evolutionIntervalSecs,
    settings.evolutionDecisionThreshold,
    settings.evoProfile,
    settings.evoCooldownHours,
    settings.llmScenarioRoutingEnabled,
    settings.llmScenarioRoutes,
    setWorkspace,
  ]);

  if (!status) return null;

  const isGrowing = status.growth_running;
  const isRunning = status.rhythm_running;
  const isActive = status.enabled && status.alive;
  const isBusy = chatting || isGrowing || isRunning;

  let dotColor = "bg-gray-400 dark:bg-gray-600";
  let label = t("lifePulse.sleeping");
  if (!status.enabled) {
    dotColor = "bg-gray-400 dark:bg-gray-600";
    label = t("lifePulse.sleeping");
  } else if (chatting) {
    dotColor = "bg-violet-500";
    label = t("lifePulse.chatting");
  } else if (isGrowing) {
    dotColor = "bg-emerald-500";
    label = t("lifePulse.growing");
  } else if (isRunning) {
    dotColor = "bg-amber-500";
    label = t("lifePulse.working");
  } else if (isActive) {
    dotColor = "bg-sky-500";
    label = t("lifePulse.idle");
  }

  const latest: LifePulseActivity | undefined = activities[0];

  return (
    <div className="flex items-center gap-2">
      {/* Dot + label + latest activity */}
      <button
        type="button"
        onClick={() => toggle(!status.enabled)}
        className="flex items-center gap-1.5 px-2 py-1 rounded-md text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
        title={status.enabled ? t("lifePulse.toggleRest") : t("lifePulse.toggleWake")}
      >
        <span className="relative flex h-2 w-2 shrink-0">
          {isActive && isBusy && (
            <span
              className={`absolute inset-0 rounded-full ${dotColor} opacity-75 animate-ping`}
            />
          )}
          <span className={`relative inline-flex h-2 w-2 rounded-full ${dotColor}`} />
        </span>
        <span className="text-[11px] font-medium">{label}</span>
      </button>
      {latest && (
        <span className="hidden sm:inline-flex items-center gap-1 text-[11px] text-ink-mute/70 dark:text-ink-dark-mute/70 max-w-[180px] truncate">
          <span className="tabular-nums">{formatPulseTime(latest.ts)}</span>
          <span className="truncate">{latest.label}</span>
        </span>
      )}
    </div>
  );
}

export default function StatusPanel() {
  const { t } = useI18n();
  const [tab, setTab] = useState<StatusPanelTab>("evolution");
  const { logEntries, logFiles, memoryHints, memoryFiles, outputFiles, rollLlmUsageMonthIfNeeded } =
    useStatusStore();
  const { settings } = useSettingsStore();
  const workspace = settings.workspace?.trim() || ".";

  useEffect(() => {
    rollLlmUsageMonthIfNeeded();
  }, [rollLlmUsageMonthIfNeeded]);

  const openDir = (module: string) => () => {
    invoke("skilllite_open_directory", { module, workspace }).catch((err) => {
      console.error("[skilllite-assistant] skilllite_open_directory failed:", err);
      useUiToastStore
        .getState()
        .show(
          translate("status.openDirFailed", {
            module,
            err: formatInvokeError(err),
          }),
          "error"
        );
    });
  };

  const memHasMore = memoryFiles.length > PREVIEW_LIMIT || memoryHints.length > 0 || memoryFiles.length > 0;
  const logHasMore = logFiles.length > 0 || logEntries.length > PREVIEW_LIMIT || logEntries.length > 0;
  const outputHasMore = outputFiles.length > PREVIEW_LIMIT || outputFiles.length > 0;

  const tabBtnClass = (active: boolean) =>
    `flex-1 min-w-0 py-2 px-1 text-xs font-medium rounded-t-md border-b-2 transition-colors ${
      active
        ? "border-accent text-ink dark:text-ink-dark"
        : "border-transparent text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
    }`;

  return (
    <div className="box-border flex h-full min-h-0 w-full min-w-0 max-w-full flex-col p-4 text-sm break-words">
      <div
        role="tablist"
        aria-label={t("status.panelAria")}
        className="mb-1 flex shrink-0 gap-0 border-b border-border/80 dark:border-border-dark/80"
      >
        <button
          type="button"
          role="tab"
          id="status-tab-trigger-evolution"
          aria-controls="status-tab-panel-evolution"
          aria-selected={tab === "evolution"}
          tabIndex={0}
          onClick={() => setTab("evolution")}
          className={tabBtnClass(tab === "evolution")}
        >
          {t("status.tabEvolution")}
        </button>
        <button
          type="button"
          role="tab"
          id="status-tab-trigger-archive"
          aria-controls="status-tab-panel-archive"
          aria-selected={tab === "archive"}
          tabIndex={0}
          onClick={() => setTab("archive")}
          className={tabBtnClass(tab === "archive")}
        >
          {t("status.tabArchive")}
        </button>
      </div>

      {tab === "evolution" && (
        <div
          role="tabpanel"
          id="status-tab-panel-evolution"
          aria-labelledby="status-tab-trigger-evolution"
          className="flex min-h-0 min-w-0 flex-1 flex-col pt-3"
        >
          <EvolutionStatusSummary
            onOpenDetail={() => openDetailWindow("evolution")}
            className="flex-1 min-h-0 mb-0"
            metricsClassName="flex-1 min-h-[min(42vh,16rem)] justify-between"
          />
        </div>
      )}

      {tab === "archive" && (
        <div
          role="tabpanel"
          id="status-tab-panel-archive"
          aria-labelledby="status-tab-trigger-archive"
          className="min-h-0 min-w-0 flex-1 overflow-y-auto pt-3"
        >
          <SummarySection
            title={t("status.memory")}
            onClickMore={() => openDetailWindow("mem")}
            onOpenDir={openDir("memory")}
            hasMore={memHasMore || memoryFiles.length > 0}
          >
            <MemoryPreview files={memoryFiles} hints={memoryHints} limit={PREVIEW_LIMIT} />
          </SummarySection>

          <SummarySection
            title={t("status.log")}
            onClickMore={() => openDetailWindow("log")}
            onOpenDir={openDir("log")}
            hasMore={logHasMore}
          >
            <LogFilePreview files={logFiles} entries={logEntries} limit={PREVIEW_LIMIT} />
          </SummarySection>

          <SummarySection
            title={t("status.output")}
            onClickMore={() => openDetailWindow("output")}
            onOpenDir={openDir("output")}
            hasMore={outputHasMore}
          >
            <OutputPreview files={outputFiles} limit={PREVIEW_LIMIT} />
          </SummarySection>
        </div>
      )}
    </div>
  );
}
