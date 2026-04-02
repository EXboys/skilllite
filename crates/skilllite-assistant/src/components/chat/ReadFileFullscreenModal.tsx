import { useEffect, useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n";
import { readFileResultLooksTruncated } from "../../utils/readFileToolMeta";

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
  const [draft, setDraft] = useState(initialPlainBody);
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const truncated = readFileResultLooksTruncated(rawResult);
  const canSave = Boolean(sourcePath?.trim()) && !truncated && !saving;

  useEffect(() => {
    if (open) {
      setDraft(initialPlainBody);
      setNotice(null);
    }
  }, [open, initialPlainBody]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    },
    [onClose]
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
        })
      );
    } finally {
      setSaving(false);
    }
  };

  if (!open) return null;

  const title =
    sourcePath?.trim() ||
    t("chat.readFileFullscreenFallbackTitle");

  return createPortal(
    <div
      className="fixed inset-0 z-[300] flex flex-col bg-paper dark:bg-paper-dark text-ink dark:text-ink-dark"
      role="dialog"
      aria-modal="true"
      aria-label={t("chat.readFileFullscreenAria")}
    >
      <header className="shrink-0 flex flex-wrap items-center gap-2 px-4 py-3 border-b border-border dark:border-border-dark bg-white/90 dark:bg-paper-dark/95 backdrop-blur-sm">
        <h2 className="text-sm font-semibold truncate min-w-0 flex-1" title={title}>
          {title}
        </h2>
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
        <div className="flex items-center gap-2 shrink-0">
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
      <textarea
        className="flex-1 min-h-0 w-full resize-none p-4 font-mono text-sm leading-relaxed bg-white dark:bg-black/25 text-ink dark:text-ink-dark border-0 focus:outline-none focus:ring-0"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        spellCheck={false}
      />
    </div>,
    document.body
  );
}
