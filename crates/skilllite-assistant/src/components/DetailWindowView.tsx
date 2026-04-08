import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import {
  useStatusStore,
  STATUS_STORE_PERSIST_KEY,
  STATUS_STORE_BROADCAST,
  type TaskItem,
  type LogEntry,
} from "../stores/useStatusStore";
import { MarkdownContent } from "./shared/MarkdownContent";
import { groupMemoryFiles, memoryPathUnderTopGroup, sortedMemoryGroupKeys } from "../utils/fileUtils";
import { useRecentData } from "../hooks/useRecentData";
import { useDetailMemoryFileCache } from "../hooks/useDetailMemoryFileCache";
import { EvolutionDetailBody } from "./EvolutionSection";
import { translate, useI18n } from "../i18n";
import {
  parseDetailEvolutionFocusTxnFromHash,
  parseDetailEvolutionTabFromHash,
  parseDetailModuleFromHash,
} from "../utils/detailWindow";
import { SETTINGS_STORE_PERSIST_KEY, useSettingsStore } from "../stores/useSettingsStore";

/** 读取失败时在 state 中使用的哨兵，避免把某语言文案写死进比较逻辑 */
const DETAIL_READ_FAILED = "__DETAIL_READ_FAILED__";

export type DetailModule = "plan" | "mem" | "log" | "output" | "evolution";

function TaskList({ tasks }: { tasks: TaskItem[] }) {
  const { t } = useI18n();
  if (tasks.length === 0) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">{t("detail.noPlan")}</p>
    );
  }
  return (
    <ul className="space-y-1.5">
      {tasks.map((task) => (
        <li
          key={task.id}
          className={`flex items-start gap-2 text-sm ${
            task.completed
              ? "text-ink-mute dark:text-ink-dark-mute line-through"
              : "text-ink dark:text-ink-dark-mute"
          }`}
        >
          <span className="shrink-0 mt-0.5 text-accent">{task.completed ? "✓" : "○"}</span>
          <span>{task.description}</span>
          {task.tool_hint && (
            <span className="text-ink-mute dark:text-ink-dark-mute shrink-0">[{task.tool_hint}]</span>
          )}
        </li>
      ))}
    </ul>
  );
}

