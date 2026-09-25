import { useState } from "react";
import { useI18n } from "../i18n";
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
  const { t } = useI18n();
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
            {t("schedule.applyJson")}
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
            {t("common.cancel")}
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
          {t("schedule.enableGlobal")}
        </label>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className={labelCls}>{t("schedule.maxRunsDay")}</label>
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
          <label className={labelCls}>{t("schedule.minInterval")}</label>
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
        <p className="text-xs font-medium text-ink dark:text-ink-dark-mute">{t("schedule.jobList")}</p>
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
            {t("schedule.editRawJson")}
          </button>
          <button
            type="button"
            onClick={() => {
              setJobs([...data.jobs, newJob()]);
            }}
            className="text-xs px-2 py-1 rounded-md border border-border dark:border-border-dark hover:bg-gray-100 dark:hover:bg-white/5"
          >
            {t("schedule.addJob")}
          </button>
        </div>
      </div>

      {data.jobs.length === 0 && (
        <p className="text-xs text-ink-mute dark:text-ink-dark-mute py-2">
          {t("schedule.noJobs")}
        </p>
      )}

      <div className="space-y-4">
        {data.jobs.map((job, index) => (
          <div
            key={index}
            className="rounded-lg border border-border dark:border-border-dark p-3 space-y-3 bg-gray-50/80 dark:bg-surface-dark/40"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="text-xs font-medium text-ink dark:text-ink-dark">
                {t("schedule.jobN", { n: index + 1 })}
              </span>
              <div className="flex items-center gap-2">
                <label className="flex items-center gap-1.5 text-[11px] text-ink-mute dark:text-ink-dark-mute cursor-pointer">
                  <input
                    type="checkbox"
                    checked={job.enabled}
                    onChange={(e) => updateJob(index, { enabled: e.target.checked })}
                    className="rounded border-border dark:border-border-dark"
                  />
                  {t("schedule.enable")}
                </label>
                <button
                  type="button"
                  onClick={() => setJobs(data.jobs.filter((_, i) => i !== index))}
                  className="text-[11px] text-red-600 dark:text-red-400 hover:underline"
                >
                  {t("schedule.remove")}
                </button>
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div>
                <label className={labelCls}>{t("schedule.jobId")}</label>
                <input
                  type="text"
                  value={job.id}
                  onChange={(e) => updateJob(index, { id: e.target.value })}
                  placeholder={t("schedule.jobIdPh")}
                  className={inputCls}
                />
              </div>
              <div>
                <label className={labelCls}>{t("schedule.sessionKey")}</label>
                <input
                  type="text"
                  value={job.session_key}
                  onChange={(e) => updateJob(index, { session_key: e.target.value })}
                  placeholder={t("schedule.sessionKeyPh")}
                  className={inputCls}
                />
              </div>
            </div>

            <div>
              <label className={labelCls}>{t("schedule.trigger")}</label>
              <div className="flex rounded-lg border border-border dark:border-border-dark overflow-hidden flex-wrap">
                {(
                  [
                    ["interval", t("schedule.mode.interval")] as const,
                    ["daily", t("schedule.mode.daily")] as const,
                    ["once", t("schedule.mode.once")] as const,
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
                {t("schedule.triggerHelp")}
              </p>
            </div>

            {job.trigger_mode === "interval" && (
            <div>
              <label className={labelCls}>{t("schedule.intervalSec")}</label>
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
                    {t(`schedule.preset.${p.seconds}`)}
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
              <label className={labelCls}>{t("schedule.dailyTimes")}</label>
              {job.daily_times.map((hhmm, ti) => (
                <div key={ti} className="flex gap-2 items-center">
                  <input
                    type="time"
                    value={timeInputValue(hhmm)}
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
                    {t("schedule.removeTime")}
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
                {t("schedule.addTime")}
              </button>
            </div>
            )}

            {job.trigger_mode === "once" && (
            <div>
              <label className={labelCls}>{t("schedule.onceAt")}</label>
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
              <label className={labelCls}>{t("schedule.goal")}</label>
              <textarea
                value={job.goal}
                onChange={(e) => updateJob(index, { goal: e.target.value })}
                placeholder={t("schedule.goalPh")}
                rows={2}
                className={`${inputCls} resize-y min-h-[2.5rem]`}
              />
            </div>

            <div>
              <label className={labelCls}>{t("schedule.steps")}</label>
              <textarea
                value={job.steps_prompt}
                onChange={(e) => updateJob(index, { steps_prompt: e.target.value })}
                placeholder={t("schedule.stepsPh")}
                rows={4}
                className={`${inputCls} resize-y min-h-[5rem]`}
              />
            </div>

            <div>
              <label className={labelCls}>{t("schedule.extra")}</label>
              <textarea
                value={job.message}
                onChange={(e) => updateJob(index, { message: e.target.value })}
                placeholder={t("schedule.extraPh")}
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
