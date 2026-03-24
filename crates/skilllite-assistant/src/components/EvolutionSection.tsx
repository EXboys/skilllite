import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MarkdownContent } from "./shared/MarkdownContent";
import { useSettingsStore } from "../stores/useSettingsStore";

export interface EvolutionLogEntryDto {
  ts: string;
  event_type: string;
  target_id: string | null;
  reason: string | null;
}

export interface EvolutionStatusPayload {
  mode_key: string;
  mode_label: string;
  interval_secs: number;
  decision_threshold: number;
  unprocessed_decisions: number;
  last_run_ts: string | null;
  judgement_label: string | null;
  judgement_reason: string | null;
  recent_events: EvolutionLogEntryDto[];
  pending_skill_count: number;
  db_error: string | null;
}

export interface PendingSkillDto {
  name: string;
  needs_review: boolean;
  preview: string;
}

function formatInterval(secs: number): string {
  if (secs >= 3600 && secs % 3600 === 0) {
    return `每 ${secs / 3600} 小时`;
  }
  if (secs % 60 === 0) {
    return `每 ${secs / 60} 分钟`;
  }
  return `每 ${secs} 秒`;
}

function formatTs(ts: string): string {
  if (ts.length >= 16) return ts.slice(0, 16).replace("T", " ");
  return ts;
}

function eventIcon(eventType: string): string {
  switch (eventType) {
    case "rule_added":
      return "✓";
    case "example_added":
      return "📖";
    case "skill_generated":
      return "✨";
    case "skill_pending":
      return "🆕";
    case "skill_refined":
      return "🔧";
    case "skill_confirmed":
      return "✅";
    case "evolution_judgement":
      return "🧭";
    case "evolution_run":
      return "◆";
    case "auto_rollback":
      return "⚠";
    default:
      if (eventType.includes("rolled_back")) return "↩";
      if (eventType.includes("retired")) return "🗑";
      return "·";
  }
}

function useEvolutionStatus() {
  const { settings } = useSettingsStore();
  const workspace = settings.workspace || ".";
  const [status, setStatus] = useState<EvolutionStatusPayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await invoke<EvolutionStatusPayload>("skilllite_load_evolution_status", {
        workspace,
      });
      setStatus(s);
    } catch (e) {
      setError(String(e));
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, [workspace]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { status, loading, error, refresh, workspace };
}

