import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useStatusStore, type TaskItem, type LogEntry } from "../stores/useStatusStore";

export type DetailModule = "plan" | "mem" | "log" | "output";

/** 从 hash 解析模块类型，如 #detail/plan -> "plan" */
export function parseDetailModuleFromHash(): DetailModule | null {
  const hash = window.location.hash;
  const m = hash.match(/^#?detail\/(plan|mem|log|output)$/);
  return (m?.[1] as DetailModule) ?? null;
}

function groupMemoryFiles(files: string[]): Record<string, string[]> {
  const groups: Record<string, string[]> = {};
  for (const f of files) {
    const parts = f.split("/");
    const key = parts.length > 1 ? parts[0] : ".";
    (groups[key] ??= []).push(f);
  }
  return groups;
}

function TaskList({ tasks }: { tasks: TaskItem[] }) {
  if (tasks.length === 0) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">暂无任务计划</p>
    );
  }
  return (
    <ul className="space-y-1.5">
      {tasks.map((t) => (
        <li
          key={t.id}
          className={`flex items-start gap-2 text-sm ${
            t.completed ? "text-ink-mute dark:text-ink-dark-mute line-through" : "text-ink dark:text-ink-dark-mute"
          }`}
        >
          <span className="shrink-0 mt-0.5 text-accent">{t.completed ? "✓" : "○"}</span>
          <span>{t.description}</span>
          {t.tool_hint && (
            <span className="text-ink-mute dark:text-ink-dark-mute shrink-0">[{t.tool_hint}]</span>
          )}
        </li>
      ))}
    </ul>
  );
}

function LogList({ entries }: { entries: LogEntry[] }) {
  if (entries.length === 0) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">暂无日志</p>
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
          {e.type === "tool_result" && (e.isError ? "✗" : "✓")}
          {e.name && <span className="font-medium">{e.name}: </span>}
          <span className="break-words">{e.text}</span>
        </li>
      ))}
    </ul>
  );
}

