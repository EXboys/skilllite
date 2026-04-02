import { useMemo } from "react";
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import css from "highlight.js/lib/languages/css";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import plaintext from "highlight.js/lib/languages/plaintext";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import javascript from "highlight.js/lib/languages/javascript";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

import { MarkdownContent } from "../shared/MarkdownContent";

import "highlight.js/styles/github-dark.min.css";

let hljsReady = false;
function ensureHljsRegistered() {
  if (hljsReady) {
    return;
  }
  hljs.registerLanguage("bash", bash);
  hljs.registerLanguage("css", css);
  hljs.registerLanguage("json", json);
  hljs.registerLanguage("markdown", markdown);
  hljs.registerLanguage("plaintext", plaintext);
  hljs.registerLanguage("python", python);
  hljs.registerLanguage("rust", rust);
  hljs.registerLanguage("typescript", typescript);
  hljs.registerLanguage("javascript", javascript);
  hljs.registerLanguage("xml", xml);
  hljs.registerLanguage("yaml", yaml);
  hljsReady = true;
}

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

function looksLikeMarkdown(body: string): boolean {
  const t = body.trim();
  if (t.length === 0) {
    return false;
  }
  const lines = t.split("\n");
  const firstMeaningful =
    lines.map((l) => l.trim()).find((l) => l.length > 0) ?? "";
  if (/^#{1,6}\s+\S/.test(firstMeaningful)) {
    return true;
  }
  if (t.startsWith("---")) {
    const after = t.slice(3).split("\n");
    if (after.some((l) => l.trim() === "---")) {
      return true;
    }
  }
  let mdHints = 0;
  for (const line of lines) {
    const s = line.trim();
    if (/^#{1,6}\s/.test(s)) {
      mdHints += 1;
    } else if (/^[-*]\s+\S/.test(s) || /^\d+\.\s+\S/.test(s)) {
      mdHints += 1;
    }
  }
  return mdHints >= 2 && lines.length < 500;
}

function detectHighlightLanguage(body: string): string {
  const head = body.slice(0, 12_000);
  const trimmed = head.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    try {
      JSON.parse(trimmed.length > 200_000 ? trimmed.slice(0, 200_000) : trimmed);
      return "json";
    } catch {
      /* not JSON */
    }
  }
  if (/^(use |mod |fn |pub |impl |type |enum |struct |const |static |#\[)/m.test(head)) {
    return "rust";
  }
  if (
    /^(import |export |interface |type \w|declare |function |\s*const |\s*let )/m.test(head)
  ) {
    return "typescript";
  }
  if (/^(def |class |from \w+ import |import \w+)/m.test(head)) {
    return "python";
  }
  if (/^(\w+:|\s*-\s+\w+:\s|apiVersion:|kind:)/m.test(head) && /:\s/.test(head)) {
    return "yaml";
  }
  return "plaintext";
}

function highlightCode(body: string): string {
  ensureHljsRegistered();
  const lang = detectHighlightLanguage(body);
  try {
    return hljs.highlight(body, { language: lang }).value;
  } catch {
    return hljs.highlight(body, { language: "plaintext" }).value;
  }
}

/** read_file 成功结果：避免整段被当作 Markdown 误解析；Markdown 文件渲染，其余语法高亮 */
export function ReadFileToolResultView({ result }: { result: string }) {
  const trimmed = result.trim();
  const isBinary = trimmed.startsWith("[Binary file");

  const parsed = useMemo(() => parseReadFileToolResult(result), [result]);

  const body = useMemo(() => {
    if (parsed.kind !== "lines") {
      return "";
    }
    return parsed.lines.map((l) => l.text).join("\n");
  }, [parsed]);

  const isMarkdownBody = useMemo(
    () => body.length > 0 && looksLikeMarkdown(body),
    [body],
  );

  const highlighted = useMemo(() => {
    if (!body || isMarkdownBody) {
      return "";
    }
    return highlightCode(body);
  }, [body, isMarkdownBody]);

  if (isBinary) {
    return (
      <pre className="mt-1.5 text-xs font-mono whitespace-pre-wrap break-words rounded-lg border border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06] px-3 py-2 text-ink dark:text-ink-dark max-h-48 overflow-auto">
        {result.trimEnd()}
      </pre>
    );
  }

  if (parsed.kind === "plain") {
    return (
      <pre className="mt-1.5 text-xs font-mono whitespace-pre-wrap break-words rounded-lg border border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06] px-3 py-2 text-ink dark:text-ink-dark max-h-96 overflow-auto leading-relaxed">
        {result.trimEnd()}
      </pre>
    );
  }

  const suffix = parsed.suffix.trimEnd();
  const lineCount = parsed.lines.length;
  const startLine = parsed.lines[0]?.n ?? 1;
  const endLine = parsed.lines[parsed.lines.length - 1]?.n ?? startLine;

  if (isMarkdownBody) {
    return (
      <div className="mt-1.5 space-y-2 min-w-0">
        <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
          第 {startLine}–{endLine} 行（共 {lineCount} 行）· Markdown 预览
        </p>
        <div className="max-h-96 overflow-auto rounded-lg border border-border/70 dark:border-border-dark/70 bg-white/80 dark:bg-black/20 px-3 py-2">
          <MarkdownContent content={body} />
        </div>
        {suffix ? (
          <pre className="text-[11px] font-mono whitespace-pre-wrap text-ink-mute dark:text-ink-dark-mute border-t border-border/50 dark:border-border-dark/50 pt-2">
            {suffix}
          </pre>
        ) : null}
      </div>
    );
  }

  return (
    <div className="mt-1.5 space-y-2 min-w-0">
      <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
        第 {startLine}–{endLine} 行（共 {lineCount} 行）· 语法高亮
      </p>
      <div className="rounded-lg border border-border/70 dark:border-border-dark/70 max-h-96 overflow-y-auto bg-[#0d1117]">
        <pre className="m-0 p-3 text-xs leading-5 whitespace-pre">
          <code
            className="hljs"
            // highlight.js 已对特殊字符转义
            dangerouslySetInnerHTML={{ __html: highlighted }}
          />
        </pre>
      </div>
      {suffix ? (
        <pre className="text-[11px] font-mono whitespace-pre-wrap text-ink-mute dark:text-ink-dark-mute border-t border-border/50 dark:border-border-dark/50 pt-2">
          {suffix}
        </pre>
      ) : null}
    </div>
  );
}

/** list_directory：等宽保留树形结构 */
export function ListDirectoryToolResultView({ result }: { result: string }) {
  return (
    <div className="mt-1.5 min-w-0">
      <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute mb-1.5">
        目录树（ASCII）
      </p>
      <pre className="text-xs font-mono leading-snug whitespace-pre overflow-x-auto max-h-96 rounded-lg border border-border/60 dark:border-border-dark/60 bg-ink/[0.04] dark:bg-white/[0.06] px-3 py-2.5 text-ink dark:text-ink-dark shadow-inner shadow-ink/[0.03]">
        {result.trimEnd()}
      </pre>
    </div>
  );
}
