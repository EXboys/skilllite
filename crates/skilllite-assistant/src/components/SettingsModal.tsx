import { useState, useEffect, useCallback } from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore, type Provider, type SandboxLevel } from "../stores/useSettingsStore";
import ScheduleEditor from "./ScheduleEditor";
import ModelComboBox from "./ModelComboBox";
import { API_MODEL_PRESETS } from "../utils/modelPresets";
import {
  type ScheduleForm,
  emptyScheduleForm,
  parseScheduleJson,
  scheduleFormToJson,
  validateScheduleForm,
} from "../utils/scheduleForm";

interface OllamaProbeResult {
  available: boolean;
  models: string[];
  has_embedding: boolean;
}

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
}

type SettingsTabId = "llm" | "workspace" | "agent" | "schedule";

const SETTINGS_TABS: { id: SettingsTabId; label: string }[] = [
  { id: "llm", label: "模型与 API" },
  { id: "workspace", label: "工作区与沙箱" },
  { id: "agent", label: "Agent 预算" },
  { id: "schedule", label: "定时任务" },
];

const SANDBOX_INFO: Record<SandboxLevel, { short: string; desc: string }> = {
  1: { short: "无沙箱", desc: "脚本直接在主机执行，无隔离。仅适合完全信任的本地脚本。" },
  2: { short: "基础隔离", desc: "限制文件访问与网络，适合日常开发。" },
  3: { short: "完全沙箱", desc: "严格隔离（Seatbelt / seccomp），推荐第三方技能。" },
};

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
  const [maxIterationsStr, setMaxIterationsStr] = useState("");
  const [maxToolCallsPerTaskStr, setMaxToolCallsPerTaskStr] = useState("");

  const [activeTab, setActiveTab] = useState<SettingsTabId>("llm");
  const [scheduleData, setScheduleData] = useState<ScheduleForm | null>(null);
  const [scheduleLoadError, setScheduleLoadError] = useState<string | null>(null);

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
      setMaxIterationsStr(
        settings.maxIterations != null ? String(settings.maxIterations) : ""
      );
      setMaxToolCallsPerTaskStr(
        settings.maxToolCallsPerTask != null ? String(settings.maxToolCallsPerTask) : ""
      );
      setOllamaProbe(null);
      setActiveTab("llm");
      setScheduleLoadError(null);
      setScheduleData(null);
    }
  }, [open, settings]);

  useEffect(() => {
    if (!open) return;
    const ws = workspace.trim() || ".";
    const syncWs = settings.workspace?.trim() || ".";
    const delay = ws === syncWs ? 0 : 450;
    let cancelled = false;
    const id = window.setTimeout(() => {
      (async () => {
        try {
          const j = await invoke<string>("skilllite_read_schedule", { workspace: ws });
          if (!cancelled) {
            const parsed = parseScheduleJson(j);
            if (parsed.ok) {
              setScheduleData(parsed.data);
              setScheduleLoadError(null);
            } else {
              setScheduleData(emptyScheduleForm());
              setScheduleLoadError(parsed.error);
            }
          }
        } catch (e) {
          if (!cancelled) {
            setScheduleLoadError(String(e));
            setScheduleData(emptyScheduleForm());
          }
        }
      })();
    }, delay);
    return () => {
      cancelled = true;
      window.clearTimeout(id);
    };
  }, [open, workspace, settings.workspace]);

  useEffect(() => {
    if (open && provider === "ollama") {
      probeOllama();
    }
  }, [open, provider, probeOllama]);

  const parsePositiveIntField = (s: string): number | undefined => {
    const t = s.trim();
    if (!t) return undefined;
    const n = Number(t);
    if (!Number.isInteger(n) || n < 1) return undefined;
    return n;
  };

  const handleSave = async () => {
    if (!scheduleData) {
      setScheduleLoadError("定时配置尚未加载完成");
      setActiveTab("schedule");
      return;
    }
    const invalid = validateScheduleForm(scheduleData);
    if (invalid) {
      setScheduleLoadError(invalid);
      setActiveTab("schedule");
      return;
    }
    const jsonStr = scheduleFormToJson(scheduleData);
    try {
      await invoke("skilllite_write_schedule", {
        workspace: workspace.trim() || ".",
        json: jsonStr,
      });
    } catch (e) {
      setScheduleLoadError(String(e));
      setActiveTab("schedule");
      return;
    }
    setScheduleLoadError(null);

    const shared = {
      sandboxLevel,
      swarmEnabled,
      swarmUrl: swarmUrl.trim(),
      maxIterations: parsePositiveIntField(maxIterationsStr),
      maxToolCallsPerTask: parsePositiveIntField(maxToolCallsPerTaskStr),
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
        className="w-full max-w-xl rounded-xl bg-white dark:bg-paper-dark shadow-xl border border-border dark:border-border-dark flex flex-col max-h-[min(90vh,720px)]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Fixed header + tabs */}
        <div className="px-5 pt-5 pb-0 border-b border-border dark:border-border-dark shrink-0">
          <h2 className="text-base font-semibold text-ink dark:text-ink-dark pb-3">
            设置
          </h2>
          <div className="flex gap-1 overflow-x-auto pb-0 -mx-1 px-1">
            {SETTINGS_TABS.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => setActiveTab(tab.id)}
                className={`shrink-0 px-3 py-2 text-xs font-medium rounded-t-lg border-b-2 transition-colors ${
                  activeTab === tab.id
                    ? "border-accent text-accent bg-accent/5"
                    : "border-transparent text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
                }`}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-4 min-h-0">

          {activeTab === "llm" && (
          <div className="space-y-4">
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
          </div>
          )}

          {activeTab === "workspace" && (
          <div className="space-y-4">
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
            <p className="mt-1 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              修改路径后，「定时任务」页会按新路径加载{" "}
              <code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded">.skilllite/schedule.json</code>。
            </p>
          </div>

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
          )}

          {activeTab === "agent" && (
          <div className="space-y-4">
          {/* ── Agent loop limits（对齐 SKILLLITE_MAX_*） ── */}
          <div>
            <p className="text-xs font-medium text-ink dark:text-ink-dark-mute mb-2">
              Agent 循环预算
            </p>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className={labelCls}>最大迭代轮次</label>
                <input
                  type="number"
                  min={1}
                  inputMode="numeric"
                  value={maxIterationsStr}
                  onChange={(e) => setMaxIterationsStr(e.target.value)}
                  placeholder="默认 50"
                  className={inputCls}
                />
              </div>
              <div>
                <label className={labelCls}>每任务工具上限</label>
                <input
                  type="number"
                  min={1}
                  inputMode="numeric"
                  value={maxToolCallsPerTaskStr}
                  onChange={(e) => setMaxToolCallsPerTaskStr(e.target.value)}
                  placeholder="默认 15"
                  className={inputCls}
                />
              </div>
            </div>
            <p className="mt-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              留空则使用工作区 <code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded">.env</code>{" "}
              或内置默认值（<code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded">SKILLLITE_MAX_ITERATIONS</code>、
              <code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded">SKILLLITE_MAX_TOOL_CALLS_PER_TASK</code>）。
            </p>
          </div>
          </div>
          )}

          {activeTab === "schedule" && (
          <div className="space-y-3">
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              与 CLI <code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded text-[11px]">skilllite schedule tick</code>{" "}
              共用 <code className="font-mono text-[11px] bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded">.skilllite/schedule.json</code>。
              触发支持：按间隔 / 每天固定本地时刻 / 仅一次本地时刻。「目标」「执行步骤」与可选「补充说明」会合并为一条用户消息。
              非 dry-run 需在环境中设置{" "}
              <code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded text-[11px]">SKILLLITE_SCHEDULE_ENABLED=1</code>。
            </p>
            {scheduleData === null ? (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute py-4">
                {scheduleLoadError ? scheduleLoadError : "正在加载定时配置…"}
              </p>
            ) : (
              <ScheduleEditor
                data={scheduleData}
                onChange={setScheduleData}
                error={scheduleLoadError}
                onClearError={() => setScheduleLoadError(null)}
                onError={setScheduleLoadError}
                inputCls={inputCls}
                labelCls={labelCls}
              />
            )}
            <button
              type="button"
              onClick={async () => {
                const ws = workspace.trim() || ".";
                try {
                  const j = await invoke<string>("skilllite_read_schedule", { workspace: ws });
                  const parsed = parseScheduleJson(j);
                  if (parsed.ok) {
                    setScheduleData(parsed.data);
                    setScheduleLoadError(null);
                  } else {
                    setScheduleData(emptyScheduleForm());
                    setScheduleLoadError(parsed.error);
                  }
                } catch (e) {
                  setScheduleLoadError(String(e));
                  setScheduleData(emptyScheduleForm());
                }
              }}
              className="text-xs text-accent hover:underline"
            >
              从磁盘重新加载
            </button>
          </div>
          )}
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
