import { useState, useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import { useStatusStore, type TaskItem, type LogEntry } from "../stores/useStatusStore";
import { useSettingsStore } from "../stores/useSettingsStore";
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
          <span className="block break-words line-clamp-4 text-left">{e.text}</span>
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

const SKILL_LIST_MAX_HEIGHT = 200;

/** 将 skill 名称转为短标签（如 xiaohongshu-writer → 小红书） */
function skillDisplayName(name: string): string {
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
  const { settings } = useSettingsStore();
  const workspace = settings.workspace || ".";
  const [skillNames, setSkillNames] = useState<string[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loadingList, setLoadingList] = useState(false);
  const [repairing, setRepairing] = useState(false);
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
      setRepairResult(out || "修复完成");
    } catch (e) {
      setRepairResult(String(e));
      setResultIsError(true);
    } finally {
      setRepairing(false);
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
      setAddResult(out || "已添加");
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
    <section className="mb-4">
      {/* 标题行：技能 + 数量 + 操作 */}
      <div className="flex items-center justify-between gap-2 mb-2">
        <span className="font-medium text-ink dark:text-ink-dark shrink-0">技能</span>
        {skillNames.length > 0 && (
          <span className="text-xs text-ink-mute dark:text-ink-dark-mute shrink-0">
            {skillNames.length} 个
          </span>
        )}
        <div className="flex items-center gap-0.5 min-w-0">
          <button
            type="button"
            onClick={loadSkills}
            disabled={loadingList}
            className="p-1.5 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 transition-colors"
            title="刷新列表"
            aria-label="刷新"
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
            title="全选"
          >
            全选
          </button>
          <button
            type="button"
            onClick={selectNone}
            className="text-xs px-1.5 py-1 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            title="取消选择"
          >
            取消
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
            placeholder="owner/repo 或 owner/repo@skill-name"
            className="flex-1 min-w-0 rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-2.5 py-1.5 text-xs placeholder:text-ink-mute dark:placeholder:text-ink-dark-mute"
          />
          <button
            type="button"
            onClick={runAdd}
            disabled={adding || !addSource.trim()}
            className="shrink-0 px-2.5 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {adding ? "添加中…" : "添加"}
          </button>
        </div>
        <div className="flex items-center gap-2 flex-wrap">
          <button
            type="button"
            onClick={() => openUrl(SKILLS_SH_URL)}
            className="text-xs text-ink-mute dark:text-ink-dark-mute hover:text-accent hover:underline text-left"
          >
            在 skills.sh 浏览更多技能
          </button>
          {addResult != null && (
            <span className={`text-xs ${addResultIsError ? "text-red-600 dark:text-red-400" : "text-ink-mute dark:text-ink-dark-mute"}`}>
              {addResult}
            </span>
          )}
        </div>
      </div>

      {/* 技能列表：卡片式 */}
      <div
        className="rounded-lg border border-border dark:border-border-dark bg-gray-50/50 dark:bg-surface-dark/50 overflow-y-auto mb-3"
        style={{ maxHeight: SKILL_LIST_MAX_HEIGHT }}
      >
        {loadingList ? (
          <div className="p-4 flex items-center justify-center gap-2 text-xs text-ink-mute dark:text-ink-dark-mute">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="animate-spin shrink-0">
              <path d="M21 12a9 9 0 1 1-6.219-8.56" />
            </svg>
            加载中…
          </div>
        ) : skillNames.length === 0 ? (
          <div className="p-4 text-xs text-ink-mute dark:text-ink-dark-mute text-center leading-relaxed">
            <p>未找到技能</p>
            <p className="text-[11px] mt-1 mb-2">当前工作区没有 .skills 目录，点击下方按钮下载默认技能包</p>
            <button
              type="button"
              onClick={runInitSkills}
              disabled={initializing}
              className="px-3 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {initializing ? "初始化中…" : "初始化技能"}
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
                  <span className="truncate text-xs font-medium flex-1 min-w-0">{skillDisplayName(name)}</span>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      invoke("skilllite_open_skill_directory", { workspace, skillName: name }).catch((err) => {
                        console.error("[skilllite-assistant] open_skill_directory failed:", err);
                        setRepairResult(`打开目录失败: ${err}`);
                        setResultIsError(true);
                      });
                    }}
                    className="p-1 rounded text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 shrink-0"
                    title="打开技能目录"
                    aria-label="打开目录"
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
        disabled={repairing || skillNames.length === 0}
        className="w-full text-sm px-3 py-2 rounded-lg bg-accent text-white font-medium hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {repairing ? "修复中…" : selected.size > 0 ? `修复选中 (${selected.size})` : "修复全部失败技能"}
      </button>

      {/* 修复结果 */}
      {repairResult !== null && (
        <div
          className={`mt-2 p-2.5 rounded-lg text-xs whitespace-pre-wrap max-h-28 overflow-auto ${
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
