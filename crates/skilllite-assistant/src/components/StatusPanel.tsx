import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import { open as openUrl } from "@tauri-apps/plugin-shell";
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
import { getLocale, translate, useI18n } from "../i18n";
import { buildAssistantBridgeConfig } from "../utils/buildAssistantBridgeConfig";

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
      {hasEntries && !hasFiles && (
        <LogList entries={entries} limit={limit} />
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

const openDir = (module: string) => () => {
  invoke("skilllite_open_directory", { module }).catch((err) => {
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

const SKILL_LIST_MAX_HEIGHT = 200;

/** 将 skill 名称转为短标签（如 xiaohongshu-writer → 小红书）；英文界面保留原名 */
function skillDisplayName(name: string, locale: "zh" | "en"): string {
  if (locale === "en") return name;
  const map: Record<string, string> = {
    "xiaohongshu-writer": "小红书",
    "check-weather-forecast": "天气预报",
    "data-analysis": "数据分析",
    "http-request": "HTTP 请求",
    "nodejs-test": "Node 测试",
    "skill-creator": "技能创建",
    "text-processor": "文本处理",
    weather: "天气",
    calculator: "计算器",
  };
  return map[name] || name;
}

const SKILLS_SH_URL = "https://skills.sh/";

function SkillRepairSection() {
  const { t } = useI18n();
  const locale = getLocale();
  const { settings } = useSettingsStore();
  const workspace = settings.workspace || ".";
  const [skillNames, setSkillNames] = useState<string[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loadingList, setLoadingList] = useState(false);
  const [repairing, setRepairing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [repairResult, setRepairResult] = useState<string | null>(null);
  const [resultIsError, setResultIsError] = useState(false);
  const [addSource, setAddSource] = useState("");
  const [adding, setAdding] = useState(false);
  const [addResult, setAddResult] = useState<string | null>(null);
  const [addResultIsError, setAddResultIsError] = useState(false);
  const [initializing, setInitializing] = useState(false);
  const [initError, setInitError] = useState<string | null>(null);

  const loadSkills = useCallback(async () => {
    setLoadingList(true);
    setRepairResult(null);
    try {
      const names = await invoke<string[]>("skilllite_list_skills", { workspace });
      setSkillNames(names);
      setSelected(new Set());
    } catch (e) {
      console.error("[skilllite-assistant] skilllite_list_skills failed:", e);
      setSkillNames([]);
    } finally {
      setLoadingList(false);
    }
  }, [workspace]);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  const toggleOne = (name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const selectAll = () => setSelected(new Set(skillNames));
  const selectNone = () => setSelected(new Set());

  const runRepair = async () => {
    setRepairing(true);
    setRepairResult(null);
    setResultIsError(false);
    try {
      const toRepair = selected.size > 0 ? Array.from(selected) : [];
      const out = await invoke<string>("skilllite_repair_skills", {
        workspace,
        skillNames: toRepair,
      });
      setRepairResult(out || t("status.repairComplete"));
    } catch (e) {
      setRepairResult(String(e));
      setResultIsError(true);
    } finally {
      setRepairing(false);
    }
  };

  const runDelete = async () => {
    if (selected.size === 0) {
      useUiToastStore.getState().show(t("status.deleteSkillNeedSelect"), "error");
      return;
    }
    const n = selected.size;
    if (!window.confirm(t("status.deleteSkillConfirm", { n }))) return;
    setDeleting(true);
    setRepairResult(null);
    setResultIsError(false);
    try {
      const names = Array.from(selected);
      const out = await invoke<string>("skilllite_remove_skills", {
        workspace,
        skillNames: names,
      });
      setRepairResult(out || t("status.deleteSkill"));
      setSelected(new Set());
      loadSkills();
    } catch (e) {
      setRepairResult(String(e));
      setResultIsError(true);
    } finally {
      setDeleting(false);
    }
  };

  const runAdd = async () => {
    const source = addSource.trim();
    if (!source) return;
    setAdding(true);
    setAddResult(null);
    setAddResultIsError(false);
    try {
      const out = await invoke<string>("skilllite_add_skill", {
        workspace,
        source,
        force: false,
      });
      setAddResult(out || t("status.skillAdded"));
      setAddSource("");
      loadSkills();
    } catch (e) {
      setAddResult(String(e));
      setAddResultIsError(true);
    } finally {
      setAdding(false);
    }
  };

  const runInitSkills = async () => {
    setInitializing(true);
    setInitError(null);
    try {
      await invoke("skilllite_init_workspace", { dir: workspace });
      await loadSkills();
    } catch (e) {
      setInitError(String(e));
    } finally {
      setInitializing(false);
    }
  };

  return (
    <section className="mb-4 min-w-0">
      {/* 标题行：技能 + 数量 + 操作 */}
      <div className="flex flex-wrap items-center justify-between gap-x-2 gap-y-1 mb-2 min-w-0">
        <span className="font-medium text-ink dark:text-ink-dark shrink-0">{t("status.skills")}</span>
        {skillNames.length > 0 && (
          <span className="text-xs text-ink-mute dark:text-ink-dark-mute shrink-0">
            {t("status.countSkills", { n: skillNames.length })}
          </span>
        )}
        <div className="flex items-center gap-0.5 min-w-0">
          <button
            type="button"
            onClick={loadSkills}
            disabled={loadingList}
            className="p-1.5 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 transition-colors"
            title={t("status.refresh")}
            aria-label={t("status.refresh")}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={loadingList ? "animate-spin" : ""}>
              <path d="M21 2v6h-6" />
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
              <path d="M3 22v-6h6" />
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
            </svg>
          </button>
          <button
            type="button"
            onClick={selectAll}
            disabled={skillNames.length === 0}
            className="text-xs px-1.5 py-1 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 transition-colors"
            title={t("status.selectAll")}
          >
            {t("status.selectAll")}
          </button>
          <button
            type="button"
            onClick={selectNone}
            className="text-xs px-1.5 py-1 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            title={t("status.selectNoneTitle")}
          >
            {t("status.deselect")}
          </button>
        </div>
      </div>

      {/* 添加技能：来源输入 + 添加按钮 + skills.sh 链接 */}
      <div className="mb-2 flex flex-col gap-1.5">
        <div className="flex gap-1.5">
          <input
            type="text"
            value={addSource}
            onChange={(e) => setAddSource(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && runAdd()}
            placeholder={t("status.skillPlaceholder")}
            className="flex-1 min-w-0 rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-2.5 py-1.5 text-xs placeholder:text-ink-mute dark:placeholder:text-ink-dark-mute"
          />
          <button
            type="button"
            onClick={runAdd}
            disabled={adding || !addSource.trim()}
            className="shrink-0 px-2.5 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {adding ? t("status.addingSkill") : t("status.addSkill")}
          </button>
        </div>
        <div className="flex items-center gap-2 flex-wrap">
          <button
            type="button"
            onClick={() => openUrl(SKILLS_SH_URL)}
            className="text-xs text-ink-mute dark:text-ink-dark-mute hover:text-accent hover:underline text-left"
          >
            {t("status.browseSkillsSh")}
          </button>
          {addResult != null && (
            <span
              className={`min-w-0 max-w-full break-words text-xs ${addResultIsError ? "text-red-600 dark:text-red-400" : "text-ink-mute dark:text-ink-dark-mute"}`}
            >
              {addResult}
            </span>
          )}
        </div>
      </div>

      {/* 技能列表：卡片式 */}
      <div
        className="min-w-0 max-w-full rounded-lg border border-border dark:border-border-dark bg-gray-50/50 dark:bg-surface-dark/50 overflow-x-hidden overflow-y-auto mb-3"
        style={{ maxHeight: SKILL_LIST_MAX_HEIGHT }}
      >
        {loadingList ? (
          <div className="p-4 flex items-center justify-center gap-2 text-xs text-ink-mute dark:text-ink-dark-mute">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="animate-spin shrink-0">
              <path d="M21 12a9 9 0 1 1-6.219-8.56" />
            </svg>
            {t("common.loading")}
          </div>
        ) : skillNames.length === 0 ? (
          <div className="p-4 text-xs text-ink-mute dark:text-ink-dark-mute text-center leading-relaxed">
            <p>{t("status.noSkillsFound")}</p>
            <p className="text-[11px] mt-1 mb-2">{t("status.noSkillsHint")}</p>
            <button
              type="button"
              onClick={runInitSkills}
              disabled={initializing}
              className="px-3 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {initializing ? t("status.initializing") : t("status.initSkills")}
            </button>
            {initError && (
              <p className="mt-2 text-xs text-red-600 dark:text-red-400 break-words text-left">{initError}</p>
            )}
          </div>
        ) : (
          <ul className="p-1.5 space-y-0.5">
            {skillNames.map((name) => (
              <li key={name}>
                <label
                  title={name}
                  className={`flex items-center gap-2.5 px-2.5 py-1.5 rounded-md cursor-pointer transition-colors ${
                    selected.has(name)
                      ? "bg-accent/10 dark:bg-accent/20 text-accent dark:text-accent"
                      : "hover:bg-ink/5 dark:hover:bg-white/5 text-ink dark:text-ink-dark"
                  }`}
                >
                  <input
                    type="checkbox"
                    checked={selected.has(name)}
                    onChange={() => toggleOne(name)}
                    className="rounded border-border dark:border-border-dark text-accent focus:ring-accent/40 shrink-0"
                  />
                  <span className="truncate text-xs font-medium flex-1 min-w-0">
                    {skillDisplayName(name, locale)}
                  </span>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      invoke("skilllite_open_skill_directory", { workspace, skillName: name }).catch((err) => {
                        console.error("[skilllite-assistant] open_skill_directory failed:", err);
                        const msg = formatInvokeError(err);
                        setRepairResult(translate("status.openSkillDirResult", { err: msg }));
                        setResultIsError(true);
                        useUiToastStore
                          .getState()
                          .show(translate("status.openSkillDirFailed", { err: msg }), "error");
                      });
                    }}
                    className="p-1 rounded text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 shrink-0"
                    title={t("status.openFolder")}
                    aria-label={t("status.openFolderAria")}
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                    </svg>
                  </button>
                </label>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* 修复按钮 */}
      <button
        type="button"
        onClick={runRepair}
        disabled={repairing || deleting || skillNames.length === 0}
        className="w-full text-sm px-3 py-2 rounded-lg bg-accent text-white font-medium hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {repairing
          ? t("status.repairing")
          : selected.size > 0
            ? t("status.repairSelected", { n: selected.size })
            : t("status.repairAll")}
      </button>

      <button
        type="button"
        onClick={runDelete}
        disabled={deleting || repairing || skillNames.length === 0 || selected.size === 0}
        className="w-full mt-2 text-sm px-3 py-2 rounded-lg border border-red-300 dark:border-red-800/80 text-red-700 dark:text-red-300 font-medium hover:bg-red-50 dark:hover:bg-red-950/30 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {deleting
          ? t("status.deletingSkills")
          : selected.size > 0
            ? t("status.deleteSelected", { n: selected.size })
            : t("status.deleteSkill")}
      </button>

      {/* 修复结果 */}
      {repairResult !== null && (
        <div
          className={`mt-2 max-w-full min-w-0 break-words p-2.5 rounded-lg text-xs whitespace-pre-wrap max-h-28 overflow-y-auto overflow-x-hidden ${
            resultIsError
              ? "bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 border border-red-200 dark:border-red-800/50"
              : "bg-green-50 dark:bg-green-900/20 text-green-800 dark:text-green-200 border border-green-200 dark:border-green-800/50"
          }`}
        >
          {repairResult}
        </div>
      )}
    </section>
  );
}

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
    void invoke("skilllite_life_pulse_set_llm_overrides", {
      config: buildAssistantBridgeConfig(settings),
    }).catch((e) => {
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
  const { logEntries, logFiles, memoryHints, memoryFiles, outputFiles } = useStatusStore();

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
    <div className="box-border w-full min-w-0 max-w-full p-4 text-sm break-words">
      <div
        role="tablist"
        aria-label={t("status.panelAria")}
        className="flex gap-0 mb-1 border-b border-border/80 dark:border-border-dark/80"
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
          className="min-w-0 pt-3"
        >
          <EvolutionStatusSummary onOpenDetail={() => openDetailWindow("evolution")} />
          <SkillRepairSection />
        </div>
      )}

      {tab === "archive" && (
        <div
          role="tabpanel"
          id="status-tab-panel-archive"
          aria-labelledby="status-tab-trigger-archive"
          className="min-w-0 pt-3"
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
