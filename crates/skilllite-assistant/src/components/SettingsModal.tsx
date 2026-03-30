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

type SettingsTabId = "llm" | "workspace" | "agent" | "schedule";

export default function SettingsModal({ open, onClose }: SettingsModalProps) {
  const { t, locale, setLocale } = useI18n();
  const settingsTabs = useMemo(
    () =>
      [
        { id: "llm" as const, label: t("settings.tab.llm") },
        { id: "workspace" as const, label: t("settings.tab.workspace") },
        { id: "agent" as const, label: t("settings.tab.agent") },
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
    const trimmed = s.trim();
    if (!trimmed) return undefined;
    const n = Number(trimmed);
    if (!Number.isInteger(n) || n < 1) return undefined;
    return n;
  };

  const handleSave = async () => {
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
            {t("settings.title")}
          </h2>
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
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-4 min-h-0">

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
        <div className="px-5 py-3 border-t border-border dark:border-border-dark flex justify-end gap-2 shrink-0">
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
    </div>
  );
}
