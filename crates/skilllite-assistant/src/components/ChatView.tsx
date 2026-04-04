import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useShallow } from "zustand/react/shallow";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useStatusStore } from "../stores/useStatusStore";
import { useSessionStore } from "../stores/useSessionStore";
import { useChatEvents } from "../hooks/useChatEvents";
import { useRecentData } from "../hooks/useRecentData";
import { MessageList } from "./chat/MessageList";
import { ChatInput } from "./chat/ChatInput";
import { InputPlanStrip } from "./chat/InputPlanStrip";
import type { ChatMessage } from "../types/chat";
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
import { buildAssistantBridgeConfig } from "../utils/buildAssistantBridgeConfig";
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
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [isClearing, setIsClearing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [transcriptError, setTranscriptError] = useState<string | null>(null);
  const evolutionPollTimersRef = useRef<Map<string, number>>(new Map());
  const evolutionLastStatusRef = useRef<Map<string, string>>(new Map());
  const { settings, setSettings } = useSettingsStore();
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

  // Synchronous clear: if session key changed without remount (HMR), force-clear in render
  const [activeKey, setActiveKey] = useState(currentSessionKey);
  if (activeKey !== currentSessionKey) {
    setActiveKey(currentSessionKey);
    setMessages([]);
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
    onTurnComplete: () => {
      refreshRecentData();
      notifyRuntimeStatusMayHaveChanged();
    },
  });

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
        const entries = await invoke<
          Array<{
            id: string;
            role: string;
            content: string;
            name?: string;
            is_error?: boolean;
            ui?: Record<string, unknown> | null;
          }>
        >("skilllite_load_transcript", {
          sessionKey: currentSessionKey,
        });
        if (cancelled) return;
        if (!entries || entries.length === 0) return;
        const msgs: ChatMessage[] = [];
        let pendingReadFilePath: string | null = null;
        for (const e of entries) {
          if (e.role === "skilllite_ui" && e.ui && typeof e.ui === "object") {
            const u = e.ui as Record<string, unknown>;
            const kind = u.kind;
            if (kind === "confirmation") {
              msgs.push({
                id: e.id,
                type: "confirmation",
                prompt: String(u.prompt ?? ""),
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
            pendingReadFilePath =
              name.replace(/-/g, "_") === "read_file"
                ? tryParseReadFilePathFromToolArgs(e.content)
                : null;
            if (isChatHiddenToolName(name)) continue;
            msgs.push({
              id: e.id,
              type: "tool_call" as const,
              name,
              args: e.content,
            });
            continue;
          }
          if (e.role === "tool_result") {
            const name = e.name ?? "";
            const sourcePath =
              name.replace(/-/g, "_") === "read_file"
                ? pendingReadFilePath ?? undefined
                : undefined;
            pendingReadFilePath = null;
            if (isChatHiddenToolName(name)) continue;
            msgs.push({
              id: e.id,
              type: "tool_result" as const,
              name,
              result: e.content,
              isError: e.is_error ?? false,
              sourcePath,
            });
            continue;
          }
          msgs.push({
            id: e.id,
            type: (e.role === "user" ? "user" : "assistant") as "user" | "assistant",
            content: e.content,
          });
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
        m.type === "confirmation" && !m.resolved
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
          const s = await invoke<{
            unprocessed_decisions: number;
            pending_skill_count: number;
            last_run_ts: string | null;
          }>("skilllite_load_evolution_status", {
            workspace: settings.workspace || ".",
            config: buildAssistantBridgeConfig(settings),
          });
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
    setIsClearing(true);
    setNotice(t("chat.clearingNotice"));
    try {
      await invoke("skilllite_clear_transcript", {
        sessionKey: currentSessionKey,
        workspace: settings.workspace || ".",
      });
      setMessages([]);
      setError(null);
      statusActions.clearAll();
      refreshRecentData();
      setNotice(t("chat.clearedNotice"));
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
    settings.workspace,
    currentSessionKey,
    statusActions,
    refreshRecentData,
    t,
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

  const sendMessage = useCallback(async (rawText: string) => {
    const text = rawText.trim();
    if (!text || loading || isClearing) return;

    if (text === "/new" || text === "/reset") {
      setInput("");
      setSettings({ showStarterPrompts: false });
      await handleClear();
      return;
    }

    setInput("");
    setError(null);
    setSettings({ showStarterPrompts: false });
    statusActions.clearPlan();
    statusActions.setLatestOutput("");
    setMessages((prev) => [
      ...prev,
      { id: crypto.randomUUID(), type: "user", content: text },
    ]);
    setLoading(true);

    // Always pass `config` (may be `{}`) so the bridge can clear SKILLLITE_SWARM_URL when Swarm
    // is off in settings; otherwise workspace-saved env would still enable delegation.
    const config = buildAssistantBridgeConfig(settings);

    try {
      await invoke("skilllite_chat_stream", {
        message: text,
        workspace: settings.workspace || ".",
        sessionKey: currentSessionKey,
        config,
      });
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
  }, [
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
    currentSessionKey,
    statusActions,
    t,
  ]);

  const handleSend = async () => {
    await sendMessage(input);
  };

  const showStarterPrompts =
    settings.showStarterPrompts === true && messages.length === 0 && !loading && !isClearing;

  const chatInputFooter = (
    <label className="flex items-center gap-2 text-xs text-ink-mute dark:text-ink-dark-mute cursor-pointer select-none">
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
  );

  const chatInputProps = {
    value: input,
    onChange: setInput,
    onSend: handleSend,
    onStop: handleStop,
    disabled: loading || isClearing,
    loading,
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
                onClick={() => void sendMessage(action.prompt)}
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