/** 右侧面板摘要 */
export function EvolutionStatusSummary({ onOpenDetail }: { onOpenDetail: () => void }) {
  const { status, loading, error, refresh, workspace } = useEvolutionStatus();

  if (loading && !status) {
    return (
      <section className="mb-4">
        <div className="flex items-center justify-between mb-2">
          <span className="font-medium text-ink dark:text-ink-dark">自进化</span>
        </div>
        <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
      </section>
    );
  }

  if (error && !status) {
    return (
      <section className="mb-4">
        <div className="flex items-center justify-between mb-2">
          <span className="font-medium text-ink dark:text-ink-dark">自进化</span>
          <button
            type="button"
            onClick={() => void refresh()}
            className="text-xs text-accent hover:underline"
          >
            重试
          </button>
        </div>
        <p className="text-xs text-red-600 dark:text-red-400">{error}</p>
      </section>
    );
  }

  const s = status!;
  const scheduleHint =
    s.mode_key === "disabled"
      ? "已禁用，后台不会自动进化"
      : `${formatInterval(s.interval_secs)} 检查一次；未处理决策 ≥ ${s.decision_threshold} 条也会触发`;

  return (
    <section className="mb-4">
      <div className="flex items-center justify-between mb-2 gap-2">
        <button
          type="button"
          onClick={onOpenDetail}
          className="flex-1 min-w-0 text-left font-medium text-ink dark:text-ink-dark group hover:text-accent dark:hover:text-accent"
        >
          <span>自进化</span>
          <span className="text-xs font-normal text-ink-mute group-hover:text-accent dark:text-ink-dark-mute dark:group-hover:text-accent inline-flex items-center gap-0.5 ml-0.5 transition-colors">
            详情与审核
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M9 18l6-6-6-6" />
            </svg>
          </span>
        </button>
        <button
          type="button"
          onClick={() => void refresh()}
          disabled={loading}
          className="p-1.5 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 shrink-0"
          title="刷新"
          aria-label="刷新进化状态"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={loading ? "animate-spin" : ""}
          >
            <path d="M21 2v6h-6" />
            <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
            <path d="M3 22v-6h6" />
            <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
          </svg>
        </button>
      </div>

      <div
        className="rounded-lg border border-border/60 dark:border-border-dark/60 bg-gray-50/50 dark:bg-surface-dark/50 px-2.5 py-2 space-y-1.5 text-xs text-ink dark:text-ink-dark cursor-pointer"
        onClick={onOpenDetail}
        role="button"
        onKeyDown={(e) => e.key === "Enter" && onOpenDetail()}
        tabIndex={0}
      >
        {s.db_error && (
          <p className="text-amber-700 dark:text-amber-400">{s.db_error}</p>
        )}
        <div className="flex justify-between gap-2">
          <span className="text-ink-mute dark:text-ink-dark-mute shrink-0">模式</span>
          <span className="text-right font-medium">{s.mode_label}</span>
        </div>
        <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute leading-snug">{scheduleHint}</p>
        <div className="flex justify-between gap-2">
          <span className="text-ink-mute dark:text-ink-dark-mute">待处理决策</span>
          <span>{s.unprocessed_decisions}</span>
        </div>
        <div className="flex justify-between gap-2">
          <span className="text-ink-mute dark:text-ink-dark-mute">上次进化运行</span>
          <span className="text-right truncate">
            {s.last_run_ts ? formatTs(s.last_run_ts) : "—"}
          </span>
        </div>
        {s.judgement_label && (
          <div className="pt-1 border-t border-border/40 dark:border-border-dark/40">
            <span className="text-ink-mute dark:text-ink-dark-mute">审核判断 </span>
            <span className="font-medium">{s.judgement_label}</span>
            {s.judgement_reason && (
              <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute mt-0.5 line-clamp-2">
                {s.judgement_reason}
              </p>
            )}
          </div>
        )}
        <div className="flex justify-between gap-2 pt-0.5">
          <span className="text-ink-mute dark:text-ink-dark-mute">待确认技能</span>
          <span className={s.pending_skill_count > 0 ? "text-accent font-semibold" : ""}>
            {s.pending_skill_count}
          </span>
        </div>
        <p className="text-[10px] text-ink-mute/80 dark:text-ink-dark-mute/80 truncate" title={workspace}>
          工作区: {workspace}
        </p>
      </div>
    </section>
  );
}

