/** Top-level directory segment for memory/output paths, or "." for files at virtual root. */
export function memoryTopLevelGroupKey(path: string): string {
  const parts = path.split("/");
  return parts.length > 1 ? parts[0]! : ".";
}

/** Path under the top-level group (e.g. `notes/a/b.md` → `a/b.md`). Single-segment paths unchanged. */
export function memoryPathUnderTopGroup(path: string): string {
  const parts = path.split("/");
  if (parts.length <= 1) return parts[0] ?? path;
  return parts.slice(1).join("/");
}

/** Stable ordering: root-level files first, then folder names A–Z. */
export function sortedMemoryGroupKeys(groups: Record<string, unknown>): string[] {
  return Object.keys(groups).sort((a, b) => {
    if (a === ".") return -1;
    if (b === ".") return 1;
    return a.localeCompare(b);
  });
}

/** Group files by top-level directory. Used for memory files and output files. */
export function groupMemoryFiles(files: string[]): Record<string, string[]> {
  const groups: Record<string, string[]> = {};
  for (const f of files) {
    const key = memoryTopLevelGroupKey(f);
    (groups[key] ??= []).push(f);
  }
  for (const k of Object.keys(groups)) {
    groups[k]!.sort((a, b) => a.localeCompare(b));
  }
  return groups;
}

/** Group entries (e.g. memory summaries) by the same top-level rule as `groupMemoryFiles`. */
export function groupMemoryEntriesByTopDir<T extends { path: string }>(entries: T[]): Record<string, T[]> {
  const groups: Record<string, T[]> = {};
  for (const e of entries) {
    const key = memoryTopLevelGroupKey(e.path);
    (groups[key] ??= []).push(e);
  }
  for (const k of Object.keys(groups)) {
    groups[k]!.sort((a, b) => a.path.localeCompare(b.path));
  }
  return groups;
}
