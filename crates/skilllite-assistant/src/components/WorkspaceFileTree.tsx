import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
  useTransition,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";

export type WorkspaceListEntryDto = {
  relative_path: string;
  is_dir: boolean;
};

type WorkspaceListPayloadDto = {
  entries: WorkspaceListEntryDto[];
  truncated: boolean;
  max_entries: number;
  max_depth: number;
};

type TreeNode = {
  name: string;
  path: string;
  isDir: boolean;
  children: TreeNode[];
};

function buildTree(entries: WorkspaceListEntryDto[]): TreeNode[] {
  const root: TreeNode[] = [];
  for (const e of entries) {
    const parts = e.relative_path.split("/").filter(Boolean);
    let level = root;
    let prefix = "";
    for (let i = 0; i < parts.length; i++) {
      const seg = parts[i]!;
      prefix = prefix ? `${prefix}/${seg}` : seg;
      const atEnd = i === parts.length - 1;
      const isDir = atEnd ? e.is_dir : true;
      let node = level.find((n) => n.name === seg);
      if (!node) {
        node = { name: seg, path: prefix, isDir, children: [] };
        level.push(node);
      } else if (atEnd) {
        node.isDir = e.is_dir;
      }
      level = node.children;
    }
  }
  const sortRec = (nodes: TreeNode[]) => {
    nodes.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    });
    for (const n of nodes) sortRec(n.children);
  };
  sortRec(root);
  return root;
}

function TreeSkeleton({ label }: { label: string }) {
  return (
    <div
      className="px-2 py-2 space-y-2"
      aria-busy="true"
      aria-label={label}
      role="status"
    >
      {Array.from({ length: 8 }).map((_, i) => (
        <div
          key={i}
          className="h-4 rounded-md bg-ink/10 dark:bg-white/10 motion-safe:animate-pulse"
          style={{ marginLeft: 4 + (i % 4) * 10 }}
        />
      ))}
    </div>
  );
}

function TreeRows({
  nodes,
  depth,
  expanded,
  toggleDir,
  selectedPath,
  onSelectFile,
}: {
  nodes: TreeNode[];
  depth: number;
  expanded: Set<string>;
  toggleDir: (p: string) => void;
  selectedPath: string | null;
  onSelectFile: (p: string) => void;
}) {
  const { t } = useI18n();
  return (
    <ul className="list-none m-0 p-0">
      {nodes.map((node) => (
        <li key={node.path} className="m-0 p-0">
          <button
            type="button"
            onClick={() => {
              if (node.isDir) toggleDir(node.path);
              else onSelectFile(node.path);
            }}
            className={`w-full text-left flex items-center gap-1.5 py-0.5 px-1 rounded text-[13px] font-mono transition-colors ${
              !node.isDir && selectedPath === node.path
                ? "bg-accent/15 text-accent dark:text-blue-300"
                : "text-ink dark:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5"
            }`}
            style={{ paddingLeft: 4 + depth * 12 }}
            title={node.path}
          >
            <span className="shrink-0 w-4 text-center" aria-hidden>
              {node.isDir ? (expanded.has(node.path) ? "▾" : "▸") : " "}
            </span>
            <span className="shrink-0" aria-hidden>
              {node.isDir ? "📁" : "📄"}
            </span>
            <span className="truncate">{node.name}</span>
          </button>
          {node.isDir && expanded.has(node.path) && node.children.length > 0 ? (
            <TreeRows
              nodes={node.children}
              depth={depth + 1}
              expanded={expanded}
              toggleDir={toggleDir}
              selectedPath={selectedPath}
              onSelectFile={onSelectFile}
            />
          ) : null}
        </li>
      ))}
      {nodes.length === 0 && depth === 0 ? (
        <li className="px-2 py-3 text-xs text-ink-mute dark:text-ink-dark-mute">
          {t("ide.treeEmpty")}
        </li>
      ) : null}
    </ul>
  );
}

interface WorkspaceFileTreeProps {
  workspace: string;
  selectedPath: string | null;
  onSelectFile: (relativePath: string) => void;
  refreshToken?: number;
}