function PendingSkillReviewCard({
  skill,
  workspace,
  onChanged,
}: {
  skill: PendingSkillDto;
  workspace: string;
  onChanged: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [fullMd, setFullMd] = useState<string | null>(null);
  const [loadingFull, setLoadingFull] = useState(false);
  const [acting, setActing] = useState<"confirm" | "reject" | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const loadFull = async () => {
    if (fullMd !== null) {
      setExpanded(!expanded);
      return;
    }
    setLoadingFull(true);
    try {
      const md = await invoke<string>("skilllite_read_pending_skill_md", {
        workspace,
        skillName: skill.name,
      });
      setFullMd(md);
      setExpanded(true);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setLoadingFull(false);
    }
  };

  const confirm = async () => {
    setActing("confirm");
    setMsg(null);
    try {
      await invoke("skilllite_confirm_pending_skill", { workspace, skillName: skill.name });
      setMsg("已加入已确认技能");
      onChanged();
    } catch (e) {
      setMsg(String(e));
    } finally {
      setActing(null);
    }
  };

  const reject = async () => {
    setActing("reject");
    setMsg(null);
    try {
      await invoke("skilllite_reject_pending_skill", { workspace, skillName: skill.name });
      setMsg("已拒绝并删除");
      onChanged();
    } catch (e) {
      setMsg(String(e));
    } finally {
      setActing(null);
    }
  };

  const displayMd = expanded && fullMd !== null ? fullMd : skill.preview;
  const showShort = !expanded || fullMd === null;

  return (
    <div className="rounded-lg border border-border dark:border-border-dark bg-white/60 dark:bg-paper-dark/60 p-3 space-y-2">
      <div className="flex items-center justify-between gap-2 flex-wrap">
        <span className="text-sm font-semibold text-ink dark:text-ink-dark">{skill.name}</span>
        {skill.needs_review && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-100 dark:bg-amber-900/40 text-amber-900 dark:text-amber-200">
            建议人工过目
          </span>
        )}
      </div>
      <button
        type="button"
        onClick={() => void loadFull()}
        className="text-xs text-accent hover:underline"
        disabled={loadingFull}
      >
        {loadingFull ? "加载全文…" : expanded ? "收起全文" : "查看 / 展开全文"}
      </button>
      <div
        className={`prose prose-sm max-w-none dark:prose-invert [&_pre]:text-xs [&_code]:text-xs overflow-y-auto border border-border/50 dark:border-border-dark/50 rounded-md p-2 bg-gray-50/80 dark:bg-surface-dark/50 ${
          showShort ? "max-h-48" : "max-h-[min(70vh,520px)]"
        }`}
      >
        {displayMd ? (
          <MarkdownContent content={displayMd} />
        ) : (
          <p className="text-xs text-ink-mute">（无 SKILL.md 内容）</p>
        )}
      </div>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => void confirm()}
          disabled={acting !== null}
          className="px-3 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:opacity-90 disabled:opacity-50"
        >
          {acting === "confirm" ? "处理中…" : "确认加入"}
        </button>
        <button
          type="button"
          onClick={() => void reject()}
          disabled={acting !== null}
          className="px-3 py-1.5 rounded-lg border border-border dark:border-border-dark text-xs text-ink dark:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50"
        >
          {acting === "reject" ? "处理中…" : "拒绝"}
        </button>
      </div>
      {msg && <p className="text-xs text-ink-mute dark:text-ink-dark-mute">{msg}</p>}
    </div>
  );
}

