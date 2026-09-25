import {
  useState,
  useEffect,
  useCallback,
  useMemo,
  useRef,
  type ChangeEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openImageDialog } from "@tauri-apps/plugin-dialog";
import { useShallow } from "zustand/react/shallow";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useStatusStore } from "../stores/useStatusStore";
import { useSessionStore } from "../stores/useSessionStore";
import { useChatEvents } from "../hooks/useChatEvents";
import { useRecentData } from "../hooks/useRecentData";
import {
  formatProfileShortLabel,
  listProfilesForQuickSwitch,
  matchActiveProfileId,
  removeLlmProfileWithRoutingCleanup,
} from "../utils/llmProfiles";
import { useAssistantChrome } from "../contexts/AssistantChromeContext";
import { MessageList } from "./chat/MessageList";
import { ChatInput } from "./chat/ChatInput";
import { InputPlanStrip } from "./chat/InputPlanStrip";
import type {
  ChatImagePreview,
  ChatMessage,
  TurnLlmUsage,
} from "../types/chat";

const MAX_CHAT_IMAGES = 6;
const MAX_IMAGE_FILE_BYTES = 5 * 1024 * 1024;

type TranscriptEntryDto = {
  id: string;
  role: string;
  content: string;
  tool_call_id?: string;
  name?: string;
  is_error?: boolean;
  ui?: Record<string, unknown> | null;
  images?: ChatImagePreview[];
  llm_usage?: unknown;
};

type PendingImage = {
  id: string;
  media_type: string;
  data_base64: string;
  preview_url: string;
};

/** Handles `data:image/png;base64,...` and `data:image/jpeg;charset=UTF-8;base64,...`. */
function parseDataUrl(dataUrl: string): { media_type: string; data_base64: string } | null {
  const s = dataUrl.trim();
  const marker = ";base64,";
  const idx = s.indexOf(marker);
  if (idx < 0 || !s.startsWith("data:")) return null;
  let media_type = s.slice("data:".length, idx).trim();
  const paramIdx = media_type.indexOf(";");
  if (paramIdx >= 0) {
    media_type = media_type.slice(0, paramIdx).trim();
  }
  const data_base64 = s.slice(idx + marker.length).trim();
  if (!media_type || !data_base64) return null;
  return { media_type, data_base64 };
}

function isTauriWebview(): boolean {
  return (
    typeof window !== "undefined" &&
    Boolean((window as unknown as Record<string, unknown>).__TAURI_INTERNALS__)
  );
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(file);
  });
}

/** Restore `turnLlmUsage` from transcript `message.llm_usage` (Rust / Tauri payload). */
function parseTranscriptLlmUsage(raw: unknown): TurnLlmUsage | undefined {
  if (raw == null || typeof raw !== "object") return undefined;
  const o = raw as Record<string, unknown>;
  const p = typeof o.prompt_tokens === "number" ? o.prompt_tokens : Number(o.prompt_tokens);
  const c =
    typeof o.completion_tokens === "number" ? o.completion_tokens : Number(o.completion_tokens);
  const t = typeof o.total_tokens === "number" ? o.total_tokens : Number(o.total_tokens);
  if (!Number.isFinite(p) || p < 0 || !Number.isFinite(c) || c < 0 || !Number.isFinite(t) || t < 0) {
    return undefined;
  }
  return {
    prompt_tokens: Math.floor(p),
    completion_tokens: Math.floor(c),
    total_tokens: Math.floor(t),
  };
}
import { isChatHiddenToolName } from "../utils/chatNoise";
import { notifyRuntimeStatusMayHaveChanged } from "../utils/runtimeStatusRefresh";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import { useI18n, translate } from "../i18n";
import {
  evolutionNoteForDisplay,
  evolutionStatusHeadline,
  isEvolutionNoMaterialChanges,
} from "../utils/evolutionDisplay";
import { buildAssistantBridgeConfigForScenario } from "../utils/llmScenarioRouting";
import {
  runWithScenarioFallbackNotified,
} from "../utils/llmScenarioFallbackToast";
import {
  type StructuredLlmInvokeResult,
  unwrapStructuredLlmInvokeResult,
} from "../utils/llmScenarioFallback";
import { serializeChatMessagesForFollowup } from "../utils/followupSuggestions";
import { tryParseReadFilePathFromToolArgs } from "../utils/readFileToolMeta";

