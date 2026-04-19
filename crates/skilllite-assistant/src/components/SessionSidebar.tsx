import { useState, useEffect, useCallback, useRef } from "react";
import {
  useSessionStore,
  type SessionInfo,
} from "../stores/useSessionStore";
import { getLocale, translate, useI18n } from "../i18n";
import { useAssistantChrome } from "../contexts/AssistantChromeContext";

function formatSessionListTime(unixStr: string): string {
  const ts = parseInt(unixStr, 10);
  if (!ts || ts === 0) return "";
  const date = new Date(ts * 1000);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
  const loc = getLocale();
  const localeTag = loc === "zh" ? "zh-CN" : "en-US";
  if (diffDays === 0) {
    return date.toLocaleTimeString(localeTag, {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    });
  }
  if (diffDays === 1) return translate("runtime.time.yesterday");
  if (diffDays < 7) return translate("runtime.time.daysAgo", { n: diffDays });
  return date.toLocaleDateString(localeTag, { month: "numeric", day: "numeric" });
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
  const { t } = useI18n();
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
              {formatSessionListTime(session.updated_at)}
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
        aria-label={t("common.more")}
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
            {t("session.rename")}
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
              {t("session.delete")}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default function SessionSidebar() {
  const { t } = useI18n();
  const { openSettingsToTab } = useAssistantChrome();
  const {
    currentSessionKey,
    sessions,
    sessionsLoadError,
    clearSessionsLoadError,
    loadSessions,
    createSession,
    switchSession,
    renameSession,
    deleteSession,
  } = useSessionStore();
  const [isCreating, setIsCreating] = useState(false);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const handleCreate = useCallback(async () => {
    if (isCreating) return;
    setIsCreating(true);
    try {
      const prefix = t("session.newPrefix");
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
  }, [isCreating, sessions, createSession, t]);

  const handleRename = useCallback(
    async (key: string, newName: string) => {
      await renameSession(key, newName);
    },
    [renameSession]
  );

  const handleDelete = useCallback(
    async (key: string) => {
      const session = sessions.find((s) => s.session_key === key);
      const name = session?.display_name ?? key;
      if (!window.confirm(translate("session.deleteConfirm", { name }))) return;
      await deleteSession(key);
      await loadSessions();
    },
    [deleteSession, loadSessions, sessions]
  );

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border dark:border-border-dark shrink-0">
        <span className="text-xs font-medium text-ink-mute dark:text-ink-dark-mute uppercase tracking-wider">
          {t("session.title")}
        </span>
        <button
          type="button"
          onClick={handleCreate}
          disabled={isCreating}
          className="p-1 rounded text-ink-mute hover:text-accent dark:hover:text-accent hover:bg-ink/5 dark:hover:bg-white/5 transition-colors disabled:opacity-50"
          title={t("session.new")}
          aria-label={t("session.new")}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
        </button>
      </div>

      {sessionsLoadError && (
        <div className="mx-2 mt-2 rounded-lg border border-amber-200 dark:border-amber-800/60 bg-amber-50 dark:bg-amber-900/20 px-2.5 py-2 text-xs text-amber-900 dark:text-amber-100">
          <p className="font-medium">{t("session.loadFailedTitle")}</p>
          <p className="mt-1 break-words opacity-90">{sessionsLoadError}</p>
          <button
            type="button"
            className="mt-2 text-amber-800 dark:text-amber-200 underline"
            onClick={() => {
              clearSessionsLoadError();
              void loadSessions();
            }}
          >
            {t("session.retry")}
          </button>
        </div>
      )}

      <div className="flex-1 overflow-y-auto p-2 space-y-1">
        {sessions.map((session) => (
          <SessionItem
            key={session.session_key}
            session={session}
            isActive={session.session_key === currentSessionKey}
            onSwitch={() => void switchSession(session.session_key)}
            onRename={(newName) => handleRename(session.session_key, newName)}
            onDelete={() => handleDelete(session.session_key)}
          />
        ))}

        {sessions.length === 0 && (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute text-center py-4">
            {t("session.empty")}
          </p>
        )}
      </div>

      <div className="shrink-0 border-t border-border dark:border-border-dark px-3 py-2 bg-ink/[0.02] dark:bg-white/[0.02]">
        <button
          type="button"
          onClick={() => openSettingsToTab("environment")}
          className="w-full rounded-lg px-2 py-2 text-left transition-colors hover:bg-ink/5 dark:hover:bg-white/5 focus:outline-none focus-visible:ring-1 focus-visible:ring-accent/50"
        >
          <span className="block text-[11px] font-medium text-ink dark:text-ink-dark">
            {t("session.runtimeShortcutTitle")}
          </span>
          <span className="mt-0.5 block text-[10px] text-ink-mute dark:text-ink-dark-mute leading-snug">
            {t("session.runtimeShortcutHint")}
          </span>
        </button>
      </div>
    </div>
  );
}
