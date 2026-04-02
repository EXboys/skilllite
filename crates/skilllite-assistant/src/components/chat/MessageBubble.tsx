import { memo } from "react";
import { MarkdownContent } from "../shared/MarkdownContent";
import {
  ListDirectoryToolResultView,
  ReadFileToolResultView,
} from "./FileToolResults";
import { StructuredPayload, getConversationToolResultMarkdown } from "./StructuredPayload";
import type { ChatMessage } from "../../types/chat";
import {
  evolutionNoteForDisplay,
  evolutionStatusHeadline,
} from "../../utils/evolutionDisplay";
import {
  plannerNudgeCurrentTaskHint,
  splitPlannerBoilerplate,
} from "../../utils/plannerNudgeUi";
import { useI18n } from "../../i18n";

function splitProgressStatusKey(
  key: string | undefined
): [string, string] | null {
  if (!key?.trim()) {
    return null;
  }
  const parts = key.split("/").map((p) => p.trim());
  if (parts.length < 2 || !parts[0] || !parts[1]) {
    return null;
  }
  return [parts[0], parts[1]];
}

interface MessageBubbleProps {
  message: ChatMessage;
  /** 当前设置中的工作区路径，用于 read_file 全屏保存 */
  workspace?: string;
  onConfirm?: (id: string, approved: boolean) => void;
  onClarify?: (id: string, action: string, hint?: string) => void;
  onEvolutionAction?: (id: string, option: string) => void;
}

/** Shared chat bubble chrome: width cap, radius, border, shadow, type scale */
const bubbleShell =
  "max-w-[min(85%,36rem)] rounded-2xl border text-sm leading-relaxed shadow-sm shadow-ink/[0.06] dark:shadow-none";

const bubbleUser =
  `${bubbleShell} ml-8 px-4 py-2.5 bg-accent-light dark:bg-accent-light-dark/90 border-accent/25 dark:border-accent/35 text-ink dark:text-ink-dark [&_a]:text-accent dark:[&_a]:text-blue-300 [&_a]:underline`;

const bubbleAssistant =
  `${bubbleShell} mr-4 px-4 py-2.5 bg-white dark:bg-paper-dark border-border dark:border-border-dark text-ink dark:text-ink-dark [&_a]:text-accent dark:[&_a]:text-blue-300`;

const bubbleMuted =
  `${bubbleShell} mr-4 px-4 py-3 bg-ink/[0.03] dark:bg-white/[0.05] border-border dark:border-border-dark text-ink dark:text-ink-dark`;

function PlannerBoilerplateFold({
  boilerplate,
  summaryLabel,
}: {
  boilerplate: string;
  summaryLabel: string;
}) {
  return (
    <details className="group mt-2 rounded-lg border border-border/70 dark:border-border-dark/80 bg-ink/[0.025] dark:bg-white/[0.04] px-2.5 py-1.5 text-left">
      <summary className="cursor-pointer select-none list-none flex items-center gap-1.5 text-xs text-ink-mute dark:text-ink-dark-mute [&::-webkit-details-marker]:hidden">
        <span
          className="shrink-0 inline-block text-[10px] opacity-75 transition-transform duration-200 group-open:rotate-90"
          aria-hidden
        >
          ▸
        </span>
        <span>{summaryLabel}</span>
      </summary>
      <pre className="mt-2 max-h-72 overflow-y-auto whitespace-pre-wrap break-words text-[11px] leading-relaxed text-ink/90 dark:text-ink-dark/90 font-mono border-t border-border/50 dark:border-border-dark/60 pt-2">
        {boilerplate}
      </pre>
    </details>
  );
}

/** Markdown body; when not streaming, folds echoed planner nudge blocks below a one-line summary. */
function ChatMarkdownWithPlannerFold({
  content,
  streaming,
}: {
  content: string;
  streaming?: boolean;
}) {
  const { t } = useI18n();
  if (streaming || !content) {
    return <MarkdownContent content={content} />;
  }
  const { main, boilerplate } = splitPlannerBoilerplate(content);
  if (!boilerplate) {
    return <MarkdownContent content={content} />;
  }
  const hint = plannerNudgeCurrentTaskHint(boilerplate);
  const summaryLabel =
    hint != null
      ? `${t("chat.plannerNudgeFoldSummary")} · ${hint}`
      : t("chat.plannerNudgeFoldSummary");
  return (
    <>
      {main.length > 0 ? <MarkdownContent content={main} /> : null}
      <PlannerBoilerplateFold boilerplate={boilerplate} summaryLabel={summaryLabel} />
    </>
  );
}

