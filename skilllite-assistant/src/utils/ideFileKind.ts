export type IdeEditorFileKind = "text" | "markdown" | "image" | "video";

const MARKDOWN_EXT = new Set(["md", "mdx", "mdc", "markdown"]);
const IMAGE_EXT = new Set([
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "svg",
  "bmp",
  "ico",
  "avif",
]);
const VIDEO_EXT = new Set(["mp4", "webm", "ogv", "mov", "m4v", "mkv"]);

function extensionOfRelativePath(relativePath: string): string {
  const base = relativePath.trim().split(/[/\\]/).pop() ?? "";
  const dot = base.lastIndexOf(".");
  return dot >= 0 ? base.slice(dot + 1).toLowerCase() : "";
}

export function ideFileKindFromPath(relativePath: string): IdeEditorFileKind {
  const ext = extensionOfRelativePath(relativePath);
  if (MARKDOWN_EXT.has(ext)) return "markdown";
  if (IMAGE_EXT.has(ext)) return "image";
  if (VIDEO_EXT.has(ext)) return "video";
  return "text";
}
