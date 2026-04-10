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
import IdePanelResizeHandle from "./IdePanelResizeHandle";
import { useRecentData } from "../hooks/useRecentData";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useIdeFileOpenerStore } from "../stores/useIdeFileOpenerStore";
import { useSessionStore } from "../stores/useSessionStore";
import { useUiToastStore } from "../stores/useUiToastStore";
import { useI18n } from "../i18n";

const IDE_MIN_EDITOR_PX = 160;
const IDE_DEFAULT_SIDEBAR_PX = 220;
const IDE_MIN_SIDEBAR_PX = 180;
const IDE_MAX_SIDEBAR_PX = 400;
const IDE_DEFAULT_CHAT_PX = 420;
const IDE_MIN_CHAT_PX = 260;
const IDE_MAX_CHAT_PX = 520;

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
  const leftPanelCollapsed = settings.sessionPanelCollapsed ?? false;
  const setLeftPanelCollapsed = (v: boolean) => setSettings({ sessionPanelCollapsed: v });
  const [rightPanelCollapsed, setRightPanelCollapsed] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const ideLayout = settings.ideLayout === true;
  type IdeLeftTab = "files" | "sessions";
  const [ideLeftTab, setIdeLeftTab] = useState<IdeLeftTab>("files");
  const [ideSelectedFile, setIdeSelectedFile] = useState<string | null>(null);
  const [ideTreeRefresh, setIdeTreeRefresh] = useState(0);
  const ideRowRef = useRef<HTMLDivElement>(null);
  const [liveIdeSidebarW, setLiveIdeSidebarW] = useState<number | null>(null);
  const [liveIdeChatW, setLiveIdeChatW] = useState<number | null>(null);
  const { refreshRecentData } = useRecentData();
  const showOnboarding = settings.onboardingCompleted === false;

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

  const setMainLayoutMode = useCallback(
    (mode: "chat" | "ide") => {
      if (mode === "ide") {
        setSettings({ ideLayout: true, sessionPanelCollapsed: false });
      } else {
        setSettings({ ideLayout: false });
      }
    },
    [setSettings]
  );

  const ideSidebarW =
    liveIdeSidebarW ?? settings.ideSidebarWidthPx ?? IDE_DEFAULT_SIDEBAR_PX;
  const ideChatW =
    liveIdeChatW ?? settings.ideChatWidthPx ?? IDE_DEFAULT_CHAT_PX;

  const clampIdeSidebar = useCallback(
    (w: number) => {
      const cw = ideRowRef.current?.clientWidth ?? window.innerWidth;
      const chat =
        liveIdeChatW ?? settings.ideChatWidthPx ?? IDE_DEFAULT_CHAT_PX;
      const max = Math.max(
        IDE_MIN_SIDEBAR_PX,
        Math.min(IDE_MAX_SIDEBAR_PX, cw - chat - IDE_MIN_EDITOR_PX)
      );
      return Math.round(Math.min(Math.max(w, IDE_MIN_SIDEBAR_PX), max));
    },
    [liveIdeChatW, settings.ideChatWidthPx]
  );

  const clampIdeChat = useCallback(
    (w: number) => {
      const cw = ideRowRef.current?.clientWidth ?? window.innerWidth;
      const sb = leftPanelCollapsed
        ? 0
        : (liveIdeSidebarW ??
          settings.ideSidebarWidthPx ??
          IDE_DEFAULT_SIDEBAR_PX);
      const max = Math.max(
        IDE_MIN_CHAT_PX,
        Math.min(IDE_MAX_CHAT_PX, cw - sb - IDE_MIN_EDITOR_PX)
      );
      return Math.round(Math.min(Math.max(w, IDE_MIN_CHAT_PX), max));
    },
    [leftPanelCollapsed, liveIdeSidebarW, settings.ideSidebarWidthPx]
  );

  return (
    <div className="flex flex-col h-screen bg-surface dark:bg-surface-dark">
      {/* Top bar */}
      <header className="flex items-center justify-between h-12 px-4 border-b border-border dark:border-border-dark bg-white dark:bg-paper-dark shrink-0">
        <div className="flex items-center gap-2 min-w-0 flex-1">
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
          <div
            className="flex shrink-0 rounded-lg border border-border dark:border-border-dark bg-ink/[0.04] dark:bg-white/[0.06] p-0.5 gap-0.5"
            role="tablist"
            aria-label={t("main.modeSwitchAria")}
          >
            <button
              type="button"
              role="tab"
              aria-selected={!ideLayout}
              tabIndex={!ideLayout ? 0 : -1}
              onClick={() => setMainLayoutMode("chat")}
              className={`px-2.5 py-1 min-h-[1.75rem] text-sm font-medium rounded-md transition-colors ${
                !ideLayout
                  ? "bg-white dark:bg-paper-dark text-accent shadow-sm ring-1 ring-black/[0.06] dark:ring-white/10"
                  : "text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
              }`}
              title={t("main.modeChatHint")}
            >
              {t("main.modeChat")}
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={ideLayout}
              tabIndex={ideLayout ? 0 : -1}
              onClick={() => setMainLayoutMode("ide")}
              className={`px-2.5 py-1 min-h-[1.75rem] text-sm font-medium rounded-md transition-colors ${
                ideLayout
                  ? "bg-white dark:bg-paper-dark text-accent shadow-sm ring-1 ring-black/[0.06] dark:ring-white/10"
                  : "text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
              }`}
              title={t("main.ideLayoutHint")}
            >
              {t("main.ideLayout")}
            </button>
          </div>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <LifePulseBadge />
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
          <div
            ref={ideRowRef}
            className="flex flex-1 min-w-0 min-h-0"
          >
            {!leftPanelCollapsed && (
              <>
                <aside
                  style={{ width: ideSidebarW }}
                  className="min-w-0 shrink-0 bg-white dark:bg-paper-dark flex flex-col min-h-0"
                >
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
                <IdePanelResizeHandle
                  ariaLabel={t("ide.resizeSidebar")}
                  direction={1}
                  getStartWidth={() => ideSidebarW}
                  clamp={clampIdeSidebar}
                  onDrag={setLiveIdeSidebarW}
                  onCommit={(w) => {
                    setSettings({ ideSidebarWidthPx: w });
                    setLiveIdeSidebarW(null);
                  }}
                />
              </>
            )}
            <section className="flex-1 min-w-0 min-h-0 overflow-hidden bg-surface dark:bg-surface-dark">
              <WorkspaceIdeEditor
                workspace={settings.workspace || "."}
                relativePath={ideSelectedFile}
                onSaved={() => setIdeTreeRefresh((n) => n + 1)}
              />
            </section>
            <IdePanelResizeHandle
              ariaLabel={t("ide.resizeChat")}
              direction={-1}
              getStartWidth={() => ideChatW}
              clamp={clampIdeChat}
              onDrag={setLiveIdeChatW}
              onCommit={(w) => {
                setSettings({ ideChatWidthPx: w });
                setLiveIdeChatW(null);
              }}
            />
            <main
              style={{ width: ideChatW }}
              className="min-w-0 shrink-0 overflow-hidden bg-white dark:bg-paper-dark"
            >
              <ChatView key={currentSessionKey} />
            </main>
          </div>
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
      </div>
      <SettingsModal open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      {showOnboarding && <OnboardingModal />}
    </div>
  );
}
