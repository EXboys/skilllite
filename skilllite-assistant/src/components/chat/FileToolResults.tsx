import { useMemo, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";

import { MarkdownContent } from "../shared/MarkdownContent";
import { useIdeFileOpenerStore } from "../../stores/useIdeFileOpenerStore";
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

/** 对话内约 5 行可滚动：text-xs + leading-5 (1.25rem)×5 + 垂直内边距；read_file 与 list_directory 共用 */
const FILE_CHAT_PREVIEW_5L_CLASS =
  "max-h-[calc(1.25rem*5+1rem)] overflow-auto overscroll-y-contain";

const readPreviewClickShell =
  "rounded-lg border cursor-pointer transition-colors hover:border-accent/45 dark:hover:border-accent/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:ring-offset-2 focus-visible:ring-offset-white dark:focus-visible:ring-offset-paper-dark";

function shouldIgnoreReadPreviewClick(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return Boolean(target.closest("a"));
}

function maybeActivateReadPreview(
  e: MouseEvent<HTMLDivElement>,
  openViewer: () => void,
): void {
  if (shouldIgnoreReadPreviewClick(e.target)) return;
  const sel = window.getSelection()?.toString();
  if (sel?.trim()) return;
  openViewer();
}

function onReadPreviewKeyDown(
  e: KeyboardEvent<HTMLDivElement>,
  openViewer: () => void,
): void {
  if (e.key !== "Enter" && e.key !== " ") return;
  e.preventDefault();
  openViewer();
}

function ReadFilePreviewClickable({
  ariaLabel,
  hint,
  borderTone,
  bgTone,
  children,
  openViewer,
}: {
  ariaLabel: string;
  hint: string;
  borderTone: string;
  bgTone: string;
  children: ReactNode;
  openViewer: () => void;
}) {
  return (
    <div className="space-y-1 min-w-0">
      <div
        role="button"
        tabIndex={0}
        aria-label={ariaLabel}
        onClick={(e) => maybeActivateReadPreview(e, openViewer)}
        onKeyDown={(e) => onReadPreviewKeyDown(e, openViewer)}
        className={`${readPreviewClickShell} ${borderTone} ${bgTone} ${FILE_CHAT_PREVIEW_5L_CLASS}`}
      >
        {children}
      </div>
      <p className="text-[10px] text-ink-mute dark:text-ink-dark-mute leading-snug">
        {hint}
      </p>
    </div>
  );
}

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
  const canOpenInIde = Boolean(sourcePath?.trim());
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

  const openViewer = () => {
    const p = sourcePath?.trim();
    if (p) {
      useIdeFileOpenerStore.getState().openFileFromChat(p);
      return;
    }
    setFullscreen(true);
  };

  if (isBinary) {
    const binaryAria = canOpenInIde
      ? t("chat.readFilePreviewAriaIde")
      : t("chat.readFilePreviewAriaFullscreen");
    const binaryHint = canOpenInIde
      ? t("chat.readFilePreviewHintIde")
      : t("chat.readFilePreviewHintFullscreen");
    return (
      <div className="mt-1.5 min-w-0">
        <ReadFilePreviewClickable
          ariaLabel={binaryAria}
          hint={binaryHint}
          borderTone="border-border/60 dark:border-border-dark/60"
          bgTone="bg-ink/[0.04] dark:bg-white/[0.06]"
          openViewer={openViewer}
        >
          <pre className="m-0 text-xs font-mono whitespace-pre-wrap break-words px-3 py-2 text-ink dark:text-ink-dark leading-relaxed">
            {result.trimEnd()}
          </pre>
        </ReadFilePreviewClickable>
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

  if (parsed.kind === "plain") {
    const previewAria = canOpenInIde
      ? t("chat.readFilePreviewAriaIde")
      : t("chat.readFilePreviewAriaFullscreen");
    const previewHint = canOpenInIde
      ? t("chat.readFilePreviewHintIde")
      : t("chat.readFilePreviewHintFullscreen");
    return (
      <div className="mt-1.5 space-y-2 min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={openViewer}
            className="text-xs px-2.5 py-1 rounded-md border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
          >
            {canOpenInIde ? t("chat.readFileOpenInIde") : t("chat.readFileFullView")}
          </button>
          {truncated && (
            <span className="text-[11px] text-amber-700 dark:text-amber-300">
              {t("chat.readFileTruncatedHint")}
            </span>
          )}
        </div>
        <ReadFilePreviewClickable
          ariaLabel={previewAria}
          hint={previewHint}
          borderTone="border-border/60 dark:border-border-dark/60"
          bgTone="bg-ink/[0.04] dark:bg-white/[0.06]"
          openViewer={openViewer}
        >
          <pre className="m-0 text-xs font-mono whitespace-pre-wrap break-words px-3 py-2 text-ink dark:text-ink-dark leading-relaxed">
            {result.trimEnd()}
          </pre>
        </ReadFilePreviewClickable>
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
        onClick={openViewer}
        className="text-xs px-2.5 py-1 rounded-md border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
      >
        {canOpenInIde ? t("chat.readFileOpenInIde") : t("chat.readFileFullView")}
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
    const mdAria = canOpenInIde
      ? t("chat.readFilePreviewAriaIde")
      : t("chat.readFilePreviewAriaFullscreen");
    const mdHint = canOpenInIde
      ? t("chat.readFilePreviewHintIde")
      : t("chat.readFilePreviewHintFullscreen");
    return (
      <div className="mt-1.5 space-y-2 min-w-0">
        {toolbar}
        <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
          第 {startLine}–{endLine} 行（共 {lineCount} 行）· Markdown 预览
        </p>
        <ReadFilePreviewClickable
          ariaLabel={mdAria}
          hint={mdHint}
          borderTone="border-border/70 dark:border-border-dark/70"
          bgTone="bg-white/80 dark:bg-black/20"
          openViewer={openViewer}
        >
          <div className="px-3 py-2 min-w-0">
            <MarkdownContent content={body} />
          </div>
        </ReadFilePreviewClickable>
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

  const codeAria = canOpenInIde
    ? t("chat.readFilePreviewAriaIde")
    : t("chat.readFilePreviewAriaFullscreen");
  const codeHint = canOpenInIde
    ? t("chat.readFilePreviewHintIde")
    : t("chat.readFilePreviewHintFullscreen");

  return (
    <div className="mt-1.5 space-y-2 min-w-0">
      {toolbar}
      <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
        第 {startLine}–{endLine} 行（共 {lineCount} 行）·{" "}
        {hljsTooLarge ? t("chat.readFilePlainPreview") : t("chat.readFileSyntaxHighlight")}
      </p>
      <ReadFilePreviewClickable
        ariaLabel={codeAria}
        hint={codeHint}
        borderTone="border-border/70 dark:border-border-dark/70"
        bgTone="bg-[#0d1117]"
        openViewer={openViewer}
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
      </ReadFilePreviewClickable>
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
  const { t } = useI18n();
  return (
    <div className="mt-1.5 min-w-0">
      <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute mb-1.5">
        目录树（ASCII）
      </p>
      <pre
        className={`text-xs font-mono leading-snug whitespace-pre rounded-lg border border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06] px-3 py-2.5 text-ink dark:text-ink-dark shadow-inner shadow-ink/[0.03] ${FILE_CHAT_PREVIEW_5L_CLASS}`}
      >
        {result.trimEnd()}
      </pre>
      <p className="text-[10px] text-ink-mute dark:text-ink-dark-mute mt-1 leading-snug">
        {t("chat.listDirectoryPreviewHint")}
      </p>
    </div>
  );
}