/** 独立详情窗口：时间线 + 待审核列表 */
export function EvolutionDetailBody() {
  const { status, loading, error, refresh, workspace } = useEvolutionStatus();
  const [pending, setPending] = useState<PendingSkillDto[]>([]);
  const [pendingLoading, setPendingLoading] = useState(true);

  const loadPending = useCallback(async () => {
    setPendingLoading(true);
    try {
      const list = await invoke<PendingSkillDto[]>("skilllite_list_evolution_pending", { workspace });
      setPending(list);
    } catch {
      setPending([]);
    } finally {
      setPendingLoading(false);
    }
  }, [workspace]);

  useEffect(() => {
    void loadPending();
  }, [loadPending]);

  const onSkillChanged = useCallback(() => {
    void loadPending();
    void refresh();
  }, [loadPending, refresh]);

  if (error && !status) {
    return (
      <div className="p-4">
        <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        <button type="button" className="mt-2 text-sm text-accent" onClick={() => void refresh()}>
          重试
        </button>
      </div>
    );
  }

  const s = status;

  return (
    <div className="space-y-6 p-1">
      <div className="flex items-start justify-between gap-2">
        <button
          type="button"
          onClick={() => void refresh()}
          disabled={loading}
          className="text-xs text-accent hover:underline disabled:opacity-50 shrink-0"
        >
          {loading ? "刷新中…" : "刷新状态"}
        </button>
      </div>

      {s?.db_error && (
        <p className="text-sm text-amber-700 dark:text-amber-400">{s.db_error}</p>
      )}

      {s && (
        <section className="space-y-2">
          <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">调度与配置</h2>
          <ul className="text-xs text-ink dark:text-ink-dark space-y-1.5 bg-gray-50/80 dark:bg-surface-dark/50 rounded-lg p-3 border border-border/50 dark:border-border-dark/50">
            <li>
              <span className="text-ink-mute dark:text-ink-dark-mute">模式：</span>
              {s.mode_label}
            </li>
            <li>
              <span className="text-ink-mute dark:text-ink-dark-mute">周期触发：</span>
              {s.mode_key === "disabled" ? "—" : formatInterval(s.interval_secs)}
            </li>
            <li>
              <span className="text-ink-mute dark:text-ink-dark-mute">决策数触发阈值：</span>
              {s.decision_threshold}（当前未处理 {s.unprocessed_decisions}）
            </li>
            <li>
              <span className="text-ink-mute dark:text-ink-dark-mute">上次 evolution_run：</span>
              {s.last_run_ts ? formatTs(s.last_run_ts) : "暂无记录"}
            </li>
            <li className="text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
              说明：周期与阈值可在工作区 .env 中设置 SKILLLITE_EVOLUTION_INTERVAL_SECS、
              SKILLLITE_EVOLUTION_DECISION_THRESHOLD；SKILLLITE_EVOLUTION=0 可关闭进化。
            </li>
          </ul>
        </section>
      )}

      {s?.judgement_label && (
        <section className="space-y-2">
          <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">系统审核判断</h2>
          <div className="rounded-lg border border-border dark:border-border-dark p-3 text-sm">
            <p className="font-medium text-ink dark:text-ink-dark">{s.judgement_label}</p>
            {s.judgement_reason && (
              <p className="text-xs text-ink-mute dark:text-ink-dark-mute mt-2 whitespace-pre-wrap">
                {s.judgement_reason}
              </p>
            )}
          </div>
        </section>
      )}

      <section className="space-y-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">待确认技能（人工审核）</h2>
          <button
            type="button"
            onClick={() => void loadPending()}
            className="text-xs text-accent hover:underline"
          >
            刷新列表
          </button>
        </div>
        {pendingLoading ? (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute">加载中…</p>
        ) : pending.length === 0 ? (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">
            暂无待确认技能。进化生成的新技能会出现在 .skills/_evolved/_pending/。
          </p>
        ) : (
          <div className="space-y-4">
            {pending.map((p) => (
              <PendingSkillReviewCard
                key={p.name}
                skill={p}
                workspace={workspace}
                onChanged={onSkillChanged}
              />
            ))}
          </div>
        )}
      </section>

      <section className="space-y-2">
        <h2 className="text-sm font-semibold text-ink dark:text-ink-dark">最近进化事件</h2>
        {!s?.recent_events.length ? (
          <p className="text-xs text-ink-mute dark:text-ink-dark-mute italic">暂无事件记录</p>
        ) : (
          <ul className="space-y-2 text-xs">
            {s.recent_events.map((e, i) => (
              <li
                key={`${e.ts}-${e.event_type}-${i}`}
                className="border-b border-border/40 dark:border-border-dark/40 pb-2 last:border-0"
              >
                <div className="flex items-start gap-2">
                  <span className="shrink-0 w-4 text-center">{eventIcon(e.event_type)}</span>
                  <div className="min-w-0 flex-1">
                    <div className="text-ink-mute dark:text-ink-dark-mute font-mono text-[11px]">
                      {formatTs(e.ts)}
                    </div>
                    <div className="font-medium text-ink dark:text-ink-dark">{e.event_type}</div>
                    {e.target_id && (
                      <div className="text-ink-mute dark:text-ink-dark-mute truncate">
                        target: {e.target_id}
                      </div>
                    )}
                    {e.reason && (
                      <p className="text-ink-mute dark:text-ink-dark-mute mt-0.5 whitespace-pre-wrap break-words">
                        {e.reason.length > 280 ? `${e.reason.slice(0, 280)}…` : e.reason}
                      </p>
                    )}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
