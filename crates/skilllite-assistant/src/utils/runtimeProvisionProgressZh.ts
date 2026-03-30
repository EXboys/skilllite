import { getLocale } from "../i18n";

/** 沙箱侧进度多为英文；中文界面下转为简短中文便于侧边栏展示 */
export function formatRuntimeProvisionProgress(raw: string): string {
  if (getLocale() === "en") {
    return raw;
  }
  const t = raw.trim();
  if (t.startsWith("Downloading...")) {
    return t.replace(/^Downloading\.\.\./, "下载中");
  }
  const exact: Record<string, string> = {
    "This skill requires Python but none was found on your system. Preparing automatically...":
      "正在准备 Python 运行时…",
    "Downloading Python runtime...": "正在下载 Python 运行时…",
    "Verifying integrity...": "正在校验完整性…",
    "Extracting...": "正在解压…",
    "Python runtime is ready.": "Python 运行时已就绪",
    "Primary source unreachable, trying fallback mirror...":
      "主源不可用，正在尝试备用镜像…",
    "This skill requires Node.js but none was found on your system. Preparing automatically...":
      "正在准备 Node.js 运行时…",
    "Downloading Node.js runtime...": "正在下载 Node.js 运行时…",
    "Node.js runtime is ready.": "Node.js 运行时已就绪",
  };
  if (exact[t] !== undefined) return exact[t];
  return raw;
}
