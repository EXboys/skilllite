import type { Extension } from "@codemirror/state";
import type { LanguageSupport } from "@codemirror/language";
import { cpp } from "@codemirror/lang-cpp";
import { css } from "@codemirror/lang-css";
import { go } from "@codemirror/lang-go";
import { html } from "@codemirror/lang-html";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { xml } from "@codemirror/lang-xml";
import { yaml } from "@codemirror/lang-yaml";
import { basicDark } from "@uiw/codemirror-theme-basic/dark";
import { basicLight } from "@uiw/codemirror-theme-basic/light";
import { detectHighlightLanguage } from "./readFileHljs";

function langFromExtension(ext: string): LanguageSupport | null {
  switch (ext) {
    case "py":
    case "pyw":
    case "pyi":
      return python();
    case "rs":
      return rust();
    case "c":
    case "h":
    case "cc":
    case "cpp":
    case "cxx":
    case "hpp":
    case "hxx":
    case "hh":
      return cpp();
    case "json":
    case "jsonc":
      return json();
    case "css":
    case "scss":
    case "less":
      return css();
    case "html":
    case "htm":
      return html();
    case "md":
    case "mdx":
    case "mdc":
    case "markdown":
      return markdown();
    case "xml":
    case "svg":
    case "xsd":
      return xml();
    case "yaml":
    case "yml":
      return yaml();
    case "go":
      return go();
    case "ts":
    case "mts":
    case "cts":
      return javascript({ typescript: true });
    case "tsx":
      return javascript({ jsx: true, typescript: true });
    case "jsx":
      return javascript({ jsx: true });
    case "js":
    case "mjs":
    case "cjs":
      return javascript();
    default:
      return null;
  }
}

function langFromContentHeuristic(snippet: string): LanguageSupport | null {
  const hint = detectHighlightLanguage(snippet);
  switch (hint) {
    case "json":
      return json();
    case "rust":
      return rust();
    case "typescript":
      return javascript({ typescript: true });
    case "python":
      return python();
    case "yaml":
      return yaml();
    default:
      return null;
  }
}

/**
 * CodeMirror 语言包：优先路径后缀，否则用打开时的正文片段做启发式（不随每次按键变化，避免重挂载丢光标）。
 */
export function readFileCodeMirrorLanguage(
  sourcePath: string | undefined,
  initialSnippet: string,
): Extension[] {
  const path = sourcePath?.trim() ?? "";
  const dot = path.lastIndexOf(".");
  const ext = dot >= 0 ? path.slice(dot + 1).toLowerCase() : "";
  if (ext) {
    const fromPath = langFromExtension(ext);
    if (fromPath) {
      return [fromPath];
    }
  }
  const head = initialSnippet.slice(0, 24_000);
  const fromText = langFromContentHeuristic(head);
  return fromText ? [fromText] : [];
}

export function readFileCodeMirrorTheme(isDark: boolean): Extension[] {
  const theme = isDark ? basicDark : basicLight;
  return Array.isArray(theme) ? [...theme] : [theme];
}
