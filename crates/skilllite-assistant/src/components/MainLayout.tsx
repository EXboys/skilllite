import { useState, useEffect, useRef, useCallback } from "react";
import { useGlobalShortcut } from "../hooks/useGlobalShortcut";
import ChatView from "./ChatView";
import StatusPanel from "./StatusPanel";
import SessionSidebar from "./SessionSidebar";
import SettingsModal from "./SettingsModal";
import OnboardingModal from "./OnboardingModal";
import { useRecentData } from "../hooks/useRecentData";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useSessionStore } from "../stores/useSessionStore";

export default function MainLayout() {
  useGlobalShortcut();
  const { settings, setSettings } = useSettingsStore();
  const currentSessionKey = useSessionStore((s) => s.currentSessionKey);
  const sessions = useSessionStore((s) => s.sessions);
  const renameSession = useSessionStore((s) => s.renameSession);
  const leftPanelCollapsed = settings.sessionPanelCollapsed ?? false;
  const setLeftPanelCollapsed = (v: boolean) => setSettings({ sessionPanelCollapsed: v });
  const [rightPanelCollapsed, setRightPanelCollapsed] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { refreshRecentData } = useRecentData();
  const showOnboarding = settings.onboardingCompleted === false;

  const currentSession = sessions.find(
    (s) => s.session_key === currentSessionKey
  );
  const currentSessionName = currentSession?.display_name ?? (currentSessionKey === "default" ? "默认会话" : currentSessionKey);

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
  }, [refreshRecentData]);

  return (
    <div className="flex flex-col h-screen bg-surface dark:bg-surface-dark">
      {/* Top bar */}
      <header className="flex items-center justify-between h-12 px-4 border-b border-border dark:border-border-dark bg-white dark:bg-paper-dark shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <button
            type="button"
            onClick={() => setLeftPanelCollapsed(!leftPanelCollapsed)}
            className="p-1.5 text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark rounded-md hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            aria-label={leftPanelCollapsed ? "展开会话列表" : "收起会话列表"}
            title={leftPanelCollapsed ? "展开会话列表" : "收起会话列表"}
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
              aria-label="会话标题"
            />
          ) : (
            <button
              type="button"
              onClick={() => setTitleEditing(true)}
              className="text-sm text-ink-mute dark:text-ink-dark-mute truncate text-left max-w-[min(280px,40vw)] hover:text-ink dark:hover:text-ink-dark rounded px-1 -mx-1 py-0.5 hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
              title="点击修改会话标题"
            >
              — {currentSessionName}
            </button>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => setSettingsOpen(true)}
            className="px-2 py-1.5 text-sm text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent rounded-md hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            aria-label="Settings"
          >
            设置
          </button>
        </div>
      </header>

      {/* Main content: SessionSidebar | Chat | StatusPanel */}
      <div className="flex flex-1 min-h-0">
        {/* Left: Session Sidebar */}
        {!leftPanelCollapsed && (
          <aside className="w-[220px] min-w-[180px] shrink-0 border-r border-border dark:border-border-dark bg-white dark:bg-paper-dark">
            <SessionSidebar />
          </aside>
        )}

        {/* Center: ChatView — key 强制卸载/重挂载以隔离会话间状态 */}
        <main className="flex-1 min-w-0 overflow-hidden">
          <ChatView key={currentSessionKey} />
        </main>

        {/* Right: StatusPanel */}
        <aside
          className={`flex flex-col border-l border-border dark:border-border-dark bg-white dark:bg-paper-dark transition-all duration-200 ${
            rightPanelCollapsed ? "w-10 shrink-0" : "w-[280px] min-w-[200px] shrink-0"
          }`}
        >
          <div className="flex items-center h-10 px-2 border-b border-border dark:border-border-dark shrink-0">
            <button
              type="button"
              onClick={() => setRightPanelCollapsed(!rightPanelCollapsed)}
              className="ml-auto p-2 text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark rounded-md hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
              aria-label={rightPanelCollapsed ? "Expand panel" : "Collapse panel"}
              title={rightPanelCollapsed ? "展开" : "收起"}
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
                className={rightPanelCollapsed ? "rotate-180" : ""}
              >
                <path d="M15 18l-6-6 6-6" />
              </svg>
            </button>
          </div>
          {!rightPanelCollapsed && (
            <div className="flex-1 overflow-auto">
              <StatusPanel />
            </div>
          )}
        </aside>
      </div>

      <SettingsModal open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      {showOnboarding && <OnboardingModal />}
    </div>
  );
}
