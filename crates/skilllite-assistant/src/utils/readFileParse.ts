const READ_FILE_LINE_RE = /^\s*(\d+)\|(.*)$/;

export type ParsedReadFile =
  | { kind: "lines"; lines: { n: number; text: string }[]; suffix: string }
  | { kind: "plain"; text: string };

export function parseReadFileToolResult(raw: string): ParsedReadFile {
  const lines = raw.split("\n");
  const numbered: { n: number; text: string }[] = [];
  let i = 0;
  for (; i < lines.length; i++) {
    const m = lines[i].match(READ_FILE_LINE_RE);
    if (!m) {
      break;
    }
    numbered.push({ n: Number.parseInt(m[1], 10), text: m[2] });
  }
  if (numbered.length === 0) {
    return { kind: "plain", text: raw };
  }
  const suffix = lines.slice(i).join("\n");
  return { kind: "lines", lines: numbered, suffix };
}