function LogList({ entries }: { entries: LogEntry[] }) {
  const { t } = useI18n();
  if (entries.length === 0) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">{t("detail.noLog")}</p>
    );
  }
  return (
    <ul className="space-y-1">
      {entries.slice().reverse().map((e) => (
        <li
          key={e.id}
          className={`text-sm font-mono ${
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
          <span className="break-words">{e.text}</span>
        </li>
      ))}
    </ul>
  );
}

function LogFileContent({ files, entries }: { files: string[]; entries: LogEntry[] }) {
  const { t } = useI18n();
  const [expandedFile, setExpandedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const loadGenRef = useRef(0);

  const hasFiles = files.length > 0;
  const hasEntries = entries.length > 0;

  if (!hasFiles && !hasEntries) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">{t("detail.noLog")}</p>
    );
  }

  const handleFileClick = async (filename: string) => {
    if (expandedFile === filename) {
      loadGenRef.current += 1;
      setExpandedFile(null);
      setFileContent(null);
      setLoading(false);
      return;
    }
    const myGen = ++loadGenRef.current;
    setLoading(true);
    setExpandedFile(filename);
    setFileContent(null);
    try {
      const content = await invoke<string>("skilllite_read_log_file", { filename });
      if (myGen !== loadGenRef.current) return;
      setFileContent(content);
    } catch {
      if (myGen !== loadGenRef.current) return;
      setFileContent(DETAIL_READ_FAILED);
    } finally {
      if (myGen === loadGenRef.current) setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      {hasFiles && (
        <div className="space-y-1.5">
          <div className="text-sm font-medium text-ink-mute dark:text-ink-dark-mute">
            {t("detail.logRecent")}
          </div>
          <ul className="space-y-0.5">
            {files.map((f, i) => (
              <li key={`file-${i}`}>
                <button
                  type="button"
                  onClick={() => handleFileClick(f)}
                  className="text-sm text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent w-full text-left flex items-center gap-2 py-1 transition-colors"
                  title={f}
                >
                  <span className="shrink-0">📄</span>
                  <span className="truncate flex-1">{f}</span>
                  <span className="text-ink-mute/80 shrink-0">{expandedFile === f ? "▼" : "▶"}</span>
                </button>
                {expandedFile === f && (
                  <div className="mt-1.5 mb-2 ml-5 p-3 rounded-lg bg-blue-50/60 dark:bg-zinc-700/25 text-sm text-ink/85 dark:text-zinc-400 overflow-y-auto max-h-80 border border-blue-100 dark:border-zinc-600/40 shadow-sm">
                    {loading ? (
                      <span className="text-ink-mute dark:text-zinc-500">{t("detail.loading")}</span>
                    ) : fileContent ? (
                      fileContent === DETAIL_READ_FAILED ? (
                        <span className="text-red-500 text-sm">{t("detail.readFailed")}</span>
                      ) : (
                        <pre className="whitespace-pre-wrap text-xs font-mono break-words leading-relaxed">
                          {fileContent}
                        </pre>
                      )
                    ) : null}
                  </div>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      {hasEntries && (
        <div className="space-y-1.5">
          {hasFiles && (
            <div className="text-sm font-medium text-ink-mute dark:text-ink-dark-mute pt-2 border-t border-border dark:border-border-dark">
              {t("detail.liveLog")}
            </div>
          )}
          <LogList entries={entries} />
        </div>
      )}
    </div>
  );
}

function MemoryContent({ files, hints }: { files: string[]; hints: string[] }) {
  const { t } = useI18n();
  const [expandedFile, setExpandedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const loadGenRef = useRef(0);
  const hoverPrefetchTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const { getCached, touch, prefetchPath } = useDetailMemoryFileCache(files);

  const hasFiles = files.length > 0;
  const hasHints = hints.length > 0;

  useEffect(() => {
    return () => {
      for (const id of hoverPrefetchTimersRef.current.values()) clearTimeout(id);
      hoverPrefetchTimersRef.current.clear();
    };
  }, []);

  const schedulePrefetchOnHover = useCallback(
    (path: string) => {
      if (getCached(path) !== undefined) return;
      const prev = hoverPrefetchTimersRef.current.get(path);
      if (prev) clearTimeout(prev);
      const id = setTimeout(() => {
        hoverPrefetchTimersRef.current.delete(path);
        prefetchPath(path);
      }, 160);
      hoverPrefetchTimersRef.current.set(path, id);
    },
    [getCached, prefetchPath],
  );

  const cancelHoverPrefetch = useCallback((path: string) => {
    const id = hoverPrefetchTimersRef.current.get(path);
    if (id) {
      clearTimeout(id);
      hoverPrefetchTimersRef.current.delete(path);
    }
  }, []);

  const handleFileClick = async (path: string) => {
    if (expandedFile === path) {
      loadGenRef.current += 1;
      setExpandedFile(null);
      setFileContent(null);
      setLoading(false);
      return;
    }
    const cached = getCached(path);
    if (cached !== undefined) {
      loadGenRef.current += 1;
      setLoading(false);
      setExpandedFile(path);
      setFileContent(cached);
      return;
    }
    const myGen = ++loadGenRef.current;
    setLoading(true);
    setExpandedFile(path);
    setFileContent(null);
    try {
      const content = await invoke<string>("skilllite_read_memory_file", {
        relativePath: path,
      });
      if (myGen !== loadGenRef.current) return;
      touch(path, content);
      setFileContent(content);
    } catch {
      if (myGen !== loadGenRef.current) return;
      setFileContent(DETAIL_READ_FAILED);
    } finally {
      if (myGen === loadGenRef.current) setLoading(false);
    }
  };

  if (!hasFiles && !hasHints) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">{t("detail.noMemory")}</p>
    );
  }

  const groups = groupMemoryFiles(files);
  const groupKeys = sortedMemoryGroupKeys(groups);

  return (
    <div className="space-y-3">
      <ul className="space-y-1.5">
        {groupKeys.map((group) => {
          const paths = groups[group]!;
          return (
          <li key={group}>
            {group !== "." && (
              <div className="text-sm font-medium text-ink-mute dark:text-ink-dark-mute mb-1">{group}/</div>
            )}
            <ul className="space-y-0.5">
              {paths.map((f) => (
                <li key={f}>
                  <button
                    type="button"
                    onClick={() => void handleFileClick(f)}
                    onPointerEnter={() => schedulePrefetchOnHover(f)}
                    onPointerLeave={() => cancelHoverPrefetch(f)}
                    className="text-sm text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent w-full text-left flex items-center gap-2 py-1 transition-colors"
                    title={f}
                  >
                    <span className="shrink-0">📄</span>
                    <span className="truncate flex-1 font-mono text-[13px]">{memoryPathUnderTopGroup(f)}</span>
                    <span className="text-ink-mute/80 shrink-0">{expandedFile === f ? "▼" : "▶"}</span>
                  </button>
                  {expandedFile === f && (
                    <div className="mt-1.5 mb-2 ml-5 p-3 rounded-lg bg-blue-50/60 dark:bg-zinc-700/25 text-sm text-ink/85 dark:text-zinc-400 overflow-y-auto max-h-80 border border-blue-100 dark:border-zinc-600/40 shadow-sm">
                      {loading ? (
                        <span className="text-ink-mute dark:text-zinc-500">{t("detail.loading")}</span>
                      ) : fileContent !== null ? (
                        fileContent === DETAIL_READ_FAILED ? (
                          <span className="text-red-500 text-sm">{t("detail.readFailed")}</span>
                        ) : (
                          <div className="prose prose-sm max-w-none dark:prose-invert [&_pre]:text-xs [&_pre]:bg-black/5 [&_pre]:dark:bg-white/5 [&_pre]:rounded-md [&_pre]:p-2 [&_code]:text-xs [&_*]:!text-ink/80 [&_*]:dark:!text-zinc-400 [&_strong]:!text-ink [&_strong]:dark:!text-zinc-300 [&_h1]:!text-ink [&_h1]:dark:!text-zinc-300 [&_h2]:!text-ink [&_h2]:dark:!text-zinc-300 [&_h3]:!text-ink [&_h3]:dark:!text-zinc-300">
                            <MarkdownContent content={fileContent} />
                          </div>
                        )
                      ) : null}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          </li>
          );
        })}
      </ul>
      {hasHints && (
        <>
          {hasFiles && (
            <div className="text-sm font-medium text-ink-mute dark:text-ink-dark-mute pt-2 border-t border-border dark:border-border-dark">
              {t("detail.recentOps")}
            </div>
          )}
          <ul className="space-y-0.5">
            {hints.slice().reverse().map((h, i) => (
              <li key={`hint-${i}`} className="text-sm text-ink-mute dark:text-ink-dark-mute truncate">
                {h}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

const IMAGE_EXTENSIONS = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"];
const getImageMime = (path: string): string | null => {
  const lower = path.toLowerCase();
  if (lower.endsWith(".png")) return "image/png";
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
  if (lower.endsWith(".gif")) return "image/gif";
  if (lower.endsWith(".webp")) return "image/webp";
  if (lower.endsWith(".svg")) return "image/svg+xml";
  return null;
};
const isImageFile = (path: string) => IMAGE_EXTENSIONS.some((ext) => path.toLowerCase().endsWith(ext));

function OutputFileContent({ files, workspace }: { files: string[]; workspace: string }) {
  const { t } = useI18n();
  const [expandedFile, setExpandedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const loadGenRef = useRef(0);

  if (files.length === 0) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">{t("detail.noOutput")}</p>
    );
  }

  const groups = groupMemoryFiles(files);
  const groupKeys = sortedMemoryGroupKeys(groups);

  const handleFileClick = async (path: string) => {
    if (expandedFile === path) {
      loadGenRef.current += 1;
      setExpandedFile(null);
      setFileContent(null);
      setLoading(false);
      return;
    }
    const myGen = ++loadGenRef.current;
    setLoading(true);
    setExpandedFile(path);
    setFileContent(null);
    try {
      if (isImageFile(path)) {
        const base64 = await invoke<string>("skilllite_read_output_file_base64", {
          relativePath: path,
          workspace,
        });
        const mime = getImageMime(path) ?? "image/png";
        if (myGen !== loadGenRef.current) return;
        setFileContent(`data:${mime};base64,${base64}`);
      } else {
        const content = await invoke<string>("skilllite_read_output_file", {
          relativePath: path,
          workspace,
        });
        if (myGen !== loadGenRef.current) return;
        setFileContent(content);
      }
    } catch {
      if (myGen !== loadGenRef.current) return;
      setFileContent(DETAIL_READ_FAILED);
    } finally {
      if (myGen === loadGenRef.current) setLoading(false);
    }
  };

  return (
    <div className="space-y-3">
      <ul className="space-y-1.5">
        {groupKeys.map((group) => {
          const paths = groups[group]!;
          return (
          <li key={group}>
            {group !== "." && (
              <div className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-1">{group}/</div>
            )}
            <ul className="space-y-0.5">
              {paths.map((f) => (
                <li key={f}>
                  <button
                    type="button"
                    onClick={() => handleFileClick(f)}
                    className="text-sm text-gray-600 dark:text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 w-full text-left flex items-center gap-2 py-1"
                    title={f}
                  >
                    <span className="shrink-0">📄</span>
                    <span className="truncate flex-1 font-mono text-[13px]">{memoryPathUnderTopGroup(f)}</span>
                    <span className="text-gray-400 shrink-0">{expandedFile === f ? "▼" : "▶"}</span>
                  </button>
                  {expandedFile === f && (
                    <div className="mt-2 p-3 rounded-lg bg-gray-100 dark:bg-gray-700/50 text-sm overflow-y-auto max-h-80 border border-gray-200 dark:border-gray-600">
                      {loading ? (
                        <span className="text-gray-500">{t("detail.loading")}</span>
                      ) : fileContent ? (
                        fileContent === DETAIL_READ_FAILED ? (
                          <span className="text-red-500 text-sm">{t("detail.readFailed")}</span>
                        ) : fileContent.startsWith("data:image/") ? (
                          <img
                            src={fileContent}
                            alt={f}
                            className="max-w-full max-h-80 object-contain rounded-md"
                          />
                        ) : f.endsWith(".html") || f.endsWith(".htm") ? (
                          <iframe
                            srcDoc={fileContent}
                            sandbox="allow-same-origin"
                            title={f}
                            className="w-full min-h-[200px] border-0 rounded-md bg-paper dark:bg-surface-dark"
                          />
                        ) : f.endsWith(".md") ? (
                          <div className="prose prose-sm max-w-none dark:prose-invert [&_pre]:text-xs [&_code]:text-xs">
                            <MarkdownContent content={fileContent} />
                          </div>
                        ) : (
                          <pre className="whitespace-pre-wrap text-xs break-words">{fileContent}</pre>
                        )
                      ) : null}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          </li>
          );
        })}
      </ul>
    </div>
  );
}

export default function DetailWindowView() {
  const { t } = useI18n();
  const [module, setModule] = useState<DetailModule | null>(null);
  const { refreshRecentData } = useRecentData();
  const { tasks, logEntries, logFiles, memoryHints, memoryFiles, outputFiles } = useStatusStore();
  const { settings } = useSettingsStore();
  const workspace = settings.workspace?.trim() || ".";

  const titles = useMemo(
    () =>
      ({
        plan: t("detail.title.plan"),
        mem: t("detail.title.mem"),
        log: t("detail.title.log"),
        output: t("detail.title.output"),
        evolution: t("detail.title.evolution"),
      }) satisfies Record<DetailModule, string>,
    [t]
  );

  /** 详情窗首帧解析一次，避免 hash 读数在重渲染间抖动 */
  const evolutionEntryParams = useMemo(
    () => ({
      tab: parseDetailEvolutionTabFromHash() ?? undefined,
      focusTxn: parseDetailEvolutionFocusTxnFromHash(),
    }),
    []
  );

  useEffect(() => {
    setModule(parseDetailModuleFromHash() as DetailModule | null);
  }, []);

  useEffect(() => {
    refreshRecentData();
  }, [refreshRecentData, workspace]);

  // 独立 WebView 与主窗口内存不共享：主窗口写入 persist 后，由此拉取最新 tasks（含 clearPlan 后空计划）
  useEffect(() => {
    const pull = () => {
      void useStatusStore.persist.rehydrate();
    };
    queueMicrotask(pull);
    let bc: BroadcastChannel | undefined;
    try {
      bc = new BroadcastChannel(STATUS_STORE_BROADCAST);
      bc.onmessage = () => pull();
    } catch {
      /* ignore */
    }
    const onStorage = (e: StorageEvent) => {
      if (e.key === STATUS_STORE_PERSIST_KEY) pull();
    };
    const onVisible = () => {
      if (document.visibilityState === "visible") pull();
    };
    window.addEventListener("storage", onStorage);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      bc?.close();
      window.removeEventListener("storage", onStorage);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);

  // 工作区等设置同理：与主窗口持久化对齐（URL ?w= 优先，见 EvolutionSection）
  useEffect(() => {
    const pull = () => {
      void useSettingsStore.persist.rehydrate();
    };
    queueMicrotask(pull);
    const onStorage = (e: StorageEvent) => {
      if (e.key === SETTINGS_STORE_PERSIST_KEY) pull();
    };
    const onVisible = () => {
      if (document.visibilityState === "visible") pull();
    };
    window.addEventListener("storage", onStorage);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("storage", onStorage);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);

  if (!module) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-surface dark:bg-surface-dark">
        <p className="text-ink-mute">{t("detail.invalid")}</p>
      </div>
    );
  }

  const dirModule =
    module === "mem"
      ? "memory"
      : module === "plan"
        ? "plan"
        : module === "evolution"
          ? "evolution"
          : module;
  const handleOpenDir = async () => {
    try {
      await invoke("skilllite_open_directory", { module: dirModule, workspace });
    } catch (err) {
      console.error("[skilllite-assistant] skilllite_open_directory failed:", err);
      useUiToastStore
        .getState()
        .show(translate("toast.openDirFailed", { err: formatInvokeError(err) }), "error");
    }
  };

  return (
    <div className="flex flex-col min-h-screen bg-paper dark:bg-paper-dark">
      <header className="flex items-center justify-between px-4 py-3 border-b border-border dark:border-border-dark shrink-0">
        <h1 className="text-base font-semibold text-ink dark:text-ink-dark">{titles[module]}</h1>
        <button
          type="button"
          onClick={handleOpenDir}
          className="p-2 text-ink-mute hover:text-ink dark:hover:text-ink-dark rounded-md hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
          aria-label={t("detail.openDir")}
          title={t("detail.openInFm")}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
        </button>
      </header>
      <main className="flex-1 overflow-y-auto p-4">
        {module === "plan" && <TaskList tasks={tasks} />}
        {module === "mem" && <MemoryContent files={memoryFiles} hints={memoryHints} />}
        {module === "log" && <LogFileContent files={logFiles} entries={logEntries} />}
        {module === "output" && (
          <OutputFileContent files={outputFiles} workspace={workspace} />
        )}
        {module === "evolution" && (
          <EvolutionDetailBody
            initialTab={evolutionEntryParams.tab}
            initialFocusTxn={evolutionEntryParams.focusTxn}
          />
        )}
      </main>
    </div>
  );
}
