import { useState, useEffect, useRef, useMemo } from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore, type SandboxLevel } from "../stores/useSettingsStore";
import ModelComboBox from "./ModelComboBox";
import { API_MODEL_PRESETS, presetApiBaseForModelId } from "../utils/modelPresets";
import { findSavedProfileForModel, persistCurrentLlmAsProfile } from "../utils/llmProfiles";
import { useI18n } from "../i18n";

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

const inputClsOnboarding =
  "w-full rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-ink dark:text-ink-dark placeholder-ink-mute text-sm focus:ring-2 focus:ring-accent/40 focus:border-accent outline-none";

export default function OnboardingModal() {
  const { t } = useI18n();
  const starterLines = useMemo(
    () => [t("onboarding.starter1"), t("onboarding.starter2"), t("onboarding.starter3")],
    [t]
  );
  const { settings, setSettings } = useSettingsStore();
  const [step, setStep] = useState<Step>("mode");
  const [mode, setMode] = useState<Mode | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("gpt-4o");
  const [apiBase, setApiBase] = useState("");
  const apiBaseReuseRef = useRef("");
  apiBaseReuseRef.current = apiBase;
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
    const prevProfiles = useSettingsStore.getState().settings.llmProfiles;
    if (mode === "ollama") {
      const m = ollamaModel || "llama3.2";
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: m,
        workspace: ws,
        llmProfiles: persistCurrentLlmAsProfile(prevProfiles, {
          provider: "ollama",
          model: m,
          apiBase: "http://localhost:11434/v1",
          apiKey: "ollama",
        }),
        ...shared,
      });
    } else {
      const m = model.trim() || "gpt-4o";
      const ab = apiBase.trim();
      const key = apiKey.trim();
      setSettings({
        provider: "api",
        apiKey: key,
        model: m,
        workspace: ws,
        apiBase: ab,
        llmProfiles: persistCurrentLlmAsProfile(prevProfiles, {
          provider: "api",
          model: m,
          apiBase: ab,
          apiKey: key,
        }),
        ...shared,
      });
    }
  };

  /** 健康检查通过后写入工作区/沙箱/模型等，但不标记引导完成（仍显示成功页与「进入聊天」） */
  const persistAllButNotCompleted = () => {
    const ws = workspace.trim() || ".";
    const shared = { sandboxLevel, workspace: ws };
    const prevProfiles = useSettingsStore.getState().settings.llmProfiles;
    if (mode === "ollama") {
      const m = ollamaModel || "llama3.2";
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: m,
        llmProfiles: persistCurrentLlmAsProfile(prevProfiles, {
          provider: "ollama",
          model: m,
          apiBase: "http://localhost:11434/v1",
          apiKey: "ollama",
        }),
        ...shared,
      });
    } else if (mode === "api") {
      const m = model.trim() || "gpt-4o";
      const ab = apiBase.trim();
      const key = apiKey.trim();
      setSettings({
        provider: "api",
        apiKey: key,
        model: m,
        apiBase: ab,
        llmProfiles: persistCurrentLlmAsProfile(prevProfiles, {
          provider: "api",
          model: m,
          apiBase: ab,
          apiKey: key,
        }),
        ...shared,
      });
    }
  };

  /** 离开「API / Ollama 配置」时立即写入本地存储，避免仅走完健康检查才保存导致 Key 丢失 */
  const persistLlmAndGoToWorkspace = () => {
    const prevProfiles = useSettingsStore.getState().settings.llmProfiles;
    if (mode === "api") {
      const m = model.trim() || "gpt-4o";
      const ab = apiBase.trim();
      const key = apiKey.trim();
      setSettings({
        provider: "api",
        apiKey: key,
        model: m,
        apiBase: ab,
        llmProfiles: persistCurrentLlmAsProfile(prevProfiles, {
          provider: "api",
          model: m,
          apiBase: ab,
          apiKey: key,
        }),
      });
    } else if (mode === "ollama") {
      const m = ollamaModel || "llama3.2";
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: m,
        llmProfiles: persistCurrentLlmAsProfile(prevProfiles, {
          provider: "ollama",
          model: m,
          apiBase: "http://localhost:11434/v1",
          apiKey: "ollama",
        }),
      });
    }
    setStep("workspace");
  };

  const handleBrowseWorkspace = async () => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      title: t("settings.pickWorkspace"),
      defaultPath: workspace || undefined,
    });
    if (selected) setWorkspace(selected);
  };

  const handleCreateExample = async () => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      title: t("onboarding.pickExampleParent"),
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
      setHealthError(t("onboarding.envNotReady"));
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
        apiKey: mode === "api" ? apiKey.trim() : undefined,
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
          {t("onboarding.welcomeTitle")}
        </h2>
        <p className="text-sm text-ink-mute dark:text-ink-dark-mute mb-6">
          {t("onboarding.welcomeSubtitle")}
        </p>
        <button
          type="button"
          onClick={() =>
            setSettings({
              onboardingCompleted: true,
              showStarterPrompts: false,
            })
          }
          className="absolute top-4 right-4 text-xs text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
        >
          {t("onboarding.skipLater")}
        </button>

        {step === "mode" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              {t("onboarding.step1Mode")}
            </p>
            {ollamaLoading && mode === null && (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-2">
                {t("onboarding.detectingOllamaLocal")}
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
                  {t("onboarding.modeApiTitle")}
                </span>
                <span className="text-xs text-ink-mute dark:text-ink-dark-mute">
                  {t("onboarding.modeApiDesc")}
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
                  {t("onboarding.modeOllamaTitle")}
                </span>
                <span className="text-xs text-ink-mute dark:text-ink-dark-mute">
                  {t("onboarding.modeOllamaDesc")}
                </span>
              </button>
            </div>
          </>
        )}

        {step === "config" && mode === "api" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              {t("onboarding.step2ApiConfig")}
            </p>
            <div className="space-y-3 mb-4">
              <div>
                <label className="block text-xs text-ink-mute dark:text-ink-dark-mute mb-1">
                  {t("settings.model")}
                </label>
                <ModelComboBox
                  value={model}
                  onChange={(next) => {
                    setModel(next);
                    const profiles = useSettingsStore.getState().settings.llmProfiles;
                    const p = findSavedProfileForModel(
                      profiles,
                      "api",
                      next,
                      apiBaseReuseRef.current
                    );
                    if (p) {
                      setApiKey(p.apiKey);
                      setApiBase(p.apiBase);
                      apiBaseReuseRef.current = p.apiBase;
                    } else {
                      setApiKey("");
                      const presetBase = presetApiBaseForModelId(next);
                      if (presetBase) {
                        setApiBase(presetBase);
                        apiBaseReuseRef.current = presetBase;
                      } else {
                        setApiBase("");
                        apiBaseReuseRef.current = "";
                      }
                    }
                  }}
                  onPresetSelect={(preset) => {
                    if (preset.apiBase) {
                      apiBaseReuseRef.current = preset.apiBase;
                      setApiBase(preset.apiBase);
                    }
                  }}
                  presets={API_MODEL_PRESETS}
                  placeholder={t("settings.modelPlaceholder")}
                  inputCls={inputClsOnboarding}
                />
              </div>
              <div>
                <label className="block text-xs text-ink-mute dark:text-ink-dark-mute mb-1">
                  {t("settings.apiKey")}
                </label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder={t("onboarding.apiKeySaveHint")}
                  className={inputClsOnboarding}
                />
              </div>
              <div>
                <label className="block text-xs text-ink-mute dark:text-ink-dark-mute mb-1">
                  {t("settings.apiBase")}
                </label>
                <input
                  type="text"
                  value={apiBase}
                  onChange={(e) => setApiBase(e.target.value)}
                  placeholder={t("settings.apiBasePlaceholder")}
                  className={inputClsOnboarding}
                />
                {apiBase.trim() !== "" && (
                  <p className="mt-1 text-xs text-ink-mute dark:text-ink-dark-mute">
                    {API_MODEL_PRESETS.find((p) => p.value === model)?.apiBase === apiBase.trim()
                      ? t("onboarding.apiBaseAutoOnboarding")
                      : t("settings.apiBaseCustom")}
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
                {t("onboarding.back")}
              </button>
              <button
                type="button"
                onClick={persistLlmAndGoToWorkspace}
                disabled={!canContinueApi}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {t("onboarding.next")}
              </button>
            </div>
          </>
        )}

        {step === "config" && mode === "ollama" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              {t("onboarding.step2Ollama")}
            </p>
            {ollamaAutoDetected && (
              <p className="text-xs text-green-600 dark:text-green-400 mb-3">
                {t("onboarding.ollamaAutoBanner")}
              </p>
            )}
            {ollamaLoading ? (
              <p className="text-sm text-ink-mute mb-4">{t("onboarding.ollamaDetecting")}</p>
            ) : ollamaModels.length > 0 ? (
              <p className="text-sm text-ink-mute dark:text-ink-dark-mute mb-4">
                {t("onboarding.ollamaFoundN", { n: ollamaModels.length })}
              </p>
            ) : (
              <p className="text-sm text-ink-mute dark:text-ink-dark-mute mb-4">
                {t("onboarding.ollamaMissingHelp")}
              </p>
            )}
            {!ollamaLoading && ollamaModels.length > 0 && (
              <div className="mb-2">
                <label className="block text-xs text-ink-mute mb-1">{t("onboarding.useModel")}</label>
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
                {t("onboarding.back")}
              </button>
              <button
                type="button"
                onClick={persistLlmAndGoToWorkspace}
                disabled={!canContinueOllama}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {t("onboarding.next")}
              </button>
            </div>
          </>
        )}

        {step === "workspace" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              {t("onboarding.step3Workspace")}
            </p>
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-3">
              {t("onboarding.workspaceIntro")}
            </p>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={workspace}
                onChange={(e) => setWorkspace(e.target.value)}
                placeholder={t("onboarding.workspaceInputPh")}
                className="flex-1 min-w-0 rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-sm"
              />
              <button
                type="button"
                onClick={handleBrowseWorkspace}
                className="shrink-0 px-3 py-2 rounded-lg border border-border dark:border-border-dark text-sm font-medium hover:bg-gray-100 dark:hover:bg-white/5"
              >
                {t("common.browse")}
              </button>
            </div>
            <button
              type="button"
              onClick={handleCreateExample}
              disabled={initCreating}
              className="w-full py-2 rounded-lg border border-dashed border-border dark:border-border-dark text-sm text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark hover:border-accent/50 transition-colors disabled:opacity-50"
            >
              {initCreating ? t("onboarding.createExampleLoading") : t("onboarding.createExampleCta")}
            </button>
            {initError && (
              <p className="mt-2 text-xs text-red-600 dark:text-red-400">{initError}</p>
            )}
            <div className="mt-3 rounded-lg bg-ink/5 dark:bg-white/5 px-3 py-2 text-xs text-ink-mute dark:text-ink-dark-mute">
              {t("onboarding.currentPath", { path: workspaceDisplay })}
            </div>
            <div className="flex justify-end gap-2 mt-4">
              <button
                type="button"
                onClick={() => setStep("config")}
                className="px-3 py-1.5 text-sm text-ink-mute"
              >
                {t("onboarding.back")}
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
                {t("onboarding.next")}
              </button>
            </div>
          </>
        )}

        {step === "sandbox" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              {t("onboarding.step4Sandbox")}
            </p>
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-3">
              {t("onboarding.sandboxIntro")}
            </p>
            <div className="space-y-2 mb-4">
              {([
                {
                  level: 3 as const,
                  title: t("onboarding.sandboxL3Title"),
                  desc: t("onboarding.sandboxL3Desc"),
                },
                {
                  level: 2 as const,
                  title: t("onboarding.sandboxL2Title"),
                  desc: t("onboarding.sandboxL2Desc"),
                },
                {
                  level: 1 as const,
                  title: t("onboarding.sandboxL1Title"),
                  desc: t("onboarding.sandboxL1Desc"),
                },
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
                {t("onboarding.back")}
              </button>
              <button
                type="button"
                onClick={handleRunHealthCheck}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium"
              >
                {t("onboarding.checkAndContinue")}
              </button>
            </div>
          </>
        )}

        {step === "health" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              {t("onboarding.step5Health")}
            </p>
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-3">
              {t("onboarding.healthIntro")}
            </p>
            {healthChecking && (
              <div className="rounded-lg border border-border dark:border-border-dark px-3 py-3 text-sm text-ink-mute dark:text-ink-dark-mute">
                {t("onboarding.healthRunning")}
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
                  { label: t("onboarding.healthBinary"), item: healthResult.binary },
                  { label: t("onboarding.healthProvider"), item: healthResult.provider },
                  { label: t("onboarding.healthWorkspace"), item: healthResult.workspace },
                  { label: t("onboarding.healthDataDir"), item: healthResult.data_dir },
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
                {t("onboarding.returnEdit")}
              </button>
              <button
                type="button"
                onClick={handleRunHealthCheck}
                disabled={healthChecking}
                className="px-4 py-1.5 text-sm rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark disabled:opacity-50"
              >
                {t("onboarding.recheck")}
              </button>
            </div>
          </>
        )}

        {step === "success" && (
          <>
            <p className="text-sm font-medium text-ink dark:text-ink-dark mb-2">
              {t("onboarding.step6Done")}
            </p>
            <div className="rounded-lg border border-green-200 dark:border-green-800/50 bg-green-50 dark:bg-green-900/20 px-3 py-3 text-sm text-green-800 dark:text-green-200 mb-3">
              {t("onboarding.successBlurb")}
            </div>
            <div className="space-y-2 mb-4">
              {starterLines.map((action) => (
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
                {t("onboarding.returnEdit")}
              </button>
              <button
                type="button"
                onClick={applySettingsAndFinish}
                className="px-4 py-1.5 text-sm rounded-lg bg-accent text-white font-medium"
              >
                {t("onboarding.enterChat")}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
