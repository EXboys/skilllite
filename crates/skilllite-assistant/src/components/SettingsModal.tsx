import { useState, useEffect, useLayoutEffect, useCallback, useMemo, useRef } from "react";
import { ask, message, open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import {
  useSettingsStore,
  type McpServerConfig,
  type Provider,
  type SandboxLevel,
} from "../stores/useSettingsStore";
import ScheduleEditor from "./ScheduleEditor";
import ModelComboBox from "./ModelComboBox";
import { API_MODEL_PRESETS, presetApiBaseForModelId } from "../utils/modelPresets";
import {
  findSavedProfileForModel,
  persistCurrentLlmAsProfile,
} from "../utils/llmProfiles";
import {
  type ScheduleForm,
  emptyScheduleForm,
  parseScheduleJson,
  scheduleFormToJson,
  validateScheduleForm,
} from "../utils/scheduleForm";
import { useI18n } from "../i18n";
import { useStatusStore } from "../stores/useStatusStore";
import type { AssistantSettingsTabId } from "../contexts/AssistantChromeContext";
import EnvironmentSettingsSection from "./EnvironmentSettingsSection";
import { SettingsNavIcon } from "./settings/SettingsNavIcon";

/** Local list row: stable key for React (do not use user `id` in `key` or inputs remount on every edit). */
type McpRowState = McpServerConfig & { _rowKey: string };

function newMcpRowState(): McpRowState {
  return {
    id: "",
    enabled: true,
    command: "",
    args: [],
    _rowKey: crypto.randomUUID(),
  };
}

function mcpRowStateFromSaved(s: McpServerConfig): McpRowState {
  return {
    id: s.id,
    enabled: s.enabled,
    command: s.command,
    args: [...s.args],
    ...(s.cwd ? { cwd: s.cwd } : {}),
    _rowKey: crypto.randomUUID(),
  };
}

/** Parse one JSON object from an external array (e.g. SKILLLITE_MCP_SERVERS_JSON). */
function mcpRowFromUnknown(item: unknown, index: number): McpRowState {
  if (!item || typeof item !== "object" || Array.isArray(item)) {
    throw new Error(`Entry ${index + 1}: expected an object`);
  }
  const o = item as Record<string, unknown>;
  const id = typeof o.id === "string" ? o.id.trim() : "";
  const command = typeof o.command === "string" ? o.command.trim() : "";
  if (!id) {
    throw new Error(`Entry ${index + 1}: missing or empty "id"`);
  }
  if (!command) {
    throw new Error(`Entry ${index + 1}: missing or empty "command"`);
  }
  const enabled = typeof o.enabled === "boolean" ? o.enabled : true;
  let args: string[] = [];
  if (o.args !== undefined) {
    if (!Array.isArray(o.args)) {
      throw new Error(`Entry ${index + 1}: "args" must be an array`);
    }
    args = o.args.map((a, j) => {
      if (typeof a !== "string" && typeof a !== "number") {
        throw new Error(`Entry ${index + 1}: args[${j}] must be string or number`);
      }
      return String(a);
    });
  }
  let cwd: string | undefined;
  if (o.cwd !== undefined) {
    if (typeof o.cwd !== "string") {
      throw new Error(`Entry ${index + 1}: "cwd" must be a string`);
    }
    cwd = o.cwd.trim() || undefined;
  }
  return mcpRowStateFromSaved({
    id,
    enabled,
    command,
    args,
    ...(cwd ? { cwd } : {}),
  });
}

interface OllamaProbeResult {
  available: boolean;
  models: string[];
  has_embedding: boolean;
}

interface AssistantUninstallInfo {
  platform: string;
  executableParent: string;
  macosAppBundlePath: string | null;
  tauriAppDataDir: string;
  skillliteChatRoot: string;
  skillliteDataRoot: string;
  canScheduleMacosBundleRemoval: boolean;
  isDevBuild: boolean;
}

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
  /** 打开时默认切到的设置标签（缺省为「模型与 API」） */
  initialTabId?: AssistantSettingsTabId;
}

type SettingsTabId = AssistantSettingsTabId;

