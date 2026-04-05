import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { useElementHeightPx } from "../hooks/useElementHeightPx";
import { EditorView } from "@codemirror/view";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import {
  readFileCodeMirrorLanguage,
  readFileCodeMirrorTheme,
} from "../utils/readFileCodeMirror";

function usePrefersDarkMedia(): boolean {
  return useSyncExternalStore(
    (onStoreChange) => {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      mq.addEventListener("change", onStoreChange);
      return () => mq.removeEventListener("change", onStoreChange);
    },
    () => window.matchMedia("(prefers-color-scheme: dark)").matches,
    () => false
  );
}

interface WorkspaceIdeEditorProps {
  workspace: string;
  relativePath: string | null;
  onSaved?: () => void;
}

export default function WorkspaceIdeEditor({
  workspace,
  relativePath,
  onSaved,
}: WorkspaceIdeEditorProps) {
  const { t } = useI18n();
  const prefersDark = usePrefersDarkMedia();
  const [baseline, setBaseline] = useState("");
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);

  const ws = workspace.trim() || ".";

  useEffect(() => {
    setNotice(null);
    setLoadErr(null);
    if (!relativePath?.trim()) {
      setBaseline("");
      setDraft("");
      setLoading(false);
      return;
    }
    const path = relativePath.trim();
    let cancelled = false;
    setDraft("");
    setBaseline("");
    setLoading(true);
    void (async () => {
      try {
        const text = await invoke<string>("skilllite_read_workspace_file", {
          workspace: ws,
          relativePath: path,
        });
        if (cancelled) return;
        setBaseline(text);
        setDraft(text);
        setLoadErr(null);
      } catch (e) {
        if (cancelled) return;
        setBaseline("");
        setDraft("");
        setLoadErr(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [relativePath, ws]);

  const dirty = relativePath?.trim() ? draft !== baseline : false;

  const ideMirrorActive = Boolean(relativePath?.trim()) && !loadErr;
  const { ref: ideMirrorHostRef, heightPx: ideMirrorHeightPx } = useElementHeightPx(
    ideMirrorActive,
    200,
  );

  const codeMirrorExtensions = useMemo(
    () => [
      ...readFileCodeMirrorTheme(prefersDark),
      ...readFileCodeMirrorLanguage(relativePath ?? undefined, draft),
      EditorView.lineWrapping,
    ],
    [prefersDark, relativePath, draft]
  );

  const save = useCallback(async () => {
    const path = relativePath?.trim();
    if (!path || !dirty) return;
    setNotice(null);
    try {
      await invoke("skilllite_write_workspace_file", {
        workspace: ws,
        relativePath: path,
        content: draft,
      });
      setBaseline(draft);
      setNotice(t("ide.saveOk"));
      onSaved?.();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : String(e));
    }
  }, [relativePath, dirty, draft, ws, onSaved, t]);

  if (!relativePath?.trim()) {
    return (
      <div className="h-full min-h-0 flex flex-col items-center justify-center text-center px-6 text-sm text-ink-mute dark:text-ink-dark-mute border-l border-r border-border dark:border-border-dark bg-white/40 dark:bg-paper-dark/40">
        <p className="max-w-sm leading-relaxed">{t("ide.editorPlaceholder")}</p>
        <p className="mt-3 text-[11px] opacity-80">{t("ide.editorHintToggleIde")}</p>
      </div>
    );
  }

  return (
    <div className="h-full min-h-0 flex flex-col bg-white dark:bg-paper-dark border-l border-r border-border dark:border-border-dark">
      <header className="shrink-0 flex flex-wrap items-center gap-2 px-3 py-2 border-b border-border dark:border-border-dark">
        <h2
          className="text-xs font-semibold font-mono truncate min-w-0 flex-1 basis-[60%]"
          title={relativePath ?? undefined}
        >
          {relativePath}
        </h2>
        {loading ? (
          <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute">{t("common.loading")}</span>
        ) : null}
        <button
          type="button"
          disabled={!dirty || loading || Boolean(loadErr)}
          onClick={() => void save()}
          className="px-2.5 py-1 text-xs rounded-lg bg-accent text-white font-medium hover:bg-accent-hover disabled:opacity-40 disabled:pointer-events-none"
        >
          {t("common.save")}
        </button>
      </header>
      {loadErr ? (
        <div className="shrink-0 px-3 py-2 text-xs text-red-600 dark:text-red-400 border-b border-border/60 dark:border-border-dark/60">
          {loadErr}
        </div>
      ) : null}
      {notice ? (
        <div className="shrink-0 px-3 py-1.5 text-[11px] border-b border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06]">
          {notice}
        </div>
      ) : null}
      <div
        ref={ideMirrorHostRef}
        className="flex-1 min-h-0 min-w-0 overflow-hidden flex flex-col"
      >
        {loadErr ? null : (
          <CodeMirror
            key={relativePath ?? "ide"}
            value={draft}
            height={`${ideMirrorHeightPx}px`}
            theme="none"
            extensions={codeMirrorExtensions}
            onChange={(v) => setDraft(v)}
            editable={!loading}
            basicSetup={{
              lineNumbers: true,
              foldGutter: true,
              highlightSelectionMatches: true,
            }}
            className="w-full min-h-0 overflow-hidden text-sm [&_.cm-editor]:outline-none [&_.cm-scroller]:overflow-auto"
            indentWithTab
            spellCheck={false}
          />
        )}
      </div>
    </div>
  );
}
