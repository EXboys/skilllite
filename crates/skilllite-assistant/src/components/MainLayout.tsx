import { useState, useEffect, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useGlobalShortcut } from "../hooks/useGlobalShortcut";
import ChatView from "./ChatView";
import StatusPanel, { LifePulseBadge } from "./StatusPanel";
import SessionSidebar from "./SessionSidebar";
import WorkspaceFileTree from "./WorkspaceFileTree";
import WorkspaceIdeEditor from "./WorkspaceIdeEditor";
import SettingsModal from "./SettingsModal";
import OnboardingModal from "./OnboardingModal";
import { useRecentData } from "../hooks/useRecentData";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useIdeFileOpenerStore } from "../stores/useIdeFileOpenerStore";
import { useSessionStore } from "../stores/useSessionStore";
import { useUiToastStore } from "../stores/useUiToastStore";
import { useI18n } from "../i18n";

export default function MainLayout() {
  const { t } = useI18n();
  useGlobalShortcut();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<{
      message?: string;
      kind?: string;
      /** 后端显式分级；缺省时按错误处理 */
      severity?: "info" | "error";
    }>("skilllite-chrome-bootstrap", (ev) => {
      const msg = ev.payload?.message;
      if (!msg) return;
      const variant =
        ev.payload.severity === "info" ? "info" : "error";
      useUiToastStore.getState().show(msg, variant);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);
  const { settings, setSettings } = useSettingsStore();
  const currentSessionKey = useSessionStore((s) => s.currentSessionKey);
  const sessions = useSessionStore((s) => s.sessions);
  const renameSession = useSessionStore((s) => s.renameSession);
  const leftPanelCollapsed = settings.sessionPanelCollapsed ?? false;
  const setLeftPanelCollapsed = (v: boolean) => setSettings({ sessionPanelCollapsed: v });
  const [rightPanelCollapsed, setRightPanelCollapsed] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const ideLayout = settings.ideLayout === true;
  type IdeLeftTab = "files" | "sessions";
  const [ideLeftTab, setIdeLeftTab] = useState<IdeLeftTab>("files");
  const [ideSelectedFile, setIdeSelectedFile] = useState<string | null>(null);
  const [ideTreeRefresh, setIdeTreeRefresh] = useState(0);
  const { refreshRecentData } = useRecentData();
  const showOnboarding = settings.onboardingCompleted === false;

  const currentSession = sessions.find(
    (s) => s.session_key === currentSessionKey
  );
  const currentSessionName =
    currentSession?.display_name ??
    (currentSessionKey === "default" ? t("session.defaultDisplayName") : currentSessionKey);

  const [titleEditing, setTitleEditing] = useState(false);
  const [titleDraft, setTitleDraft] = useState(currentSessionName);
  const titleInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!titleEditing) setTitleDraft(currentSessionName);
  }, [currentSessionName, titleEditing]);

  useEffect(() => {
    if (titleEditing && titleInputRef.current) {
      titleInputRef.current.focus();
      titleInputRef.current.select();
    }
  }, [titleEditing]);

  const commitSessionTitle = useCallback(async () => {
    const t = titleDraft.trim();
    if (t && t !== currentSessionName) {
      await renameSession(currentSessionKey, t);
    }
    setTitleEditing(false);
  }, [titleDraft, currentSessionName, currentSessionKey, renameSession]);

  useEffect(() => {
    refreshRecentData();
  }, [refreshRecentData, settings.workspace]);

  useEffect(() => {
    setIdeSelectedFile(null);
  }, [settings.workspace]);

  const pendingIdeFile = useIdeFileOpenerStore((s) => s.pendingRelativePath);
  useEffect(() => {
    if (!pendingIdeFile) return;
    useIdeFileOpenerStore.getState().clearPending();
    setIdeSelectedFile(pendingIdeFile);
    setIdeLeftTab("files");
    setSettings({ ideLayout: true, sessionPanelCollapsed: false });
  }, [pendingIdeFile, setSettings]);

  const toggleIdeLayout = useCallback(() => {
    const next = !ideLayout;
    setSettings(
      next
        ? { ideLayout: true, sessionPanelCollapsed: false }
        : { ideLayout: false }
    );
  }, [ideLayout, setSettings]);

  return (
    <div className="flex flex-col h-screen bg-surface dark:bg-surface-dark">
      {/* Top bar */}
      <header className="flex items-center justify-between h-12 px-4 border-b border-border dark:border-border-dark bg-white dark:bg-paper-dark shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <button
            type="button"
            onClick={() => setLeftPanelCollapsed(!leftPanelCollapsed)}
            className="p-1.5 text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark rounded-md hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            aria-label={
              leftPanelCollapsed
                ? ideLayout
                  ? t("main.expandIdeSidebar")
                  : t("main.expandSessions")
                : ideLayout
                  ? t("main.collapseIdeSidebar")
                  : t("main.collapseSessions")
            }
            title={
              leftPanelCollapsed
                ? ideLayout
                  ? t("main.expandIdeSidebar")
                  : t("main.expandSessions")
                : ideLayout
                  ? t("main.collapseIdeSidebar")
                  : t("main.collapseSessions")
            }
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <line x1="3" y1="6" x2="21" y2="6" />
              <line x1="3" y1="12" x2="21" y2="12" />
              <line x1="3" y1="18" x2="21" y2="18" />
            </svg>
          </button>
          <h1 className="text-base font-semibold tracking-tight text-ink dark:text-ink-dark shrink-0">
            SkillLite
          </h1>
          {titleEditing ? (
            <input
              ref={titleInputRef}
              type="text"
              value={titleDraft}
              onChange={(e) => setTitleDraft(e.target.value)}
              onBlur={() => void commitSessionTitle()}
              onKeyDown={(e) => {
                if (e.key === "Enter") void commitSessionTitle();
                if (e.key === "Escape") {
                  setTitleDraft(currentSessionName);
                  setTitleEditing(false);
                }
              }}
              className="text-sm text-ink dark:text-ink-dark min-w-[8rem] max-w-[min(280px,40vw)] px-1 py-0.5 rounded border border-accent/40 bg-white dark:bg-paper-dark outline-none focus:ring-1 focus:ring-accent"
              aria-label={t("main.sessionTitleAria")}
            />
          ) : (
            <button
              type="button"
              onClick={() => setTitleEditing(true)}
              className="text-sm text-ink-mute dark:text-ink-dark-mute truncate text-left max-w-[min(280px,40vw)] hover:text-ink dark:hover:text-ink-dark rounded px-1 -mx-1 py-0.5 hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
              title={t("main.editSessionTitle")}
            >
              — {currentSessionName}
            </button>
          )}
        </div>
        <div className="flex items-center gap-1">
          <LifePulseBadge />
          <button
            type="button"
            onClick={toggleIdeLayout}
            className={`px-2 py-1.5 text-sm rounded-md transition-colors ${
              ideLayout
                ? "text-accent bg-accent/10 dark:bg-accent/15"
                : "text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent hover:bg-ink/5 dark:hover:bg-white/5"
            }`}
            aria-pressed={ideLayout}
            aria-label={t("main.ideLayoutHint")}
            title={t("main.ideLayoutHint")}
          >
            {t("main.ideLayout")}
          </button>
          <button
            type="button"
            onClick={() => setSettingsOpen((v) => !v)}
            className={`px-2 py-1.5 text-sm rounded-md transition-colors ${
              settingsOpen
                ? "text-accent bg-accent/10 dark:bg-accent/15"
                : "text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent hover:bg-ink/5 dark:hover:bg-white/5"
            }`}
            aria-label={t("main.settings")}
            aria-pressed={settingsOpen}
          >
            {t("main.settings")}
          </button>
        </div>
      </header>

      {/* Main content */}
      <div className="flex flex-1 min-h-0">
        {ideLayout ? (
          <>
            {!leftPanelCollapsed && (
              <aside className="w-[220px] min-w-[180px] shrink-0 border-r border-border dark:border-border-dark bg-white dark:bg-paper-dark flex flex-col min-h-0">
                <div className="shrink-0 flex border-b border-border dark:border-border-dark">
                  <button
                    type="button"
                    onClick={() => setIdeLeftTab("files")}
                    className={`flex-1 py-2 text-xs font-medium transition-colors ${
                      ideLeftTab === "files"
                        ? "text-accent border-b-2 border-accent bg-accent/5 dark:bg-accent/10"
                        : "text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
                    }`}
                  >
                    {t("ide.tabFiles")}
                  </button>
                  <button
                    type="button"
                    onClick={() => setIdeLeftTab("sessions")}
                    className={`flex-1 py-2 text-xs font-medium transition-colors ${
                      ideLeftTab === "sessions"
                        ? "text-accent border-b-2 border-accent bg-accent/5 dark:bg-accent/10"
                        : "text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
                    }`}
                  >
                    {t("ide.tabSessions")}
                  </button>
                </div>
                <div className="flex-1 min-h-0 overflow-hidden">
                  {ideLeftTab === "files" ? (
                    <WorkspaceFileTree
                      workspace={settings.workspace || "."}
                      selectedPath={ideSelectedFile}
                      onSelectFile={setIdeSelectedFile}
                      refreshToken={ideTreeRefresh}
                    />
                  ) : (
                    <div className="h-full min-h-0 overflow-y-auto overflow-x-hidden">
                      <SessionSidebar />
                    </div>
                  )}
                </div>
              </aside>
            )}
            <section className="flex-1 min-w-0 min-h-0 overflow-hidden bg-surface dark:bg-surface-dark">
              <WorkspaceIdeEditor
                workspace={settings.workspace || "."}
                relativePath={ideSelectedFile}
                onSaved={() => setIdeTreeRefresh((n) => n + 1)}
              />
            </section>
            <main className="w-[min(420px,38vw)] min-w-[260px] max-w-[520px] shrink-0 overflow-hidden border-l border-border dark:border-border-dark bg-white dark:bg-paper-dark">
              <ChatView key={currentSessionKey} />
            </main>
          </>
        ) : (
          <>
            {!leftPanelCollapsed && (
              <aside className="w-[220px] min-w-[180px] shrink-0 border-r border-border dark:border-border-dark bg-white dark:bg-paper-dark">
                <SessionSidebar />
              </aside>
            )}
            <main className="flex-1 min-w-0 overflow-hidden">
              <ChatView key={currentSessionKey} />
            </main>
            <div className="flex min-h-0 shrink-0">
              <aside
                className={`relative flex flex-col bg-white dark:bg-paper-dark transition-[width] duration-200 shrink-0 ${
                  rightPanelCollapsed ? "w-10 min-w-10" : "w-[280px] min-w-[200px]"
                }`}
              >
                <div
                  className="pointer-events-none absolute inset-y-0 left-0 w-px bg-border dark:bg-border-dark"
                  aria-hidden
                />
                <button
                  type="button"
                  onClick={() => setRightPanelCollapsed(!rightPanelCollapsed)}
                  className="absolute left-0 top-1/2 z-10 flex h-7 w-7 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border border-border dark:border-border-dark bg-white dark:bg-paper-dark text-ink-mute dark:text-ink-dark-mute shadow-sm hover:bg-ink/5 dark:hover:bg-white/10 hover:text-ink dark:hover:text-ink-dark transition-colors"
                  aria-label={
                    rightPanelCollapsed ? t("main.expandPanel") : t("main.collapsePanel")
                  }
                  title={
                    rightPanelCollapsed ? t("main.expandPanel") : t("main.collapsePanel")
                  }
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
                    aria-hidden
                  >
                    {rightPanelCollapsed ? (
                      <path d="M15 18l-6-6 6-6" />
                    ) : (
                      <path d="M9 18l6-6-6-6" />
                    )}
                  </svg>
                </button>
                {!rightPanelCollapsed && (
                  <div className="min-h-0 min-w-0 flex-1 overflow-y-auto overflow-x-hidden pl-5 pr-3 pb-3 pt-3">
                    <StatusPanel />
                  </div>
                )}
              </aside>
            </div>
          </>
        )}
        <SettingsModal open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      </div>
      {showOnboarding && <OnboardingModal />}
    </div>
  );
}
