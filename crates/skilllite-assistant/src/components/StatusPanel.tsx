import { useState, useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { useStatusStore, type TaskItem, type LogEntry } from "../stores/useStatusStore";
import { groupMemoryFiles } from "../utils/fileUtils";

export type DetailModule = "plan" | "mem" | "log" | "output" | null;

const MODULE_TITLES: Record<NonNullable<DetailModule>, string> = {
  plan: "任务计划",
  mem: "记忆",
  log: "执行日志",
  output: "输出",
};

/** 打开全新窗口显示详情，紧贴当前窗口右侧 */
async function openDetailWindow(module: NonNullable<DetailModule>) {
  const mainWindow = getCurrentWindow();
  const [pos, size] = await Promise.all([
    mainWindow.outerPosition(),
    mainWindow.outerSize(),
  ]);
  const x = pos.x + size.width;
  const y = pos.y;

  const label = `detail-${module}`;
  const base = `${window.location.origin}${window.location.pathname || "/"}`.replace(/\/$/, "");
  const url = `${base}#detail/${module}`;
  new WebviewWindow(label, {
    url,
    title: MODULE_TITLES[module],
    x: Math.round(x),
    y: Math.round(y),
    width: 420,
    height: 560,
    resizable: true,
  });
}

const PREVIEW_LIMIT = 3;

function TaskList({ tasks, limit }: { tasks: TaskItem[]; limit?: number }) {
  if (tasks.length === 0) {
    return (
      <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">暂无任务计划</p>
    );
  }
  const show = limit ? tasks.slice(0, limit) : tasks;
  return (
    <ul className="space-y-1">
      {show.map((t) => (
        <li
          key={t.id}
          className={`flex items-start gap-2 text-xs ${
            t.completed ? "text-ink-mute dark:text-ink-dark-mute line-through" : "text-ink dark:text-ink-dark-mute"
          }`}
        >
          <span className="shrink-0 mt-0.5 text-accent">
            {t.completed ? "✓" : "○"}
          </span>
          <span>{t.description}</span>
          {t.tool_hint && (
            <span className="text-ink-mute dark:text-ink-dark-mute shrink-0">
              [{t.tool_hint}]
            </span>
          )}
        </li>
      ))}
    </ul>
  );
}

function LogList({ entries, limit }: { entries: LogEntry[]; limit?: number }) {
  if (entries.length === 0) {
    return (
      <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">暂无日志</p>
    );
  }
  const reversed = entries.slice().reverse();
  const show = limit ? reversed.slice(0, limit) : reversed;
  return (
    <ul className="space-y-1">
      {show.map((e) => (
        <li
          key={e.id}
          className={`text-xs font-mono ${
            e.isError ? "text-red-600 dark:text-red-400" : "text-ink-mute dark:text-ink-dark-mute"
          }`}
        >
          <span className="text-ink-mute/80 dark:text-ink-dark-mute/80">[{e.time}]</span>{" "}
          {e.type === "tool_call" && "→"}
          {e.type === "tool_result" && (e.isError ? "✗" : "✓")}
          {e.name && <span className="font-medium">{e.name}: </span>}
          <span className="truncate block">{e.text}</span>
        </li>
      ))}
    </ul>
  );
}

function OutputPreview({ files, limit = 3 }: { files: string[]; limit?: number }) {
  if (files.length === 0) {
    return (
      <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">暂无输出文件</p>
    );
  }
  const show = limit ? files.slice(0, limit) : files;
  return (
    <ul className="space-y-0.5">
      {show.map((f, i) => (
        <li key={i} className="text-xs text-ink-mute dark:text-ink-dark-mute truncate flex items-center gap-1">
          <span className="shrink-0">📄</span>
          <span className="truncate">{f.split("/").pop() ?? f}</span>
        </li>
      ))}
      {files.length > limit && (
        <li className="text-xs text-ink-mute dark:text-ink-dark-mute truncate pt-1">
          + {files.length - limit} 个文件
        </li>
      )}
    </ul>
  );
}

function MemoryPreview({ files, hints, limit }: { files: string[]; hints: string[]; limit?: number }) {
  const hasFiles = files.length > 0;
  const hasHints = hints.length > 0;
  if (!hasFiles && !hasHints) {
    return (
      <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">暂无记忆</p>
    );
  }
  const flatFiles = Object.values(groupMemoryFiles(files)).flat();
  const showFiles = limit ? flatFiles.slice(0, limit) : flatFiles;
  return (
    <ul className="space-y-0.5">
      {showFiles.map((f, i) => (
        <li key={i} className="text-xs text-ink-mute dark:text-ink-dark-mute truncate flex items-center gap-1">
          <span className="shrink-0">📄</span>
          <span className="truncate">{f.split("/").pop() ?? f}</span>
        </li>
      ))}
      {hasHints && limit && (
        <li className="text-xs text-ink-mute dark:text-ink-dark-mute truncate pt-1">
          + {hints.length} 条最近操作
        </li>
      )}
    </ul>
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
  return (
    <section className="mb-4">
      <div className="flex items-center justify-between mb-2">
        <button
          type="button"
          onClick={onClickMore}
          className="flex-1 min-w-0 text-left font-medium text-ink dark:text-ink-dark group hover:text-accent dark:hover:text-accent"
        >
          <span>{title}</span>
          {hasMore && (
            <span className="text-xs font-normal text-ink-mute group-hover:text-accent dark:text-ink-dark-mute dark:group-hover:text-accent inline-flex items-center gap-0.5 ml-0.5 transition-colors">
              更多
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
            aria-label="打开目录"
            title="在文件管理器中打开"
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
        className={hasMore ? "cursor-pointer" : ""}
      >
        {children}
      </div>
    </section>
  );
}

const openDir = (module: string) => () => {
  invoke("skilllite_open_directory", { module }).catch((err) => {
    console.error("[skilllite-assistant] skilllite_open_directory failed:", err);
  });
};

const SKILL_LIST_MAX_HEIGHT = 180;

function SkillRepairSection() {
  const [skillNames, setSkillNames] = useState<string[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loadingList, setLoadingList] = useState(false);
  const [repairing, setRepairing] = useState(false);
  const [repairResult, setRepairResult] = useState<string | null>(null);

  const loadSkills = useCallback(async () => {
    setLoadingList(true);
    setRepairResult(null);
    try {
      const names = await invoke<string[]>("skilllite_list_skills", { workspace: null });
      setSkillNames(names);
      setSelected(new Set());
    } catch (e) {
      console.error("[skilllite-assistant] skilllite_list_skills failed:", e);
      setSkillNames([]);
    } finally {
      setLoadingList(false);
    }
  }, []);

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
    try {
      const toRepair = selected.size > 0 ? Array.from(selected) : [];
      const out = await invoke<string>("skilllite_repair_skills", {
        workspace: null,
        skillNames: toRepair,
      });
      setRepairResult(out || "完成");
    } catch (e) {
      setRepairResult(String(e));
    } finally {
      setRepairing(false);
    }
  };

  return (
    <section className="mb-4">
      <div className="flex items-center justify-between mb-2">
        <span className="font-medium text-ink dark:text-ink-dark">技能修复</span>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={loadSkills}
            disabled={loadingList}
            className="text-xs px-2 py-1 rounded text-ink-mute hover:text-ink dark:text-ink-dark-mute dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50"
          >
            刷新
          </button>
          <button
            type="button"
            onClick={selectAll}
            className="text-xs px-2 py-1 rounded text-ink-mute hover:text-ink dark:text-ink-dark-mute dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5"
          >
            全选
          </button>
          <button
            type="button"
            onClick={selectNone}
            className="text-xs px-2 py-1 rounded text-ink-mute hover:text-ink dark:text-ink-dark-mute dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5"
          >
            取消
          </button>
        </div>
      </div>
      <div
        className="space-y-1 mb-2 overflow-y-auto"
        style={{ maxHeight: SKILL_LIST_MAX_HEIGHT }}
      >
        {loadingList ? (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
        ) : skillNames.length === 0 ? (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">未找到技能（需 .skills 或 skills 目录）</p>
        ) : (
          skillNames.map((name) => (
            <label key={name} className="flex items-center gap-2 text-xs cursor-pointer hover:text-accent dark:hover:text-accent">
              <input
                type="checkbox"
                checked={selected.has(name)}
                onChange={() => toggleOne(name)}
                className="rounded border-border dark:border-border-dark"
              />
              <span className="truncate">{name}</span>
            </label>
          ))
        )}
      </div>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={runRepair}
          disabled={repairing || skillNames.length === 0}
          className="text-xs px-3 py-1.5 rounded bg-accent text-white hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {repairing ? "修复中…" : selected.size > 0 ? `修复选中 (${selected.size})` : "修复全部失败技能"}
        </button>
      </div>
      {repairResult !== null && (
        <pre className="mt-2 p-2 rounded bg-ink/5 dark:bg-white/5 text-xs text-ink-mute dark:text-ink-dark-mute whitespace-pre-wrap max-h-32 overflow-auto">
          {repairResult}
        </pre>
      )}
    </section>
  );
}

export default function StatusPanel() {
  const { tasks, logEntries, memoryHints, memoryFiles, outputFiles } = useStatusStore();

  const planHasMore = tasks.length > PREVIEW_LIMIT || tasks.length > 0;
  const memHasMore = memoryFiles.length > PREVIEW_LIMIT || memoryHints.length > 0 || memoryFiles.length > 0;
  const logHasMore = logEntries.length > PREVIEW_LIMIT || logEntries.length > 0;
  const outputHasMore = outputFiles.length > PREVIEW_LIMIT || outputFiles.length > 0;

  return (
    <div className="p-4 text-sm">
      <SummarySection
        title="任务计划"
        onClickMore={() => openDetailWindow("plan")}
        onOpenDir={openDir("plan")}
        hasMore={planHasMore || tasks.length > 0}
      >
        <TaskList tasks={tasks} limit={PREVIEW_LIMIT} />
      </SummarySection>

      <SummarySection
        title="记忆"
        onClickMore={() => openDetailWindow("mem")}
        onOpenDir={openDir("memory")}
        hasMore={memHasMore || memoryFiles.length > 0}
      >
        <MemoryPreview files={memoryFiles} hints={memoryHints} limit={PREVIEW_LIMIT} />
      </SummarySection>

      <SummarySection
        title="执行日志"
        onClickMore={() => openDetailWindow("log")}
        onOpenDir={openDir("log")}
        hasMore={logHasMore || logEntries.length > 0}
      >
        <LogList entries={logEntries} limit={PREVIEW_LIMIT} />
      </SummarySection>

      <SummarySection
        title="输出"
        onClickMore={() => openDetailWindow("output")}
        onOpenDir={openDir("output")}
        hasMore={outputHasMore}
      >
        <OutputPreview files={outputFiles} limit={PREVIEW_LIMIT} />
      </SummarySection>

      <SkillRepairSection />
    </div>
  );
}