function MemoryContent({ files, hints }: { files: string[]; hints: string[] }) {
  const [expandedFile, setExpandedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const hasFiles = files.length > 0;
  const hasHints = hints.length > 0;
  if (!hasFiles && !hasHints) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">暂无记忆</p>
    );
  }

  const groups = groupMemoryFiles(files);

  const handleFileClick = async (path: string) => {
    if (expandedFile === path) {
      setExpandedFile(null);
      setFileContent(null);
      return;
    }
    setLoading(true);
    setExpandedFile(path);
    try {
      const content = await invoke<string>("skilllite_read_memory_file", {
        relativePath: path,
      });
      setFileContent(content);
    } catch {
      setFileContent("* 无法读取文件内容 *");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-3">
      <ul className="space-y-1.5">
        {Object.entries(groups).map(([group, paths]) => (
          <li key={group}>
            {group !== "." && (
              <div className="text-sm font-medium text-ink-mute dark:text-ink-dark-mute mb-1">{group}/</div>
            )}
            <ul className="space-y-0.5">
              {paths.map((f, i) => (
                <li key={`file-${i}`}>
                  <button
                    type="button"
                    onClick={() => handleFileClick(f)}
                    className="text-sm text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent w-full text-left flex items-center gap-2 py-1 transition-colors"
                    title={f}
                  >
                    <span className="shrink-0">📄</span>
                    <span className="truncate flex-1">{f.split("/").pop() ?? f}</span>
                    <span className="text-ink-mute/80 shrink-0">{expandedFile === f ? "▼" : "▶"}</span>
                  </button>
                  {expandedFile === f && (
                    <div className="mt-2 p-3 rounded-md bg-surface dark:bg-surface-dark text-sm overflow-y-auto max-h-80 border border-border dark:border-border-dark">
                      {loading ? (
                        <span className="text-ink-mute">加载中...</span>
                      ) : fileContent ? (
                        <div className="prose prose-sm max-w-none dark:prose-invert [&_pre]:text-xs [&_code]:text-xs">
                          <ReactMarkdown remarkPlugins={[remarkGfm]}>{fileContent}</ReactMarkdown>
                        </div>
                      ) : null}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          </li>
        ))}
      </ul>
      {hasHints && (
        <>
          {hasFiles && (
            <div className="text-sm font-medium text-ink-mute dark:text-ink-dark-mute pt-2 border-t border-border dark:border-border-dark">
              最近操作
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

const TITLES: Record<DetailModule, string> = {
  plan: "任务计划",
  mem: "记忆",
  log: "执行日志",
  output: "输出",
};

const markdownComponents = {
  p: ({ children }: { children?: React.ReactNode }) => <p className="mb-2 last:mb-0">{children}</p>,
  ul: ({ children }: { children?: React.ReactNode }) => <ul className="list-disc list-inside mb-2">{children}</ul>,
  ol: ({ children }: { children?: React.ReactNode }) => <ol className="list-decimal list-inside mb-2">{children}</ol>,
  code: ({ className, children }: { className?: string; children?: React.ReactNode }) =>
    !className ? (
      <code className="px-1.5 py-0.5 rounded-md bg-ink/10 dark:bg-white/10 font-mono text-sm">{children}</code>
    ) : (
      <code className={`block p-3 rounded-md text-sm overflow-x-auto ${className ?? ""}`}>{children}</code>
    ),
  pre: ({ children }: { children?: React.ReactNode }) => (
    <pre className="mb-2 overflow-x-auto rounded-md bg-ink/5 dark:bg-white/5 p-3 text-sm">{children}</pre>
  ),
  a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
    <a href={href} target="_blank" rel="noopener noreferrer" className="underline text-accent hover:text-accent-hover">
      {children}
    </a>
  ),
};

function OutputFileContent({ files }: { files: string[] }) {
  const [expandedFile, setExpandedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  if (files.length === 0) {
    return (
      <p className="text-sm text-ink-mute dark:text-ink-dark-mute italic">暂无输出文件</p>
    );
  }

  const groups = groupMemoryFiles(files);

  const handleFileClick = async (path: string) => {
    if (expandedFile === path) {
      setExpandedFile(null);
      setFileContent(null);
      return;
    }
    setLoading(true);
    setExpandedFile(path);
    try {
      const content = await invoke<string>("skilllite_read_output_file", {
        relativePath: path,
      });
      setFileContent(content);
    } catch {
      setFileContent("* 无法读取文件内容 *");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-3">
      <ul className="space-y-1.5">
        {Object.entries(groups).map(([group, paths]) => (
          <li key={group}>
            {group !== "." && (
              <div className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-1">{group}/</div>
            )}
            <ul className="space-y-0.5">
              {paths.map((f, i) => (
                <li key={`file-${i}`}>
                  <button
                    type="button"
                    onClick={() => handleFileClick(f)}
                    className="text-sm text-gray-600 dark:text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 w-full text-left flex items-center gap-2 py-1"
                    title={f}
                  >
                    <span className="shrink-0">📄</span>
                    <span className="truncate flex-1">{f.split("/").pop() ?? f}</span>
                    <span className="text-gray-400 shrink-0">{expandedFile === f ? "▼" : "▶"}</span>
                  </button>
                  {expandedFile === f && (
                    <div className="mt-2 p-3 rounded-lg bg-gray-100 dark:bg-gray-700/50 text-sm overflow-y-auto max-h-80 border border-gray-200 dark:border-gray-600">
                      {loading ? (
                        <span className="text-gray-500">加载中...</span>
                      ) : fileContent ? (
                        f.endsWith(".html") || f.endsWith(".htm") ? (
                          <iframe
                            srcDoc={fileContent}
                            sandbox="allow-same-origin"
                            title={f}
                            className="w-full min-h-[200px] border-0 rounded-md bg-paper dark:bg-surface-dark"
                          />
                        ) : f.endsWith(".md") ? (
                          <div className="prose prose-sm max-w-none dark:prose-invert [&_pre]:text-xs [&_code]:text-xs">
                            <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                              {fileContent}
                            </ReactMarkdown>
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
        ))}
      </ul>
    </div>
  );
}

export default function DetailWindowView() {
  const [module, setModule] = useState<DetailModule | null>(null);
  const setRecentData = useStatusStore((s) => s.setRecentData);
  const { tasks, logEntries, memoryHints, memoryFiles, outputFiles } = useStatusStore();

  useEffect(() => {
    const m = parseDetailModuleFromHash();
    setModule(m ?? null);
  }, []);

  useEffect(() => {
    invoke<{
      memory_files: string[];
      output_files: string[];
      plan: { task: string; steps: { id: number; description: string; completed: boolean }[] } | null;
    }>("skilllite_load_recent")
      .then((data) => {
        setRecentData({
          memoryFiles: data.memory_files ?? [],
          outputFiles: data.output_files ?? [],
          plan: data.plan
            ? {
                task: data.plan.task,
                steps: data.plan.steps.map((s) => ({
                  id: s.id,
                  description: s.description,
                  completed: s.completed,
                })),
              }
            : undefined,
        });
      })
      .catch(() => {});
  }, [setRecentData]);

  const handleClose = () => {
    getCurrentWindow().close();
  };

  if (!module) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-surface dark:bg-surface-dark">
        <p className="text-ink-mute">无效的详情视图</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col min-h-screen bg-paper dark:bg-paper-dark">
      <header className="flex items-center justify-between px-4 py-3 border-b border-border dark:border-border-dark shrink-0">
        <h1 className="text-base font-semibold text-ink dark:text-ink-dark">{TITLES[module]}</h1>
        <button
          type="button"
          onClick={handleClose}
          className="p-2 text-ink-mute hover:text-ink dark:hover:text-ink-dark rounded-md hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
          aria-label="关闭窗口"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
      </header>
      <main className="flex-1 overflow-y-auto p-4">
        {module === "plan" && <TaskList tasks={tasks} />}
        {module === "mem" && <MemoryContent files={memoryFiles} hints={memoryHints} />}
        {module === "log" && <LogList entries={logEntries} />}
        {module === "output" && <OutputFileContent files={outputFiles} />}
      </main>
    </div>
  );
}
