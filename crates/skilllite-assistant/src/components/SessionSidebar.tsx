import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  useSessionStore,
  type SessionInfo,
} from "../stores/useSessionStore";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import { RUNTIME_STATUS_REFRESH_EVENT } from "../utils/runtimeStatusRefresh";
import { formatRuntimeProvisionProgress } from "../utils/runtimeProvisionProgressZh";
import { getLocale, translate, useI18n } from "../i18n";

type RuntimeSource = "system" | "cache" | "none";

interface RuntimeUiLine {
  source: RuntimeSource;
  label: string;
  revealPath: string | null;
  /** 后端可选：系统优先时说明缓存内是否仍有 SkillLite 下载包 */
  detail?: string | null;
}

interface RuntimeUiSnapshot {
  python: RuntimeUiLine;
  node: RuntimeUiLine;
  cacheRoot: string | null;
  cacheRootAbs: string | null;
}

interface ProvisionRuntimeItem {
  requested: boolean;
  ok: boolean;
  message: string;
}

interface ProvisionRuntimesResult {
  python: ProvisionRuntimeItem;
  node: ProvisionRuntimeItem;
}

/** Python / Node 是否仍缺可用来源（需首次拉内置包） */
function needsFirstTimeRuntimeProvision(r: RuntimeUiSnapshot | null): boolean {
  if (!r) return true;
  return r.python.source === "none" || r.node.source === "none";
}

type ProvisionPhase = "pending" | "python" | "node";

function runtimeSourceBadgeLabel(source: RuntimeSource): string {
  return translate(`runtime.badge.${source}`);
}

