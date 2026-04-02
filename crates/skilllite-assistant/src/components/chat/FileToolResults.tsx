import { useMemo, useState } from "react";

import { MarkdownContent } from "../shared/MarkdownContent";
import { parseReadFileToolResult, type ParsedReadFile } from "../../utils/readFileParse";
import { ReadFileFullscreenModal } from "./ReadFileFullscreenModal";
import { useI18n } from "../../i18n";
import {
  highlightCodeToHtml,
  looksLikeMarkdown,
} from "../../utils/readFileHljs";
import {
  plainTextBodyFromReadFileResult,
  readFileResultLooksTruncated,
} from "../../utils/readFileToolMeta";

export type { ParsedReadFile };
export { parseReadFileToolResult };

/** 超大文件避免 highlight.js 阻塞主线程 */
const HLJS_MAX_CHARS = 96_000;
const PREVIEW_MAX_H_CLASS = "max-h-[min(70vh,42rem)]";

/** read_file 成功结果：避免整段被当作 Markdown 误解析；Markdown 文件渲染，其余语法高亮 */
export function ReadFileToolResultView({
  result,
  sourcePath,
  workspace = ".",
}: {
  result: string;
  sourcePath?: string;
  workspace?: string;
}) {
  const { t } = useI18n();
  const [fullscreen, setFullscreen] = useState(false);
  const trimmed = result.trim();
  const isBinary = trimmed.startsWith("[Binary file");
  const truncated = readFileResultLooksTruncated(result);

  const parsed = useMemo(() => parseReadFileToolResult(result), [result]);

  const body = useMemo(() => {
    if (parsed.kind !== "lines") {
      return "";
    }
    return parsed.lines.map((l) => l.text).join("\n");
  }, [parsed]);

  const plainForEdit = useMemo(() => plainTextBodyFromReadFileResult(result), [result]);

  const isMarkdownBody = useMemo(
    () => body.length > 0 && looksLikeMarkdown(body),
    [body],
  );

  const highlighted = useMemo(() => {
    if (!body || isMarkdownBody || body.length > HLJS_MAX_CHARS) {
      return "";
    }
    return highlightCodeToHtml(body);
  }, [body, isMarkdownBody]);

  const hljsTooLarge = body.length > HLJS_MAX_CHARS && !isMarkdownBody && body.length > 0;

  const openFull = () => setFullscreen(true);

  if (isBinary) {
    return (
      <pre className="mt-1.5 text-xs font-mono whitespace-pre-wrap break-words rounded-lg border border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06] px-3 py-2 text-ink dark:text-ink-dark max-h-48 overflow-auto">
        {result.trimEnd()}
      </pre>
    );
  }

  if (parsed.kind === "plain") {
    return (
      <div className="mt-1.5 space-y-2 min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={openFull}
            className="text-xs px-2.5 py-1 rounded-md border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
          >
            {t("chat.readFileFullView")}
          </button>
          {truncated && (
            <span className="text-[11px] text-amber-700 dark:text-amber-300">
              {t("chat.readFileTruncatedHint")}
            </span>
          )}
        </div>
        <pre
          className={`text-xs font-mono whitespace-pre-wrap break-words rounded-lg border border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06] px-3 py-2 text-ink dark:text-ink-dark overflow-auto leading-relaxed ${PREVIEW_MAX_H_CLASS}`}
        >
          {result.trimEnd()}
        </pre>
        <ReadFileFullscreenModal
          open={fullscreen}
          onClose={() => setFullscreen(false)}
          initialPlainBody={plainForEdit}
          rawResult={result}
          sourcePath={sourcePath}
          workspace={workspace}
        />
      </div>
    );
  }

  const suffix = parsed.suffix.trimEnd();
  const lineCount = parsed.lines.length;
  const startLine = parsed.lines[0]?.n ?? 1;
  const endLine = parsed.lines[parsed.lines.length - 1]?.n ?? startLine;

  const toolbar = (
    <div className="flex flex-wrap items-center gap-2">
      <button
        type="button"
        onClick={openFull}
        className="text-xs px-2.5 py-1 rounded-md border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
      >
        {t("chat.readFileFullView")}
      </button>
      {truncated && (
        <span className="text-[11px] text-amber-700 dark:text-amber-300">
          {t("chat.readFileTruncatedBadge")}
        </span>
      )}
      {hljsTooLarge && (
        <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
          {t("chat.readFileHljsLimited")}
        </span>
      )}
    </div>
  );

  if (isMarkdownBody) {
    return (
      <div className="mt-1.5 space-y-2 min-w-0">
        {toolbar}
        <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
          第 {startLine}–{endLine} 行（共 {lineCount} 行）· Markdown 预览
        </p>
        <div
          className={`overflow-auto rounded-lg border border-border/70 dark:border-border-dark/70 bg-white/80 dark:bg-black/20 px-3 py-2 ${PREVIEW_MAX_H_CLASS}`}
        >
          <MarkdownContent content={body} />
        </div>
        {suffix ? (
          <pre className="text-[11px] font-mono whitespace-pre-wrap text-ink-mute dark:text-ink-dark-mute border-t border-border/50 dark:border-border-dark/50 pt-2">
            {suffix}
          </pre>
        ) : null}
        <ReadFileFullscreenModal
          open={fullscreen}
          onClose={() => setFullscreen(false)}
          initialPlainBody={plainForEdit}
          rawResult={result}
          sourcePath={sourcePath}
          workspace={workspace}
        />
      </div>
    );
  }

  return (
    <div className="mt-1.5 space-y-2 min-w-0">
      {toolbar}
      <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
        第 {startLine}–{endLine} 行（共 {lineCount} 行）·{" "}
        {hljsTooLarge ? t("chat.readFilePlainPreview") : t("chat.readFileSyntaxHighlight")}
      </p>
      <div
        className={`rounded-lg border border-border/70 dark:border-border-dark/70 overflow-y-auto bg-[#0d1117] ${PREVIEW_MAX_H_CLASS}`}
      >
        {hljsTooLarge || !highlighted ? (
          <pre className="m-0 p-3 text-xs leading-5 whitespace-pre text-[#e6edf3] font-mono">
            {body}
          </pre>
        ) : (
          <pre className="m-0 p-3 text-xs leading-5 whitespace-pre">
            <code
              className="hljs"
              dangerouslySetInnerHTML={{ __html: highlighted }}
            />
          </pre>
        )}
      </div>
      {suffix ? (
        <pre className="text-[11px] font-mono whitespace-pre-wrap text-ink-mute dark:text-ink-dark-mute border-t border-border/50 dark:border-border-dark/50 pt-2">
          {suffix}
        </pre>
      ) : null}
      <ReadFileFullscreenModal
        open={fullscreen}
        onClose={() => setFullscreen(false)}
        initialPlainBody={plainForEdit}
        rawResult={result}
        sourcePath={sourcePath}
        workspace={workspace}
      />
    </div>
  );
}

/** list_directory：等宽保留树形结构 */
export function ListDirectoryToolResultView({ result }: { result: string }) {
  return (
    <div className="mt-1.5 min-w-0">
      <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute mb-1.5">
        目录树（ASCII）
      </p>
      <pre className="text-xs font-mono leading-snug whitespace-pre overflow-x-auto max-h-96 rounded-lg border border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06] px-3 py-2.5 text-ink dark:text-ink-dark shadow-inner shadow-ink/[0.03]">
        {result.trimEnd()}
      </pre>
    </div>
  );
}
