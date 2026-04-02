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

import "highlight.js/styles/github-dark.min.css";

let hljsReady = false;

export function ensureReadFileHljs(): void {
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

export function looksLikeMarkdown(body: string): boolean {
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

export function detectHighlightLanguage(body: string): string {
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

export function highlightCodeToHtml(body: string): string {
  ensureReadFileHljs();
  const lang = detectHighlightLanguage(body);
  try {
    return hljs.highlight(body, { language: lang }).value;
  } catch {
    return hljs.highlight(body, { language: "plaintext" }).value;
  }
}

/** 路径后缀或内容启发式：全屏预览用 Markdown 渲染 */
export function preferMarkdownPreview(text: string, sourcePath?: string): boolean {
  const lower = sourcePath?.toLowerCase().trim() ?? "";
  if (
    lower.endsWith(".md") ||
    lower.endsWith(".mdx") ||
    lower.endsWith(".mdc") ||
    lower.endsWith(".markdown")
  ) {
    return true;
  }
  return looksLikeMarkdown(text);
}

/** 全屏代码预览上限，避免超大文本卡死主线程 */
export const READ_FILE_FULLSCREEN_PREVIEW_MAX = 200_000;
