import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  useSessionStore,
  type SessionInfo,
} from "../stores/useSessionStore";

type RuntimeSource = "system" | "cache" | "none";

interface RuntimeUiLine {
  source: RuntimeSource;
  label: string;
  revealPath: string | null;
}

interface RuntimeUiSnapshot {
  python: RuntimeUiLine;
  node: RuntimeUiLine;
  cacheRoot: string | null;
  cacheRootAbs: string | null;
}

function runtimeSourceBadge(source: RuntimeSource): string {
  switch (source) {
    case "system":
      return "系统";
    case "cache":
      return "已下载";
    default:
      return "未就绪";
  }
}

function formatTime(unixStr: string): string {
  const ts = parseInt(unixStr, 10);
  if (!ts || ts === 0) return "";
  const date = new Date(ts * 1000);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) {
    return date.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    });
  }
  if (diffDays === 1) return "昨天";
  if (diffDays < 7) return `${diffDays}天前`;
  return date.toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
}

function SessionItem({
  session,
  isActive,
  onSwitch,
  onRename,
  onDelete,
}: {
  session: SessionInfo;
  isActive: boolean;
  onSwitch: () => void;
  onRename: (newName: string) => void;
  onDelete: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(session.display_name);
  const [showMenu, setShowMenu] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!editing) setEditValue(session.display_name);
  }, [session.display_name, editing]);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editing]);

  useEffect(() => {
    if (!showMenu) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setShowMenu(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showMenu]);

  const handleSubmitRename = () => {
    const trimmed = editValue.trim();
    if (trimmed && trimmed !== session.display_name) {
      onRename(trimmed);
    }
    setEditing(false);
  };

  return (
    <div className="relative group">
      <button
        type="button"
        onClick={onSwitch}
        className={`w-full text-left px-3 py-2 rounded-lg transition-colors ${
          isActive
            ? "bg-accent/10 dark:bg-accent/20 border border-accent/30"
            : "hover:bg-ink/5 dark:hover:bg-white/5 border border-transparent"
        }`}
      >
        {editing ? (
          <input
            ref={inputRef}
            type="text"
            value={editValue}
            onChange={(e) => setEditValue(e.target.value)}
            onBlur={handleSubmitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSubmitRename();
              if (e.key === "Escape") {
                setEditValue(session.display_name);
                setEditing(false);
              }
            }}
            onClick={(e) => e.stopPropagation()}
            className="w-full text-sm font-medium bg-transparent border-b border-accent outline-none text-ink dark:text-ink-dark"
          />
        ) : (
          <div className="flex items-center gap-2">
            <span
              className={`w-1.5 h-1.5 rounded-full shrink-0 ${
                isActive ? "bg-accent" : "bg-ink-mute/30 dark:bg-ink-dark-mute/30"
              }`}
            />
            <span className="text-sm font-medium text-ink dark:text-ink-dark truncate flex-1">
              {session.display_name}
            </span>
            <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute shrink-0">
              {formatTime(session.updated_at)}
            </span>
          </div>
        )}
        {!editing && session.message_preview && (
          <p className="mt-0.5 ml-3.5 text-xs text-ink-mute dark:text-ink-dark-mute truncate">
            {session.message_preview}
          </p>
        )}
      </button>

      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          setShowMenu(!showMenu);
        }}
        className="absolute top-1.5 right-1.5 p-1 rounded opacity-0 group-hover:opacity-100 text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 transition-all"
        aria-label="更多操作"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="12" cy="5" r="1" />
          <circle cx="12" cy="12" r="1" />
          <circle cx="12" cy="19" r="1" />
        </svg>
      </button>

      {showMenu && (
        <div
          ref={menuRef}
          className="absolute right-0 top-8 z-50 bg-white dark:bg-paper-dark border border-border dark:border-border-dark rounded-lg shadow-lg py-1 min-w-[100px]"
        >
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              setShowMenu(false);
              setEditValue(session.display_name);
              setEditing(true);
            }}
            className="w-full text-left px-3 py-1.5 text-xs text-ink dark:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5"
          >
            重命名
          </button>
          {session.session_key !== "default" && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                setShowMenu(false);
                onDelete();
              }}
              className="w-full text-left px-3 py-1.5 text-xs text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20"
            >
              删除
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default function SessionSidebar() {
  const {
    currentSessionKey,
    sessions,
    loadSessions,
    createSession,
    switchSession,
    renameSession,
    deleteSession,
  } = useSessionStore();
  const [isCreating, setIsCreating] = useState(false);
  const [runtime, setRuntime] = useState<RuntimeUiSnapshot | null>(null);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  useEffect(() => {
    invoke<RuntimeUiSnapshot>("skilllite_runtime_status")
      .then(setRuntime)
      .catch(() => setRuntime(null));
  }, []);

  const revealInFileManager = useCallback((path: string | null | undefined) => {
    if (!path?.trim()) return;
    invoke("skilllite_reveal_in_file_manager", { path: path.trim() }).catch((err) => {
      console.error("[skilllite-assistant] reveal_in_file_manager failed:", err);
    });
  }, []);

  const handleCreate = useCallback(async () => {
    if (isCreating) return;
    setIsCreating(true);
    try {
      const prefix = "新会话";
      const usedNumbers = sessions
        .map((s) => s.display_name)
        .filter((n) => n === prefix || n.startsWith(`${prefix} `))
        .map((n) => (n === prefix ? 1 : parseInt(n.slice(prefix.length + 1), 10)))
        .filter((n) => !isNaN(n));
      const next = usedNumbers.length === 0 ? 1 : Math.max(...usedNumbers) + 1;
      const name = next === 1 ? prefix : `${prefix} ${next}`;
      await createSession(name);
    } finally {
      setIsCreating(false);
    }
  }, [isCreating, sessions, createSession]);

  const handleRename = useCallback(
    async (key: string, newName: string) => {
      try {
        await renameSession(key, newName);
      } catch (err) {
        console.error("[skilllite-assistant] rename session failed:", err);
      }
    },
    [renameSession]
  );

  const handleDelete = useCallback(
    async (key: string) => {
      const session = sessions.find((s) => s.session_key === key);
      const name = session?.display_name ?? key;
      if (!window.confirm(`确定要删除会话「${name}」吗？删除后无法恢复。`)) return;
      try {
        await deleteSession(key);
        loadSessions();
      } catch (err) {
        console.error("[skilllite-assistant] delete session failed:", err);
      }
    },
    [deleteSession, loadSessions, sessions]
  );

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border dark:border-border-dark shrink-0">
        <span className="text-xs font-medium text-ink-mute dark:text-ink-dark-mute uppercase tracking-wider">
          会话
        </span>
        <button
          type="button"
          onClick={handleCreate}
          disabled={isCreating}
          className="p-1 rounded text-ink-mute hover:text-accent dark:hover:text-accent hover:bg-ink/5 dark:hover:bg-white/5 transition-colors disabled:opacity-50"
          title="新建会话"
          aria-label="新建会话"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-1">
        {sessions.map((session) => (
          <SessionItem
            key={session.session_key}
            session={session}
            isActive={session.session_key === currentSessionKey}
            onSwitch={() => switchSession(session.session_key)}
            onRename={(newName) => handleRename(session.session_key, newName)}
            onDelete={() => handleDelete(session.session_key)}
          />
        ))}

        {sessions.length === 0 && (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute text-center py-4">
            暂无会话
          </p>
        )}
      </div>

      {runtime && (
        <div className="shrink-0 border-t border-border dark:border-border-dark px-3 py-2 space-y-1.5 bg-ink/[0.02] dark:bg-white/[0.02]">
          <div className="text-[10px] font-medium text-ink-mute dark:text-ink-dark-mute uppercase tracking-wider">
            技能运行时
          </div>
          <div className="space-y-1">
            {(
              [
                { key: "py", title: "Python", line: runtime.python },
                { key: "node", title: "Node", line: runtime.node },
              ] as const
            ).map(({ key, title, line }) => {
              const canReveal = Boolean(line.revealPath);
              const rowClass = `flex items-start gap-2 min-w-0 w-full text-[11px] leading-snug rounded-md -mx-1 px-1 py-0.5 transition-colors ${
                canReveal
                  ? "text-left cursor-pointer hover:bg-ink/5 dark:hover:bg-white/5 focus:outline-none focus-visible:ring-1 focus-visible:ring-accent/50"
                  : ""
              }`;
              const badge = (
                <span
                  className={`shrink-0 rounded px-1 py-px text-[10px] font-medium ${
                    line.source === "system"
                      ? "bg-emerald-500/15 text-emerald-800 dark:text-emerald-300"
                      : line.source === "cache"
                        ? "bg-amber-500/15 text-amber-900 dark:text-amber-200"
                        : "bg-ink/10 text-ink-mute dark:text-ink-dark-mute"
                  }`}
                >
                  {runtimeSourceBadge(line.source)}
                </span>
              );
              const text = (
                <div className="min-w-0 flex-1">
                  <span className="text-ink-mute dark:text-ink-dark-mute">{title}</span>
                  <span className="text-ink dark:text-ink-dark ml-1">{line.label}</span>
                </div>
              );
              return canReveal ? (
                <button
                  key={key}
                  type="button"
                  onClick={() => revealInFileManager(line.revealPath)}
                  className={rowClass}
                  title={`${line.label} — 点击在文件管理器中打开`}
                >
                  {badge}
                  {text}
                </button>
              ) : (
                <div key={key} className={rowClass} title={line.label}>
                  {badge}
                  {text}
                </div>
              );
            })}
          </div>
          {runtime.cacheRoot && (
            <button
              type="button"
              disabled={!runtime.cacheRootAbs}
              onClick={() => revealInFileManager(runtime.cacheRootAbs)}
              className="text-[10px] text-ink-mute dark:text-ink-dark-mute font-mono truncate w-full text-left rounded px-1 py-0.5 -mx-1 transition-colors hover:text-accent dark:hover:text-accent hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-ink-mute dark:disabled:hover:text-ink-dark-mute focus:outline-none focus-visible:ring-1 focus-visible:ring-accent/50"
              title={
                runtime.cacheRootAbs
                  ? `缓存目录 — 点击在文件管理器中打开`
                  : "无法打开缓存目录"
              }
            >
              缓存目录 {runtime.cacheRoot}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
