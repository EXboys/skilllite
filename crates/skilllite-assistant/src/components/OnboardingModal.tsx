import { useState, useEffect, useRef } from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "../stores/useSettingsStore";

type Step = "mode" | "config" | "workspace";
type Mode = "api" | "ollama";

export default function OnboardingModal() {
  const { setSettings } = useSettingsStore();
  const [step, setStep] = useState<Step>("mode");
  const [mode, setMode] = useState<Mode | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("gpt-4o");
  const [workspace, setWorkspace] = useState("");
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [ollamaModel, setOllamaModel] = useState<string | null>(null);
  const [ollamaLoading, setOllamaLoading] = useState(false);
  const [ollamaAutoDetected, setOllamaAutoDetected] = useState(false);
  const [initCreating, setInitCreating] = useState(false);
  const [initError, setInitError] = useState("");
  const initialProbeDoneRef = useRef(false);
  const userChoseModeRef = useRef(false);

  // 零配置默认：挂载时检测本机 Ollama，可用则预选「使用本地模型」并进入配置步骤（若用户尚未点击任一选项）
  useEffect(() => {
    let cancelled = false;
    setOllamaLoading(true);
    invoke<{ available: boolean; models: string[]; has_embedding: boolean }>("skilllite_probe_ollama")
      .then((r) => {
        if (cancelled || userChoseModeRef.current) return;
        if (r.available) {
          const chatModels = r.models.filter((m) => !m.includes("embed"));
          if (chatModels.length > 0) {
            initialProbeDoneRef.current = true;
            setOllamaModels(chatModels);
            setOllamaModel(chatModels[0]);
            setMode("ollama");
            setStep("config");
            setOllamaAutoDetected(true);
          }
        }
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setOllamaLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (step === "mode") return;
    if (mode === "ollama") {
      if (initialProbeDoneRef.current) {
        setOllamaLoading(false);
        return;
      }
      setOllamaLoading(true);
      invoke<{ available: boolean; models: string[]; has_embedding: boolean }>("skilllite_probe_ollama")
        .then((r) => {
          if (r.available) {
            const chatModels = r.models.filter((m) => !m.includes("embed"));
            setOllamaModels(chatModels);
            if (chatModels.length > 0) setOllamaModel(chatModels[0]);
          }
        })
        .finally(() => setOllamaLoading(false));
    }
  }, [mode, step]);

  const handleComplete = () => {
    const ws = workspace.trim() || ".";
    if (mode === "ollama") {
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: ollamaModel || "llama3.2",
        workspace: ws,
        onboardingCompleted: true,
      });
    } else {
      setSettings({
        provider: "api",
        apiKey: apiKey.trim(),
        model: model.trim() || "gpt-4o",
        workspace: ws,
        onboardingCompleted: true,
      });
    }
  };

  const handleBrowseWorkspace = async () => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      title: "选择工作区目录",
      defaultPath: workspace || undefined,
    });
    if (selected) setWorkspace(selected);
  };

  const handleCreateExample = async () => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      title: "选择要创建示例工作区的目录",
    });
    if (!selected) return;
    setInitCreating(true);
    setInitError("");
    try {
      await invoke("skilllite_init_workspace", { dir: selected });
      setWorkspace(selected);
    } catch (e) {
      setInitError(String(e));
    } finally {
      setInitCreating(false);
    }
  };

  const canFinishWorkspace = true;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-ink/50 dark:bg-black/60 backdrop-blur-sm">
      <div
        className="relative w-full max-w-lg rounded-xl bg-white dark:bg-paper-dark p-6 shadow-2xl border border-border dark:border-border-dark"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold text-ink dark:text-ink-dark mb-1">
          欢迎使用 SkillLite
        </h2>
        <p className="text-sm text-ink-mute dark:text-ink-dark-mute mb-6">
          按下面步骤完成配置即可开始使用
        </p>
        <button
          type="button"
          onClick={() =>
            setSettings({
              onboardingCompleted: true,
              workspace: ".",
            })
          }
          className="absolute top-4 right-4 text-xs text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
        >
          稍后设置
        </button>

        {step === "mode" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              1. 选择使用方式
            </p>
            {ollamaLoading && mode === null && (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-2">
                正在检测本机 Ollama…
              </p>
            )}
            <div className="flex flex-col gap-2">
              <button
                type="button"
                onClick={() => {
                  userChoseModeRef.current = true;
                  setMode("api");
                  setStep("config");
                }}
                className="w-full text-left px-4 py-3 rounded-lg border border-border dark:border-border-dark hover:bg-gray-50 dark:hover:bg-white/5 transition-colors"
              >
                <span className="font-medium text-ink dark:text-ink-dark block">
                  配置 API Key
                </span>
                <span className="text-xs text-ink-mute dark:text-ink-dark-mute">
                  OpenAI / DeepSeek / 其他兼容接口
                </span>
              </button>
              <button
                type="button"
                onClick={() => {
                  userChoseModeRef.current = true;
                  setMode("ollama");
                  setStep("config");
                }}
                className="w-full text-left px-4 py-3 rounded-lg border border-border dark:border-border-dark hover:bg-gray-50 dark:hover:bg-white/5 transition-colors"
              >
                <span className="font-medium text-ink dark:text-ink-dark block">
                  使用本地 Ollama
                </span>
                <span className="text-xs text-ink-mute dark:text-ink-dark-mute">
                  无需 Key，需本机已安装并运行 Ollama
                </span>
              </button>
            </div>
          </>
        )}

        {step === "config" && mode === "api" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              2. 填写 API 配置
            </p>
            <div className="space-y-3 mb-4">
              <div>
                <label className="block text-xs text-ink-mute dark:text-ink-dark-mute mb-1">
                  API Key
                </label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="sk-..."
                  className="w-full rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-ink-mute dark:text-ink-dark-mute mb-1">
                  模型（可选）
                </label>
                <input
                  type="text"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder="gpt-4o"
                  className="w-full rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-sm"
                />
              </div>
            </div>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setStep("mode")}
                className="px-3 py-1.5 text-sm text-ink-mute"
              >
                上一步
              </button>
              <button
                type="button"
                onClick={() => setStep("workspace")}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium"
              >
                下一步
              </button>
            </div>
          </>
        )}

        {step === "config" && mode === "ollama" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              2. 本地 Ollama
            </p>
            {ollamaAutoDetected && (
              <p className="text-xs text-green-600 dark:text-green-400 mb-3">
                已检测到本机 Ollama，已为您选择「使用本地模型」，无需 API Key 即可聊天。
              </p>
            )}
            {ollamaLoading ? (
              <p className="text-sm text-ink-mute mb-4">正在检测 Ollama…</p>
            ) : ollamaModels.length > 0 ? (
              <p className="text-sm text-ink-mute dark:text-ink-dark-mute mb-4">
                已检测到 {ollamaModels.length} 个本地模型
              </p>
            ) : (
              <p className="text-sm text-ink-mute dark:text-ink-dark-mute mb-4">
                未检测到 Ollama 或未安装模型。请先安装并运行 Ollama，或在上一步选择「配置 API Key」。
              </p>
            )}
            {!ollamaLoading && ollamaModels.length > 0 && (
              <div className="mb-2">
                <label className="block text-xs text-ink-mute mb-1">使用模型</label>
                <select
                  value={ollamaModel || ""}
                  onChange={(e) => setOllamaModel(e.target.value)}
                  className="w-full rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-sm"
                >
                  {ollamaModels.map((m) => (
                    <option key={m} value={m}>{m}</option>
                  ))}
                </select>
              </div>
            )}
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => {
                  setStep("mode");
                  setOllamaAutoDetected(false);
                }}
                className="px-3 py-1.5 text-sm text-ink-mute"
              >
                上一步
              </button>
              <button
                type="button"
                onClick={() => setStep("workspace")}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium"
              >
                下一步
              </button>
            </div>
          </>
        )}

        {step === "workspace" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              3. 选择工作区
            </p>
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-3">
              工作区用于存放技能与对话数据，可后续在设置中修改
            </p>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={workspace}
                onChange={(e) => setWorkspace(e.target.value)}
                placeholder="选择或输入路径"
                className="flex-1 min-w-0 rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-sm"
              />
              <button
                type="button"
                onClick={handleBrowseWorkspace}
                className="shrink-0 px-3 py-2 rounded-lg border border-border dark:border-border-dark text-sm font-medium hover:bg-gray-100 dark:hover:bg-white/5"
              >
                浏览
              </button>
            </div>
            <button
              type="button"
              onClick={handleCreateExample}
              disabled={initCreating}
              className="w-full py-2 rounded-lg border border-dashed border-border dark:border-border-dark text-sm text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark hover:border-accent/50 transition-colors disabled:opacity-50"
            >
              {initCreating ? "创建中…" : "在所选目录创建示例工作区"}
            </button>
            {initError && (
              <p className="mt-2 text-xs text-red-600 dark:text-red-400">{initError}</p>
            )}
            <div className="flex justify-end gap-2 mt-4">
              <button
                type="button"
                onClick={() => setStep("config")}
                className="px-3 py-1.5 text-sm text-ink-mute"
              >
                上一步
              </button>
              <button
                type="button"
                onClick={handleComplete}
                disabled={!canFinishWorkspace}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium disabled:opacity-50 disabled:cursor-not-allowed"
              >
                完成
              </button>
            </div>
          </>
        )}

      </div>
    </div>
  );
}