export default function SettingsModal({
  open,
  onClose,
  initialTabId,
}: SettingsModalProps) {
  const { t, locale, setLocale } = useI18n();
  /** 左侧分组导航：连接 → 工作区/环境 → Agent 调度 → 维护 */
  const settingsNavGroups = useMemo(
    () =>
      [
        {
          id: "connection",
          titleKey: "settings.navGroup.connection" as const,
          tabs: [{ id: "llm" as const, label: t("settings.tab.llm") }],
        },
        {
          id: "workspace",
          titleKey: "settings.navGroup.workspace" as const,
          tabs: [
            { id: "workspace" as const, label: t("settings.tab.workspace") },
            { id: "mcp" as const, label: t("settings.tab.mcp") },
            { id: "environment" as const, label: t("settings.tab.environment") },
          ],
        },
        {
          id: "automation",
          titleKey: "settings.navGroup.automation" as const,
          tabs: [
            { id: "agent" as const, label: t("settings.tab.agent") },
            { id: "evolution" as const, label: t("settings.tab.evolution") },
            { id: "schedule" as const, label: t("settings.tab.schedule") },
          ],
        },
        {
          id: "maintenance",
          titleKey: "settings.navGroup.maintenance" as const,
          tabs: [{ id: "uninstall" as const, label: t("settings.tab.uninstall") }],
        },
      ] as const,
    [t]
  );
  const { settings, setSettings } = useSettingsStore();
  const [provider, setProvider] = useState<Provider>(settings.provider || "api");
  const [apiKey, setApiKey] = useState(settings.apiKey);
  const [model, setModel] = useState(settings.model);
  const [workspace, setWorkspace] = useState(settings.workspace);
  const [apiBase, setApiBase] = useState(settings.apiBase);
  /** 与 `ModelComboBox` 内「先 preset 再 onChange」顺序配合，用于按 apiBase 命中已保存 Key。 */
  const apiBaseReuseRef = useRef(settings.apiBase);
  apiBaseReuseRef.current = apiBase;

  const [sandboxLevel, setSandboxLevel] = useState<SandboxLevel>(settings.sandboxLevel ?? 3);
  const [swarmEnabled, setSwarmEnabled] = useState(settings.swarmEnabled ?? false);
  const [swarmUrl, setSwarmUrl] = useState(settings.swarmUrl ?? "");
  const [mcpRows, setMcpRows] = useState<McpRowState[]>([]);
  /** Raw JSON paste buffer for MCP server list (same shape as SKILLLITE_MCP_SERVERS_JSON). */
  const [mcpBulkJson, setMcpBulkJson] = useState("");
  const [ideLayout, setIdeLayout] = useState(settings.ideLayout === true);
  const [autoApproveToolConfirmations, setAutoApproveToolConfirmations] = useState(
    settings.autoApproveToolConfirmations === true
  );
  const [maxIterationsStr, setMaxIterationsStr] = useState("");
  const [maxToolCallsPerTaskStr, setMaxToolCallsPerTaskStr] = useState("");
  const [contextSoftLimitStr, setContextSoftLimitStr] = useState("");
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
  const [uninstallInfo, setUninstallInfo] = useState<AssistantUninstallInfo | null>(null);

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
    if (!open || activeTab !== "uninstall") return;
    void (async () => {
      try {
        const info = await invoke<AssistantUninstallInfo>("assistant_uninstall_info");
        setUninstallInfo(info);
      } catch {
        setUninstallInfo(null);
      }
    })();
  }, [open, activeTab]);

  const runQuitUninstall = useCallback(
    async (removeUserData: boolean) => {
      const detail = removeUserData
        ? t("settings.uninstall.quitWithDataDetail")
        : t("settings.uninstall.quitAppOnlyDetail");
      const q = removeUserData
        ? t("settings.uninstall.quitWithDataAsk")
        : t("settings.uninstall.quitAppOnlyAsk");
      const ok = await ask(`${detail}\n\n${q}`, {
        title: t("settings.uninstall.title"),
        kind: "warning",
        okLabel: removeUserData
          ? t("settings.uninstall.confirmOkWithData")
          : t("settings.uninstall.confirmOkAppOnly"),
        cancelLabel: t("common.cancel"),
      });
      if (!ok) return;
      try {
        await invoke("skilllite_stop").catch(() => {});
        await invoke("assistant_quit_uninstall", {
          removeUserData,
          removeAppBundle: true,
        });
      } catch (e: unknown) {
        const err = e instanceof Error ? e.message : String(e);
        await message(t("settings.uninstall.failed", { err }), {
          title: t("settings.uninstall.title"),
          kind: "error",
        });
      }
    },
    [t]
  );

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  const llmUsageMonth = useStatusStore((s) => s.llmUsageMonth);
  const rollLlmUsageMonthIfNeeded = useStatusStore((s) => s.rollLlmUsageMonthIfNeeded);
  useEffect(() => {
    if (!open) return;
    rollLlmUsageMonthIfNeeded();
  }, [open, rollLlmUsageMonthIfNeeded]);

  useLayoutEffect(() => {
    if (!open) return;
    setActiveTab(initialTabId ?? "llm");
  }, [open, initialTabId]);

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
      setMcpRows(
        settings.mcpServers?.length
          ? settings.mcpServers.map(mcpRowStateFromSaved)
          : []
      );
      setIdeLayout(settings.ideLayout === true);
      setAutoApproveToolConfirmations(settings.autoApproveToolConfirmations === true);
      setMaxIterationsStr(
        settings.maxIterations != null ? String(settings.maxIterations) : ""
      );
      setMaxToolCallsPerTaskStr(
        settings.maxToolCallsPerTask != null ? String(settings.maxToolCallsPerTask) : ""
      );
      setContextSoftLimitStr(
        settings.contextSoftLimitChars != null ? String(settings.contextSoftLimitChars) : ""
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
      setScheduleLoadError(null);
      setScheduleData(null);
      apiBaseReuseRef.current = settings.apiBase ?? "";
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

  /** 非负整数；空为沿用默认；0 表示关闭 `SKILLLITE_CONTEXT_SOFT_LIMIT_CHARS` 预收缩。 */
  const parseContextSoftLimitChars = (s: string): number | undefined => {
    const trimmed = s.trim();
    if (!trimmed) return undefined;
    const n = parseInt(trimmed, 10);
    if (!Number.isFinite(n) || n < 0) return undefined;
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

  const handleApiModelChange = useCallback(
    (next: string) => {
      setModel(next);
      if (provider !== "api") return;
      const p = findSavedProfileForModel(
        settings.llmProfiles,
        "api",
        next,
        apiBaseReuseRef.current
      );
      if (p) {
        setApiKey(p.apiKey);
        setApiBase(p.apiBase);
        apiBaseReuseRef.current = p.apiBase;
      } else {
        // 无已保存项时勿沿用上一模型的 Key/Base（避免 Minimax 显示在 Gemini 等仅用 .env 的配置上）
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
    },
    [provider, settings.llmProfiles]
  );

  const handleOllamaModelChange = useCallback(
    (next: string) => {
      setModel(next);
      if (provider !== "ollama") return;
      const p = findSavedProfileForModel(settings.llmProfiles, "ollama", next);
      if (p) {
        setApiKey(p.apiKey);
        setApiBase(p.apiBase);
      } else {
        setApiKey("ollama");
        setApiBase("http://localhost:11434/v1");
      }
    },
    [provider, settings.llmProfiles]
  );

  const handleSave = async () => {
    const mcpServers = mcpRows
      .filter((r) => r.id.trim() && r.command.trim())
      .map((r) => ({
        id: r.id.trim(),
        enabled: r.enabled,
        command: r.command.trim(),
        args: r.args.map((a) => a.trim()).filter((a) => a.length > 0),
        ...(r.cwd?.trim() ? { cwd: r.cwd.trim() as string } : {}),
      }));

    const shared = {
      ideLayout,
      sandboxLevel,
      swarmEnabled,
      swarmUrl: swarmUrl.trim(),
      autoApproveToolConfirmations,
      maxIterations: parsePositiveIntField(maxIterationsStr),
      maxToolCallsPerTask: parsePositiveIntField(maxToolCallsPerTaskStr),
      contextSoftLimitChars: parseContextSoftLimitChars(contextSoftLimitStr),
      evolutionIntervalSecs: parseEvolutionIntervalSecs(evolutionIntervalStr),
      evolutionDecisionThreshold: parsePositiveIntField(evolutionDecisionStr),
      evoProfile:
        evoProfileChoice === "inherit" ? undefined : (evoProfileChoice as "demo" | "conservative"),
      evoCooldownHours: parseCooldownHoursField(evoCooldownStr),
      mcpServers,
    };
    if (provider === "ollama") {
      const m = model.trim() || "llama3.2";
      const llmProfiles = persistCurrentLlmAsProfile(settings.llmProfiles, {
        provider: "ollama",
        model: m,
        apiBase: "http://localhost:11434/v1",
        apiKey: "ollama",
      });
      setSettings({
        provider: "ollama",
        apiKey: "ollama",
        apiBase: "http://localhost:11434/v1",
        model: m,
        workspace: workspace.trim() || ".",
        llmProfiles,
        ...shared,
      });
    } else {
      const m = model.trim() || "gpt-4o";
      const ab = apiBase.trim();
      const key = apiKey.trim();
      const llmProfiles = persistCurrentLlmAsProfile(settings.llmProfiles, {
        provider: "api",
        model: m,
        apiBase: ab,
        apiKey: key,
      });
      setSettings({
        provider: "api",
        apiKey: key,
        model: m,
        workspace: workspace.trim() || ".",
        apiBase: ab,
        llmProfiles,
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

  if (!open) {
    return null;
  }

  return (
    <div
      className="flex flex-1 min-h-0 min-w-0 w-full flex-col bg-white dark:bg-paper-dark"
      role="main"
      aria-labelledby="settings-page-title"
    >
      <div className="flex min-h-0 flex-1 flex-row">
        <nav
          className="flex w-[min(15rem,36vw)] shrink-0 flex-col gap-5 overflow-y-auto border-r border-border dark:border-border-dark bg-ink/[0.02] dark:bg-white/[0.02] px-2 py-4"
          aria-label={t("settings.navAria")}
        >
          {settingsNavGroups.map((group) => (
            <div key={group.id} className="min-w-0">
              <div className="px-3 pb-2 text-[10px] font-semibold uppercase tracking-wider text-ink-mute dark:text-ink-dark-mute">
                {t(group.titleKey)}
              </div>
              <div className="flex flex-col gap-0.5">
                {group.tabs.map((tab) => {
                  const selected = activeTab === tab.id;
                  return (
                    <button
                      key={tab.id}
                      type="button"
                      onClick={() => setActiveTab(tab.id)}
                      className={`group flex w-full min-w-0 items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors ${
                        selected
                          ? "bg-accent/15 text-accent"
                          : "text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5 hover:text-ink dark:hover:text-ink-dark"
                      }`}
                    >
                      <SettingsNavIcon
                        tabId={tab.id}
                        className={
                          selected
                            ? "h-[1.125rem] w-[1.125rem] text-accent"
                            : "h-[1.125rem] w-[1.125rem] text-ink-mute group-hover:text-ink dark:text-ink-dark-mute dark:group-hover:text-ink-dark"
                        }
                      />
                      <span className="min-w-0 flex-1 truncate">{tab.label}</span>
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </nav>

        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border dark:border-border-dark px-5 py-4">
            <h2
              id="settings-page-title"
              className="truncate text-lg font-semibold text-ink dark:text-ink-dark"
            >
              {t("settings.title")}
            </h2>
            <button
              type="button"
              onClick={onClose}
              className="shrink-0 rounded-md p-2 text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/10 hover:text-ink dark:hover:text-ink-dark transition-colors"
              aria-label={t("common.close")}
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
                <path d="M18 6 6 18" />
                <path d="m6 6 12 12" />
              </svg>
            </button>
          </div>

          <div className="min-h-0 flex-1 space-y-4 overflow-y-auto overflow-x-hidden px-5 py-4">

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
                <label className={labelCls}>{t("settings.model")}</label>
                <ModelComboBox
                  value={model}
                  onChange={handleApiModelChange}
                  onPresetSelect={(preset) => {
                    if (preset.apiBase) {
                      apiBaseReuseRef.current = preset.apiBase;
                      setApiBase(preset.apiBase);
                    }
                  }}
                  presets={API_MODEL_PRESETS}
                  placeholder={t("settings.modelPlaceholder")}
                  inputCls={inputCls}
                />
              </div>
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute -mt-1">
                {t("settings.llmProfilesAutoHint")}
              </p>
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
                        onChange={handleOllamaModelChange}
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

          <section
            className="rounded-lg border border-border/70 dark:border-border-dark/70 bg-ink/[0.02] dark:bg-white/[0.03] px-2.5 py-2"
            aria-label={t("status.llmUsageBannerAria")}
          >
            <div className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
              {t("status.llmUsageBannerTitle", { month: llmUsageMonth.monthKey })}
            </div>
            <p className="mt-1 text-xs tabular-nums leading-snug text-ink dark:text-ink-dark">
              {t("status.llmUsageMonthSummary", {
                inTok: llmUsageMonth.prompt_tokens.toLocaleString(),
                outTok: llmUsageMonth.completion_tokens.toLocaleString(),
                totalTok: llmUsageMonth.total_tokens.toLocaleString(),
              })}
            </p>
            <p className="mt-1 text-[10px] leading-snug text-ink-mute/90 dark:text-ink-dark-mute/90">
              {t("status.llmUsageBannerHint")}
            </p>
          </section>
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
                placeholder={t("settings.workspacePathPlaceholder")}
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
            <p className="mt-1 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              {t("settings.workspaceDefaultNote")}
            </p>
          </div>

          <div>
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className={labelCls}>{t("settings.ideLayout")}</div>
                <p className="mt-0.5 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed max-w-md">
                  {t("settings.ideLayoutHint")}
                </p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={ideLayout}
                onClick={() => {
                  const next = !ideLayout;
                  setIdeLayout(next);
                  setSettings(
                    next
                      ? { ideLayout: true, sessionPanelCollapsed: false }
                      : { ideLayout: false }
                  );
                }}
                className={`relative inline-flex h-5 w-9 shrink-0 rounded-full border-2 border-transparent transition-colors cursor-pointer ${
                  ideLayout ? "bg-accent" : "bg-gray-300 dark:bg-gray-600"
                }`}
              >
                <span
                  className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow transform transition-transform ${
                    ideLayout ? "translate-x-4" : "translate-x-0"
                  }`}
                />
              </button>
            </div>
          </div>

          {/* ── Sandbox Level（独立选项卡，避免与输入框同形态的整框分段条） ── */}
          <div>
            <label id="settings-sandbox-level-label" className={labelCls}>
              {t("settings.sandboxLevel")}
            </label>
            <div
              className="flex flex-col gap-2 sm:flex-row sm:items-stretch"
              role="radiogroup"
              aria-labelledby="settings-sandbox-level-label"
            >
              {([1, 2, 3] as const).map((level) => {
                const selected = sandboxLevel === level;
                const sk = `l${level}` as "l1" | "l2" | "l3";
                return (
                  <button
                    key={level}
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    onClick={() => setSandboxLevel(level)}
                    className={`flex-1 rounded-xl border-2 px-3 py-2.5 text-left transition-all outline-none focus-visible:ring-2 focus-visible:ring-accent/45 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-paper-dark ${
                      selected
                        ? "border-accent bg-accent/10 shadow-sm dark:bg-accent/[0.14]"
                        : "border-border dark:border-border-dark bg-white dark:bg-black/20 hover:border-ink/20 dark:hover:border-white/25"
                    }`}
                  >
                    <span
                      className={`text-sm font-semibold block ${
                        selected ? "text-accent" : "text-ink dark:text-ink-dark"
                      }`}
                    >
                      L{level} · {t(`settings.sandbox.${sk}.short`)}
                    </span>
                  </button>
                );
              })}
            </div>
            <p className="mt-2 text-xs text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              {t(`settings.sandbox.${sbKey}.desc`)}
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

          {activeTab === "mcp" && (
          <div className="space-y-4">
          <div>
            <p className="text-xs font-medium text-ink dark:text-ink-dark-mute mb-2">
              {t("settings.mcpOutbound")}
            </p>
            <p className="text-xs text-ink-mute dark:text-ink-dark-mute mb-3 leading-relaxed">
              {t("settings.mcpOutboundHint")}
            </p>
            <div className="mb-4 rounded-xl border border-border dark:border-border-dark p-3 space-y-2 bg-white/30 dark:bg-black/15">
              <label className={labelCls}>{t("settings.mcpBulkJsonLabel")}</label>
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute">
                {t("settings.mcpBulkJsonHint")}
              </p>
              <textarea
                value={mcpBulkJson}
                onChange={(e) => setMcpBulkJson(e.target.value)}
                rows={8}
                spellCheck={false}
                className={`${inputCls} font-mono text-xs`}
                placeholder={t("settings.mcpBulkJsonPlaceholder")}
              />
              <div className="flex flex-wrap gap-2 justify-end">
                <button
                  type="button"
                  className="text-xs px-3 py-1.5 rounded-lg border border-border dark:border-border-dark hover:bg-black/5 dark:hover:bg-white/10"
                  onClick={() => {
                    const payload: McpServerConfig[] = mcpRows.map(
                      ({ _rowKey: _k, ...rest }) => rest
                    );
                    setMcpBulkJson(JSON.stringify(payload, null, 2));
                  }}
                >
                  {t("settings.mcpExportJson")}
                </button>
                <button
                  type="button"
                  className="text-xs px-3 py-1.5 rounded-lg bg-accent text-white hover:opacity-90"
                  onClick={async () => {
                    try {
                      const parsed: unknown = JSON.parse(mcpBulkJson.trim());
                      if (!Array.isArray(parsed)) {
                        throw new Error(
                          t("settings.mcpJsonExpectArray")
                        );
                      }
                      const rows = parsed.map((item, idx) => mcpRowFromUnknown(item, idx));
                      setMcpRows(rows);
                      setMcpBulkJson(JSON.stringify(rows.map(({ _rowKey: _k, ...r }) => r), null, 2));
                      await message(t("settings.mcpJsonApplied"), { kind: "info" });
                    } catch (e: unknown) {
                      const err = e instanceof Error ? e.message : String(e);
                      await message(t("settings.mcpJsonInvalid", { err }), {
                        title: t("settings.mcpApplyJson"),
                        kind: "error",
                      });
                    }
                  }}
                >
                  {t("settings.mcpApplyJson")}
                </button>
              </div>
            </div>
            <div className="flex justify-end mb-3">
              <button
                type="button"
                className="text-xs px-3 py-1.5 rounded-lg border border-border dark:border-border-dark hover:bg-black/5 dark:hover:bg-white/10"
                onClick={() => setMcpRows((rows) => [...rows, newMcpRowState()])}
              >
                {t("settings.mcpAdd")}
              </button>
            </div>
            <div className="space-y-3">
              {mcpRows.map((row, i) => (
                <div
                  key={row._rowKey}
                  className="rounded-xl border border-border dark:border-border-dark p-3 space-y-2 bg-white/40 dark:bg-black/20"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-medium text-ink dark:text-ink-dark">
                      #{i + 1}
                    </span>
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        role="switch"
                        aria-checked={row.enabled}
                        onClick={() =>
                          setMcpRows((rows) =>
                            rows.map((r, j) =>
                              j === i ? { ...r, enabled: !r.enabled } : r
                            )
                          )
                        }
                        className={`relative inline-flex h-5 w-9 shrink-0 rounded-full border-2 border-transparent transition-colors cursor-pointer ${
                          row.enabled ? "bg-accent" : "bg-gray-300 dark:bg-gray-600"
                        }`}
                      >
                        <span
                          className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow transform transition-transform ${
                            row.enabled ? "translate-x-4" : "translate-x-0"
                          }`}
                        />
                      </button>
                      <button
                        type="button"
                        className="text-xs text-red-600 dark:text-red-400 hover:underline"
                        onClick={() =>
                          setMcpRows((rows) => rows.filter((_, j) => j !== i))
                        }
                      >
                        {t("settings.mcpRemove")}
                      </button>
                    </div>
                  </div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                    <div>
                      <label className={labelCls}>{t("settings.mcpId")}</label>
                      <input
                        type="text"
                        value={row.id}
                        onChange={(e) =>
                          setMcpRows((rows) =>
                            rows.map((r, j) =>
                              j === i ? { ...r, id: e.target.value } : r
                            )
                          )
                        }
                        className={inputCls}
                        placeholder="my-mcp"
                      />
                    </div>
                    <div>
                      <label className={labelCls}>{t("settings.mcpCwd")}</label>
                      <input
                        type="text"
                        value={row.cwd ?? ""}
                        onChange={(e) =>
                          setMcpRows((rows) =>
                            rows.map((r, j) =>
                              j === i
                                ? { ...r, cwd: e.target.value || undefined }
                                : r
                            )
                          )
                        }
                        className={inputCls}
                        placeholder=""
                      />
                    </div>
                  </div>
                  <div>
                    <label className={labelCls}>{t("settings.mcpCommand")}</label>
                    <input
                      type="text"
                      value={row.command}
                      onChange={(e) =>
                        setMcpRows((rows) =>
                          rows.map((r, j) =>
                            j === i ? { ...r, command: e.target.value } : r
                          )
                        )
                      }
                      className={inputCls}
                      placeholder="npx"
                    />
                  </div>
                  <div>
                    <label className={labelCls}>{t("settings.mcpArgs")}</label>
                    <textarea
                      value={row.args.join("\n")}
                      onChange={(e) => {
                        const args = e.target.value
                          .split(/\r?\n/)
                          .map((line) => line.trim())
                          .filter((line) => line.length > 0);
                        setMcpRows((rows) =>
                          rows.map((r, j) => (j === i ? { ...r, args } : r))
                        );
                      }}
                      rows={3}
                      className={`${inputCls} font-mono text-xs`}
                      placeholder="-y&#10;@modelcontextprotocol/server-everything"
                    />
                  </div>
                </div>
              ))}
            </div>
          </div>
          </div>
          )}

          {activeTab === "environment" && <EnvironmentSettingsSection />}

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
            <div className="mt-3">
              <label className={labelCls}>{t("settings.contextSoftLimitChars")}</label>
              <input
                type="number"
                min={0}
                inputMode="numeric"
                value={contextSoftLimitStr}
                onChange={(e) => setContextSoftLimitStr(e.target.value)}
                placeholder={t("settings.defaultPlaceholder250k")}
                className={inputCls}
              />
              <p className="mt-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                {t("settings.contextSoftLimitCharsHint")}
              </p>
            </div>
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

          {activeTab === "uninstall" && (
          <div className="space-y-4">
            <div className="rounded-xl border border-border dark:border-border-dark overflow-hidden bg-gray-50/90 dark:bg-surface-dark/40 shadow-sm">
              <div className="border-l-[3px] border-l-amber-500 dark:border-l-amber-600">
                <div className="px-3.5 pt-3.5 pb-3 border-b border-border/70 dark:border-border-dark/80 bg-white/70 dark:bg-black/20">
                  <h3 className="text-sm font-semibold text-ink dark:text-ink-dark-mute tracking-tight">
                    {t("settings.uninstall.title")}
                  </h3>
                  <div className="mt-2 space-y-1.5 text-xs text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                    <p>{t("settings.uninstall.introPrimary")}</p>
                    <p className="text-[11px] opacity-[0.92]">{t("settings.uninstall.introSecondary")}</p>
                  </div>
                </div>

                {uninstallInfo?.isDevBuild ? (
                  <div className="px-3.5 py-2 border-b border-amber-200/70 dark:border-amber-900/40 bg-amber-50/70 dark:bg-amber-950/25">
                    <p className="text-[11px] text-amber-950 dark:text-amber-100/90 leading-relaxed">
                      {t("settings.uninstall.devNote")}
                    </p>
                  </div>
                ) : null}

                <div className="px-3.5 py-3 space-y-4 bg-white/80 dark:bg-black/25">
                  <div className="space-y-2">
                    <p className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                      {t("settings.uninstall.locateHeading")}
                    </p>
                    <button
                      type="button"
                      disabled={!uninstallInfo}
                      onClick={() => {
                        void (async () => {
                          try {
                            await invoke("assistant_reveal_install_location");
                          } catch (e: unknown) {
                            const err = e instanceof Error ? e.message : String(e);
                            await message(t("settings.uninstall.failed", { err }), {
                              title: t("settings.uninstall.title"),
                              kind: "error",
                            });
                          }
                        })();
                      }}
                      className="group flex w-full items-center gap-3 rounded-xl border-2 border-border dark:border-border-dark bg-white dark:bg-black/20 px-3 py-2.5 text-left shadow-sm outline-none transition-all hover:border-ink/22 dark:hover:border-white/28 hover:shadow-md active:scale-[0.995] focus-visible:ring-2 focus-visible:ring-accent/45 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-paper-dark disabled:pointer-events-none disabled:opacity-40 disabled:shadow-none"
                    >
                      <div className="min-w-0 flex-1">
                        <span className="block text-sm font-semibold text-ink dark:text-ink-dark">
                          {t("settings.uninstall.reveal")}
                        </span>
                        <span className="mt-0.5 block text-[10px] text-ink-mute dark:text-ink-dark-mute leading-snug">
                          {t("settings.uninstall.revealSub")}
                        </span>
                      </div>
                      <svg
                        className="h-4 w-4 shrink-0 text-ink-mute transition-colors group-hover:text-accent dark:text-ink-dark-mute dark:group-hover:text-accent"
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                        aria-hidden
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M9 5l7 7-7 7"
                        />
                      </svg>
                    </button>
                  </div>

                  <div className="space-y-2 rounded-xl border border-dashed border-amber-300/90 bg-amber-50/40 p-2.5 dark:border-amber-800/50 dark:bg-amber-950/20">
                    <p className="px-0.5 text-[10px] font-semibold uppercase tracking-wide text-amber-950/90 dark:text-amber-200/95">
                      {t("settings.uninstall.uninstallHeading")}
                    </p>
                    <div className="flex flex-col gap-2.5">
                      <button
                        type="button"
                        disabled={!uninstallInfo}
                        onClick={() => void runQuitUninstall(false)}
                        className="group flex w-full items-center gap-3 rounded-xl border-2 border-amber-800/25 bg-amber-600 px-3 py-2.5 text-left text-white shadow-md outline-none transition-all hover:border-amber-950/35 hover:bg-amber-700 active:scale-[0.995] focus-visible:ring-2 focus-visible:ring-amber-300 focus-visible:ring-offset-2 dark:border-amber-400/30 dark:bg-amber-600 dark:hover:bg-amber-500 dark:focus-visible:ring-amber-400/80 dark:focus-visible:ring-offset-paper-dark disabled:pointer-events-none disabled:opacity-40 disabled:shadow-none"
                      >
                        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-black/15 dark:bg-black/20">
                          <svg
                            className="h-5 w-5 text-white"
                            fill="none"
                            viewBox="0 0 24 24"
                            stroke="currentColor"
                            strokeWidth={2}
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            aria-hidden
                          >
                            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
                            <polyline points="16 17 21 12 16 7" />
                            <line x1="21" x2="9" y1="12" y2="12" />
                          </svg>
                        </div>
                        <div className="min-w-0 flex-1">
                          <span className="block text-sm font-semibold text-white">
                            {t("settings.uninstall.quitAppOnly")}
                          </span>
                          <span className="mt-0.5 block text-[10px] text-amber-100/95 leading-snug">
                            {t("settings.uninstall.quitAppOnlySub")}
                          </span>
                        </div>
                        <svg
                          className="h-4 w-4 shrink-0 text-white/75 transition-colors group-hover:text-white"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                          aria-hidden
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M9 5l7 7-7 7"
                          />
                        </svg>
                      </button>
                      <button
                        type="button"
                        disabled={!uninstallInfo}
                        onClick={() => void runQuitUninstall(true)}
                        className="group flex w-full items-center gap-3 rounded-xl border-2 border-red-900/20 bg-red-600 px-3 py-2.5 text-left text-white shadow-md outline-none transition-all hover:border-red-950/30 hover:bg-red-700 active:scale-[0.995] focus-visible:ring-2 focus-visible:ring-red-400 focus-visible:ring-offset-2 dark:border-red-400/25 dark:bg-red-700 dark:hover:bg-red-600 dark:focus-visible:ring-red-500/70 dark:focus-visible:ring-offset-paper-dark disabled:pointer-events-none disabled:opacity-40 disabled:shadow-none"
                      >
                        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-black/15 dark:bg-black/25">
                          <svg
                            className="h-5 w-5 text-white"
                            fill="none"
                            viewBox="0 0 24 24"
                            stroke="currentColor"
                            strokeWidth={2}
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            aria-hidden
                          >
                            <path d="M3 6h18" />
                            <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                            <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                            <line x1="10" x2="10" y1="11" y2="17" />
                            <line x1="14" x2="14" y1="11" y2="17" />
                          </svg>
                        </div>
                        <div className="min-w-0 flex-1">
                          <span className="block text-sm font-semibold text-white">
                            {t("settings.uninstall.quitWithData")}
                          </span>
                          <span className="mt-0.5 block text-[10px] text-red-100/95 leading-snug">
                            {t("settings.uninstall.quitWithDataSub")}
                          </span>
                        </div>
                        <svg
                          className="h-4 w-4 shrink-0 text-white/75 transition-colors group-hover:text-white"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                          aria-hidden
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M9 5l7 7-7 7"
                          />
                        </svg>
                      </button>
                    </div>
                  </div>
                </div>

                {uninstallInfo ? (
                  <details
                    open
                    className="group border-t border-border/70 dark:border-border-dark/80 bg-white/40 dark:bg-black/10"
                  >
                    <summary className="px-3.5 py-2.5 cursor-pointer select-none list-none flex items-center justify-between gap-2 text-xs font-medium text-ink dark:text-ink-dark-mute hover:bg-black/[0.03] dark:hover:bg-white/[0.04] [&::-webkit-details-marker]:hidden">
                      <span>{t("settings.uninstall.pathsSummary")}</span>
                      <svg
                        className="w-3.5 h-3.5 shrink-0 text-ink-mute dark:text-ink-dark-mute transition-transform group-open:rotate-180"
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                        aria-hidden
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M19 9l-7 7-7-7"
                        />
                      </svg>
                    </summary>
                    <div className="px-3.5 pb-3.5 space-y-2.5">
                      <div>
                        <div className="text-[10px] font-medium uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                          {t("settings.uninstall.pathAppData")}
                        </div>
                        <div className="mt-1 rounded-md bg-black/[0.05] dark:bg-white/[0.06] px-2.5 py-1.5 text-[11px] font-mono text-ink/90 dark:text-ink-dark-mute/95 break-all leading-snug">
                          {uninstallInfo.tauriAppDataDir}
                        </div>
                      </div>
                      <div>
                        <div className="text-[10px] font-medium uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                          {t("settings.uninstall.pathChat")}
                        </div>
                        <div className="mt-1 rounded-md bg-black/[0.05] dark:bg-white/[0.06] px-2.5 py-1.5 text-[11px] font-mono text-ink/90 dark:text-ink-dark-mute/95 break-all leading-snug">
                          {uninstallInfo.skillliteChatRoot}
                        </div>
                      </div>
                    </div>
                  </details>
                ) : (
                  <div className="px-3.5 py-2.5 border-t border-border/70 dark:border-border-dark/80 bg-white/40 dark:bg-black/10">
                    <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
                      {t("settings.uninstall.loadFailed")}
                    </p>
                  </div>
                )}
              </div>
            </div>
          </div>
          )}
        </div>

        {/* Fixed footer */}
        <div className="flex shrink-0 justify-end gap-2 border-t border-border dark:border-border-dark bg-white dark:bg-paper-dark px-5 py-3">
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
    </div>
  );
}
