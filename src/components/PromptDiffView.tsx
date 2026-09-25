import { Fragment, type ReactNode } from "react";
import { useI18n } from "../i18n";

type DiffLineKind = "added" | "removed" | "unchanged";
interface DiffLine {
  kind: DiffLineKind;
  text: string;
}

function computeDiff(original: string, current: string): DiffLine[] {
  const a = original.split("\n");
  const b = current.split("\n");
  const m = a.length,
    n = b.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
  for (let i = 1; i <= m; i++)
    for (let j = 1; j <= n; j++)
      dp[i][j] =
        a[i - 1] === b[j - 1]
          ? dp[i - 1][j - 1] + 1
          : Math.max(dp[i - 1][j], dp[i][j - 1]);
  const result: DiffLine[] = [];
  let i = m,
    j = n;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && a[i - 1] === b[j - 1]) {
      result.push({ kind: "unchanged", text: a[i - 1] });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      result.push({ kind: "added", text: b[j - 1] });
      j--;
    } else {
      result.push({ kind: "removed", text: a[i - 1] });
      i--;
    }
  }
  return result.reverse();
}

const cellBase =
  "px-2 py-0.5 text-[11px] font-mono leading-relaxed break-words whitespace-pre-wrap border-b border-ink/[0.06] dark:border-white/[0.08] align-top";

export function PromptDiffView({
  original,
  current,
}: {
  original: string;
  current: string;
}) {
  const { t } = useI18n();
  const lines = computeDiff(original, current);
  const addedCount = lines.filter((l) => l.kind === "added").length;
  const removedCount = lines.filter((l) => l.kind === "removed").length;

  if (lines.length === 0) {
    return (
      <div className="p-3 text-[11px] text-ink-mute dark:text-ink-dark-mute italic">
        {t("evolution.diff.sameContent")}
      </div>
    );
  }

  const gridRows: ReactNode[] = lines.map((l, ri) => {
    if (l.kind === "unchanged") {
      return (
        <Fragment key={ri}>
          <div className={`${cellBase} text-ink dark:text-ink-dark`}>
            <span className="mr-1 select-none text-ink-mute/40 dark:text-ink-dark-mute/40"> </span>
            <span>{l.text || "\u00a0"}</span>
          </div>
          <div className={`${cellBase} text-ink dark:text-ink-dark`}>
            <span className="mr-1 select-none text-ink-mute/40 dark:text-ink-dark-mute/40"> </span>
            <span>{l.text || "\u00a0"}</span>
          </div>
        </Fragment>
      );
    }
    if (l.kind === "removed") {
      return (
        <Fragment key={ri}>
          <div
            className={`${cellBase} bg-red-100/90 dark:bg-red-900/25 text-red-800 dark:text-red-300/90 line-through border-l-2 border-red-400 dark:border-red-600/60`}
          >
            <span className="mr-1 select-none text-red-500/80 dark:text-red-400/80">−</span>
            <span>{l.text || "\u00a0"}</span>
          </div>
          <div
            className={`${cellBase} bg-ink/[0.02] dark:bg-white/[0.02] text-ink-mute/30 dark:text-ink-dark-mute/30`}
          >
            <span className="select-none">{"\u00a0"}</span>
          </div>
        </Fragment>
      );
    }
    return (
      <Fragment key={ri}>
        <div
          className={`${cellBase} bg-ink/[0.02] dark:bg-white/[0.02] text-ink-mute/30 dark:text-ink-dark-mute/30`}
        >
          <span className="select-none">{"\u00a0"}</span>
        </div>
        <div
          className={`${cellBase} bg-green-100/90 dark:bg-green-900/30 text-green-800 dark:text-green-300 border-l-2 border-green-500 dark:border-green-600/50`}
        >
          <span className="mr-1 select-none text-green-600 dark:text-green-400">+</span>
          <span>{l.text || "\u00a0"}</span>
        </div>
      </Fragment>
    );
  });

  return (
    <div className="text-[11px] font-mono leading-relaxed rounded-md border border-border/40 dark:border-border-dark/50 overflow-hidden">
      <div className="flex flex-wrap items-center gap-3 px-3 py-1.5 bg-gray-100/80 dark:bg-surface-dark/50 border-b border-border/40 dark:border-border-dark/40 text-[10px]">
        {addedCount > 0 && (
          <span className="text-green-600 dark:text-green-400">
            {t("evolution.diff.statsAdded", { n: addedCount })}
          </span>
        )}
        {removedCount > 0 && (
          <span className="text-red-500 dark:text-red-400/80">
            {t("evolution.diff.statsRemoved", { n: removedCount })}
          </span>
        )}
        {addedCount === 0 && removedCount === 0 && (
          <span className="text-ink-mute dark:text-ink-dark-mute">{t("evolution.diff.statsNone")}</span>
        )}
        <span className="text-ink-mute/80 dark:text-ink-dark-mute/80">{t("evolution.diff.fullTextHint")}</span>
      </div>
      <div className="grid grid-cols-2 border-b border-border/40 dark:border-border-dark/40 bg-gray-100/60 dark:bg-surface-dark/40 text-[10px] text-ink-mute dark:text-ink-dark-mute">
        <span className="px-2 py-1 border-r border-border/40 dark:border-border-dark/40 font-medium">
          {t("evolution.diff.compareColumnLeft")}
        </span>
        <span className="px-2 py-1 font-medium">{t("evolution.diff.compareColumnRight")}</span>
      </div>
      <div className="max-h-[min(70vh,28rem)] overflow-y-auto overflow-x-hidden">
        <div className="grid grid-cols-2 min-w-0">{gridRows}</div>
      </div>
    </div>
  );
}
