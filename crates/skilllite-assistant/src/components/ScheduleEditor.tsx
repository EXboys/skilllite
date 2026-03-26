import { useState } from "react";
import {
  type ScheduleForm,
  type ScheduleJobForm,
  type ScheduleTriggerMode,
  scheduleFormToJson,
  parseScheduleJson,
  validateScheduleForm,
  INTERVAL_PRESETS,
  normalizeDailyTime,
} from "../utils/scheduleForm";

function defaultOnceAtLocal(): string {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  d.setHours(9, 0, 0, 0);
  const y = d.getFullYear();
  const mo = (d.getMonth() + 1).toString().padStart(2, "0");
  const day = d.getDate().toString().padStart(2, "0");
  return `${y}-${mo}-${day}T09:00`;
}

function timeInputValue(hhmm: string): string {
  const n = normalizeDailyTime(hhmm);
  const m = n.match(/^(\d{1,2}):(\d{2})$/);
  if (!m) return "09:00";
  return `${m[1].padStart(2, "0")}:${m[2]}`;
}

function newJob(): ScheduleJobForm {
  return {
    id: "",
    enabled: true,
    trigger_mode: "interval",
    interval_seconds: 86_400,
    daily_times: [],
    once_at: "",
    session_key: "",
    goal: "",
    steps_prompt: "",
    message: "",
  };
}

interface ScheduleEditorProps {
  data: ScheduleForm;
  onChange: (next: ScheduleForm) => void;
  error: string | null;
  onClearError: () => void;
  onError: (message: string | null) => void;
  inputCls: string;
  labelCls: string;
}

