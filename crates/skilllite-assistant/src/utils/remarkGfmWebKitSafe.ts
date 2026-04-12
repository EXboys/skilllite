/**
 * GFM for `react-markdown`, aligned with `remark-gfm` but **without** GFM autolink-literal.
 *
 * `mdast-util-gfm-autolink-literal@2` uses `(?<=^|\\s|\\p{P}|\\p{S})(…)` with flag `u`.
 * Older WKWebView (Tauri desktop) rejects that pattern at RegExp construction time:
 * `SyntaxError: Invalid regular expression: invalid group specifier name`.
 *
 * Trade-off: raw `user@domain` / `https://…` in prose are not auto-linked; explicit
 * `[text](url)` and `<https://…>` still work.
 */
import type { Options as RemarkGfmOptions } from "remark-gfm";
import type { Processor } from "unified";
import { gfmFootnoteFromMarkdown, gfmFootnoteToMarkdown } from "mdast-util-gfm-footnote";
import {
  gfmStrikethroughFromMarkdown,
  gfmStrikethroughToMarkdown,
} from "mdast-util-gfm-strikethrough";
import { gfmTableFromMarkdown, gfmTableToMarkdown } from "mdast-util-gfm-table";
import {
  gfmTaskListItemFromMarkdown,
  gfmTaskListItemToMarkdown,
} from "mdast-util-gfm-task-list-item";
import { gfmFootnote } from "micromark-extension-gfm-footnote";
import { gfmStrikethrough } from "micromark-extension-gfm-strikethrough";
import { gfmTable } from "micromark-extension-gfm-table";
import { gfmTaskListItem } from "micromark-extension-gfm-task-list-item";
import { combineExtensions } from "micromark-util-combine-extensions";

const emptyOpts = {} as const;

/** Keys written by remark plugins onto `processor.data()` (not in unified’s narrow `Data` type). */
type GfmPluginData = {
  micromarkExtensions?: unknown[];
  fromMarkdownExtensions?: unknown[];
  toMarkdownExtensions?: { extensions: unknown[] }[];
};

export default function remarkGfmWebKitSafe(
  this: Processor,
  options?: RemarkGfmOptions | null | undefined
): undefined {
  const self = this;
  const settings = options ?? emptyOpts;

  const data = self.data() as GfmPluginData;
  const micromarkExtensions =
    data.micromarkExtensions ?? (data.micromarkExtensions = []);
  const fromMarkdownExtensions =
    data.fromMarkdownExtensions ?? (data.fromMarkdownExtensions = []);
  const toMarkdownExtensions =
    data.toMarkdownExtensions ?? (data.toMarkdownExtensions = []);

  micromarkExtensions.push(
    combineExtensions([
      gfmFootnote(),
      gfmStrikethrough(settings),
      gfmTable(),
      gfmTaskListItem(),
    ])
  );

  fromMarkdownExtensions.push([
    gfmFootnoteFromMarkdown(),
    gfmStrikethroughFromMarkdown(),
    gfmTableFromMarkdown(),
    gfmTaskListItemFromMarkdown(),
  ]);

  toMarkdownExtensions.push({
    extensions: [
      gfmFootnoteToMarkdown(settings),
      gfmStrikethroughToMarkdown(),
      gfmTableToMarkdown(settings),
      gfmTaskListItemToMarkdown(),
    ],
  });

  return undefined;
}
