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
    <div className="flex flex-col h-screen bg-gray-50 dark:bg-gray-900">
      {/* Top bar */}
      <header className="flex items-center justify-between h-12 px-4 border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 shrink-0">
        <h1 className="text-lg font-semibold text-gray-900 dark:text-white">
          SkillLite Assistant
        </h1>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setSettingsOpen(true)}
            className="px-2 py-1 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
            aria-label="Settings"
          >
            Settings
          </button>
          <button
            type="button"
            onClick={handleHideToTray}
            className="px-2 py-1 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
            aria-label="Hide to tray"
            title="隐藏到托盘"
          >
            Tray
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
          className={`flex flex-col border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 transition-all duration-200 ${
            rightPanelCollapsed ? "w-10 shrink-0" : "w-[280px] min-w-[200px] shrink-0"
          }`}
        >
          <div className="flex items-center h-10 px-2 border-b border-gray-200 dark:border-gray-700 shrink-0">
            <button
              type="button"
              onClick={() => setRightPanelCollapsed(!rightPanelCollapsed)}
              className="ml-auto p-2 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
              aria-label={rightPanelCollapsed ? "Expand panel" : "Collapse panel"}
              title={rightPanelCollapsed ? "Expand" : "Collapse"}
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
