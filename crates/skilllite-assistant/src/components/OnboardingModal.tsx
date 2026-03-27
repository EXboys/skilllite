import { useState, useEffect, useRef } from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore, type SandboxLevel } from "../stores/useSettingsStore";
import ModelComboBox from "./ModelComboBox";
import { API_MODEL_PRESETS } from "../utils/modelPresets";

type Step = "mode" | "config" | "workspace" | "sandbox" | "health" | "success";
type Mode = "api" | "ollama";

interface OllamaProbeResult {
  available: boolean;
  models: string[];
  has_embedding: boolean;
}

interface HealthCheckItem {
  ok: boolean;
  message: string;
}

interface OnboardingHealthCheckResult {
  binary: HealthCheckItem;
  provider: HealthCheckItem;
  workspace: HealthCheckItem;
  data_dir: HealthCheckItem;
  ok: boolean;
}

const STARTER_ACTIONS = [
  "介绍一下当前工作区适合怎么使用 SkillLite",
  "列出当前工作区里可用的技能，并告诉我先试哪个",
  "推荐一个最适合新手的入门任务并直接带我开始",
];

const inputClsOnboarding =
  "w-full rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-ink dark:text-ink-dark placeholder-ink-mute text-sm focus:ring-2 focus:ring-accent/40 focus:border-accent outline-none";

