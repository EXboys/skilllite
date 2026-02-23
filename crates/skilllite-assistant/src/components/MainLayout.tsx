import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useGlobalShortcut } from "../hooks/useGlobalShortcut";
import { invoke } from "@tauri-apps/api/core";
import ChatView from "./ChatView";
import StatusPanel from "./StatusPanel";
import SettingsModal from "./SettingsModal";
import { useStatusStore } from "../stores/useStatusStore";

export default function MainLayout() {
  useGlobalShortcut();
  const [rightPanelCollapsed, setRightPanelCollapsed] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const setRecentData = useStatusStore((s) => s.setRecentData);

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

  const handleHideToTray = async () => {
    const win = getCurrentWindow();
    await win.hide();
  };

  return (
    <div className="flex flex-col h-screen bg-surface dark:bg-surface-dark">
      {/* Top bar */}
      <header className="flex items-center justify-between h-12 px-4 border-b border-border dark:border-border-dark bg-white dark:bg-paper-dark shrink-0">
        <h1 className="text-base font-semibold tracking-tight text-ink dark:text-ink-dark">
          SkillLite
        </h1>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => setSettingsOpen(true)}
            className="px-2 py-1.5 text-sm text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent rounded-md hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            aria-label="Settings"
          >
            设置
          </button>
          <button
            type="button"
            onClick={handleHideToTray}
            className="px-2 py-1.5 text-sm text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark rounded-md hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            aria-label="Hide to tray"
            title="隐藏到托盘"
          >
            最小化
          </button>
        </div>
      </header>

      {/* Main content: Chat | StatusPanel */}
      <div className="flex flex-1 min-h-0">
        {/* Left: ChatView */}
        <main className="flex-1 min-w-0 overflow-hidden">
          <ChatView />
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
    </div>
  );
}
