//! LLM message shape, completion trait, and think-block stripping.

use crate::Result;

/// Minimal message format for evolution LLM calls (no tool calling).
#[derive(Debug, Clone)]
pub struct EvolutionMessage {
    pub role: String,
    pub content: Option<String>,
}

impl EvolutionMessage {
    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.to_string()),
        }
    }

    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content.to_string()),
        }
    }
}

/// LLM completion interface for evolution.
///
/// The agent implements this trait to provide LLM access. Evolution uses it
/// for prompt learning, skill synthesis, and external knowledge extraction.
#[async_trait::async_trait]
pub trait EvolutionLlm: Send + Sync {
    /// Non-streaming chat completion. Returns the assistant's text content.
    async fn complete(
        &self,
        messages: &[EvolutionMessage],
        model: &str,
        temperature: f64,
    ) -> Result<String>;
}

// ─── LLM response post-processing ────────────────────────────────────────────

fn strip_paired_xml_block(mut s: String, open: &str, close: &str) -> String {
    while let Some(start) = s.find(open) {
        let after_open = start + open.len();
        if let Some(rel) = s[after_open..].find(close) {
            let end = after_open + rel + close.len();
            s.replace_range(start..end, "");
        } else {
            s.replace_range(start.., "");
            break;
        }
    }
    s
}

fn strip_orphan_think_closing_tags(mut s: String) -> String {
    for tag in [
        "</think>",
        "</Redacted_Thinking>",
        "</thinking>",
        "</Thinking>",
        "</reasoning>",
        "</Reasoning>",
    ] {
        while s.contains(tag) {
            s = s.replace(tag, "");
        }
    }
    s
}

/// Normalize user-visible assistant text: remove paired reasoning XML and orphan closers, then
/// apply [`strip_think_blocks`] for remaining edge cases (e.g. unclosed `<think` prefixes).
///
/// Some providers stream `<think>…` in `content` and stop before the visible answer;
/// [`strip_think_blocks`] alone can leave a stray `</think>` or drop everything when
/// there is no text after the last closing tag. Call this on full assistant bodies before UI or
/// transcript display.
pub fn sanitize_visible_llm_text(content: &str) -> String {
    let s = strip_paired_xml_block(content.trim().to_string(), "<think>", "</think>");
    let s = strip_paired_xml_block(s, "<Redacted_Thinking>", "</Redacted_Thinking>");
    let s = strip_paired_xml_block(s, "<thinking>", "</thinking>");
    let s = strip_paired_xml_block(s, "<Thinking>", "</Thinking>");
    let s = strip_paired_xml_block(s, "<reasoning>", "</reasoning>");
    let s = strip_paired_xml_block(s, "<Reasoning>", "</Reasoning>");
    let s = strip_orphan_think_closing_tags(s);
    strip_think_blocks(&s).trim().to_string()
}

/// Strip reasoning/thinking blocks emitted by various models.
/// Handles `<redacted_thinking>`, `<thinking>`, `<reasoning>` tags (DeepSeek, QwQ, open-source variants).
/// Returns the content after the last closing tag, or the original string if none found.
/// Should be called at the LLM layer so all downstream consumers get clean output.
pub fn strip_think_blocks(content: &str) -> &str {
    const OPENING_TAGS: &[&str] = &[
        "<redacted_thinking>",
        "<think\n",
        "<thinking>",
        "<thinking\n",
        "<reasoning>",
        "<reasoning\n",
    ];

    // Case 1: find the last closing tag, take content after it
    let mut best_end: Option<usize> = None;
    for tag in ["</redacted_thinking>", "</thinking>", "</reasoning>"] {
        if let Some(pos) = content.rfind(tag) {
            let end = pos + tag.len();
            let take = match best_end {
                None => true,
                Some(bp) => end > bp,
            };
            if take {
                best_end = Some(end);
            }
        }
    }
    if let Some(end) = best_end {
        let after = content[end..].trim();
        if !after.is_empty() {
            return after;
        }
    }

    // Case 2: unclosed think tag (model hit token limit mid-thought).
    // Take content before the opening tag if it contains useful text.
    if best_end.is_none() {
        for tag in OPENING_TAGS {
            if let Some(pos) = content.find(tag) {
                let before = content[..pos].trim();
                if !before.is_empty() {
                    return before;
                }
            }
        }
    }

    content
}

#[cfg(test)]
mod sanitize_visible_tests {
    use super::sanitize_visible_llm_text;

    #[test]
    fn orphan_closing_redacted_only() {
        assert_eq!(sanitize_visible_llm_text("</think>"), "");
    }

    #[test]
    fn paired_redacted_keeps_following_answer() {
        let raw = "<think>plan\n</think>\n\nHello!";
        assert_eq!(sanitize_visible_llm_text(raw), "Hello!");
    }

    #[test]
    fn two_paired_blocks_then_answer() {
        let raw = "<think>a</think>\nX\n<think>b</think>\nY";
        assert_eq!(sanitize_visible_llm_text(raw), "X\n\nY");
    }

    /// 桌面会话里常见：工具轮后正文前只剩闭合标签（网关/模型拆段）。
    #[test]
    fn fixture_orphan_close_then_cjk_summary_line() {
        let raw = "</think>\n\n总结：今日无新增阻塞项。";
        assert_eq!(sanitize_visible_llm_text(raw), "总结：今日无新增阻塞项。");
    }

    /// 推理块内为中文，闭合后接英文用户可见句（UTF-8 边界安全）。
    #[test]
    fn fixture_redacted_block_with_cjk_then_visible_english() {
        let raw = "<think>用户要的是简要列表。\n</think>\n\nHere is the list:\n- A\n- B";
        assert_eq!(
            sanitize_visible_llm_text(raw),
            "Here is the list:\n- A\n- B"
        );
    }

    /// 仅有开标签、无闭合（流被工具打断）：丢弃未成对块，不向前泄漏标签名。
    #[test]
    fn fixture_unclosed_redacted_open_only_dropped() {
        let raw = "<think>plan step 1, call tool…";
        assert_eq!(sanitize_visible_llm_text(raw), "");
    }

    /// 部分代理返回 TitleCase 标签对。
    #[test]
    fn fixture_title_case_thinking_pair_stripped() {
        let raw = "<Thinking>internal\nline</Thinking>\n\n答复给用户。";
        assert_eq!(sanitize_visible_llm_text(raw), "答复给用户。");
    }

    /// 孤立 `</Thinking>` 与正文混排。
    #[test]
    fn fixture_orphan_thinking_close_prefix() {
        let raw = "</thinking>\n下一步建议重启会话。";
        assert_eq!(sanitize_visible_llm_text(raw), "下一步建议重启会话。");
    }

    /// redacted + reasoning 两段后再跟一句可见话（多供应商堆叠）。
    #[test]
    fn fixture_chained_redacted_then_reasoning_then_answer() {
        let raw = "<think>r1</think>\n<reasoning>r2</reasoning>\n最终：完成。";
        assert_eq!(sanitize_visible_llm_text(raw), "最终：完成。");
    }
}
