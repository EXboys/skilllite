import { parseReadFileToolResult } from "./readFileParse";

/** Extract `path` / `file_path` from read_file tool JSON arguments. */
export function tryParseReadFilePathFromToolArgs(args: string): string | null {
  try {
    let v: unknown = JSON.parse(args);
    if (typeof v === "string") {
      try {
        v = JSON.parse(v);
      } catch {
        return null;
      }
    }
    if (!v || typeof v !== "object") return null;
    const o = v as Record<string, unknown>;
    const p = o.path ?? o.file_path;
    if (typeof p !== "string") return null;
    const t = p.trim();
    return t.length > 0 ? t : null;
  } catch {
    return null;
  }
}

export function readFileResultLooksTruncated(raw: string): boolean {
  return raw.includes("content truncated") || raw.includes("结果已截断");
}

/** Plain file text from read_file tool output (drops `line|` prefixes for the numbered block). */
export function plainTextBodyFromReadFileResult(raw: string): string {
  const parsed = parseReadFileToolResult(raw);
  if (parsed.kind !== "lines") {
    return raw;
  }
  return parsed.lines.map((l) => l.text).join("\n");
}
