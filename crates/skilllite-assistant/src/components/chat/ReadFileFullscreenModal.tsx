import {
  useEffect,
  useState,
  useCallback,
  useMemo,
  useSyncExternalStore,
} from "react";
import { useElementHeightPx } from "../../hooks/useElementHeightPx";
import { createPortal } from "react-dom";
import CodeMirror from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n";
import { readFileResultLooksTruncated } from "../../utils/readFileToolMeta";
import {
  highlightCodeToHtml,
  preferMarkdownPreview,
  READ_FILE_FULLSCREEN_PREVIEW_MAX,
} from "../../utils/readFileHljs";
import {
  readFileCodeMirrorLanguage,
  readFileCodeMirrorTheme,
} from "../../utils/readFileCodeMirror";
import { MarkdownContent } from "../shared/MarkdownContent";

function usePrefersDarkMedia(): boolean {
  return useSyncExternalStore(
    (onStoreChange) => {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      mq.addEventListener("change", onStoreChange);
      return () => mq.removeEventListener("change", onStoreChange);
    },
    () => window.matchMedia("(prefers-color-scheme: dark)").matches,
    () => false,
  );
}

type ViewMode = "edit" | "preview";

interface ReadFileFullscreenModalProps {
  open: boolean;
  onClose: () => void;
  /** Joined line bodies (no `N|` prefixes). */
  initialPlainBody: string;
  /** Raw tool result (detect truncation). */
  rawResult: string;
  sourcePath?: string;
  workspace: string;
}