export default function WorkspaceFileTree({
  workspace,
  selectedPath,
  onSelectFile,
  refreshToken = 0,
}: WorkspaceFileTreeProps) {
  const { t } = useI18n();
  const [, startTransition] = useTransition();
  const [entries, setEntries] = useState<WorkspaceListEntryDto[]>([]);
  const [truncated, setTruncated] = useState(false);
  const [listCaps, setListCaps] = useState<{ max: number; depth: number }>({
    max: 5000,
    depth: 14,
  });
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const loadGenRef = useRef(0);

  const deferredEntries = useDeferredValue(entries);
  const tree = useMemo(() => buildTree(deferredEntries), [deferredEntries]);
  const treeCatchUpPending = deferredEntries !== entries;

  const load = useCallback(async () => {
    const gen = ++loadGenRef.current;
    setLoading(true);
    setError(null);
    try {
      const payload = await invoke<WorkspaceListPayloadDto>(
        "skilllite_list_workspace_entries",
        {
          workspace: workspace.trim() || ".",
        }
      );
      if (gen !== loadGenRef.current) return;

      const list = Array.isArray(payload?.entries) ? payload.entries : [];
      const caps = {
        max: typeof payload?.max_entries === "number" ? payload.max_entries : 5000,
        depth: typeof payload?.max_depth === "number" ? payload.max_depth : 14,
      };
      const isTruncated = Boolean(payload?.truncated);

      startTransition(() => {
        if (gen !== loadGenRef.current) return;
        setEntries(list);
        setTruncated(isTruncated);
        setListCaps(caps);
      });
    } catch (e) {
      if (gen !== loadGenRef.current) return;
      setEntries([]);
      setTruncated(false);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (gen === loadGenRef.current) {
        setLoading(false);
      }
    }
  }, [workspace, startTransition]);

  useEffect(() => {
    void load();
  }, [load, refreshToken]);

  const toggleDir = useCallback((p: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(p)) next.delete(p);
      else next.add(p);
      return next;
    });
  }, []);

  const showInitialSkeleton = loading && entries.length === 0 && !error;
  const showUpdatingHint = loading && entries.length > 0;

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="shrink-0 flex items-center justify-between gap-2 px-2 py-1.5 border-b border-border/60 dark:border-border-dark/60">
        <div className="min-w-0 flex flex-col gap-0.5">
          <span className="text-[11px] font-medium text-ink-mute dark:text-ink-dark-mute uppercase tracking-wide">
            {t("ide.workspaceFiles")}
          </span>
          {showUpdatingHint ? (
            <span className="text-[10px] text-accent/90 dark:text-accent truncate">
              {t("ide.treeUpdating")}
            </span>
          ) : null}
        </div>
        <button
          type="button"
          onClick={() => void load()}
          className="shrink-0 inline-flex items-center gap-1.5 text-[11px] px-2 py-0.5 rounded border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5"
          aria-busy={loading}
        >
          {loading ? (
            <svg
              className="w-3 h-3 animate-spin shrink-0"
              viewBox="0 0 24 24"
              fill="none"
              aria-hidden
            >
              <circle
                cx="12"
                cy="12"
                r="9"
                className="opacity-25"
                stroke="currentColor"
                strokeWidth="3"
              />
              <path
                d="M21 12a9 9 0 0 0-9-9"
                className="opacity-100"
                stroke="currentColor"
                strokeWidth="3"
                strokeLinecap="round"
              />
            </svg>
          ) : null}
          {loading ? t("common.loading") : t("ide.refreshTree")}
        </button>
      </div>
      {truncated && !error ? (
        <div
          className="shrink-0 mx-2 mt-2 px-2 py-1.5 rounded-md border border-amber-200/80 dark:border-amber-700/50 bg-amber-50/90 dark:bg-amber-950/30 text-[10px] leading-snug text-amber-950 dark:text-amber-100/90"
          role="status"
        >
          {t("ide.treeTruncatedHint", {
            max: listCaps.max,
            depth: listCaps.depth,
          })}
        </div>
      ) : null}
      {error ? (
        <div className="p-2 text-xs text-red-600 dark:text-red-400 shrink-0">{error}</div>
      ) : null}
      <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden py-1 relative">
        {showInitialSkeleton ? (
          <TreeSkeleton label={t("ide.treeIndexing")} />
        ) : null}
        {!error && !showInitialSkeleton ? (
          <>
            {treeCatchUpPending ? (
              <div
                className="pointer-events-none absolute inset-x-0 top-0 h-0.5 bg-accent/40 motion-safe:animate-pulse z-10"
                aria-hidden
              />
            ) : null}
            <TreeRows
              nodes={tree}
              depth={0}
              expanded={expanded}
              toggleDir={toggleDir}
              selectedPath={selectedPath}
              onSelectFile={onSelectFile}
            />
          </>
        ) : null}
      </div>
      {!error && deferredEntries.length > 0 && !showInitialSkeleton ? (
        <div className="shrink-0 px-2 py-1 border-t border-border/40 dark:border-border-dark/40 text-[10px] text-ink-mute dark:text-ink-dark-mute">
          {t("ide.treeEntryCount", { n: deferredEntries.length })}
          {treeCatchUpPending ? ` · ${t("ide.treeUpdating")}` : ""}
        </div>
      ) : null}
    </div>
  );
}
