import { useState, useEffect, useCallback, useMemo } from "react";
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
import { useI18n } from "../i18n";

interface OllamaProbeResult {
  available: boolean;
  models: string[];
  has_embedding: boolean;
}

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
}

type SettingsTabId = "llm" | "workspace" | "agent" | "evolution" | "schedule";

export default function SettingsModal({ open, onClose }: SettingsModalProps) {
  const { t, locale, setLocale } = useI18n();
  const settingsTabs = useMemo(
    () =>
      [
        { id: "llm" as const, label: t("settings.tab.llm") },
        { id: "workspace" as const, label: t("settings.tab.workspace") },
        { id: "agent" as const, label: t("settings.tab.agent") },
        { id: "evolution" as const, label: t("settings.tab.evolution") },
        { id: "schedule" as const, label: t("settings.tab.schedule") },
      ] as const,
    [t]
  );
  const { settings, setSettings } = useSettingsStore();
  const [provider, setProvider] = useState<Provider>(settings.provider || "api");
  const [apiKey, setApiKey] = useState(settings.apiKey);
  const [model, setModel] = useState(settings.model);
  const [workspace, setWorkspace] = useState(settings.workspace);
  const [apiBase, setApiBase] = useState(settings.apiBase);

  const [sandboxLevel, setSandboxLevel] = useState<SandboxLevel>(settings.sandboxLevel ?? 3);
  const [swarmEnabled, setSwarmEnabled] = useState(settings.swarmEnabled ?? false);
  const [swarmUrl, setSwarmUrl] = useState(settings.swarmUrl ?? "");
  const [autoApproveToolConfirmations, setAutoApproveToolConfirmations] = useState(
    settings.autoApproveToolConfirmations === true
  );
  const [maxIterationsStr, setMaxIterationsStr] = useState("");
  const [maxToolCallsPerTaskStr, setMaxToolCallsPerTaskStr] = useState("");
  const [evolutionIntervalStr, setEvolutionIntervalStr] = useState("");
  const [evolutionDecisionStr, setEvolutionDecisionStr] = useState("");
  const [evoProfileChoice, setEvoProfileChoice] = useState<"inherit" | "demo" | "conservative">(
    "inherit"
  );
  const [evoCooldownStr, setEvoCooldownStr] = useState("");

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
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

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
      setAutoApproveToolConfirmations(settings.autoApproveToolConfirmations === true);
      setMaxIterationsStr(
        settings.maxIterations != null ? String(settings.maxIterations) : ""
      );
      setMaxToolCallsPerTaskStr(
        settings.maxToolCallsPerTask != null ? String(settings.maxToolCallsPerTask) : ""
      );
      setEvolutionIntervalStr(
        settings.evolutionIntervalSecs != null ? String(settings.evolutionIntervalSecs) : ""
      );
      setEvolutionDecisionStr(
        settings.evolutionDecisionThreshold != null
          ? String(settings.evolutionDecisionThreshold)
          : ""
      );
      setEvoProfileChoice(settings.evoProfile ?? "inherit");
      setEvoCooldownStr(settings.evoCooldownHours != null ? String(settings.evoCooldownHours) : "");
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
    const trimmed = s.trim();
    if (!trimmed) return undefined;
    const n = Number(trimmed);
    if (!Number.isInteger(n) || n < 1) return undefined;
    return n;
  };

  const parseEvolutionIntervalSecs = (s: string): number | undefined => {
    const trimmed = s.trim();
    if (!trimmed) return undefined;
    const n = parseInt(trimmed, 10);
    if (!Number.isFinite(n) || n < 1) return undefined;
    return n;
  };

  const parseCooldownHoursField = (s: string): number | undefined => {
    const trimmed = s.trim();
    if (!trimmed) return undefined;
    const n = parseFloat(trimmed);
    if (!Number.isFinite(n) || n < 0) return undefined;
    return n;
  };

  const handleSave = async () => {
    const shared = {
      sandboxLevel,
      swarmEnabled,
      swarmUrl: swarmUrl.trim(),
      autoApproveToolConfirmations,
      maxIterations: parsePositiveIntField(maxIterationsStr),
      maxToolCallsPerTask: parsePositiveIntField(maxToolCallsPerTaskStr),
      evolutionIntervalSecs: parseEvolutionIntervalSecs(evolutionIntervalStr),
      evolutionDecisionThreshold: parsePositiveIntField(evolutionDecisionStr),
      evoProfile:
        evoProfileChoice === "inherit" ? undefined : (evoProfileChoice as "demo" | "conservative"),
      evoCooldownHours: parseCooldownHoursField(evoCooldownStr),
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

    // 定时配置异步加载（工作区变更时还有 debounce）；勿阻塞 LLM/工作区等设置的保存。
    if (!scheduleData) {
      onClose();
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
    onClose();
  };

  const handleBrowseWorkspace = async () => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      title: t("settings.pickWorkspace"),
      defaultPath: workspace && workspace !== "." ? workspace : undefined,
    });
    if (selected) {
      setWorkspace(selected);
    }
  };

  const sbKey = `l${sandboxLevel}` as "l1" | "l2" | "l3";

  const inputCls =
    "w-full rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-ink dark:text-ink-dark placeholder-ink-mute text-sm focus:ring-2 focus:ring-accent/40 focus:border-accent outline-none";
  const labelCls = "block text-xs font-medium text-ink dark:text-ink-dark-mute mb-1";
  /** 自进化页：略增高、等宽数字、悬停时边框微调 */
  const evolutionFieldCls = `${inputCls} min-h-[2.5rem] tabular-nums transition-[border-color,box-shadow] hover:border-ink/15 dark:hover:border-white/20`;
  const evolutionSelectCls = `${evolutionFieldCls} cursor-pointer appearance-none pr-10`;

  const ollamaModelPresets = ollamaProbe?.available
    ? ollamaProbe.models
        .filter((m) => !m.includes("embed"))
        .map((m) => ({ value: m, label: m }))
    : [];

  return (
    <aside
      className={`relative z-20 flex h-full min-h-0 shrink-0 flex-col border-l border-border dark:border-border-dark bg-white dark:bg-paper-dark transition-[width] duration-200 ease-out motion-reduce:transition-none ${
        open
          ? "w-[min(400px,38vw)] min-w-[260px] border-border dark:border-border-dark"
          : "w-0 min-w-0 overflow-hidden border-transparent"
      }`}
      aria-hidden={!open}
    >
      {open ? (
      <div className="flex h-full min-h-0 w-[min(400px,38vw)] min-w-[260px] flex-col">
        {/* Fixed header + tabs */}
        <div className="px-4 pt-4 pb-0 border-b border-border dark:border-border-dark shrink-0">
          <div className="flex items-center justify-between gap-2 pb-2">
            <h2 className="text-base font-semibold text-ink dark:text-ink-dark">
              {t("settings.title")}
            </h2>
            <button
              type="button"
              onClick={onClose}
              className="shrink-0 rounded-md p-1.5 text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/10 hover:text-ink dark:hover:text-ink-dark transition-colors"
              aria-label={t("common.close")}
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
                <path d="M18 6 6 18" />
                <path d="m6 6 12 12" />
              </svg>
            </button>
          </div>
          <div className="flex gap-1 overflow-x-auto pb-0 -mx-1 px-1">
            {settingsTabs.map((tab) => (
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
        <div className="flex-1 overflow-y-auto overflow-x-hidden px-4 py-3 space-y-4 min-h-0">

          {activeTab === "llm" && (
          <div className="space-y-4">
          <div>
            <label className={labelCls}>{t("locale.label")}</label>
            <div className="flex rounded-lg border border-border dark:border-border-dark overflow-hidden">
              <button
                type="button"
                onClick={() => setLocale("zh")}
                className={`flex-1 py-1.5 text-sm font-medium transition-colors ${
                  locale === "zh"
                    ? "bg-accent text-white"
                    : "bg-gray-50 dark:bg-surface-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5"
                }`}
              >
                {t("locale.zh")}
              </button>
              <button
                type="button"
                onClick={() => setLocale("en")}
                className={`flex-1 py-1.5 text-sm font-medium transition-colors ${
                  locale === "en"
                    ? "bg-accent text-white"
                    : "bg-gray-50 dark:bg-surface-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5"
                }`}
              >
                {t("locale.en")}
              </button>
            </div>
          </div>
          {/* ── Provider ── */}
          <div>
            <label className={labelCls}>{t("settings.providerMode")}</label>
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
                {t("settings.providerApi")}
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
                {t("settings.providerOllama")}
              </button>
            </div>
          </div>

          {/* ── API config ── */}
          {provider === "api" && (
            <>
              <div>
                <label className={labelCls}>{t("settings.apiKey")}</label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder={t("settings.apiKeyHint")}
                  className={inputCls}
                />
              </div>
              <div>
                <label className={labelCls}>{t("settings.model")}</label>
                <ModelComboBox
                  value={model}
                  onChange={setModel}
                  onPresetSelect={(preset) => {
                    if (preset.apiBase) {
                      setApiBase(preset.apiBase);
                    }
                  }}
                  presets={API_MODEL_PRESETS}
                  placeholder={t("settings.modelPlaceholder")}
                  inputCls={inputCls}
                />
              </div>
              <div>
                <label className={labelCls}>{t("settings.apiBase")}</label>
                <input
                  type="text"
                  value={apiBase}
                  onChange={(e) => setApiBase(e.target.value)}
                  placeholder={t("settings.apiBasePlaceholder")}
                  className={inputCls}
                />
                {apiBase && (
                  <p className="mt-1 text-xs text-ink-mute dark:text-ink-dark-mute">
                    {API_MODEL_PRESETS.find((p) => p.value === model)?.apiBase === apiBase
                      ? t("settings.apiBaseMatched")
                      : t("settings.apiBaseCustom")}
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
                  {t("settings.ollamaProbing")}
                </p>
              ) : ollamaProbe?.available ? (
                <>
                  {ollamaModelPresets.length > 0 ? (
                    <div>
                      <label className={labelCls}>{t("settings.model")}</label>
                      <ModelComboBox
                        value={model}
                        onChange={setModel}
                        presets={ollamaModelPresets}
                        placeholder={t("settings.modelPlaceholder")}
                        inputCls={inputCls}
                      />
                    </div>
                  ) : (
                    <p className="text-sm text-amber-600 dark:text-amber-400 py-1">
                      {t("settings.ollamaNoModels")}{" "}
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
                      ? t("settings.ollamaEmbedYes")
                      : t("settings.ollamaEmbedNo")}
                  </div>
                </>
              ) : (
                <div className="py-1">
                  <p className="text-sm text-red-600 dark:text-red-400 mb-1">
                    {t("settings.ollamaMissing")}
                  </p>
                  <p className="text-xs text-ink-mute dark:text-ink-dark-mute">
                    {t("settings.ollamaHint")}
                    <code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded">
                      ollama serve
                    </code>
                    {t("settings.ollamaHintEnd")}
                  </p>
                  <button
                    type="button"
                    onClick={probeOllama}
                    className="mt-1.5 text-sm text-accent hover:underline"
                  >
                    {t("settings.ollamaRetry")}
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
            <label className={labelCls}>{t("settings.workspacePath")}</label>
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
                {t("common.browse")}
              </button>
            </div>
            <p className="mt-1 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              {t("settings.workspaceHintPrefix")}
              <code className="bg-gray-100 dark:bg-surface-dark px-1 py-0.5 rounded">
                .skilllite/schedule.json
              </code>
              {t("settings.workspaceHintSuffix")}
            </p>
          </div>

          {/* ── Sandbox Level ── */}
          <div>
            <label className={labelCls}>{t("settings.sandboxLevel")}</label>
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
              {t(`settings.sandbox.${sbKey}.short`)} — {t(`settings.sandbox.${sbKey}.desc`)}
            </p>
          </div>

          {/* ── Swarm Network ── */}
          <div>
            <div className="flex items-center justify-between">
              <label className={`${labelCls} mb-0`}>{t("settings.swarm")}</label>
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
                  {t("settings.swarmHint")}
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
              {t("settings.agentBudget")}
            </p>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className={labelCls}>{t("settings.maxIterations")}</label>
                <input
                  type="number"
                  min={1}
                  inputMode="numeric"
                  value={maxIterationsStr}
                  onChange={(e) => setMaxIterationsStr(e.target.value)}
                  placeholder={t("settings.defaultPlaceholder50")}
                  className={inputCls}
                />
              </div>
              <div>
                <label className={labelCls}>{t("settings.maxToolCalls")}</label>
                <input
                  type="number"
                  min={1}
                  inputMode="numeric"
                  value={maxToolCallsPerTaskStr}
                  onChange={(e) => setMaxToolCallsPerTaskStr(e.target.value)}
                  placeholder={t("settings.defaultPlaceholder15")}
                  className={inputCls}
                />
              </div>
            </div>
            <p className="mt-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              {t("settings.agentBudgetHint")}
            </p>
          </div>

          <div className="rounded-lg border border-border/60 dark:border-border-dark/50 bg-gray-50/80 dark:bg-surface-dark/35 px-3 py-2.5">
            <div className="flex items-center justify-between gap-3">
              <span
                className="text-sm text-ink dark:text-ink-dark-mute"
                title={t("chat.autoApproveToolConfirmationsHint")}
              >
                {t("chat.autoApproveToolConfirmations")}
              </span>
              <button
                type="button"
                role="switch"
                aria-checked={autoApproveToolConfirmations}
                aria-label={t("chat.autoApproveToolConfirmations")}
                onClick={() => setAutoApproveToolConfirmations(!autoApproveToolConfirmations)}
                className={`relative inline-flex h-5 w-9 shrink-0 rounded-full border-2 border-transparent transition-colors cursor-pointer ${
                  autoApproveToolConfirmations ? "bg-accent" : "bg-gray-300 dark:bg-gray-600"
                }`}
              >
                <span
                  className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow transform transition-transform ${
                    autoApproveToolConfirmations ? "translate-x-4" : "translate-x-0"
                  }`}
                />
              </button>
            </div>
            <p className="mt-2 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              {t("chat.autoApproveToolConfirmationsHint")}
            </p>
          </div>
          </div>
          )}

          {activeTab === "evolution" && (
          <div className="space-y-4">
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              {t("settings.evolutionIntro")}
            </p>
            <div className="rounded-xl border border-border/60 dark:border-border-dark/50 bg-gray-50/80 dark:bg-surface-dark/35 p-4 space-y-4 shadow-[0_1px_2px_rgba(0,0,0,0.04)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.2)]">
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <div className="min-w-0">
                  <label className={labelCls}>{t("evolution.thresholds.interval")}</label>
                  <input
                    type="number"
                    min={60}
                    step={60}
                    inputMode="numeric"
                    placeholder="1800"
                    value={evolutionIntervalStr}
                    onChange={(e) => setEvolutionIntervalStr(e.target.value)}
                    className={evolutionFieldCls}
                  />
                  <p className="mt-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                    {t("evolution.thresholds.intervalHint")}
                  </p>
                </div>
                <div className="min-w-0">
                  <label className={labelCls}>{t("evolution.thresholds.decision")}</label>
                  <input
                    type="number"
                    min={1}
                    step={1}
                    inputMode="numeric"
                    placeholder="10"
                    value={evolutionDecisionStr}
                    onChange={(e) => setEvolutionDecisionStr(e.target.value)}
                    className={evolutionFieldCls}
                  />
                  <p className="mt-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                    {t("evolution.thresholds.decisionHint")}
                  </p>
                </div>
              </div>
              <div>
                <label className={labelCls} htmlFor="settings-evo-profile">
                  {t("evolution.thresholds.profile")}
                </label>
                <div className="relative">
                  <select
                    id="settings-evo-profile"
                    value={evoProfileChoice}
                    onChange={(e) => {
                      const v = e.target.value;
                      if (v === "inherit" || v === "demo" || v === "conservative") {
                        setEvoProfileChoice(v);
                      }
                    }}
                    className={evolutionSelectCls}
                  >
                    <option value="inherit">{t("evolution.thresholds.profileInherit")}</option>
                    <option value="demo">{t("evolution.profile.demo")}</option>
                    <option value="conservative">{t("evolution.profile.conservative")}</option>
                  </select>
                  <span
                    className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-ink-mute dark:text-ink-dark-mute"
                    aria-hidden
                  >
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="18"
                      height="18"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      className="opacity-70"
                    >
                      <path d="m6 9 6 6 6-6" />
                    </svg>
                  </span>
                </div>
                <p className="mt-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                  {t("evolution.thresholds.profileHint")}
                </p>
              </div>
              <div>
                <label className={labelCls}>{t("evolution.thresholds.cooldown")}</label>
                <input
                  type="number"
                  min={0}
                  step={0.25}
                  inputMode="decimal"
                  placeholder="1"
                  value={evoCooldownStr}
                  onChange={(e) => setEvoCooldownStr(e.target.value)}
                  className={evolutionFieldCls}
                />
                <p className="mt-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                  {t("evolution.thresholds.cooldownHint")}
                </p>
              </div>
            </div>
            <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed border-t border-border/40 dark:border-border-dark/40 pt-2">
              {t("evolution.thresholds.note")}
            </p>
          </div>
          )}

          {activeTab === "schedule" && (
          <div className="space-y-3">
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              {t("settings.scheduleIntro")}
            </p>
            {scheduleData === null ? (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute py-4">
                {scheduleLoadError ? scheduleLoadError : t("settings.scheduleLoading")}
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
              {t("settings.scheduleReload")}
            </button>
          </div>
          )}
        </div>

        {/* Fixed footer */}
        <div className="px-4 py-3 border-t border-border dark:border-border-dark flex justify-end gap-2 shrink-0 bg-white dark:bg-paper-dark">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-1.5 text-sm text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark transition-colors"
          >
            {t("common.cancel")}
          </button>
          <button
            type="button"
            onClick={handleSave}
            className="px-4 py-1.5 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent-hover transition-colors"
          >
            {t("common.save")}
          </button>
        </div>
      </div>
      ) : null}
    </aside>
  );
}
