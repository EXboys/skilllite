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

export function PromptDiffView({
  original,
  current,
}: {
  original: string;
  current: string;
}) {
  const CONTEXT = 3;
  const lines = computeDiff(original, current);
  const addedCount = lines.filter((l) => l.kind === "added").length;
  const removedCount = lines.filter((l) => l.kind === "removed").length;

  const visibleSet = new Set<number>();
  lines.forEach((l, idx) => {
    if (l.kind !== "unchanged") {
      for (
        let k = Math.max(0, idx - CONTEXT);
        k <= Math.min(lines.length - 1, idx + CONTEXT);
        k++
      ) {
        visibleSet.add(k);
      }
    }
  });

  const rendered: React.ReactNode[] = [];
  let ri = 0;
  while (ri < lines.length) {
    if (visibleSet.has(ri)) {
      const l = lines[ri];
      rendered.push(
        <div
          key={ri}
          className={
            l.kind === "added"
              ? "bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-300 border-l-2 border-green-500 pl-2 -ml-2"
              : l.kind === "removed"
                ? "bg-red-100 dark:bg-red-900/20 text-red-700 dark:text-red-400/70 line-through border-l-2 border-red-400 dark:border-red-700/50 pl-2 -ml-2"
                : "text-ink-mute dark:text-ink-dark-mute"
          }
        >
          {l.kind === "added" && (
            <span className="mr-1 text-green-600 dark:text-green-400 select-none">+</span>
          )}
          {l.kind === "removed" && (
            <span className="mr-1 text-red-500/70 select-none">−</span>
          )}
          {l.kind === "unchanged" && (
            <span className="mr-1 text-ink-mute/50 dark:text-ink-dark-mute/50 select-none">
              {" "}
            </span>
          )}
          <span className="whitespace-pre-wrap break-words">{l.text || "\u00a0"}</span>
        </div>,
      );
      ri++;
    } else {
      let skipped = 0;
      while (ri < lines.length && !visibleSet.has(ri)) {
        skipped++;
        ri++;
      }
      rendered.push(
        <div
          key={`skip-${ri}`}
          className="text-ink-mute/60 dark:text-ink-dark-mute/60 text-[10px] py-0.5 select-none"
        >
          ···{skipped} 行未变···
        </div>,
      );
    }
  }

  if (visibleSet.size === 0) {
    return (
      <div className="p-3 text-[11px] text-ink-mute dark:text-ink-dark-mute italic">
        内容无变化（快照与当前版本相同）
      </div>
    );
  }

  return (
    <div className="text-[11px] font-mono leading-relaxed">
      <div className="flex items-center gap-3 px-3 py-1.5 bg-gray-100/80 dark:bg-surface-dark/50 border-b border-border/40 dark:border-border-dark/40 text-[10px]">
        {addedCount > 0 && (
          <span className="text-green-600 dark:text-green-400">+{addedCount} 新增</span>
        )}
        {removedCount > 0 && (
          <span className="text-red-500 dark:text-red-400/70">−{removedCount} 移除</span>
        )}
        {addedCount === 0 && removedCount === 0 && (
          <span className="text-ink-mute dark:text-ink-dark-mute">无变化</span>
        )}
      </div>
      <div className="p-3 max-h-72 overflow-y-auto space-y-0">{rendered}</div>
    </div>
  );
}
