import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { useElementHeightPx } from "../hooks/useElementHeightPx";
import { EditorView } from "@codemirror/view";
import { useI18n } from "../i18n";
import {
  readFileCodeMirrorLanguage,
  readFileCodeMirrorTheme,
} from "../utils/readFileCodeMirror";
import { MarkdownContent } from "./shared/MarkdownContent";
import { preferMarkdownPreview } from "../utils/readFileHljs";
import { ideFileKindFromPath, type IdeEditorFileKind } from "../utils/ideFileKind";

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

type ViewMode = "edit" | "preview";

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
  const [viewMode, setViewMode] = useState<ViewMode>("edit");
  const [baseline, setBaseline] = useState("");
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const [mediaSrc, setMediaSrc] = useState<string | null>(null);

  const ws = workspace.trim() || ".";
  const pathTrim = relativePath?.trim() ?? "";
  const fileKind: IdeEditorFileKind | null = pathTrim ? ideFileKindFromPath(pathTrim) : null;
  const isMedia = fileKind === "image" || fileKind === "video";

  useEffect(() => {
    if (!pathTrim) {
      setViewMode("edit");
      return;
    }
    setViewMode(ideFileKindFromPath(pathTrim) === "markdown" ? "preview" : "edit");
  }, [pathTrim]);

  useEffect(() => {
    setNotice(null);
    setLoadErr(null);
    setMediaSrc(null);
    if (!pathTrim) {
      setBaseline("");
      setDraft("");
      setLoading(false);
      return;
    }

    const kind = ideFileKindFromPath(pathTrim);
    let cancelled = false;

    if (kind === "image" || kind === "video") {
      setBaseline("");
      setDraft("");
      setLoading(true);
      void (async () => {
        try {
          const abs = await invoke<string>("skilllite_resolve_workspace_file_path", {
            workspace: ws,
            relativePath: pathTrim,
          });
          if (cancelled) return;
          setMediaSrc(convertFileSrc(abs));
          setLoadErr(null);
        } catch (e) {
          if (cancelled) return;
          setMediaSrc(null);
          setLoadErr(e instanceof Error ? e.message : String(e));
        } finally {
          if (!cancelled) setLoading(false);
        }
      })();
      return () => {
        cancelled = true;
      };
    }

    setBaseline("");
    setDraft("");
    setLoading(true);
    void (async () => {
      try {
        const text = await invoke<string>("skilllite_read_workspace_file", {
          workspace: ws,
          relativePath: pathTrim,
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
  }, [pathTrim, ws]);

  const dirty = pathTrim && !isMedia ? draft !== baseline : false;

  const ideMirrorActive =
    Boolean(pathTrim) &&
    !loadErr &&
    !isMedia &&
    (fileKind === "text" || (fileKind === "markdown" && viewMode === "edit"));
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
    [prefersDark, relativePath, draft],
  );

  const markdownPreview = useMemo(
    () => preferMarkdownPreview(draft, relativePath ?? undefined),
    [draft, relativePath],
  );

  const save = useCallback(async () => {
    if (isMedia || !pathTrim || !dirty) return;
    setNotice(null);
    try {
      await invoke("skilllite_write_workspace_file", {
        workspace: ws,
        relativePath: pathTrim,
        content: draft,
      });
      setBaseline(draft);
      setNotice(t("ide.saveOk"));
      onSaved?.();
    } catch (e) {
      setNotice(e instanceof Error ? e.message : String(e));
    }
  }, [isMedia, pathTrim, dirty, draft, ws, onSaved, t]);

  if (!pathTrim) {
    return (
      <div className="h-full min-h-0 flex flex-col items-center justify-center text-center px-6 text-sm text-ink-mute dark:text-ink-dark-mute bg-white/40 dark:bg-paper-dark/40">
        <p className="max-w-sm leading-relaxed">{t("ide.editorPlaceholder")}</p>
        <p className="mt-3 text-[11px] opacity-80">{t("ide.editorHintToggleIde")}</p>
      </div>
    );
  }

  const tabBtn = (mode: ViewMode, label: string) => (
    <button
      key={mode}
      type="button"
      onClick={() => setViewMode(mode)}
      className={`px-2.5 py-1 text-xs rounded-md transition-colors ${
        viewMode === mode
          ? "bg-accent/15 text-accent dark:text-blue-300 font-medium"
          : "text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
      }`}
    >
      {label}
    </button>
  );

  return (
    <div className="h-full min-h-0 flex flex-col bg-white dark:bg-paper-dark">
      <header className="shrink-0 flex flex-wrap items-center gap-2 px-3 py-2 border-b border-border dark:border-border-dark">
        <h2
          className="text-xs font-semibold font-mono truncate min-w-0 flex-1 basis-[60%]"
          title={relativePath ?? undefined}
        >
          {relativePath}
        </h2>
        {fileKind === "markdown" ? (
          <div className="flex items-center rounded-lg border border-border/70 dark:border-border-dark/70 p-0.5 bg-ink/[0.02] dark:bg-white/[0.04]">
            {tabBtn("edit", t("chat.readFileTabEdit"))}
            {tabBtn("preview", t("chat.readFileTabPreview"))}
          </div>
        ) : null}
        {isMedia ? (
          <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute shrink-0">
            {t("ide.previewOnly")}
          </span>
        ) : null}
        {loading ? (
          <span className="text-[11px] text-ink-mute dark:text-ink-dark-mute">{t("common.loading")}</span>
        ) : null}
        <button
          type="button"
          disabled={!dirty || loading || Boolean(loadErr) || isMedia}
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
        {loadErr ? null : isMedia && mediaSrc ? (
          <div className="flex-1 min-h-0 flex items-center justify-center overflow-auto bg-ink/[0.03] dark:bg-black/25 p-3">
            {fileKind === "image" ? (
              <img
                src={mediaSrc}
                alt=""
                className="max-w-full max-h-full w-auto h-auto object-contain shadow-sm rounded-md border border-border/50 dark:border-border-dark/50"
              />
            ) : (
              <video
                src={mediaSrc}
                controls
                playsInline
                className="max-w-full max-h-full w-auto object-contain rounded-md border border-border/50 dark:border-border-dark/50 bg-black/80"
              >
                {t("ide.videoUnsupported")}
              </video>
            )}
          </div>
        ) : null}
        {!loadErr && !isMedia && fileKind === "markdown" && viewMode === "preview" ? (
          <div className="flex-1 min-h-0 overflow-y-auto px-4 py-3 bg-ink/[0.02] dark:bg-white/[0.03]">
            {markdownPreview ? (
              <div className="rounded-lg border border-border/70 dark:border-border-dark/70 bg-white/90 dark:bg-black/20 px-4 py-3 text-sm text-ink dark:text-ink-dark leading-relaxed">
                <MarkdownContent content={draft} />
              </div>
            ) : (
              <pre className="text-xs font-mono whitespace-pre-wrap break-words text-ink dark:text-ink-dark">
                {draft}
              </pre>
            )}
          </div>
        ) : null}
        {!loadErr && !isMedia && (fileKind === "text" || (fileKind === "markdown" && viewMode === "edit")) ? (
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
        ) : null}
      </div>
    </div>
  );
}
