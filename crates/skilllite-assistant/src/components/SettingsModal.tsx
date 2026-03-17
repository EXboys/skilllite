import { useState, useEffect, useCallback } from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore, type Provider } from "../stores/useSettingsStore";

interface OllamaProbeResult {
  available: boolean;
  models: string[];
  has_embedding: boolean;
}

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
}

export default function SettingsModal({ open, onClose }: SettingsModalProps) {
  const { settings, setSettings } = useSettingsStore();
  const [provider, setProvider] = useState<Provider>(settings.provider || "api");
  const [apiKey, setApiKey] = useState(settings.apiKey);
  const [model, setModel] = useState(settings.model);
  const [workspace, setWorkspace] = useState(settings.workspace);
  const [apiBase, setApiBase] = useState(settings.apiBase);

  const [ollamaProbe, setOllamaProbe] = useState<OllamaProbeResult | null>(null);
  const [ollamaLoading, setOllamaLoading] = useState(false);

  const probeOllama = useCallback(async () => {
    setOllamaLoading(true);
    try {
      const r = await invoke<OllamaProbeResult>("skilllite_probe_ollama");
      setOllamaProbe(r);
      if (r.available && r.models.length > 0 && !r.models.includes(model)) {
        setModel(r.models[0]);
      }
    } catch {
      setOllamaProbe({ available: false, models: [], has_embedding: false });
    } finally {
      setOllamaLoading(false);
    }
  }, [model]);

  useEffect(() => {
    if (open) {
      setProvider(settings.provider || "api");
      setApiKey(settings.apiKey);
      setModel(settings.model);
      setWorkspace(settings.workspace);
      setApiBase(settings.apiBase);
      setOllamaProbe(null);
    }
  }, [open, settings]);

  useEffect(() => {
    if (open && provider === "ollama") {
      probeOllama();
    }
  }, [open, provider, probeOllama]);

  const handleSave = () => {
    if (provider === "ollama") {
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: model.trim() || "llama3.2",
        workspace: workspace.trim() || ".",
      });
    } else {
      setSettings({
        provider: "api",
        apiKey: apiKey.trim(),
        model: model.trim() || "gpt-4o",
        workspace: workspace.trim() || ".",
        apiBase: apiBase.trim(),
      });
    }
    onClose();
  };

  const handleBrowseWorkspace = async () => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      title: "选择工作区目录",
      defaultPath: workspace && workspace !== "." ? workspace : undefined,
    });
    if (selected) {
      setWorkspace(selected);
    }
  };

  if (!open) return null;

  const inputCls =
    "w-full rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-ink dark:text-ink-dark placeholder-ink-mute text-sm focus:ring-2 focus:ring-accent/40 focus:border-accent outline-none";
  const labelCls = "block text-sm font-medium text-ink dark:text-ink-dark-mute mb-1";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 dark:bg-black/50 backdrop-blur-sm"
      onClick={onClose}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === "Escape" && onClose()}
    >
      <div
        className="w-full max-w-md rounded-lg bg-white dark:bg-paper-dark p-6 shadow-xl border border-border dark:border-border-dark"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-base font-semibold text-ink dark:text-ink-dark mb-4">
          设置
        </h2>

        {/* Provider toggle */}
        <div className="mb-4">
          <label className={labelCls}>使用方式</label>
          <div className="flex rounded-lg border border-border dark:border-border-dark overflow-hidden">
            <button
              type="button"
              onClick={() => setProvider("api")}
              className={`flex-1 py-2 text-sm font-medium transition-colors ${
                provider === "api"
                  ? "bg-accent text-white"
                  : "bg-gray-50 dark:bg-surface-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5"
              }`}
            >
              API Key
            </button>
            <button
              type="button"
              onClick={() => setProvider("ollama")}
              className={`flex-1 py-2 text-sm font-medium transition-colors ${
                provider === "ollama"
                  ? "bg-accent text-white"
                  : "bg-gray-50 dark:bg-surface-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5"
              }`}
            >
              本地 Ollama
            </button>
          </div>
        </div>

        <div className="space-y-4">
          {provider === "api" && (
            <>
              <div>
                <label className={labelCls}>API Key</label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="sk-...（留空则使用 .env 中的 OPENAI_API_KEY）"
                  className={inputCls}
                />
              </div>
              <div>
                <label className={labelCls}>模型</label>
                <input
                  type="text"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder="gpt-4o"
                  className={inputCls}
                />
              </div>
              <div>
                <label className={labelCls}>API Base URL（可选）</label>
                <input
                  type="text"
                  value={apiBase}
                  onChange={(e) => setApiBase(e.target.value)}
                  placeholder="https://api.openai.com/v1"
                  className={inputCls}
                />
              </div>
            </>
          )}

          {provider === "ollama" && (
            <>
              {ollamaLoading ? (
                <p className="text-sm text-ink-mute dark:text-ink-dark-mute py-2">
                  正在检测 Ollama…
                </p>
              ) : ollamaProbe?.available ? (
                <>
                  {ollamaProbe.models.length > 0 ? (
                    <div>
                      <label className={labelCls}>模型</label>
                      <select
                        value={model}
                        onChange={(e) => setModel(e.target.value)}
                        className={inputCls}
                      >
                        {ollamaProbe.models
                          .filter((m) => !m.includes("embed"))
                          .map((m) => (
                            <option key={m} value={m}>
                              {m}
                            </option>
                          ))}
                      </select>
                    </div>
                  ) : (
                    <p className="text-sm text-amber-600 dark:text-amber-400 py-2">
                      Ollama 已运行，但未安装模型。请先运行{" "}
                      <code className="bg-gray-100 dark:bg-surface-dark px-1.5 py-0.5 rounded text-xs">
                        ollama pull llama3.2
                      </code>
                    </p>
                  )}
                  <div className="flex items-center gap-2 text-xs text-ink-mute dark:text-ink-dark-mute">
                    <span
                      className={`inline-block w-2 h-2 rounded-full ${
                        ollamaProbe.has_embedding ? "bg-green-500" : "bg-gray-300 dark:bg-gray-600"
                      }`}
                    />
                    {ollamaProbe.has_embedding
                      ? "已检测到 Embedding 模型，记忆向量检索可用"
                      : "未检测到 Embedding 模型，记忆将使用文本检索（不影响正常聊天）"}
                  </div>
                </>
              ) : (
                <div className="py-2">
                  <p className="text-sm text-red-600 dark:text-red-400 mb-2">
                    未检测到 Ollama 服务
                  </p>
                  <p className="text-xs text-ink-mute dark:text-ink-dark-mute">
                    请确保已安装并运行 Ollama（
                    <code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded">
                      ollama serve
                    </code>
                    ）
                  </p>
                  <button
                    type="button"
                    onClick={probeOllama}
                    className="mt-2 text-sm text-accent hover:underline"
                  >
                    重新检测
                  </button>
                </div>
              )}
            </>
          )}

          {/* Workspace - shared */}
          <div>
            <label className={labelCls}>工作区路径</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={workspace}
                onChange={(e) => setWorkspace(e.target.value)}
                placeholder="."
                className={`flex-1 min-w-0 ${inputCls}`}
              />
              <button
                type="button"
                onClick={handleBrowseWorkspace}
                className="shrink-0 px-3 py-2 rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5 text-sm font-medium transition-colors"
              >
                浏览
              </button>
            </div>
          </div>
        </div>

        <div className="mt-6 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-sm text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark transition-colors"
          >
            取消
          </button>
          <button
            type="button"
            onClick={handleSave}
            className="px-4 py-2 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent-hover transition-colors"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
