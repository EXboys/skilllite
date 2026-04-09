//! LLM call sub-module: streaming/non-streaming dispatch with automatic
//! context-overflow recovery.
//!
//! Centralizes the LLM call + overflow retry pattern that was duplicated in
//! both `run_simple_loop` and `run_with_task_planning`.

use crate::Result;

use super::super::llm::{self, llm_usage_report_from_usage, ChatCompletionResponse, LlmClient};
use super::super::types::{
    get_tool_result_recovery_max_chars, ChatMessage, EventSink, LlmUsageTotals,
    ToolDefinition,
};

/// Maximum number of context overflow recovery retries before giving up.
const MAX_CONTEXT_OVERFLOW_RETRIES: usize = 3;

/// Outcome of an LLM call with overflow recovery.
pub(super) enum LlmCallOutcome {
    /// Successfully received a response.
    Response(ChatCompletionResponse),
    /// Context overflow detected; messages were truncated. Caller should retry
    /// (i.e. `continue` the loop).
    Truncated,
}

/// Call the LLM with automatic context-overflow recovery.
///
/// When `stream` is `true`, tokens are streamed to `event_sink` via
/// `chat_completion_stream`. When `false`, uses the non-streaming
/// `chat_completion` (used when planning mode suppresses streaming).
///
/// On context overflow, truncates tool messages and returns `Truncated` so the
/// caller can `continue`. After `MAX_CONTEXT_OVERFLOW_RETRIES` consecutive
/// overflows, propagates the error.
#[allow(clippy::too_many_arguments)]
pub(super) async fn call_llm_with_recovery(
    client: &LlmClient,
    model: &str,
    messages: &mut [ChatMessage],
    tools: Option<&[ToolDefinition]>,
    temperature: Option<f64>,
    stream: bool,
    event_sink: &mut dyn EventSink,
    context_overflow_retries: &mut usize,
    usage_totals: Option<&mut LlmUsageTotals>,
) -> Result<LlmCallOutcome> {
    event_sink.reset_streamed_text_for_llm_call();
    let result = if stream {
        client
            .chat_completion_stream(
                model,
                messages,
                tools,
                temperature,
                event_sink,
                usage_totals,
            )
            .await
    } else {
        client
            .chat_completion(model, messages, tools, temperature, usage_totals)
            .await
    };

    match result {
        Ok(resp) => {
            *context_overflow_retries = 0;
            let report = resp.usage.as_ref().map(llm_usage_report_from_usage);
            event_sink.on_llm_usage(report);
            Ok(LlmCallOutcome::Response(resp))
        }
        Err(e) => {
            if llm::is_context_overflow_error(&e.to_string()) {
                *context_overflow_retries += 1;
                if *context_overflow_retries >= MAX_CONTEXT_OVERFLOW_RETRIES {
                    tracing::error!(
                        "Context overflow persists after {} retries, giving up",
                        MAX_CONTEXT_OVERFLOW_RETRIES
                    );
                    return Err(e);
                }
                let base = get_tool_result_recovery_max_chars();
                let rc = match *context_overflow_retries {
                    1 => base,
                    2 => base.max(400) / 2,
                    _ => base.max(400) / 4,
                };
                tracing::warn!(
                    "Context overflow (attempt {}/{}), truncating to {} chars",
                    *context_overflow_retries,
                    MAX_CONTEXT_OVERFLOW_RETRIES,
                    rc
                );
                llm::truncate_tool_messages(messages, rc);
                Ok(LlmCallOutcome::Truncated)
            } else {
                Err(e)
            }
        }
    }
}