function provisionPhaseI18n(phase: ProvisionPhase): string {
  return translate(`runtime.phase.${phase}`);
}

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
  const [runtime, setRuntime] = useState<RuntimeUiSnapshot | null>(null);
  const [provisionBusy, setProvisionBusy] = useState(false);
  const [provisionNote, setProvisionNote] = useState<string | null>(null);
  const [provisionProgress, setProvisionProgress] = useState<{
    phase: ProvisionPhase;
    message: string;
    percent: number | null;
  } | null>(null);
  const provisioningRef = useRef(false);

  const loadRuntime = useCallback(() => {
    invoke<RuntimeUiSnapshot>("skilllite_runtime_status")
      .then(setRuntime)
      .catch(() => setRuntime(null));
  }, []);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  useEffect(() => {
    loadRuntime();
  }, [loadRuntime]);

  useEffect(() => {
    const onVisibility = () => {
      if (document.visibilityState === "visible") loadRuntime();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, [loadRuntime]);

  useEffect(() => {
    window.addEventListener(RUNTIME_STATUS_REFRESH_EVENT, loadRuntime);
    return () => window.removeEventListener(RUNTIME_STATUS_REFRESH_EVENT, loadRuntime);
  }, [loadRuntime]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<{
      phase?: string;
      message?: string;
      percent?: number | null;
    }>("skilllite-runtime-provision-progress", (ev) => {
      const msg = ev.payload?.message ?? "";
      if (!msg) return;
      const rawPhase = ev.payload?.phase ?? "";
      const phase: ProvisionPhase =
        rawPhase === "node" ? "node" : rawPhase === "python" ? "python" : "pending";
      let percent: number | null = null;
      const p = ev.payload?.percent;
      if (typeof p === "number" && !Number.isNaN(p)) {
        percent = Math.min(100, Math.max(0, Math.round(p)));
      }
      setProvisionProgress({
        phase,
        message: msg,
        percent,
      });
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, []);

  const revealInFileManager = useCallback((path: string | null | undefined) => {
    if (!path?.trim()) return;
    invoke("skilllite_reveal_in_file_manager", { path: path.trim() }).catch((err) => {
      console.error("[skilllite-assistant] reveal_in_file_manager failed:", err);
      useUiToastStore
        .getState()
        .show(translate("toast.revealFmFailed", { err: formatInvokeError(err) }), "error");
    });
  }, []);

  const runProvision = useCallback(
    async (force: boolean) => {
      if (provisioningRef.current) return;
      provisioningRef.current = true;
      setProvisionBusy(true);
      setProvisionNote(null);
      const firstTime = needsFirstTimeRuntimeProvision(runtime);
      setProvisionProgress({
        phase: "pending",
        message: force
          ? translate("runtime.provision.force")
          : firstTime
            ? translate("runtime.provision.first")
            : translate("runtime.provision.update"),
        percent: null,
      });
      try {
        const r = await invoke<ProvisionRuntimesResult>("skilllite_provision_runtimes", {
          python: true,
          node: true,
          force,
        });
        const parts: string[] = [];
        if (r.python.requested) {
          parts.push(
            translate(
              r.python.ok
                ? "runtime.provisionLine.pythonOk"
                : "runtime.provisionLine.pythonFail",
              { msg: r.python.message }
            )
          );
        }
        if (r.node.requested) {
          parts.push(
            translate(
              r.node.ok ? "runtime.provisionLine.nodeOk" : "runtime.provisionLine.nodeFail",
              { msg: r.node.message }
            )
          );
        }
        setProvisionNote(parts.join(" · ") || translate("runtime.provisionDone"));
        loadRuntime();
      } catch (e) {
        setProvisionNote(e instanceof Error ? e.message : String(e));
      } finally {
        setProvisionProgress(null);
        provisioningRef.current = false;
        setProvisionBusy(false);
      }
    },
    [loadRuntime, runtime]
  );

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

      {runtime && (
        <div className="shrink-0 border-t border-border dark:border-border-dark px-3 py-2 space-y-1.5 bg-ink/[0.02] dark:bg-white/[0.02]">
          <div className="text-[10px] font-medium text-ink-mute dark:text-ink-dark-mute uppercase tracking-wider">
            {t("session.runtimeHeader")}
          </div>
          <div className="space-y-1">
            {(
              [
                { key: "py", title: t("session.runtimeRowPython"), line: runtime.python },
                { key: "node", title: t("session.runtimeRowNode"), line: runtime.node },
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
                  {runtimeSourceBadgeLabel(line.source)}
                </span>
              );
              const text = (
                <div className="min-w-0 flex-1">
                  <div>
                    <span className="text-ink-mute dark:text-ink-dark-mute">{title}</span>
                    <span className="text-ink dark:text-ink-dark ml-1">{line.label}</span>
                  </div>
                  {line.detail ? (
                    <p className="text-[10px] text-ink-mute dark:text-ink-dark-mute mt-0.5 leading-snug">
                      {line.detail}
                    </p>
                  ) : null}
                </div>
              );
              return canReveal ? (
                <button
                  key={key}
                  type="button"
                  onClick={() => revealInFileManager(line.revealPath)}
                  className={rowClass}
                  title={translate("session.revealInFmTitle", { label: line.label })}
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
          <div className="flex flex-col gap-2 pt-1 border-t border-border/60 dark:border-border-dark/60">
            {provisionBusy ? (
              <div
                className="rounded-lg border border-accent/30 bg-accent/[0.06] dark:bg-accent/10 px-2.5 py-2 space-y-2"
                aria-live="polite"
                role="status"
                aria-busy="true"
              >
                <div className="flex items-center justify-between gap-2 min-w-0">
                  <span className="text-[11px] font-semibold text-accent shrink-0">
                    {t("session.downloadInProgress")}
                  </span>
                  <div className="flex items-center gap-1.5 min-w-0 justify-end">
                    {provisionProgress ? (
                      <>
                        <span
                          className={`shrink-0 rounded px-1 py-px text-[9px] font-semibold uppercase tracking-wide ${
                            provisionProgress.phase === "python"
                              ? "bg-emerald-500/15 text-emerald-800 dark:text-emerald-300"
                              : provisionProgress.phase === "node"
                                ? "bg-sky-500/15 text-sky-900 dark:text-sky-200"
                                : "bg-ink/10 text-ink-mute dark:text-ink-dark-mute"
                          }`}
                        >
                          {provisionPhaseI18n(provisionProgress.phase)}
                        </span>
                        {provisionProgress.percent != null ? (
                          <span className="text-[9px] font-mono text-ink-mute dark:text-ink-dark-mute tabular-nums shrink-0">
                            {provisionProgress.percent}%
                          </span>
                        ) : null}
                      </>
                    ) : null}
                  </div>
                </div>
                <div
                  className="h-1.5 w-full rounded-full bg-ink/10 dark:bg-white/10 overflow-hidden"
                  role="progressbar"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-busy={provisionProgress == null || provisionProgress.percent == null}
                  aria-valuenow={
                    provisionProgress?.percent != null
                      ? provisionProgress.percent
                      : undefined
                  }
                  aria-label={t("session.provisionProgressAria")}
                >
                  {provisionProgress?.percent != null ? (
                    <div
                      className="h-full rounded-full bg-accent transition-[width] duration-200 ease-out"
                      style={{ width: `${provisionProgress.percent}%` }}
                    />
                  ) : (
                    <div className="h-full w-full rounded-full bg-accent/25 relative overflow-hidden">
                      <div className="absolute inset-y-0 left-0 w-2/5 rounded-full bg-accent/90 motion-safe:animate-pulse" />
                    </div>
                  )}
                </div>
                <p className="text-[10px] text-ink dark:text-ink-dark leading-snug font-mono break-words">
                  {provisionProgress
                    ? formatRuntimeProvisionProgress(provisionProgress.message)
                    : t("session.provisionWait")}
                </p>
                <p className="text-[9px] text-ink-mute dark:text-ink-dark-mute leading-snug">
                  {t("session.provisionKeepOpen")}
                </p>
              </div>
            ) : (
              <div className="flex flex-col gap-1.5">
                <button
                  type="button"
                  onClick={() => void runProvision(false)}
                  title={
                    needsFirstTimeRuntimeProvision(runtime)
                      ? t("session.provisionTitleFirst")
                      : t("session.provisionTitleUpdate")
                  }
                  className="w-full rounded-md px-2 py-1.5 text-[11px] font-medium bg-accent/10 text-accent hover:bg-accent/15 dark:text-accent border border-accent/20 transition-colors"
                >
                  {needsFirstTimeRuntimeProvision(runtime)
                    ? t("session.provisionBtnFirst")
                    : t("session.provisionBtnUpdate")}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    if (!window.confirm(t("session.provisionForceConfirm"))) {
                      return;
                    }
                    void runProvision(true);
                  }}
                  className="w-full rounded-md px-2 py-1.5 text-[11px] font-medium border border-amber-500/45 text-amber-900 dark:text-amber-200 bg-amber-500/10 hover:bg-amber-500/15 dark:border-amber-400/40 transition-colors"
                  title={t("session.provisionForceTitle")}
                >
                  {t("session.provisionForceBtn")}
                </button>
              </div>
            )}
            {provisionNote && !provisionBusy ? (
              <p className="text-[10px] text-ink-mute dark:text-ink-dark-mute leading-snug px-0.5">
                {provisionNote}
              </p>
            ) : null}
          </div>
          {runtime.cacheRoot && (
            <button
              type="button"
              disabled={!runtime.cacheRootAbs}
              onClick={() => revealInFileManager(runtime.cacheRootAbs)}
              className="text-[10px] text-ink-mute dark:text-ink-dark-mute font-mono truncate w-full text-left rounded px-1 py-0.5 -mx-1 transition-colors hover:text-accent dark:hover:text-accent hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-ink-mute dark:disabled:hover:text-ink-dark-mute focus:outline-none focus-visible:ring-1 focus-visible:ring-accent/50"
              title={
                runtime.cacheRootAbs
                  ? t("session.cacheRevealTitle")
                  : t("session.cacheRevealDisabled")
              }
            >
              {t("session.runtimeDirLine", { path: runtime.cacheRoot })}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
