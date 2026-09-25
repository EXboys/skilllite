import { useState, useCallback } from "react";
import type { TaskItem } from "../../stores/useStatusStore";
import { openDetailWindow } from "../../utils/detailWindow";

interface InputPlanStripProps {
  tasks: TaskItem[];
  /** 宽度等，默认与底栏输入行同宽（w-full） */
  className?: string;
}

/**
 * 依附底栏的子模块：1px 外框，默认展开；仅左上/右上圆角，底边与输入行衔接。
 */
export function InputPlanStrip({ tasks, className = "" }: InputPlanStripProps) {
  const [collapsed, setCollapsed] = useState(false);

  const toggle = useCallback(() => {
    setCollapsed((c) => !c);
  }, []);

  if (tasks.length === 0) return null;

  const done = tasks.filter((t) => t.completed).length;
  const total = tasks.length;

  return (
    <div
      className={[
        "box-border m-0 border border-border dark:border-border-dark",
        "rounded-t-lg rounded-b-none",
        "bg-gray-50 dark:bg-surface-dark",
        "text-ink dark:text-ink-dark",
        "overflow-hidden",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div
        className={[
          "flex items-center min-h-[1.75rem]",
          !collapsed ? "border-b border-border dark:border-border-dark" : "",
        ].join(" ")}
      >
        <button
          type="button"
          onClick={toggle}
          aria-expanded={!collapsed}
          className="flex min-w-0 flex-1 items-center gap-1.5 pl-1.5 pr-1 py-1 text-left hover:bg-ink/[0.03] dark:hover:bg-white/[0.04] transition-colors"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={`shrink-0 text-ink-mute dark:text-ink-dark-mute transition-transform duration-150 ${collapsed ? "" : "rotate-90"}`}
            aria-hidden
          >
            <path d="m9 18 6-6-6-6" />
          </svg>
          <span className="text-[11px] font-semibold tabular-nums leading-none">
            {total} 步
          </span>
          {done > 0 ? (
            <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute truncate leading-none">
              · {done} 已完成
            </span>
          ) : (
            <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute truncate leading-none">
              任务计划
            </span>
          )}
        </button>
        <button
          type="button"
          onClick={() => void openDetailWindow("plan")}
          className="shrink-0 pl-1 pr-1.5 py-1 text-[11px] leading-none text-ink-mute dark:text-ink-dark-mute hover:text-ink dark:hover:text-ink-dark transition-colors"
        >
          查看更多
        </button>
      </div>

      {!collapsed && (
        <ul className="max-h-[min(40vh,12rem)] space-y-0.5 overflow-y-auto pl-1.5 pr-1 py-1.5">
          {tasks.map((t) => (
            <li
              key={t.id}
              className={`flex items-start gap-1.5 text-[11px] leading-snug ${
                t.completed
                  ? "text-ink-mute dark:text-ink-dark-mute line-through"
                  : ""
              }`}
            >
              <span className="mt-0.5 shrink-0 text-accent">{t.completed ? "✓" : "○"}</span>
              <span className="min-w-0">{t.description}</span>
              {t.tool_hint && (
                <span className="shrink-0 text-ink-mute dark:text-ink-dark-mute">
                  [{t.tool_hint}]
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