export function ReadFileFullscreenModal({
  open,
  onClose,
  initialPlainBody,
  rawResult,
  sourcePath,
  workspace,
}: ReadFileFullscreenModalProps) {
  const { t } = useI18n();
  const prefersDark = usePrefersDarkMedia();
  const [draft, setDraft] = useState(initialPlainBody);
  const [viewMode, setViewMode] = useState<ViewMode>("edit");
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const codeMirrorExtensions = useMemo(
    () => [
      ...readFileCodeMirrorTheme(prefersDark),
      ...readFileCodeMirrorLanguage(sourcePath, initialPlainBody),
      EditorView.lineWrapping,
    ],
    [prefersDark, sourcePath, initialPlainBody],
  );

  const truncated = readFileResultLooksTruncated(rawResult);
  const canSave = Boolean(sourcePath?.trim()) && !truncated && !saving;

  const markdownPreview = useMemo(
    () => preferMarkdownPreview(draft, sourcePath),
    [draft, sourcePath],
  );

  const editMirrorActive = open && viewMode === "edit";
  const { ref: editMirrorHostRef, heightPx: editMirrorHeightPx } = useElementHeightPx(
    editMirrorActive,
    220,
  );

  const codePreviewHtml = useMemo(() => {
    if (markdownPreview) return "";
    const slice =
      draft.length > READ_FILE_FULLSCREEN_PREVIEW_MAX
        ? draft.slice(0, READ_FILE_FULLSCREEN_PREVIEW_MAX)
        : draft;
    return highlightCodeToHtml(slice);
  }, [draft, markdownPreview]);

  useEffect(() => {
    if (open) {
      setDraft(initialPlainBody);
      setNotice(null);
      setViewMode("edit");
    }
  }, [open, initialPlainBody]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    },
    [onClose],
  );

  useEffect(() => {
    if (!open) return;
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, handleKeyDown]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(draft);
      setNotice(t("chat.readFileCopied"));
    } catch {
      setNotice(t("chat.readFileCopyFailed"));
    }
  };

  const save = async () => {
    if (!sourcePath?.trim() || truncated) return;
    setSaving(true);
    setNotice(null);
    try {
      await invoke("skilllite_write_workspace_file", {
        workspace: workspace || ".",
        relativePath: sourcePath.trim(),
        content: draft,
      });
      setNotice(t("chat.readFileSaveOk"));
    } catch (e) {
      setNotice(
        t("chat.readFileSaveErr", {
          msg: e instanceof Error ? e.message : String(e),
        }),
      );
    } finally {
      setSaving(false);
    }
  };

  if (!open) return null;

  const title =
    sourcePath?.trim() ||
    t("chat.readFileFullscreenFallbackTitle");

  const tabBtn = (mode: ViewMode, label: string) => (
    <button
      type="button"
      onClick={() => setViewMode(mode)}
      className={`px-3 py-1.5 text-xs rounded-md transition-colors ${
        viewMode === mode
          ? "bg-accent/15 text-accent dark:text-blue-300 font-medium"
          : "text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
      }`}
    >
      {label}
    </button>
  );

  return createPortal(
    <div
      className="fixed inset-0 z-[300] flex flex-col bg-paper dark:bg-paper-dark text-ink dark:text-ink-dark"
      role="dialog"
      aria-modal="true"
      aria-label={t("chat.readFileFullscreenAria")}
    >
      <header className="shrink-0 flex flex-wrap items-center gap-2 px-4 py-3 border-b border-border dark:border-border-dark bg-white/90 dark:bg-paper-dark/95 backdrop-blur-sm">
        <h2 className="text-sm font-semibold truncate min-w-0 flex-1 basis-full sm:basis-auto" title={title}>
          {title}
        </h2>
        <div className="flex items-center rounded-lg border border-border/70 dark:border-border-dark/70 p-0.5 bg-ink/[0.02] dark:bg-white/[0.04]">
          {tabBtn("edit", t("chat.readFileTabEdit"))}
          {tabBtn("preview", t("chat.readFileTabPreview"))}
        </div>
        {truncated && (
          <span className="text-xs text-amber-700 dark:text-amber-300 shrink-0">
            {t("chat.readFileTruncatedHint")}
          </span>
        )}
        {!sourcePath?.trim() && (
          <span className="text-xs text-ink-mute dark:text-ink-dark-mute shrink-0">
            {t("chat.readFileNoPathHint")}
          </span>
        )}
        <div className="flex items-center gap-2 shrink-0 ml-auto">
          <button
            type="button"
            onClick={() => void copy()}
            className="px-3 py-1.5 text-xs rounded-lg border border-border dark:border-border-dark hover:bg-ink/5 dark:hover:bg-white/5"
          >
            {t("chat.readFileCopy")}
          </button>
          <button
            type="button"
            disabled={!canSave}
            onClick={() => void save()}
            className="px-3 py-1.5 text-xs rounded-lg bg-accent text-white font-medium hover:bg-accent-hover disabled:opacity-40 disabled:pointer-events-none"
          >
            {t("chat.readFileSave")}
          </button>
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 text-xs rounded-lg border border-border dark:border-border-dark hover:bg-ink/5 dark:hover:bg-white/5"
          >
            {t("chat.readFileClose")}
          </button>
        </div>
      </header>
      {notice && (
        <div className="shrink-0 px-4 py-2 text-xs border-b border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06]">
          {notice}
        </div>
      )}
      {viewMode === "edit" ? (
        <div
          ref={editMirrorHostRef}
          className="flex-1 min-h-0 min-w-0 flex flex-col overflow-hidden px-2 pb-2 pt-1"
        >
          <CodeMirror
            key={`readfile-fs-${open}-${viewMode}`}
            value={draft}
            height={`${editMirrorHeightPx}px`}
            theme="none"
            extensions={codeMirrorExtensions}
            onChange={(v) => setDraft(v)}
            basicSetup={{
              lineNumbers: true,
              foldGutter: true,
              highlightSelectionMatches: true,
            }}
            className="w-full min-h-0 overflow-hidden text-sm [&_.cm-editor]:outline-none [&_.cm-scroller]:overflow-auto"
            indentWithTab
            spellCheck={false}
          />
        </div>
      ) : (
        <div className="flex-1 min-h-0 overflow-auto p-4 bg-ink/[0.02] dark:bg-white/[0.03]">
          {markdownPreview ? (
            <div className="rounded-lg border border-border/70 dark:border-border-dark/70 bg-white/90 dark:bg-black/20 px-4 py-3 text-sm text-ink dark:text-ink-dark leading-relaxed">
              <MarkdownContent content={draft} />
            </div>
          ) : draft.length > READ_FILE_FULLSCREEN_PREVIEW_MAX ? (
            <div className="space-y-2">
              <p className="text-xs text-amber-800 dark:text-amber-200">
                {t("chat.readFilePreviewCodeLimited", {
                  n: READ_FILE_FULLSCREEN_PREVIEW_MAX,
                })}
              </p>
              <pre className="m-0 p-4 rounded-lg border border-border/70 dark:border-border-dark/70 bg-[#0d1117] text-sm text-[#e6edf3] font-mono whitespace-pre-wrap break-words overflow-x-auto">
                {draft.slice(0, READ_FILE_FULLSCREEN_PREVIEW_MAX)}
              </pre>
            </div>
          ) : (
            <pre className="m-0 p-4 rounded-lg border border-border/70 dark:border-border-dark/70 bg-[#0d1117] overflow-x-auto">
              <code
                className="hljs text-sm leading-relaxed"
                dangerouslySetInnerHTML={{ __html: codePreviewHtml }}
              />
            </pre>
          )}
        </div>
      )}
    </div>,
    document.body,
  );
}