export default function OnboardingModal() {
  const { settings, setSettings } = useSettingsStore();
  const [step, setStep] = useState<Step>("mode");
  const [mode, setMode] = useState<Mode | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("gpt-4o");
  const [apiBase, setApiBase] = useState("");
  const [workspace, setWorkspace] = useState("");
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [ollamaModel, setOllamaModel] = useState<string | null>(null);
  const [ollamaLoading, setOllamaLoading] = useState(false);
  const [ollamaAutoDetected, setOllamaAutoDetected] = useState(false);
  const [sandboxLevel, setSandboxLevel] = useState<SandboxLevel>(3);
  const [initCreating, setInitCreating] = useState(false);
  const [initError, setInitError] = useState("");
  const [healthChecking, setHealthChecking] = useState(false);
  const [healthResult, setHealthResult] = useState<OnboardingHealthCheckResult | null>(null);
  const [healthError, setHealthError] = useState("");
  const initialProbeDoneRef = useRef(false);
  const userChoseModeRef = useRef(false);

  useEffect(() => {
    if (step !== "config" || mode !== "api") return;
    if (settings.apiKey) setApiKey(settings.apiKey);
    if (settings.model) setModel(settings.model);
    setApiBase(settings.apiBase ?? "");
  }, [step, mode, settings.apiKey, settings.model, settings.apiBase]);

  useEffect(() => {
    let cancelled = false;
    setOllamaLoading(true);
    const tryProbe = () => {
      if (!(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__) {
        setTimeout(tryProbe, 200);
        return;
      }
      invoke<OllamaProbeResult>("skilllite_probe_ollama")
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
    };
    tryProbe();
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
      invoke<OllamaProbeResult>("skilllite_probe_ollama")
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

  const applySettingsAndFinish = () => {
    const ws = workspace.trim() || ".";
    const shared = {
      sandboxLevel,
      onboardingCompleted: true as const,
      showStarterPrompts: true as const,
    };
    if (mode === "ollama") {
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: ollamaModel || "llama3.2",
        workspace: ws,
        ...shared,
      });
    } else {
      setSettings({
        provider: "api",
        apiKey: apiKey.trim(),
        model: model.trim() || "gpt-4o",
        workspace: ws,
        apiBase: apiBase.trim(),
        ...shared,
      });
    }
  };

  /** 健康检查通过后写入工作区/沙箱/模型等，但不标记引导完成（仍显示成功页与「进入聊天」） */
  const persistAllButNotCompleted = () => {
    const ws = workspace.trim() || ".";
    const shared = { sandboxLevel, workspace: ws };
    if (mode === "ollama") {
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: ollamaModel || "llama3.2",
        ...shared,
      });
    } else if (mode === "api") {
      setSettings({
        provider: "api",
        apiKey: apiKey.trim(),
        model: model.trim() || "gpt-4o",
        apiBase: apiBase.trim(),
        ...shared,
      });
    }
  };

  /** 离开「API / Ollama 配置」时立即写入本地存储，避免仅走完健康检查才保存导致 Key 丢失 */
  const persistLlmAndGoToWorkspace = () => {
    if (mode === "api") {
      setSettings({
        provider: "api",
        apiKey: apiKey.trim(),
        model: model.trim() || "gpt-4o",
        apiBase: apiBase.trim(),
      });
    } else if (mode === "ollama") {
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: ollamaModel || "llama3.2",
      });
    }
    setStep("workspace");
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

  const handleRunHealthCheck = async () => {
    if (!mode) return;
    if (!(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__) {
      setHealthError("应用环境尚未就绪，请稍后重试");
      return;
    }
    const ws = workspace.trim() || ".";
    setStep("health");
    setHealthChecking(true);
    setHealthError("");
    setHealthResult(null);
    try {
      const result = await invoke<OnboardingHealthCheckResult>("skilllite_health_check", {
        workspace: ws,
        provider: mode,
        api_key: mode === "api" ? apiKey.trim() : undefined,
      });
      setHealthResult(result);
      if (result.ok) {
        persistAllButNotCompleted();
        setStep("success");
      }
    } catch (e) {
      setHealthError(String(e));
    } finally {
      setHealthChecking(false);
    }
  };

  const canContinueApi = apiKey.trim().length > 0;
  const canContinueOllama = ollamaLoading || ollamaModels.length > 0;
  const workspaceDisplay = workspace.trim() || ".";

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
          先完成环境检查与工作区配置，再开始第一次聊天
        </p>
        <button
          type="button"
          onClick={() =>
            setSettings({
              onboardingCompleted: true,
              workspace: ".",
              showStarterPrompts: false,
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
                  placeholder="sk-...（与设置中一致，会保存到本应用）"
                  className={inputClsOnboarding}
                />
              </div>
              <div>
                <label className="block text-xs text-ink-mute dark:text-ink-dark-mute mb-1">
                  模型
                </label>
                <ModelComboBox
                  value={model}
                  onChange={setModel}
                  onPresetSelect={(preset) => {
                    if (preset.apiBase) setApiBase(preset.apiBase);
                  }}
                  presets={API_MODEL_PRESETS}
                  placeholder="选择模型"
                  inputCls={inputClsOnboarding}
                />
              </div>
              <div>
                <label className="block text-xs text-ink-mute dark:text-ink-dark-mute mb-1">
                  API Base URL（可选）
                </label>
                <input
                  type="text"
                  value={apiBase}
                  onChange={(e) => setApiBase(e.target.value)}
                  placeholder="https://api.openai.com/v1"
                  className={inputClsOnboarding}
                />
                {apiBase.trim() !== "" && (
                  <p className="mt-1 text-xs text-ink-mute dark:text-ink-dark-mute">
                    {API_MODEL_PRESETS.find((p) => p.value === model)?.apiBase === apiBase.trim()
                      ? "已按所选模型自动填入，可改为第三方代理地址"
                      : "自定义地址"}
                  </p>
                )}
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
                onClick={persistLlmAndGoToWorkspace}
                disabled={!canContinueApi}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium disabled:opacity-50 disabled:cursor-not-allowed"
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
                未检测到 Ollama 或未安装模型。请先安装并运行 Ollama，或返回上一步选择「配置 API Key」。
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
                onClick={persistLlmAndGoToWorkspace}
                disabled={!canContinueOllama}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium disabled:opacity-50 disabled:cursor-not-allowed"
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
                placeholder="选择或输入路径，留空默认使用当前目录"
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
            <div className="mt-3 rounded-lg bg-ink/5 dark:bg-white/5 px-3 py-2 text-xs text-ink-mute dark:text-ink-dark-mute">
              当前将使用：`{workspaceDisplay}`
            </div>
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
                onClick={() => {
                  const ws = workspace.trim() || ".";
                  setSettings({ workspace: ws });
                  setStep("sandbox");
                }}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium"
              >
                下一步
              </button>
            </div>
          </>
        )}

        {step === "sandbox" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              4. 选择沙箱安全等级
            </p>
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-3">
              沙箱决定了技能脚本的执行隔离程度，等级越高越安全，可后续在设置中修改
            </p>
            <div className="space-y-2 mb-4">
              {([
                { level: 3 as const, title: "L3 · 完全沙箱（推荐）", desc: "严格的文件/网络/进程隔离，推荐用于运行第三方技能" },
                { level: 2 as const, title: "L2 · 基础隔离", desc: "限制文件访问与网络，适合日常开发使用" },
                { level: 1 as const, title: "L1 · 无沙箱", desc: "脚本直接在主机执行，仅适合完全信任的本地脚本" },
              ]).map((opt) => (
                <button
                  key={opt.level}
                  type="button"
                  onClick={() => setSandboxLevel(opt.level)}
                  className={`w-full text-left px-4 py-3 rounded-lg border transition-colors ${
                    sandboxLevel === opt.level
                      ? "border-accent bg-accent/5 dark:bg-accent/10"
                      : "border-border dark:border-border-dark hover:bg-gray-50 dark:hover:bg-white/5"
                  }`}
                >
                  <span className={`font-medium block ${sandboxLevel === opt.level ? "text-accent" : "text-ink dark:text-ink-dark"}`}>
                    {opt.title}
                  </span>
                  <span className="text-xs text-ink-mute dark:text-ink-dark-mute">{opt.desc}</span>
                </button>
              ))}
            </div>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setStep("workspace")}
                className="px-3 py-1.5 text-sm text-ink-mute"
              >
                上一步
              </button>
              <button
                type="button"
                onClick={handleRunHealthCheck}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium"
              >
                检查并继续
              </button>
            </div>
          </>
        )}

        {step === "health" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              5. 健康检查
            </p>
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-3">
              正在检查内置引擎、当前 provider、工作区和数据目录是否可用
            </p>
            {healthChecking && (
              <div className="rounded-lg border border-border dark:border-border-dark px-3 py-3 text-sm text-ink-mute dark:text-ink-dark-mute">
                正在执行检查，请稍候…
              </div>
            )}
            {healthError && (
              <div className="rounded-lg border border-red-200 dark:border-red-800/50 bg-red-50 dark:bg-red-900/20 px-3 py-3 text-sm text-red-700 dark:text-red-300">
                {healthError}
              </div>
            )}
            {!healthChecking && healthResult && (
              <div className="space-y-2">
                {[
                  { label: "内置引擎", item: healthResult.binary },
                  { label: "模型来源", item: healthResult.provider },
                  { label: "工作区", item: healthResult.workspace },
                  { label: "数据目录", item: healthResult.data_dir },
                ].map(({ label, item }) => (
                  <div
                    key={label}
                    className={`rounded-lg border px-3 py-2 text-sm ${
                      item.ok
                        ? "border-green-200 dark:border-green-800/50 bg-green-50 dark:bg-green-900/20 text-green-800 dark:text-green-200"
                        : "border-red-200 dark:border-red-800/50 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300"
                    }`}
                  >
                    <div className="font-medium">{label}</div>
                    <div className="text-xs mt-1">{item.message}</div>
                  </div>
                ))}
              </div>
            )}
            <div className="flex justify-end gap-2 mt-4">
              <button
                type="button"
                onClick={() => setStep("sandbox")}
                className="px-3 py-1.5 text-sm text-ink-mute"
              >
                返回修改
              </button>
              <button
                type="button"
                onClick={handleRunHealthCheck}
                disabled={healthChecking}
                className="px-4 py-1.5 text-sm rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark disabled:opacity-50"
              >
                重新检查
              </button>
            </div>
          </>
        )}

        {step === "success" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              6. 准备完成
            </p>
            <div className="rounded-lg border border-green-200 dark:border-green-800/50 bg-green-50 dark:bg-green-900/20 px-3 py-3 text-sm text-green-800 dark:text-green-200 mb-3">
              环境检查已通过，接下来会进入聊天页，并显示几个适合第一次使用的入门操作。
            </div>
            <div className="space-y-2 mb-4">
              {STARTER_ACTIONS.map((action) => (
                <div
                  key={action}
                  className="rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-sm text-ink dark:text-ink-dark"
                >
                  {action}
                </div>
              ))}
            </div>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setStep("sandbox")}
                className="px-3 py-1.5 text-sm text-ink-mute"
              >
                返回修改
              </button>
              <button
                type="button"
                onClick={applySettingsAndFinish}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium"
              >
                进入聊天
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
