import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";

export type WorkspaceListEntryDto = {
  relative_path: string;
  is_dir: boolean;
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
  const [entries, setEntries] = useState<WorkspaceListEntryDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<WorkspaceListEntryDto[]>("skilllite_list_workspace_entries", {
        workspace: workspace.trim() || ".",
      });
      setEntries(Array.isArray(list) ? list : []);
    } catch (e) {
      setEntries([]);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [workspace]);

  useEffect(() => {
    void load();
  }, [load, refreshToken]);

  const tree = useMemo(() => buildTree(entries), [entries]);

  const toggleDir = useCallback((p: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(p)) next.delete(p);
      else next.add(p);
      return next;
    });
  }, []);

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="shrink-0 flex items-center justify-between gap-2 px-2 py-1.5 border-b border-border/60 dark:border-border-dark/60">
        <span className="text-[11px] font-medium text-ink-mute dark:text-ink-dark-mute uppercase tracking-wide">
          {t("ide.workspaceFiles")}
        </span>
        <button
          type="button"
          onClick={() => void load()}
          disabled={loading}
          className="text-[11px] px-2 py-0.5 rounded border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50"
        >
          {loading ? t("common.loading") : t("ide.refreshTree")}
        </button>
      </div>
      {error ? (
        <div className="p-2 text-xs text-red-600 dark:text-red-400 shrink-0">{error}</div>
      ) : null}
      <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden py-1">
        {!error ? (
          <TreeRows
            nodes={tree}
            depth={0}
            expanded={expanded}
            toggleDir={toggleDir}
            selectedPath={selectedPath}
            onSelectFile={onSelectFile}
          />
        ) : null}
      </div>
    </div>
  );
}
