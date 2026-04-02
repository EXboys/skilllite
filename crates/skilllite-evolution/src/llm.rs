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
