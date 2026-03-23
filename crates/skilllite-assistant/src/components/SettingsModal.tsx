import { useState, useEffect, useCallback, useRef } from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore, type Provider, type SandboxLevel } from "../stores/useSettingsStore";

interface OllamaProbeResult {
  available: boolean;
  models: string[];
  has_embedding: boolean;
}

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
}

interface ModelPreset {
  value: string;
  label: string;
  apiBase?: string;
}

const API_MODEL_PRESETS: ModelPreset[] = [
  // OpenAI
  { value: "gpt-5.4", label: "GPT-5.4", apiBase: "https://api.openai.com/v1" },
  { value: "gpt-5.4-pro", label: "GPT-5.4 Pro", apiBase: "https://api.openai.com/v1" },
  { value: "gpt-4o", label: "GPT-4o", apiBase: "https://api.openai.com/v1" },
  { value: "gpt-4o-mini", label: "GPT-4o Mini", apiBase: "https://api.openai.com/v1" },
  // Anthropic Claude
  { value: "claude-opus-4-6", label: "Claude Opus 4.6", apiBase: "https://api.anthropic.com/v1" },
  { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6", apiBase: "https://api.anthropic.com/v1" },
  { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5", apiBase: "https://api.anthropic.com/v1" },
  // Google Gemini
  { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro", apiBase: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash", apiBase: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { value: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite", apiBase: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  // DeepSeek
  { value: "deepseek-chat", label: "DeepSeek Chat", apiBase: "https://api.deepseek.com/v1" },
  { value: "deepseek-reasoner", label: "DeepSeek Reasoner", apiBase: "https://api.deepseek.com/v1" },
  // Qwen
  { value: "qwen-plus", label: "Qwen Plus", apiBase: "https://dashscope.aliyuncs.com/compatible-mode/v1" },
  { value: "qwen-max", label: "Qwen Max", apiBase: "https://dashscope.aliyuncs.com/compatible-mode/v1" },
  // MiniMax
  { value: "MiniMax-M2.5", label: "MiniMax M2.5", apiBase: "https://api.minimax.chat/v1" },
  { value: "MiniMax-M2.7", label: "MiniMax M2.7", apiBase: "https://api.minimax.chat/v1" },
];

const SANDBOX_INFO: Record<SandboxLevel, { short: string; desc: string }> = {
  1: { short: "无沙箱", desc: "脚本直接在主机执行，无隔离。仅适合完全信任的本地脚本。" },
  2: { short: "基础隔离", desc: "限制文件访问与网络，适合日常开发。" },
  3: { short: "完全沙箱", desc: "严格隔离（Seatbelt / seccomp），推荐第三方技能。" },
};

function ModelComboBox({
  value,
  onChange,
  onPresetSelect,
  presets,
  placeholder,
  inputCls,
}: {
  value: string;
  onChange: (v: string) => void;
  onPresetSelect?: (preset: ModelPreset) => void;
  presets: ModelPreset[];
  placeholder: string;
  inputCls: string;
}) {
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [customMode, setCustomMode] = useState(() => !presets.some((p) => p.value === value) && value !== "");
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!dropdownOpen) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [dropdownOpen]);

  const matched = presets.find((p) => p.value === value);

  if (customMode) {
    return (
      <div className="flex gap-2">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className={`flex-1 min-w-0 ${inputCls}`}
        />
        <button
          type="button"
          onClick={() => {
            setCustomMode(false);
            const first = presets[0];
            if (first && !presets.some((p) => p.value === value)) {
              onChange(first.value);
              onPresetSelect?.(first);
            }
          }}
          className="shrink-0 px-2.5 py-2 rounded-lg border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5 text-xs font-medium transition-colors"
        >
          预设
        </button>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="relative">
      <button
        type="button"
        onClick={() => setDropdownOpen(!dropdownOpen)}
        className={`${inputCls} text-left flex items-center justify-between gap-2 cursor-pointer`}
      >
        <span className={matched ? "text-ink dark:text-ink-dark" : "text-ink-mute"}>
          {matched ? matched.label : placeholder}
        </span>
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0 text-ink-mute">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>
      {dropdownOpen && (
        <div className="absolute z-10 mt-1 w-full rounded-lg border border-border dark:border-border-dark bg-white dark:bg-paper-dark shadow-lg max-h-48 overflow-y-auto">
          {presets.map((p) => (
            <button
              key={p.value}
              type="button"
              onClick={() => {
                onChange(p.value);
                onPresetSelect?.(p);
                setDropdownOpen(false);
              }}
              className={`w-full text-left px-3 py-2 text-sm transition-colors ${
                value === p.value
                  ? "bg-accent/10 text-accent font-medium"
                  : "text-ink dark:text-ink-dark hover:bg-gray-50 dark:hover:bg-white/5"
              }`}
            >
              <span>{p.label}</span>
              <span className="text-xs text-ink-mute dark:text-ink-dark-mute ml-2">{p.value}</span>
            </button>
          ))}
          <button
            type="button"
            onClick={() => { setCustomMode(true); setDropdownOpen(false); }}
            className="w-full text-left px-3 py-2 text-sm text-accent hover:bg-gray-50 dark:hover:bg-white/5 border-t border-border dark:border-border-dark"
          >
            自定义输入…
          </button>
        </div>
      )}
    </div>
  );
}

export default function SettingsModal({ open, onClose }: SettingsModalProps) {
  const { settings, setSettings } = useSettingsStore();
  const [provider, setProvider] = useState<Provider>(settings.provider || "api");
  const [apiKey, setApiKey] = useState(settings.apiKey);
  const [model, setModel] = useState(settings.model);
  const [workspace, setWorkspace] = useState(settings.workspace);
  const [apiBase, setApiBase] = useState(settings.apiBase);

  const [sandboxLevel, setSandboxLevel] = useState<SandboxLevel>(settings.sandboxLevel ?? 3);
  const [swarmEnabled, setSwarmEnabled] = useState(settings.swarmEnabled ?? false);
  const [swarmUrl, setSwarmUrl] = useState(settings.swarmUrl ?? "");

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
      setSandboxLevel(settings.sandboxLevel ?? 3);
      setSwarmEnabled(settings.swarmEnabled ?? false);
      setSwarmUrl(settings.swarmUrl ?? "");
      setOllamaProbe(null);
    }
  }, [open, settings]);

  useEffect(() => {
    if (open && provider === "ollama") {
      probeOllama();
    }
  }, [open, provider, probeOllama]);

  const handleSave = () => {
    const shared = {
      sandboxLevel,
      swarmEnabled,
      swarmUrl: swarmUrl.trim(),
    };
    if (provider === "ollama") {
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: model.trim() || "llama3.2",
        workspace: workspace.trim() || ".",
        ...shared,
      });
    } else {
      setSettings({
        provider: "api",
        apiKey: apiKey.trim(),
        model: model.trim() || "gpt-4o",
        workspace: workspace.trim() || ".",
        apiBase: apiBase.trim(),
        ...shared,
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
  const labelCls = "block text-xs font-medium text-ink dark:text-ink-dark-mute mb-1";

  const ollamaModelPresets = ollamaProbe?.available
    ? ollamaProbe.models
        .filter((m) => !m.includes("embed"))
        .map((m) => ({ value: m, label: m }))
    : [];

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 dark:bg-black/50 backdrop-blur-sm p-4"
      onClick={onClose}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === "Escape" && onClose()}
    >
      <div
        className="w-full max-w-md rounded-xl bg-white dark:bg-paper-dark shadow-xl border border-border dark:border-border-dark flex flex-col max-h-[min(90vh,640px)]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Fixed header */}
        <div className="px-5 pt-5 pb-3 border-b border-border dark:border-border-dark shrink-0">
          <h2 className="text-base font-semibold text-ink dark:text-ink-dark">
            设置
          </h2>
        </div>

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-4 min-h-0">

          {/* ── Provider ── */}
          <div>
            <label className={labelCls}>使用方式</label>
            <div className="flex rounded-lg border border-border dark:border-border-dark overflow-hidden">
              <button
                type="button"
                onClick={() => setProvider("api")}
                className={`flex-1 py-1.5 text-sm font-medium transition-colors ${
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
                className={`flex-1 py-1.5 text-sm font-medium transition-colors ${
                  provider === "ollama"
                    ? "bg-accent text-white"
                    : "bg-gray-50 dark:bg-surface-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5"
                }`}
              >
                本地 Ollama
              </button>
            </div>
          </div>

          {/* ── API config ── */}
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
                <ModelComboBox
                  value={model}
                  onChange={setModel}
                  onPresetSelect={(preset) => {
                    if (preset.apiBase) {
                      setApiBase(preset.apiBase);
                    }
                  }}
                  presets={API_MODEL_PRESETS}
                  placeholder="选择模型"
                  inputCls={inputCls}
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
                {apiBase && (
                  <p className="mt-1 text-xs text-ink-mute dark:text-ink-dark-mute">
                    {API_MODEL_PRESETS.find((p) => p.value === model)?.apiBase === apiBase
                      ? "已自动匹配，可手动修改"
                      : "自定义地址"}
                  </p>
                )}
              </div>
            </>
          )}

          {/* ── Ollama config ── */}
          {provider === "ollama" && (
            <>
              {ollamaLoading ? (
                <p className="text-sm text-ink-mute dark:text-ink-dark-mute py-1">
                  正在检测 Ollama…
                </p>
              ) : ollamaProbe?.available ? (
                <>
                  {ollamaModelPresets.length > 0 ? (
                    <div>
                      <label className={labelCls}>模型</label>
                      <ModelComboBox
                        value={model}
                        onChange={setModel}
                        presets={ollamaModelPresets}
                        placeholder="选择模型"
                        inputCls={inputCls}
                      />
                    </div>
                  ) : (
                    <p className="text-sm text-amber-600 dark:text-amber-400 py-1">
                      Ollama 已运行，但未安装模型。请先运行{" "}
                      <code className="bg-gray-100 dark:bg-surface-dark px-1.5 py-0.5 rounded text-xs">
                        ollama pull llama3.2
                      </code>
                    </p>
                  )}
                  <div className="flex items-center gap-2 text-xs text-ink-mute dark:text-ink-dark-mute">
                    <span
                      className={`inline-block w-2 h-2 rounded-full shrink-0 ${
                        ollamaProbe.has_embedding ? "bg-green-500" : "bg-gray-300 dark:bg-gray-600"
                      }`}
                    />
                    {ollamaProbe.has_embedding
                      ? "已检测到 Embedding 模型，记忆向量检索可用"
                      : "未检测到 Embedding 模型，记忆将使用文本检索"}
                  </div>
                </>
              ) : (
                <div className="py-1">
                  <p className="text-sm text-red-600 dark:text-red-400 mb-1">
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
                    className="mt-1.5 text-sm text-accent hover:underline"
                  >
                    重新检测
                  </button>
                </div>
              )}
            </>
          )}

          {/* ── Workspace ── */}
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
                className="shrink-0 px-2.5 py-2 rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5 text-sm font-medium transition-colors"
              >
                浏览
              </button>
            </div>
          </div>

          {/* ── Divider ── */}
          <div className="border-t border-border dark:border-border-dark" />

          {/* ── Sandbox Level ── */}
          <div>
            <label className={labelCls}>沙箱安全等级</label>
            <div className="flex rounded-lg border border-border dark:border-border-dark overflow-hidden">
              {([1, 2, 3] as const).map((level) => (
                <button
                  key={level}
                  type="button"
                  onClick={() => setSandboxLevel(level)}
                  className={`flex-1 py-1.5 text-sm font-medium transition-colors ${
                    sandboxLevel === level
                      ? "bg-accent text-white"
                      : "bg-gray-50 dark:bg-surface-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5"
                  }`}
                >
                  L{level}
                </button>
              ))}
            </div>
            <p className="mt-1 text-xs text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              {SANDBOX_INFO[sandboxLevel].short} — {SANDBOX_INFO[sandboxLevel].desc}
            </p>
          </div>

          {/* ── Swarm Network ── */}
          <div>
            <div className="flex items-center justify-between">
              <label className={`${labelCls} mb-0`}>Swarm P2P 网络</label>
              <button
                type="button"
                role="switch"
                aria-checked={swarmEnabled}
                onClick={() => setSwarmEnabled(!swarmEnabled)}
                className={`relative inline-flex h-5 w-9 shrink-0 rounded-full border-2 border-transparent transition-colors cursor-pointer ${
                  swarmEnabled ? "bg-accent" : "bg-gray-300 dark:bg-gray-600"
                }`}
              >
                <span
                  className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow transform transition-transform ${
                    swarmEnabled ? "translate-x-4" : "translate-x-0"
                  }`}
                />
              </button>
            </div>
            {swarmEnabled && (
              <div className="mt-2">
                <input
                  type="text"
                  value={swarmUrl}
                  onChange={(e) => setSwarmUrl(e.target.value)}
                  placeholder="http://192.168.1.10:7700"
                  className={inputCls}
                />
                <p className="mt-1 text-xs text-ink-mute dark:text-ink-dark-mute">
                  Agent 可将子任务委派给局域网内其他节点协作完成
                </p>
              </div>
            )}
          </div>
        </div>

        {/* Fixed footer */}
        <div className="px-5 py-3 border-t border-border dark:border-border-dark flex justify-end gap-2 shrink-0">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-1.5 text-sm text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark transition-colors"
          >
            取消
          </button>
          <button
            type="button"
            onClick={handleSave}
            className="px-4 py-1.5 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent-hover transition-colors"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
