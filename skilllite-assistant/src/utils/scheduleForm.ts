export type ScheduleTriggerMode = "interval" | "daily" | "once";

export interface ScheduleJobForm {
  id: string;
  enabled: boolean;
  trigger_mode: ScheduleTriggerMode;
  interval_seconds: number;
  /** Local HH:MM slots when `trigger_mode === "daily"` (at least one). */
  daily_times: string[];
  once_at: string;
  session_key: string;
  goal: string;
  steps_prompt: string;
  message: string;
}

export interface ScheduleForm {
  version: number;
  enabled: boolean;
  limits: {
    max_runs_per_day: number;
    min_interval_seconds_between_runs: number;
  };
  jobs: ScheduleJobForm[];
}

export function emptyScheduleForm(): ScheduleForm {
  return {
    version: 1,
    enabled: true,
    limits: {
      max_runs_per_day: 8,
      min_interval_seconds_between_runs: 0,
    },
    jobs: [],
  };
}

function trimOpt(s: unknown): string {
  return typeof s === "string" ? s.trim() : "";
}

/** `HH:MM` from `<input type="time">` may be `HH:MM:SS`. */
export function normalizeDailyTime(s: string): string {
  const t = s.trim();
  const m = t.match(/^(\d{1,2}):(\d{2})(?::\d{2})?$/);
  if (!m) return t;
  const h = Number(m[1]);
  const min = Number(m[2]);
  if (!Number.isFinite(h) || !Number.isFinite(min) || h > 23 || min > 59) return t;
  return `${h}:${min.toString().padStart(2, "0")}`;
}

export function validateHhmm(s: string): boolean {
  const t = normalizeDailyTime(s);
  return /^([0-1]?\d|2[0-3]):[0-5]\d$/.test(t);
}

export function validateOnceAtLocal(s: string): boolean {
  const t = s.trim();
  return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(t);
}

export function normalizeSchedule(raw: unknown): ScheduleForm {
  const j = raw as Record<string, unknown>;
  const limits = (j.limits as Record<string, unknown>) || {};
  const jobsRaw = Array.isArray(j.jobs) ? j.jobs : [];
  return {
    version: typeof j.version === "number" ? j.version : 1,
    enabled: j.enabled !== false,
    limits: {
      max_runs_per_day:
        typeof limits.max_runs_per_day === "number" ? limits.max_runs_per_day : 8,
      min_interval_seconds_between_runs:
        typeof limits.min_interval_seconds_between_runs === "number"
          ? limits.min_interval_seconds_between_runs
          : 0,
    },
    jobs: jobsRaw.map((job: Record<string, unknown>) => {
      const onceAt = trimOpt(job.once_at);
      const fromArr = Array.isArray(job.daily_times)
        ? (job.daily_times as unknown[])
            .filter((x): x is string => typeof x === "string")
            .map((x) => normalizeDailyTime(x))
            .filter((x) => x.length > 0)
        : [];
      const dailyAtLegacy = trimOpt(job.daily_at);
      let daily_times = fromArr;
      if (daily_times.length === 0 && dailyAtLegacy) {
        daily_times = [normalizeDailyTime(dailyAtLegacy)];
      }
      let trigger_mode: ScheduleTriggerMode = "interval";
      if (onceAt) trigger_mode = "once";
      else if (daily_times.length > 0) trigger_mode = "daily";
      return {
        id: typeof job.id === "string" ? job.id : "",
        enabled: job.enabled !== false,
        trigger_mode,
        interval_seconds:
          typeof job.interval_seconds === "number" && job.interval_seconds >= 1
            ? job.interval_seconds
            : 3600,
        daily_times: trigger_mode === "daily" ? daily_times : [],
        once_at: onceAt,
        session_key: typeof job.session_key === "string" ? job.session_key : "",
        goal: typeof job.goal === "string" ? job.goal : "",
        steps_prompt: typeof job.steps_prompt === "string" ? job.steps_prompt : "",
        message: typeof job.message === "string" ? job.message : "",
      };
    }),
  };
}

export function parseScheduleJson(json: string): { ok: true; data: ScheduleForm } | { ok: false; error: string } {
  try {
    const parsed = JSON.parse(json) as unknown;
    return { ok: true, data: normalizeSchedule(parsed) };
  } catch {
    return { ok: false, error: "JSON 格式无效" };
  }
}

/** Serialize for `skilllite_write_schedule`. */
export function scheduleFormToJson(data: ScheduleForm): string {
  const jobs = data.jobs.map((j) => {
    const row: Record<string, unknown> = {
      id: j.id.trim(),
      enabled: j.enabled,
      interval_seconds: j.interval_seconds,
    };
    const sk = j.session_key.trim();
    if (sk) row.session_key = sk;
    const g = j.goal.trim();
    if (g) row.goal = g;
    const sp = j.steps_prompt.trim();
    if (sp) row.steps_prompt = sp;
    row.message = j.message;

    if (j.trigger_mode === "once") {
      const o = j.once_at.trim();
      if (o) row.once_at = o;
    } else if (j.trigger_mode === "daily") {
      const times = j.daily_times.map(normalizeDailyTime).filter((t) => t.length > 0);
      if (times.length === 0) {
        row.daily_at = "09:00";
      } else if (times.length === 1) {
        row.daily_at = times[0];
      } else {
        row.daily_times = times;
      }
    }
    return row;
  });
  const out = {
    version: data.version,
    enabled: data.enabled,
    limits: data.limits,
    jobs,
  };
  return JSON.stringify(out, null, 2);
}

export function validateScheduleForm(data: ScheduleForm): string | null {
  for (let i = 0; i < data.jobs.length; i++) {
    const j = data.jobs[i];
    if (!j.id.trim()) {
      return `第 ${i + 1} 个任务缺少「任务 ID」`;
    }
    if (j.trigger_mode === "interval") {
      if (!Number.isFinite(j.interval_seconds) || j.interval_seconds < 1) {
        return `任务「${j.id}」的运行间隔（秒）须为 ≥1 的整数`;
      }
    } else if (j.trigger_mode === "daily") {
      if (j.daily_times.length < 1) {
        return `任务「${j.id}」请至少保留一个「每天」执行时刻`;
      }
      for (let k = 0; k < j.daily_times.length; k++) {
        if (!validateHhmm(j.daily_times[k])) {
          return `任务「${j.id}」第 ${k + 1} 个时刻格式须为 HH:MM（24 小时制）`;
        }
      }
    } else if (j.trigger_mode === "once") {
      if (!validateOnceAtLocal(j.once_at)) {
        return `任务「${j.id}」的「仅一次」请使用本地日期时间 YYYY-MM-DDTHH:MM`;
      }
    }
    const g = j.goal.trim();
    const sp = j.steps_prompt.trim();
    const m = j.message.trim();
    if (!g && !sp && !m) {
      return `任务「${j.id}」请至少填写「目标」「执行步骤」或「补充说明」之一`;
    }
  }
  return null;
}

export const INTERVAL_PRESETS: { label: string; seconds: number }[] = [
  { label: "5 分钟", seconds: 300 },
  { label: "1 小时", seconds: 3600 },
  { label: "6 小时", seconds: 21_600 },
  { label: "24 小时", seconds: 86_400 },
];
