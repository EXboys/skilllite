import { memo, useMemo, useState } from "react";

function tryParseJson(s: string): unknown {
  const t = s.trim();
  if (!t || (t[0] !== "{" && t[0] !== "[")) return null;
  try {
    return JSON.parse(s) as unknown;
  } catch {
    return null;
  }
}

function isPrimitive(v: unknown): v is string | number | boolean | null {
  return v === null || typeof v === "string" || typeof v === "number" || typeof v === "boolean";
}

/** 常见天气/技能扁平 JSON 字段 → 列表展示用中文标签 */
const FLAT_JSON_KEY_LABELS: Record<string, string> = {
  city: "城市",
  temperature: "温度",
  humidity: "湿度",
  weather: "天气",
  high: "最高温",
  low: "最低温",
  wind: "风向风力",
  air_quality: "空气质量",
  tip: "小贴士",
  update_time: "更新时间",
  source: "来源",
  success: "成功",
};

function labelFlatJsonKey(k: string): string {
  return FLAT_JSON_KEY_LABELS[k] ?? k.replace(/_/g, " ");
}

function parseFlatPrimitiveEntries(raw: string): [string, unknown][] | null {
  const parsed = tryParseJson(raw.trim());
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return null;
  const obj = parsed as Record<string, unknown>;
  const keys = Object.keys(obj);
  if (keys.length === 0 || keys.length > 32) return null;
  const entries = keys.map((k): [string, unknown] => [k, obj[k]]);
  if (!entries.every(([, v]) => isPrimitive(v))) return null;
  return entries;
}

/**
 * 若应使用与助手一致的气泡展示，返回 Markdown 正文；否则返回 null（走「工具结果」结构化视图）。
 * 覆盖：自然语言长文、仅含原始类型的扁平对象（如天气 JSON）。
 */
export function getConversationToolResultMarkdown(raw: string, isError: boolean): string | null {
  if (isError) return null;
  const t = raw.trim();
  if (!t) return null;

  const flat = parseFlatPrimitiveEntries(t);
  if (flat) {
    return flat
      .map(([k, v]) => {
        const label = labelFlatJsonKey(k);
        const val = v === null ? "—" : String(v);
        return `- **${label}**：${val}`;
      })
      .join("\n");
  }

  if (t.length < 48) return null;
  const c = t[0];
  if (c === "{" || c === "[") return null;
  if (tryParseJson(t) !== null) return null;
  return t;
}

const COLLAPSE_LEN = 360;

/** Flat JSON objects → readable key/value rows; otherwise collapsible formatted or raw text. */
export const StructuredPayload = memo(function StructuredPayload({ raw }: { raw: string }) {
  const [expanded, setExpanded] = useState(false);

  const parsed = useMemo(() => tryParseJson(raw), [raw]);

  const tableRows = useMemo(() => {
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return null;
    const obj = parsed as Record<string, unknown>;
    const keys = Object.keys(obj);
    if (keys.length === 0 || keys.length > 32) return null;
    const entries = keys.map((k): [string, unknown] => [k, obj[k]]);
    const allPrimitive = entries.every(([, v]) => isPrimitive(v));
    if (!allPrimitive) return null;
    return entries;
  }, [parsed]);

  if (tableRows) {
    return (
      <div className="mt-1.5 rounded-lg border border-border/70 dark:border-border-dark/70 bg-white/40 dark:bg-black/15 overflow-hidden">
        <dl className="divide-y divide-border/50 dark:divide-border-dark/40">
          {tableRows.map(([k, v]) => (
            <div
              key={k}
              className="grid grid-cols-[minmax(0,6.5rem)_1fr] sm:grid-cols-[minmax(0,8rem)_1fr] gap-x-2 gap-y-0 px-2.5 py-1.5 items-baseline"
            >
              <dt className="text-[11px] sm:text-xs font-medium text-ink-mute dark:text-ink-dark-mute truncate" title={k}>
                {k}
              </dt>
              <dd className="text-xs sm:text-sm text-ink dark:text-ink-dark break-words tabular-nums">{String(v)}</dd>
            </div>
          ))}
        </dl>
      </div>
    );
  }

  const pretty = useMemo(() => {
    if (parsed === null) return null;
    try {
      return JSON.stringify(parsed, null, 2);
    } catch {
      return null;
    }
  }, [parsed]);

  const display = pretty ?? raw;
  const long = display.length > COLLAPSE_LEN;
  const shown = !long || expanded ? display : display.slice(0, COLLAPSE_LEN) + "…";

  return (
    <div className="mt-1.5">
      <pre className="text-[11px] sm:text-xs overflow-x-auto whitespace-pre-wrap break-words rounded-lg border border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06] px-2.5 py-2 max-h-48 overflow-y-auto text-ink-mute dark:text-ink-dark-mute font-mono leading-relaxed">
        {shown}
      </pre>
      {long && (
        <button
          type="button"
          onClick={() => setExpanded((e) => !e)}
          className="mt-1 text-xs text-accent dark:text-blue-300 hover:underline"
        >
          {expanded ? "收起" : "展开完整内容"}
        </button>
      )}
    </div>
  );
});