function ConfirmationBody({ text }: { text: string }) {
  const blocks = text.split(/\n{2,}/);
  return (
    <div className="space-y-3 mb-4 max-h-52 overflow-y-auto pr-1">
      {blocks.map((block, i) => (
        <p
          key={i}
          className="whitespace-pre-wrap text-sm text-ink dark:text-ink-dark-mute leading-relaxed"
        >
          {block.trimEnd()}
        </p>
      ))}
    </div>
  );
}

function MessageBubbleInner({
  message,
  workspace = ".",
  onConfirm,
  onClarify,
  onEvolutionAction,
}: MessageBubbleProps) {
  if (message.type === "user") {
    return (
      <div className="flex justify-end">
        <div className={bubbleUser}>
          <ChatMarkdownWithPlannerFold content={message.content} />
        </div>
      </div>
    );
  }

  if (message.type === "assistant") {
    return (
      <div className="flex justify-start">
        <div className={bubbleAssistant}>
          <ChatMarkdownWithPlannerFold
            content={message.content}
            streaming={message.streaming}
          />
          {message.streaming && (
            <span className="inline-block w-2 h-4 ml-1 bg-accent animate-pulse align-middle rounded-sm" />
          )}
        </div>
      </div>
    );
  }

  if (message.type === "plan") {
    return (
      <div className="flex justify-start">
        <div className={`${bubbleMuted} border-l-[3px] border-l-accent/35 dark:border-l-accent/45 pl-3.5`}>
          <div className="flex items-center gap-2 mb-2.5">
            <span className="text-[10px] font-semibold uppercase tracking-wider text-accent dark:text-blue-300/90">
              任务计划
            </span>
            <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
              {message.tasks.filter((t) => t.completed).length}/{message.tasks.length} 已完成
            </span>
          </div>
          <ul className="space-y-2 text-sm text-ink dark:text-ink-dark">
            {message.tasks.map((t) => (
              <li key={t.id} className="flex items-start gap-2.5">
                <span
                  className={`shrink-0 mt-0.5 flex h-5 w-5 items-center justify-center rounded-full text-[11px] font-medium ${
                    t.completed
                      ? "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300"
                      : "border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute"
                  }`}
                  aria-hidden
                >
                  {t.completed ? "✓" : "○"}
                </span>
                <div className="min-w-0 flex-1">
                  <span className={t.completed ? "text-ink-mute dark:text-ink-dark-mute line-through decoration-ink-mute/40" : ""}>
                    {t.description}
                  </span>
                  {t.tool_hint && (
                    <span className="ml-1.5 align-middle rounded-md bg-ink/6 dark:bg-white/8 px-1.5 py-0.5 text-[11px] font-mono text-ink-mute dark:text-ink-dark-mute">
                      {t.tool_hint}
                    </span>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </div>
      </div>
    );
  }

  if (message.type === "tool_call") {
    return (
      <div className="flex justify-start">
        <div className={`${bubbleMuted} border-l-[3px] border-l-sky-400/50 dark:border-l-sky-500/45`}>
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1 mb-0.5">
            <span className="text-[10px] font-semibold uppercase tracking-wider text-sky-700/90 dark:text-sky-300/90">
              工具调用
            </span>
            <span className="rounded-md bg-sky-500/10 dark:bg-sky-400/15 px-2 py-0.5 font-mono text-xs font-medium text-ink dark:text-ink-dark">
              {message.name}
            </span>
          </div>
          {message.args ? <StructuredPayload raw={message.args} /> : null}
        </div>
      </div>
    );
  }

  if (message.type === "tool_result") {
    if (!message.isError && message.name === "read_file") {
      return (
        <div className="flex justify-start">
          <div
            className={`${bubbleAssistant} border-l-[3px] border-l-emerald-500/35 dark:border-l-emerald-500/40 pl-3.5 max-w-[min(92%,42rem)] w-full min-w-0`}
          >
            <p className="text-[10px] font-medium uppercase tracking-wider text-ink-mute dark:text-ink-dark-mute mb-2">
              工具结果
              <span className="ml-1.5 font-mono normal-case tracking-normal text-emerald-700/85 dark:text-emerald-300/90">
                {message.name}
              </span>
            </p>
            <ReadFileToolResultView
              result={message.result}
              sourcePath={message.sourcePath}
              workspace={workspace}
            />
          </div>
        </div>
      );
    }
    if (!message.isError && message.name === "list_directory") {
      return (
        <div className="flex justify-start">
          <div
            className={`${bubbleAssistant} border-l-[3px] border-l-emerald-500/35 dark:border-l-emerald-500/40 pl-3.5 max-w-[min(92%,42rem)] w-full min-w-0`}
          >
            <p className="text-[10px] font-medium uppercase tracking-wider text-ink-mute dark:text-ink-dark-mute mb-2">
              工具结果
              <span className="ml-1.5 font-mono normal-case tracking-normal text-emerald-700/85 dark:text-emerald-300/90">
                {message.name}
              </span>
            </p>
            <ListDirectoryToolResultView result={message.result} />
          </div>
        </div>
      );
    }
    const convMd = getConversationToolResultMarkdown(message.result, message.isError);
    if (convMd !== null) {
      return (
        <div className="flex justify-start">
          <div className={`${bubbleAssistant} border-l-[3px] border-l-emerald-500/35 dark:border-l-emerald-500/40 pl-3.5`}>
            <p className="text-[10px] font-medium uppercase tracking-wider text-ink-mute dark:text-ink-dark-mute mb-2">
              工具回复
              <span className="ml-1.5 font-mono normal-case tracking-normal text-emerald-700/85 dark:text-emerald-300/90">
                {message.name}
              </span>
            </p>
            <MarkdownContent content={convMd} />
          </div>
        </div>
      );
    }
    return (
      <div className="flex justify-start">
        <div
          className={`${bubbleShell} mr-4 px-4 py-3 border-l-[3px] ${
            message.isError
              ? "bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800/50 border-l-red-500/55"
              : "bg-ink/[0.03] dark:bg-white/[0.05] border-border dark:border-border-dark border-l-emerald-500/45"
          }`}
        >
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1 mb-0.5">
            <span
              className={`text-[10px] font-semibold uppercase tracking-wider ${
                message.isError
                  ? "text-red-700 dark:text-red-300"
                  : "text-emerald-700/90 dark:text-emerald-300/90"
              }`}
            >
              工具结果
            </span>
            <span
              className={`rounded-md px-2 py-0.5 font-mono text-xs font-medium ${
                message.isError
                  ? "bg-red-500/12 text-ink dark:text-ink-dark"
                  : "bg-emerald-500/10 dark:bg-emerald-400/15 text-ink dark:text-ink-dark"
              }`}
            >
              {message.isError ? "✗ " : "✓ "}
              {message.name}
            </span>
          </div>
          <StructuredPayload raw={message.result} />
        </div>
      </div>
    );
  }

  if (message.type === "confirmation") {
    return (
      <div className="flex justify-start">
        <div
          className={`${bubbleShell} mr-4 px-4 py-3 border-l-[3px] border-l-amber-500/50 bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-800/50`}
        >
          <div className="text-xs font-semibold uppercase tracking-wide text-amber-800 dark:text-amber-200 mb-2">
            执行确认
          </div>
          <ConfirmationBody text={message.prompt} />
          {message.resolved ? (
            <div className="text-sm text-ink-mute dark:text-ink-dark-mute">
              {message.approved ? "✓ 已允许" : "✗ 已拒绝"}
            </div>
          ) : (
            onConfirm && (
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => onConfirm(message.id, false)}
                  className="px-3 py-1.5 text-sm rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark hover:bg-gray-100 dark:hover:bg-white/5"
                >
                  拒绝
                </button>
                <button
                  type="button"
                  onClick={() => onConfirm(message.id, true)}
                  className="px-3 py-1.5 text-sm rounded-md bg-accent text-white font-medium hover:bg-accent-hover"
                >
                  允许
                </button>
              </div>
            )
          )}
        </div>
      </div>
    );
  }

  if (message.type === "clarification") {
    return (
      <div className="flex justify-start">
        <div
          className={`${bubbleShell} mr-4 px-4 py-3 bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800/50`}
        >
          <div className="text-xs font-semibold uppercase tracking-wide text-blue-800 dark:text-blue-200 mb-1.5">
            需要你的确认
          </div>
          <p className="text-sm text-ink dark:text-ink-dark-mute mb-3">
            {message.message}
          </p>
          {message.resolved ? (
            <div className="text-sm text-ink-mute dark:text-ink-dark-mute">
              {message.selectedOption === "stop"
                ? "✗ 已停止"
                : `✓ ${message.selectedOption ?? "已继续"}`}
            </div>
          ) : (
            onClarify && (
              <div className="flex flex-wrap gap-2">
                {message.suggestions.map((s) => (
                  <button
                    key={s}
                    type="button"
                    onClick={() => onClarify(message.id, "continue", s)}
                    className="px-3 py-1.5 text-sm rounded-md bg-accent text-white font-medium hover:bg-accent-hover transition-colors"
                  >
                    {s}
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => onClarify(message.id, "stop")}
                  className="px-3 py-1.5 text-sm rounded-lg border border-border dark:border-border-dark text-ink dark:text-ink-dark hover:bg-gray-100 dark:hover:bg-white/5 transition-colors"
                >
                  停止
                </button>
              </div>
            )
          )}
        </div>
      </div>
    );
  }

  if (message.type === "evolution_options") {
    return (
      <div className="flex justify-start">
        <div
          className={`${bubbleShell} mr-4 px-4 py-3 bg-purple-50 dark:bg-purple-900/20 border-purple-200 dark:border-purple-800/50`}
        >
          <div className="text-xs font-semibold uppercase tracking-wide text-purple-800 dark:text-purple-200 mb-1.5">
            能力缺口处理
          </div>
          <p className="text-sm text-ink dark:text-ink-dark-mute mb-2">{message.message}</p>
          {message.resolved ? (
            <div className="space-y-1.5 text-sm text-ink-mute dark:text-ink-dark-mute">
              <div>✓ {message.selectedOption ?? "已处理"}</div>
              {message.proposalId && (
                <div className="text-xs">
                  提案: <span className="font-mono">{message.proposalId}</span>
                </div>
              )}
              {message.progressStatus && (
                <div className="text-xs">
                  进度:{" "}
                  <span
                    className={
                      message.progressDone
                        ? "font-medium text-emerald-700 dark:text-emerald-300"
                        : "font-medium text-purple-700 dark:text-purple-300"
                    }
                  >
                    {(() => {
                      const sp = splitProgressStatusKey(message.progressStatus);
                      return sp
                        ? evolutionStatusHeadline(
                            sp[0],
                            sp[1],
                            message.progressNote
                          )
                        : message.progressStatus;
                    })()}
                  </span>
                  {message.progressUpdatedAt ? ` · ${message.progressUpdatedAt}` : ""}
                </div>
              )}
              {(() => {
                const sp = splitProgressStatusKey(message.progressStatus);
                const noteText = sp
                  ? evolutionNoteForDisplay(
                      sp[0],
                      sp[1],
                      message.progressNote
                    )
                  : message.progressNote?.trim() || null;
                return noteText ? (
                  <div className="text-xs opacity-90 whitespace-pre-wrap">{noteText}</div>
                ) : null;
              })()}
            </div>
          ) : (
            onEvolutionAction && (
              <div className="flex flex-wrap gap-2">
                {message.options.map((option) => (
                  <button
                    key={option}
                    type="button"
                    onClick={() => onEvolutionAction(message.id, option)}
                    className={`px-3 py-1.5 text-sm rounded-md transition-colors ${
                      option === "启动进化"
                        ? "bg-purple-600 text-white font-medium hover:bg-purple-700"
                        : "border border-border dark:border-border-dark text-ink dark:text-ink-dark hover:bg-gray-100 dark:hover:bg-white/5"
                    }`}
                  >
                    {option}
                  </button>
                ))}
              </div>
            )
          )}
        </div>
      </div>
    );
  }

  return null;
}

export const MessageBubble = memo(MessageBubbleInner);