export default function ScheduleEditor({
  data,
  onChange,
  error,
  onClearError,
  onError,
  inputCls,
  labelCls,
}: ScheduleEditorProps) {
  const [jsonMode, setJsonMode] = useState(false);
  const [jsonText, setJsonText] = useState("");

  const setJobs = (jobs: ScheduleJobForm[]) => {
    onChange({ ...data, jobs });
    onClearError();
  };

  const updateJob = (index: number, partial: Partial<ScheduleJobForm>) => {
    const jobs = data.jobs.map((j, i) => (i === index ? { ...j, ...partial } : j));
    setJobs(jobs);
  };

  if (jsonMode) {
    return (
      <div className="space-y-3">
        {error && <p className="text-xs text-red-600 dark:text-red-400">{error}</p>}
        <textarea
          value={jsonText}
          onChange={(e) => {
            setJsonText(e.target.value);
            onClearError();
          }}
          spellCheck={false}
          className={`w-full min-h-[280px] rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-3 py-2 text-ink dark:text-ink-dark font-mono text-xs leading-relaxed focus:ring-2 focus:ring-accent/40 focus:border-accent outline-none resize-y`}
        />
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => {
              const r = parseScheduleJson(jsonText);
              if (!r.ok) {
                onError(r.error);
                return;
              }
              const v = validateScheduleForm(r.data);
              if (v) {
                onError(v);
                return;
              }
              onChange(r.data);
              onError(null);
              onClearError();
              setJsonMode(false);
            }}
            className="text-xs px-2.5 py-1.5 rounded-lg bg-accent text-white font-medium hover:bg-accent-hover"
          >
            应用 JSON
          </button>
          <button
            type="button"
            onClick={() => {
              setJsonMode(false);
              onError(null);
              onClearError();
            }}
            className="text-xs text-ink-mute dark:text-ink-dark-mute hover:underline"
          >
            取消
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {error && <p className="text-xs text-red-600 dark:text-red-400">{error}</p>}

      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2 text-xs text-ink dark:text-ink-dark cursor-pointer">
          <input
            type="checkbox"
            checked={data.enabled}
            onChange={(e) => {
              onChange({ ...data, enabled: e.target.checked });
              onClearError();
            }}
            className="rounded border-border dark:border-border-dark"
          />
          启用定时任务（总开关）
        </label>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className={labelCls}>每日最多执行次数</label>
          <input
            type="number"
            min={1}
            inputMode="numeric"
            value={data.limits.max_runs_per_day}
            onChange={(e) => {
              const n = Number(e.target.value);
              onChange({
                ...data,
                limits: {
                  ...data.limits,
                  max_runs_per_day: Number.isFinite(n) && n >= 1 ? n : 8,
                },
              });
              onClearError();
            }}
            className={inputCls}
          />
        </div>
        <div>
          <label className={labelCls}>两次运行最小间隔（秒）</label>
          <input
            type="number"
            min={0}
            inputMode="numeric"
            value={data.limits.min_interval_seconds_between_runs}
            onChange={(e) => {
              const n = Number(e.target.value);
              onChange({
                ...data,
                limits: {
                  ...data.limits,
                  min_interval_seconds_between_runs:
                    Number.isFinite(n) && n >= 0 ? Math.floor(n) : 0,
                },
              });
              onClearError();
            }}
            className={inputCls}
          />
        </div>
      </div>

      <div className="flex items-center justify-between gap-2">
        <p className="text-xs font-medium text-ink dark:text-ink-dark-mute">任务列表</p>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => {
              setJsonText(scheduleFormToJson(data));
              onError(null);
              onClearError();
              setJsonMode(true);
            }}
            className="text-xs text-accent hover:underline"
          >
            编辑原始 JSON
          </button>
          <button
            type="button"
            onClick={() => {
              setJobs([...data.jobs, newJob()]);
            }}
            className="text-xs px-2 py-1 rounded-md border border-border dark:border-border-dark hover:bg-gray-100 dark:hover:bg-white/5"
          >
            添加任务
          </button>
        </div>
      </div>

      {data.jobs.length === 0 && (
        <p className="text-xs text-ink-mute dark:text-ink-dark-mute py-2">
          暂无任务，点击「添加任务」开始配置。
        </p>
      )}

      <div className="space-y-4">
        {data.jobs.map((job, index) => (
          <div
            key={index}
            className="rounded-lg border border-border dark:border-border-dark p-3 space-y-3 bg-gray-50/80 dark:bg-surface-dark/40"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="text-xs font-medium text-ink dark:text-ink-dark">任务 {index + 1}</span>
              <div className="flex items-center gap-2">
                <label className="flex items-center gap-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute cursor-pointer">
                  <input
                    type="checkbox"
                    checked={job.enabled}
                    onChange={(e) => updateJob(index, { enabled: e.target.checked })}
                    className="rounded border-border dark:border-border-dark"
                  />
                  启用
                </label>
                <button
                  type="button"
                  onClick={() => setJobs(data.jobs.filter((_, i) => i !== index))}
                  className="text-[11px] text-red-600 dark:text-red-400 hover:underline"
                >
                  删除
                </button>
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div>
                <label className={labelCls}>任务 ID</label>
                <input
                  type="text"
                  value={job.id}
                  onChange={(e) => updateJob(index, { id: e.target.value })}
                  placeholder="例如 daily-brief"
                  className={inputCls}
                />
              </div>
              <div>
                <label className={labelCls}>会话 Key（可选）</label>
                <input
                  type="text"
                  value={job.session_key}
                  onChange={(e) => updateJob(index, { session_key: e.target.value })}
                  placeholder="留空则使用 schedule-任务ID"
                  className={inputCls}
                />
              </div>
            </div>

            <div>
              <label className={labelCls}>触发方式（系统本地时区）</label>
              <div className="flex rounded-lg border border-border dark:border-border-dark overflow-hidden flex-wrap">
                {(
                  [
                    ["interval", "按间隔"] as const,
                    ["daily", "每天定时"] as const,
                    ["once", "仅一次"] as const,
                  ] satisfies readonly [ScheduleTriggerMode, string][]
                ).map(([mode, label]) => (
                  <button
                    key={mode}
                    type="button"
                    onClick={() => {
                      if (mode === "interval") {
                        updateJob(index, {
                          trigger_mode: "interval",
                          daily_times: [],
                          once_at: "",
                        });
                      } else if (mode === "daily") {
                        updateJob(index, {
                          trigger_mode: "daily",
                          once_at: "",
                          daily_times:
                            job.daily_times.length > 0 ? [...job.daily_times] : ["09:00"],
                        });
                      } else {
                        updateJob(index, {
                          trigger_mode: "once",
                          daily_times: [],
                          once_at: job.once_at?.trim() ? job.once_at : defaultOnceAtLocal(),
                        });
                      }
                    }}
                    className={`flex-1 min-w-[5.5rem] py-1.5 text-[11px] font-medium transition-colors ${
                      job.trigger_mode === mode
                        ? "bg-accent text-white"
                        : "bg-gray-50 dark:bg-surface-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5"
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>
              <p className="mt-1 text-[11px] text-ink-mute dark:text-ink-dark-mute leading-relaxed">
                「每天定时」= 每个时刻在每个自然日最多成功一次；可添加多个时刻。需外部周期执行{" "}
                <code className="bg-gray-100 dark:bg-surface-dark px-0.5 rounded">schedule tick</code>
                。                「仅一次」= 到达该本地时间后执行；仅成功时写入状态，失败会在下次 tick 重试。墙钟任务（每天 / 仅一次）不受全局「两次运行最小间隔」阻挡，便于失败重试。
              </p>
            </div>

            {job.trigger_mode === "interval" && (
            <div>
              <label className={labelCls}>运行间隔（秒）</label>
              <div className="flex flex-wrap gap-2 mb-2">
                {INTERVAL_PRESETS.map((p) => (
                  <button
                    key={p.seconds}
                    type="button"
                    onClick={() => updateJob(index, { interval_seconds: p.seconds })}
                    className={`text-[11px] px-2 py-1 rounded-md border transition-colors ${
                      job.interval_seconds === p.seconds
                        ? "border-accent bg-accent/10 text-accent"
                        : "border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5"
                    }`}
                  >
                    {p.label}
                  </button>
                ))}
              </div>
              <input
                type="number"
                min={1}
                inputMode="numeric"
                value={job.interval_seconds}
                onChange={(e) => {
                  const n = Number(e.target.value);
                  if (Number.isFinite(n) && n >= 1) {
                    updateJob(index, { interval_seconds: Math.floor(n) });
                  } else if (e.target.value === "") {
                    updateJob(index, { interval_seconds: 1 });
                  }
                }}
                className={inputCls}
              />
            </div>
            )}

            {job.trigger_mode === "daily" && (
            <div className="space-y-2">
              <label className={labelCls}>每天执行时刻（本地，可多个）</label>
              {job.daily_times.map((t, ti) => (
                <div key={ti} className="flex gap-2 items-center">
                  <input
                    type="time"
                    value={timeInputValue(t)}
                    onChange={(e) => {
                      const next = [...job.daily_times];
                      next[ti] = normalizeDailyTime(e.target.value);
                      updateJob(index, { daily_times: next });
                    }}
                    className={`flex-1 min-w-0 ${inputCls}`}
                  />
                  <button
                    type="button"
                    disabled={job.daily_times.length <= 1}
                    onClick={() =>
                      updateJob(index, {
                        daily_times: job.daily_times.filter((_, j) => j !== ti),
                      })
                    }
                    className="shrink-0 text-[11px] text-red-600 dark:text-red-400 hover:underline disabled:opacity-30 disabled:pointer-events-none"
                  >
                    移除
                  </button>
                </div>
              ))}
              <button
                type="button"
                onClick={() =>
                  updateJob(index, {
                    daily_times: [...job.daily_times, "12:00"],
                  })
                }
                className="text-[11px] px-2 py-1 rounded-md border border-border dark:border-border-dark hover:bg-gray-100 dark:hover:bg-white/5"
              >
                添加时刻
              </button>
            </div>
            )}

            {job.trigger_mode === "once" && (
            <div>
              <label className={labelCls}>执行时间（本地，仅一次）</label>
              <input
                type="datetime-local"
                step={60}
                value={job.once_at}
                onChange={(e) => updateJob(index, { once_at: e.target.value })}
                className={inputCls}
              />
            </div>
            )}

            <div>
              <label className={labelCls}>目标内容</label>
              <textarea
                value={job.goal}
                onChange={(e) => updateJob(index, { goal: e.target.value })}
                placeholder="这次定时运行要达成什么结果？"
                rows={2}
                className={`${inputCls} resize-y min-h-[2.5rem]`}
              />
            </div>

            <div>
              <label className={labelCls}>执行步骤（Prompt）</label>
              <textarea
                value={job.steps_prompt}
                onChange={(e) => updateJob(index, { steps_prompt: e.target.value })}
                placeholder="分步说明 Agent 应如何执行（可写约束、工具使用顺序等）"
                rows={4}
                className={`${inputCls} resize-y min-h-[5rem]`}
              />
            </div>

            <div>
              <label className={labelCls}>补充说明（可选）</label>
              <textarea
                value={job.message}
                onChange={(e) => updateJob(index, { message: e.target.value })}
                placeholder="附加在「目标 / 步骤」之后的说明；若仅填此项，则与旧版单字段 message 行为一致"
                rows={2}
                className={`${inputCls} resize-y min-h-[2.5rem]`}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