export default function ChatView() {
  const { t } = useI18n();
  const starterActions = useMemo(
    () => [
      { title: t("chat.starter1"), prompt: t("chat.starter1Prompt") },
      { title: t("chat.starter2"), prompt: t("chat.starter2Prompt") },
      { title: t("chat.starter3"), prompt: t("chat.starter3Prompt") },
    ],
    [t]
  );
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [pendingImages, setPendingImages] = useState<PendingImage[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [isClearing, setIsClearing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [transcriptError, setTranscriptError] = useState<string | null>(null);
  const evolutionPollTimersRef = useRef<Map<string, number>>(new Map());
  const evolutionLastStatusRef = useRef<Map<string, string>>(new Map());
  const { settings, setSettings } = useSettingsStore();
  const { openSettingsToTab } = useAssistantChrome();
  const [modelQuickOpen, setModelQuickOpen] = useState(false);
  const modelQuickRef = useRef<HTMLDivElement>(null);
  const currentSessionKey = useSessionStore((s) => s.currentSessionKey);
  const planTasks = useStatusStore((s) => s.tasks);
  const { refreshRecentData } = useRecentData();
  const statusActions = useStatusStore(
    useShallow((s) => ({
      addTaskPlan: s.addTaskPlan,
      updateTaskProgress: s.updateTaskProgress,
      addLog: s.addLog,
      addMemoryHint: s.addMemoryHint,
      clearPlan: s.clearPlan,
      setLatestOutput: s.setLatestOutput,
      clearAll: s.clearAll,
    }))
  );

  /** 会话切换/删除当前会话前，在清空前由渲染阶段写入，供 effect 里请求「猜你想问」。 */
  const pendingSessionEndRef = useRef<{ transcript: string } | null>(null);
  const followupReqIdRef = useRef(0);
  const messagesRef = useRef<ChatMessage[]>([]);
  messagesRef.current = messages;
  const sendInFlightRef = useRef(false);
  const [followupSuggestions, setFollowupSuggestions] = useState<string[] | null>(
    null
  );
  const [followupPanelDismissed, setFollowupPanelDismissed] = useState(false);

  /** 丢弃尚未返回的「猜你想问」请求，避免晚到的结果盖在新一轮对话上。 */
  const invalidateFollowupInFlight = useCallback(() => {
    followupReqIdRef.current += 1;
  }, []);

  const showFollowupPanel = useMemo(() => {
    if (!followupSuggestions?.length || followupPanelDismissed || loading) {
      return false;
    }
    const last = messages[messages.length - 1];
    return last?.type === "assistant" && !last.streaming;
  }, [followupSuggestions, followupPanelDismissed, loading, messages]);

  useEffect(() => {
    setFollowupPanelDismissed(false);
  }, [followupSuggestions]);

  const requestFollowupSuggestions = useCallback(async (transcript: string) => {
    const text = transcript.trim();
    if (!text) return;
    const id = ++followupReqIdRef.current;
    const s = useSettingsStore.getState().settings;
    try {
      const { result: rows } = await runWithScenarioFallbackNotified<string[]>(
        s,
        "followup",
        (config) =>
          invoke<StructuredLlmInvokeResult<string[]>>("skilllite_followup_suggestions", {
            transcript: text,
            workspace: s.workspace || ".",
            config,
          }).then(unwrapStructuredLlmInvokeResult)
      );
      if (id !== followupReqIdRef.current) return;
      const list = Array.isArray(rows)
        ? rows.map((x) => x.trim()).filter((x) => x.length > 0)
        : [];
      setFollowupSuggestions(list.length > 0 ? list.slice(0, 3) : null);
    } catch {
      if (id !== followupReqIdRef.current) return;
      setFollowupSuggestions(null);
    }
  }, []);

  const onChatTurnComplete = useCallback(() => {
    refreshRecentData();
    notifyRuntimeStatusMayHaveChanged();
    const tr = serializeChatMessagesForFollowup(messagesRef.current);
    if (tr.trim()) {
      void requestFollowupSuggestions(tr);
    }
  }, [refreshRecentData, requestFollowupSuggestions]);

  // Synchronous clear: if session key changed without remount (HMR), force-clear in render
  const [activeKey, setActiveKey] = useState(currentSessionKey);
  if (activeKey !== currentSessionKey) {
    const tr = serializeChatMessagesForFollowup(messages);
    pendingSessionEndRef.current = tr.trim() ? { transcript: tr } : null;
    setActiveKey(currentSessionKey);
    setMessages([]);
    setPendingImages([]);
    setLoading(false);
    setError(null);
    setNotice(null);
    setTranscriptError(null);
  }

  useChatEvents({
    sessionKey: currentSessionKey,
    setMessages,
    setLoading,
    setError,
    addTaskPlan: statusActions.addTaskPlan,
    updateTaskProgress: statusActions.updateTaskProgress,
    addLog: statusActions.addLog,
    addMemoryHint: statusActions.addMemoryHint,
    setLatestOutput: statusActions.setLatestOutput,
    clearPlan: statusActions.clearPlan,
    onTurnComplete: onChatTurnComplete,
  });

  useEffect(() => {
    refreshRecentData();
  }, [refreshRecentData, settings.workspace]);

  useEffect(() => {
    let cancelled = false;

    const run = async () => {
      setMessages([]);
      setLoading(false);
      setError(null);
      setNotice(null);
      setTranscriptError(null);
      statusActions.clearAll();

      try {
        await invoke("skilllite_stop");
      } catch (e) {
        if (!cancelled) {
          const msg = formatInvokeError(e);
          useUiToastStore
            .getState()
            .show(translate("toast.stopPrevFailed", { err: msg }), "error");
        }
      }

      if (cancelled) return;

      try {
        const entries = await invoke<TranscriptEntryDto[]>("skilllite_load_transcript", {
          sessionKey: currentSessionKey,
        });
        if (cancelled) return;
        if (!entries || entries.length === 0) return;
        const msgs: ChatMessage[] = [];
        const readFilePathByToolCallId = new Map<string, string>();
        for (const e of entries) {
          if (e.role === "skilllite_ui" && e.ui && typeof e.ui === "object") {
            const u = e.ui as Record<string, unknown>;
            const kind = u.kind;
            if (kind === "confirmation") {
              const rt = u.risk_tier;
              const riskTier: "low" | "confirm_required" | undefined =
                rt === "low" || rt === "confirm_required" ? rt : undefined;
              msgs.push({
                id: e.id,
                type: "confirmation",
                prompt: String(u.prompt ?? ""),
                ...(riskTier != null ? { riskTier } : {}),
                resolved: Boolean(u.resolved ?? true),
                approved: u.approved === true,
              });
              continue;
            }
            if (kind === "clarification") {
              const raw = u.suggestions;
              const suggestions = Array.isArray(raw)
                ? raw.map((x) => String(x))
                : [];
              const action = String(u.action ?? "stop");
              const hint =
                u.hint === null || u.hint === undefined
                  ? ""
                  : String(u.hint);
              msgs.push({
                id: e.id,
                type: "clarification",
                reason: String(u.reason ?? ""),
                message: String(u.message ?? ""),
                suggestions,
                resolved: Boolean(u.resolved ?? true),
                selectedOption:
                  action === "stop"
                    ? "stop"
                    : hint.length > 0
                      ? hint
                      : "已继续",
              });
              continue;
            }
            continue;
          }
          if (e.role === "tool_call") {
            const name = e.name ?? "";
            const toolCallId = e.tool_call_id?.trim();
            if (name.replace(/-/g, "_") === "read_file" && toolCallId) {
              const path = tryParseReadFilePathFromToolArgs(e.content);
              if (path) readFilePathByToolCallId.set(toolCallId, path);
            }
            if (isChatHiddenToolName(name)) continue;
            msgs.push({
              id: e.id,
              type: "tool_call" as const,
              name,
              args: e.content,
              ...(toolCallId ? { toolCallId } : {}),
            });
            continue;
          }
          if (e.role === "tool_result") {
            const name = e.name ?? "";
            const toolCallId = e.tool_call_id?.trim();
            const sourcePath =
              name.replace(/-/g, "_") === "read_file" && toolCallId
                ? readFilePathByToolCallId.get(toolCallId)
                : undefined;
            if (isChatHiddenToolName(name)) continue;
            msgs.push({
              id: e.id,
              type: "tool_result" as const,
              name,
              result: e.content,
              isError: e.is_error ?? false,
              ...(toolCallId ? { toolCallId } : {}),
              sourcePath,
            });
            continue;
          }
          const role = e.role === "user" ? "user" : "assistant";
          const uiImages = e.images;
          if (role === "user" && uiImages && uiImages.length > 0) {
            msgs.push({
              id: e.id,
              type: "user",
              content: e.content,
              images: uiImages,
            });
          } else {
            const turnLlm =
              role === "assistant" ? parseTranscriptLlmUsage(e.llm_usage) : undefined;
            msgs.push({
              id: e.id,
              type: role,
              content: e.content,
              ...(turnLlm != null ? { turnLlmUsage: turnLlm } : {}),
            });
          }
        }
        setMessages(msgs);
      } catch (err) {
        if (cancelled) return;
        const msg = formatInvokeError(err);
        setTranscriptError(msg);
        console.error("[skilllite-assistant] skilllite_load_transcript failed:", err);
      }
    };

    void run();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentSessionKey]);

  useEffect(() => {
    invalidateFollowupInFlight();
    setFollowupSuggestions(null);
    const pending = pendingSessionEndRef.current;
    pendingSessionEndRef.current = null;
    if (!pending?.transcript.trim()) return;
    void requestFollowupSuggestions(pending.transcript);
  }, [currentSessionKey, invalidateFollowupInFlight, requestFollowupSuggestions]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 1500);
    return () => window.clearTimeout(timer);
  }, [notice]);

  useEffect(() => {
    return () => {
      for (const timerId of evolutionPollTimersRef.current.values()) {
        window.clearTimeout(timerId);
      }
      evolutionPollTimersRef.current.clear();
      evolutionLastStatusRef.current.clear();
    };
  }, []);

  const stopEvolutionPoll = useCallback((messageId: string) => {
    const timerId = evolutionPollTimersRef.current.get(messageId);
    if (timerId != null) {
      window.clearTimeout(timerId);
      evolutionPollTimersRef.current.delete(messageId);
    }
  }, []);

  const startEvolutionPoll = useCallback(
    (messageId: string, proposalId: string) => {
      stopEvolutionPoll(messageId);
      let attempts = 0;
      const maxAttempts = 24; // ~2 minutes (5s interval)
      const pollOnce = async () => {
        attempts += 1;
        try {
          const status = await invoke<{
            proposal_id: string;
            status: string;
            acceptance_status: string;
            updated_at: string;
            note: string | null;
          }>("skilllite_get_evolution_proposal_status", {
            workspace: settings.workspace || ".",
            proposalId,
          });
          const doneStates = new Set([
            "executed",
            "policy_denied",
            "blocked",
            "failed",
            "archived",
          ]);
          const progressDone = doneStates.has(status.status);
          const statusKey = `${status.status}/${status.acceptance_status}`;
          const lastKey = evolutionLastStatusRef.current.get(proposalId);
          setMessages((prev) =>
            [
              ...prev.map((m) =>
                m.type === "evolution_options" && m.id === messageId
                  ? {
                      ...m,
                      proposalId,
                      progressStatus: statusKey,
                      progressUpdatedAt: status.updated_at,
                      progressNote: status.note ?? undefined,
                      progressDone,
                    }
                  : m
              ),
              ...((lastKey !== statusKey || progressDone)
                ? (() => {
                    const headline = evolutionStatusHeadline(
                      status.status,
                      status.acceptance_status,
                      status.note
                    );
                    const extra = evolutionNoteForDisplay(
                      status.status,
                      status.acceptance_status,
                      status.note
                    );
                    const isNoMat = isEvolutionNoMaterialChanges(
                      status.status,
                      status.acceptance_status,
                      status.note
                    );
                    let content =
                      `进化进度更新：提案 ${proposalId}\n` +
                      `- 进度：${headline}\n` +
                      `- 更新时间：${status.updated_at}`;
                    if (extra) {
                      content += isNoMat
                        ? `\n- 说明：${extra}`
                        : `\n- 备注：${extra}`;
                    }
                    content += `\n\n${translate("chat.evolutionProgressFooterHint")}`;
                    return [
                      {
                        id: crypto.randomUUID(),
                        type: "assistant" as const,
                        content,
                      },
                    ];
                  })()
                : []),
            ]
          );
          evolutionLastStatusRef.current.set(proposalId, statusKey);
          if (progressDone || attempts >= maxAttempts) {
            stopEvolutionPoll(messageId);
            return;
          }
        } catch {
          if (attempts >= maxAttempts) {
            stopEvolutionPoll(messageId);
            return;
          }
        }
        const timerId = window.setTimeout(pollOnce, 5000);
        evolutionPollTimersRef.current.set(messageId, timerId);
      };
      void pollOnce();
    },
    [settings.workspace, stopEvolutionPoll]
  );

  const handleConfirm = useCallback(async (id: string, approved: boolean) => {
    try {
      await invoke("skilllite_confirm", { approved });
      setMessages((prev) =>
        prev.map((m) =>
          m.type === "confirmation" && m.id === id
            ? { ...m, resolved: true, approved }
            : m
        )
      );
    } catch (e) {
      const msg = formatInvokeError(e);
      useUiToastStore
        .getState()
        .show(t("toast.confirmFailed", { err: msg }), "error");
    }
  }, [t]);

  const autoApproveInFlightRef = useRef<string | null>(null);
  useEffect(() => {
    if (!settings.autoApproveToolConfirmations) return;
    const pending = messages.find(
      (m): m is Extract<ChatMessage, { type: "confirmation" }> =>
        m.type === "confirmation" &&
        !m.resolved &&
        m.riskTier === "low"
    );
    if (!pending) return;
    if (autoApproveInFlightRef.current === pending.id) return;
    autoApproveInFlightRef.current = pending.id;
    void handleConfirm(pending.id, true).finally(() => {
      if (autoApproveInFlightRef.current === pending.id) {
        autoApproveInFlightRef.current = null;
      }
    });
  }, [messages, settings.autoApproveToolConfirmations, handleConfirm]);

  const handleClarify = async (id: string, action: string, hint?: string) => {
    try {
      const hintParam =
        hint != null && hint.trim().length > 0 ? hint : null;
      await invoke("skilllite_clarify", { action, hint: hintParam });
      const selectedLabel =
        action === "stop"
          ? "stop"
          : hintParam != null
            ? hintParam
            : t("chat.clarifyContinueNoHint");
      setMessages((prev) =>
        prev.map((m) =>
          m.type === "clarification" && m.id === id
            ? { ...m, resolved: true, selectedOption: selectedLabel }
            : m
        )
      );
      if (action === "continue") {
        setLoading(true);
      }
    } catch (e) {
      const msg = formatInvokeError(e);
      useUiToastStore
        .getState()
        .show(t("toast.clarifyFailed", { err: msg }), "error");
    }
  };

  const handleEvolutionAction = async (id: string, option: string) => {
    try {
      const target = messages.find(
        (m) => m.type === "evolution_options" && m.id === id
      );
      let selectedOption = option;
      let progressNotice: string | null = null;
      let queuedProposalId: string | null = null;
      if (option === "【启动进化】" && target?.type === "evolution_options") {
        const proposalId = await invoke<string>("skilllite_authorize_capability_evolution", {
          workspace: settings.workspace || ".",
          toolName: target.toolName,
          outcome: target.outcome,
          summary: target.message,
        });
        queuedProposalId = proposalId;
        selectedOption = `【启动进化】（已入队: ${proposalId}）`;
        progressNotice =
          `已启动进化，提案 ${proposalId} 已加入队列（queued）。` +
          "后续由后台进化调度执行；聊天会自动推送进度更新，也可在右侧「自进化 > 详情与审核」查看。";
        try {
          const { result: s } = await runWithScenarioFallbackNotified<{
            unprocessed_decisions: number;
            pending_skill_count: number;
            last_run_ts: string | null;
          }>(settings, "evolution", (config) =>
            invoke<
              StructuredLlmInvokeResult<{
                unprocessed_decisions: number;
                pending_skill_count: number;
                last_run_ts: string | null;
              }>
            >("skilllite_load_evolution_status", {
              workspace: settings.workspace || ".",
              config,
            }).then(unwrapStructuredLlmInvokeResult)
          );
          progressNotice += ` 当前未进化决策: ${s.unprocessed_decisions}，待确认技能: ${s.pending_skill_count}` +
            (s.last_run_ts ? `，上次进化运行: ${s.last_run_ts}` : "");
        } catch {
          // Ignore status fetch failures; queueing already succeeded.
        }
      }
      setMessages((prev) =>
        [
          ...prev.map((m) =>
          m.type === "evolution_options" && m.id === id
            ? {
                ...m,
                resolved: true,
                selectedOption,
                proposalId: queuedProposalId ?? m.proposalId,
                progressStatus: queuedProposalId ? "queued / pending" : m.progressStatus,
                progressUpdatedAt: queuedProposalId ? new Date().toISOString() : m.progressUpdatedAt,
                progressDone: false,
              }
            : m
          ),
          ...(progressNotice
            ? [{ id: crypto.randomUUID(), type: "assistant" as const, content: progressNotice }]
            : []),
        ]
      );
      if (queuedProposalId) {
        evolutionLastStatusRef.current.set(queuedProposalId, "queued/pending");
        startEvolutionPoll(id, queuedProposalId);
      }
    } catch (e) {
      const msg = formatInvokeError(e);
      useUiToastStore
        .getState()
        .show(t("toast.confirmFailed", { err: msg }), "error");
    }
  };

  const handleClear = useCallback(async () => {
    if (loading || isClearing) return;
    const snap = serializeChatMessagesForFollowup(messages);
    setIsClearing(true);
    setNotice(t("chat.clearingNotice"));
    try {
      await invoke("skilllite_clear_transcript", {
        sessionKey: currentSessionKey,
        workspace: settings.workspace || ".",
      });
      setMessages([]);
      setPendingImages([]);
      setError(null);
      statusActions.clearAll();
      refreshRecentData();
      setNotice(t("chat.clearedNotice"));
      if (snap.trim()) {
        void requestFollowupSuggestions(snap);
      } else {
        invalidateFollowupInFlight();
        setFollowupSuggestions(null);
      }
    } catch (err) {
      console.error("[skilllite-assistant] skilllite_clear_transcript failed:", err);
      setError(err instanceof Error ? err.message : String(err));
      setNotice(null);
    } finally {
      setIsClearing(false);
    }
  }, [
    loading,
    isClearing,
    messages,
    settings.workspace,
    currentSessionKey,
    statusActions,
    refreshRecentData,
    t,
    requestFollowupSuggestions,
    invalidateFollowupInFlight,
  ]);

  const handleStop = useCallback(async () => {
    try {
      await invoke("skilllite_stop");
      setLoading(false);
      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (last?.type === "assistant" && last?.streaming) {
          const abortMark = t("chat.aborted");
          const content = last.content
            ? `${last.content}\n\n${abortMark}`
            : abortMark;
          statusActions.setLatestOutput(content);
          return [...prev.slice(0, -1), { ...last, content, streaming: false }];
        }
        return prev;
      });
      statusActions.clearPlan();
      refreshRecentData();
    } catch (e) {
      setLoading(false);
      const msg = formatInvokeError(e);
      setError(`${translate("toast.stopFailed", { err: msg })}`);
      useUiToastStore
        .getState()
        .show(translate("toast.stopFailed", { err: msg }), "error");
    }
  }, [refreshRecentData, statusActions, t]);

  const sendMessage = useCallback(
    async (rawText: string, attachments: PendingImage[]) => {
      const text = rawText.trim();
      const hasImages = attachments.length > 0;
      if ((!text && !hasImages) || loading || isClearing) return;
      if (sendInFlightRef.current) return;
      sendInFlightRef.current = true;
      invalidateFollowupInFlight();
      setFollowupSuggestions(null);

      try {
        if (text === "/new" || text === "/reset") {
          setInput("");
          setSettings({ showStarterPrompts: false });
          await handleClear();
          return;
        }

        setInput("");
        setPendingImages([]);
        setError(null);
        setSettings({ showStarterPrompts: false });
        statusActions.clearPlan();
        statusActions.setLatestOutput("");
        const previewImages: ChatImagePreview[] = attachments.map((p) => ({
          media_type: p.media_type,
          preview_url: p.preview_url,
        }));
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            type: "user",
            content: text,
            ...(previewImages.length > 0 ? { images: previewImages } : {}),
          },
        ]);
        setLoading(true);

        const config = buildAssistantBridgeConfigForScenario(settings, "agent");
        const rpcImages =
          attachments.length > 0
            ? attachments.map(({ media_type, data_base64 }) => ({
                media_type,
                data_base64,
              }))
            : undefined;

        try {
          const payload: Record<string, unknown> = {
            message: text,
            workspace: settings.workspace || ".",
            sessionKey: currentSessionKey,
            config,
          };
          if (rpcImages && rpcImages.length > 0) {
            payload.images = rpcImages;
          }
          await invoke("skilllite_chat_stream", payload);
        } catch (e) {
          const errMsg = e instanceof Error ? e.message : String(e);
          setError(errMsg);
          setMessages((prev) => [
            ...prev,
            {
              id: crypto.randomUUID(),
              type: "assistant",
              content: t("chat.requestFailed", { msg: errMsg }),
            },
          ]);
        } finally {
          setLoading(false);
        }
      } finally {
        sendInFlightRef.current = false;
      }
    },
    [
      handleClear,
      isClearing,
      loading,
      setSettings,
      settings.apiBase,
      settings.apiKey,
      settings.model,
      settings.workspace,
      settings.sandboxLevel,
      settings.swarmEnabled,
      settings.swarmUrl,
      settings.maxIterations,
      settings.maxToolCallsPerTask,
      settings.locale,
      settings.llmScenarioRoutingEnabled,
      settings.llmScenarioRoutes,
      currentSessionKey,
      statusActions,
      t,
      invalidateFollowupInFlight,
    ]
  );

  const appendPendingImages = useCallback(
    (additions: PendingImage[]) => {
      if (additions.length === 0) return;
      setPendingImages((prev) => {
        const merged = [...prev, ...additions];
        if (merged.length > MAX_CHAT_IMAGES) {
          useUiToastStore
            .getState()
            .show(t("chat.attachMaxImages", { max: MAX_CHAT_IMAGES }), "error");
        }
        return merged.slice(0, MAX_CHAT_IMAGES);
      });
    },
    [t]
  );

  /** Tauri: native dialog + Rust read (WKWebView often blocks programmatic `<input type="file">`). */
  const openImagePicker = useCallback(async () => {
    if (loading || isClearing) return;
    if (isTauriWebview()) {
      try {
        const selected = await openImageDialog({
          multiple: true,
          filters: [
            {
              name: "Image",
              extensions: ["png", "jpg", "jpeg", "webp", "gif"],
            },
          ],
        });
        if (selected == null) return;
        const paths = Array.isArray(selected) ? selected : [selected];
        const additions: PendingImage[] = [];
        for (const path of paths) {
          try {
            const img = await invoke<{ media_type: string; data_base64: string }>(
              "skilllite_read_local_image_b64",
              { path }
            );
            const preview_url = `data:${img.media_type};base64,${img.data_base64}`;
            additions.push({
              id: crypto.randomUUID(),
              media_type: img.media_type,
              data_base64: img.data_base64,
              preview_url,
            });
          } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            useUiToastStore
              .getState()
              .show(t("chat.attachLoadErr", { msg }), "error");
          }
        }
        appendPendingImages(additions);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        useUiToastStore
          .getState()
          .show(t("chat.attachPickerErr", { msg }), "error");
        fileInputRef.current?.click();
      }
      return;
    }
    fileInputRef.current?.click();
  }, [appendPendingImages, isClearing, loading, t]);

  const handleImagePick = useCallback(
    async (e: ChangeEvent<HTMLInputElement>) => {
      const list = e.target.files;
      e.target.value = "";
      if (!list?.length) return;
      const additions: PendingImage[] = [];
      for (let i = 0; i < list.length; i++) {
        const file = list[i];
        if (!file.type.startsWith("image/")) {
          useUiToastStore.getState().show(t("chat.attachNotImage"), "error");
          continue;
        }
        if (file.size > MAX_IMAGE_FILE_BYTES) {
          useUiToastStore.getState().show(t("chat.attachTooLarge"), "error");
          continue;
        }
        try {
          const dataUrl = await readFileAsDataUrl(file);
          const parsed = parseDataUrl(dataUrl);
          if (!parsed) {
            continue;
          }
          additions.push({
            id: crypto.randomUUID(),
            media_type: parsed.media_type,
            data_base64: parsed.data_base64,
            preview_url: dataUrl,
          });
        } catch {
          useUiToastStore.getState().show(t("chat.attachReadFailed"), "error");
        }
      }
      if (additions.length === 0 && list.length > 0) {
        useUiToastStore.getState().show(t("chat.attachDecodeFailed"), "error");
      }
      appendPendingImages(additions);
    },
    [appendPendingImages, t]
  );

  const handleSend = async () => {
    await sendMessage(input, pendingImages);
  };

  const showStarterPrompts =
    settings.showStarterPrompts === true && messages.length === 0 && !loading && !isClearing;

  const quickSwitchProfiles = useMemo(
    () => listProfilesForQuickSwitch(settings.llmProfiles),
    [settings.llmProfiles]
  );
  const activeProfileId = useMemo(
    () =>
      matchActiveProfileId(settings.llmProfiles, {
        provider: settings.provider,
        model: settings.model,
        apiBase: settings.apiBase,
        apiKey: settings.apiKey,
      }),
    [
      settings.llmProfiles,
      settings.provider,
      settings.model,
      settings.apiBase,
      settings.apiKey,
    ]
  );

  const modelQuickTriggerLabel = useMemo(() => {
    if (activeProfileId) {
      const p = settings.llmProfiles?.find((x) => x.id === activeProfileId);
      if (p) return formatProfileShortLabel(p);
    }
    const m = settings.model?.trim();
    if (m) return m;
    return t("chat.modelQuickSwitch");
  }, [activeProfileId, settings.llmProfiles, settings.model, t]);

  useEffect(() => {
    if (!modelQuickOpen) return;
    const onDown = (e: MouseEvent) => {
      if (modelQuickRef.current && !modelQuickRef.current.contains(e.target as Node)) {
        setModelQuickOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [modelQuickOpen]);

  const chatInputAttachmentSlot = (
    <div className="flex flex-wrap items-end gap-2">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/png,image/jpeg,image/jpg,image/webp,image/gif"
        multiple
        className="hidden"
        onChange={(e) => void handleImagePick(e)}
      />
      <button
        type="button"
        disabled={loading || isClearing}
        onClick={() => void openImagePicker()}
        className="text-xs px-2.5 py-1 rounded-lg border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-ink/[0.04] dark:hover:bg-white/[0.06] disabled:opacity-50"
      >
        {t("chat.attachImage")}
      </button>
      {pendingImages.map((p) => (
        <div key={p.id} className="relative inline-block">
          <img
            src={p.preview_url}
            alt=""
            className="h-14 w-14 object-cover rounded-lg border border-border dark:border-border-dark"
          />
          <button
            type="button"
            aria-label={t("chat.removeAttachment")}
            disabled={loading || isClearing}
            onClick={() =>
              setPendingImages((prev) => prev.filter((x) => x.id !== p.id))
            }
            className="absolute -right-1.5 -top-1.5 h-5 w-5 rounded-full bg-red-500 text-white text-xs leading-5 opacity-90 hover:opacity-100 disabled:opacity-40"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );

  const chipBtnCls =
    "text-xs px-2.5 py-1 rounded-lg border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-ink/[0.04] dark:hover:bg-white/[0.06] transition-colors disabled:opacity-50";

  const chatInputFooter = (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between w-full min-w-0">
      <div ref={modelQuickRef} className="relative flex min-w-0 shrink max-w-full sm:max-w-[18rem]">
        <button
          type="button"
          disabled={loading || isClearing}
          onClick={() => setModelQuickOpen((o) => !o)}
          className={`${chipBtnCls} flex w-full min-w-0 items-center justify-between gap-1.5 text-left text-ink dark:text-ink-dark`}
          aria-expanded={modelQuickOpen}
          aria-haspopup="listbox"
          aria-label={t("chat.modelQuickSwitch")}
          title={t("chat.modelQuickSwitch")}
        >
          <span className="truncate font-medium">{modelQuickTriggerLabel}</span>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            className={`shrink-0 opacity-60 transition-transform ${modelQuickOpen ? "rotate-180" : ""}`}
            aria-hidden
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </button>
        {modelQuickOpen && (
          <div
            className="absolute bottom-full left-0 z-40 mb-1 w-full min-w-[11rem] max-w-[min(100vw-2rem,20rem)] rounded-lg border border-border dark:border-border-dark bg-white dark:bg-paper-dark py-1 shadow-lg"
            role="listbox"
            aria-label={t("chat.modelQuickSwitch")}
          >
            {quickSwitchProfiles.length > 0 ? (
              <>
                <p className="px-2.5 py-1 text-[10px] font-medium uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                  {t("chat.modelQuickSwitchSectionSaved")}
                </p>
                {quickSwitchProfiles.map((p) => (
                  <div
                    key={p.id}
                    className="flex w-full items-stretch gap-0.5 border-b border-border/60 dark:border-border-dark/60 last:border-b-0"
                  >
                    <button
                      type="button"
                      role="option"
                      className="flex min-w-0 flex-1 items-center px-2.5 py-1.5 text-left text-xs text-ink dark:text-ink-dark hover:bg-ink/[0.04] dark:hover:bg-white/[0.06]"
                      onClick={() => {
                        setSettings({
                          provider: p.provider,
                          model: p.model,
                          apiBase: p.apiBase,
                          apiKey: p.apiKey,
                        });
                        setModelQuickOpen(false);
                      }}
                    >
                      <span className="truncate">{formatProfileShortLabel(p)}</span>
                    </button>
                    <button
                      type="button"
                      aria-label={t("chat.modelQuickSwitchRemoveSaved")}
                      className="shrink-0 px-2 py-1.5 text-sm leading-none text-ink-mute hover:text-red-600 dark:text-ink-dark-mute dark:hover:text-red-400"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        const next = removeLlmProfileWithRoutingCleanup(
                          settings.llmProfiles,
                          p.id,
                          {
                            provider: settings.provider,
                            model: settings.model,
                            apiBase: settings.apiBase,
                            apiKey: settings.apiKey,
                          },
                          {
                            llmScenarioRoutes: settings.llmScenarioRoutes,
                            llmScenarioFallbacks: settings.llmScenarioFallbacks,
                          }
                        );
                        const { removedPrimaryRefs, removedFallbackRefs, ...patch } = next;
                        setSettings(patch);
                        const removed = removedPrimaryRefs + removedFallbackRefs;
                        if (removed > 0) {
                          useUiToastStore.getState().show(
                            t("toast.llmScenarioRefsCleaned", {
                              n: removed,
                              primary: removedPrimaryRefs,
                              fallback: removedFallbackRefs,
                            }),
                            "info"
                          );
                        }
                      }}
                    >
                      ×
                    </button>
                  </div>
                ))}
                <div className="my-1 h-px bg-border dark:bg-border-dark" />
              </>
            ) : null}
            <button
              type="button"
              role="option"
              className="flex w-full items-center px-2.5 py-1.5 text-left text-xs font-medium text-accent hover:bg-accent/5 dark:hover:bg-accent/10"
              onClick={() => {
                setModelQuickOpen(false);
                openSettingsToTab("llm");
              }}
            >
              {t("chat.modelQuickSwitchAddNew")}
            </button>
          </div>
        )}
      </div>
      <label className="flex items-center gap-2 text-xs text-ink-mute dark:text-ink-dark-mute cursor-pointer select-none sm:shrink-0 sm:ml-auto">
        <input
          type="checkbox"
          className="rounded border-border dark:border-border-dark text-accent focus:ring-accent/30"
          checked={settings.autoApproveToolConfirmations === true}
          onChange={(e) =>
            setSettings({ autoApproveToolConfirmations: e.target.checked })
          }
        />
        <span title={t("chat.autoApproveToolConfirmationsHint")}>
          {t("chat.autoApproveToolConfirmations")}
        </span>
      </label>
    </div>
  );

  const chatInputProps = {
    value: input,
    onChange: setInput,
    onSend: handleSend,
    onStop: handleStop,
    disabled: loading || isClearing,
    loading,
    attachmentSlot: chatInputAttachmentSlot,
    allowEmptySend: pendingImages.length > 0,
    footer: chatInputFooter,
    placeholder: t("chat.inputPlaceholder"),
  };

  return (
    <div className="flex flex-col h-full bg-surface dark:bg-surface-dark">
      <div className="flex justify-end items-center gap-2 py-1.5 px-3 border-b border-border dark:border-border-dark shrink-0">
        {loading && (
          <button
            type="button"
            onClick={handleStop}
            className="text-xs text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 px-2 py-1 rounded hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors font-medium"
            aria-label={t("chat.stop")}
            title={t("chat.stopTask")}
          >
            {t("chat.stop")}
          </button>
        )}
        <button
          type="button"
          onClick={handleClear}
          disabled={loading || isClearing}
          className={`text-xs px-2 py-1 rounded transition-colors inline-flex items-center gap-1.5 ${
            isClearing
              ? "text-accent bg-accent/10 dark:bg-accent/20"
              : "text-ink-mute dark:text-ink-dark-mute hover:text-accent dark:hover:text-accent hover:bg-ink/5 dark:hover:bg-white/5"
          } disabled:opacity-50`}
          aria-label={t("chat.clear")}
          title={isClearing ? t("chat.clearingNotice") : t("chat.clear")}
        >
          {isClearing && (
            <svg
              className="w-3 h-3 animate-spin"
              viewBox="0 0 24 24"
              fill="none"
              aria-hidden="true"
            >
              <circle
                cx="12"
                cy="12"
                r="9"
                className="opacity-25"
                stroke="currentColor"
                strokeWidth="3"
              />
              <path
                d="M21 12a9 9 0 0 0-9-9"
                className="opacity-100"
                stroke="currentColor"
                strokeWidth="3"
                strokeLinecap="round"
              />
            </svg>
          )}
          {isClearing ? t("chat.clearing") : t("chat.clear")}
        </button>
      </div>
      {transcriptError && (
        <div className="mx-3 mt-2 px-3 py-2 rounded-md border border-amber-200 dark:border-amber-800/50 bg-amber-50 dark:bg-amber-900/20 text-amber-900 dark:text-amber-100 text-xs">
          <span className="font-medium">{t("chat.transcriptErrorTitle")}</span>
          <span className="block mt-1 break-words opacity-90">{transcriptError}</span>
        </div>
      )}
      {isClearing && (
        <div className="mx-3 mt-2 px-3 py-2 rounded-md border border-accent/30 bg-accent/10 dark:bg-accent/20 text-accent text-xs animate-pulse">
          {t("chat.clearingHint")}
        </div>
      )}
      {showStarterPrompts && (
        <div className="mx-3 mt-3 rounded-xl border border-border dark:border-border-dark bg-white dark:bg-paper-dark p-3 shadow-sm">
          <div className="flex items-start justify-between gap-3 mb-3">
            <div>
              <p className="text-sm font-medium text-ink dark:text-ink-dark">
                {t("chat.starterTitle")}
              </p>
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute mt-1">
                {t("chat.starterDesc")}
              </p>
            </div>
            <button
              type="button"
              onClick={() => setSettings({ showStarterPrompts: false })}
              className="text-xs text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark"
            >
              {t("chat.starterHide")}
            </button>
          </div>
          <div className="grid gap-2">
            {starterActions.map((action) => (
              <button
                key={action.title}
                type="button"
                onClick={() => void sendMessage(action.prompt, [])}
                className="w-full text-left rounded-lg border border-border dark:border-border-dark px-3 py-2 text-sm text-ink dark:text-ink-dark hover:border-accent/40 hover:bg-accent/5 dark:hover:bg-accent/10 transition-colors"
              >
                <span className="font-medium block">{action.title}</span>
                <span className="text-xs text-ink-mute dark:text-ink-dark-mute">
                  {action.prompt}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}
      <MessageList
        messages={messages}
        loading={loading}
        workspace={settings.workspace || "."}
        onConfirm={handleConfirm}
        onClarify={handleClarify}
        onEvolutionAction={handleEvolutionAction}
        tailSlot={
          showFollowupPanel && followupSuggestions ? (
            <div className="mt-2 rounded-lg border border-border dark:border-border-dark bg-white dark:bg-paper-dark px-2 py-1.5">
              <div className="flex items-start justify-between gap-1.5">
                <p className="text-[11px] font-medium leading-tight text-ink-mute dark:text-ink-dark-mute">
                  {t("chat.followupTitle")}
                </p>
                <button
                  type="button"
                  onClick={() => setFollowupPanelDismissed(true)}
                  className="shrink-0 -mt-0.5 -mr-0.5 rounded p-0.5 text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/10 hover:text-ink dark:hover:text-ink-dark"
                  aria-label={t("chat.followupClose")}
                  title={t("chat.followupClose")}
                >
                  <svg
                    className="w-3.5 h-3.5"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    aria-hidden
                  >
                    <path d="M18 6L6 18M6 6l12 12" />
                  </svg>
                </button>
              </div>
              <div className="mt-1.5 flex flex-col gap-1">
                {followupSuggestions.map((q, i) => (
                  <button
                    key={`${i}-${q.slice(0, 48)}`}
                    type="button"
                    onClick={() => void sendMessage(q, [])}
                    className="w-full text-left rounded-md border border-border dark:border-border-dark px-2 py-1.5 text-xs leading-snug text-ink dark:text-ink-dark hover:border-accent/40 hover:bg-accent/5 dark:hover:bg-accent/10 transition-colors"
                  >
                    {q}
                  </button>
                ))}
              </div>
            </div>
          ) : null
        }
        tailScrollSignal={
          showFollowupPanel && followupSuggestions
            ? followupSuggestions.join("\u0001")
            : ""
        }
      />

      {error && (
        <div className="px-4 py-2.5 bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 text-sm border-t border-red-100 dark:border-red-900/40">
          {error}
        </div>
      )}
      {!error && notice && (
        <div className="px-4 py-2.5 bg-ink/5 dark:bg-white/5 text-ink-mute dark:text-ink-dark-mute text-sm border-t border-border dark:border-border-dark">
          {notice}
        </div>
      )}

      {planTasks.length > 0 ? (
        <div className="shrink-0 flex flex-col gap-0 border-t border-border dark:border-border-dark bg-white dark:bg-paper-dark pb-4 pt-0">
          <InputPlanStrip tasks={planTasks} className="w-full min-w-0 shrink-0 m-0" />
          <div className="px-4">
            <ChatInput {...chatInputProps} bare />
          </div>
        </div>
      ) : (
        <ChatInput {...chatInputProps} />
      )}
    </div>
  );
}
