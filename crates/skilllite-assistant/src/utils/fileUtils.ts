/** Group files by top-level directory. Used for memory files and output files. */
export function groupMemoryFiles(files: string[]): Record<string, string[]> {
  const groups: Record<string, string[]> = {};
  for (const f of files) {
    const parts = f.split("/");
    const key = parts.length > 1 ? parts[0] : ".";
    (groups[key] ??= []).push(f);
  }
  return groups;
}
