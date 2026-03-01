//! Evolution integration: implements EvolutionLlm for agent's LlmClient.
//!
//! Re-exports skilllite-evolution and provides the adapter to use the agent's
//! LLM client for evolution operations.

use anyhow::Result;

use skilllite_evolution::feedback::{DecisionInput, FeedbackSignal as EvolutionFeedbackSignal};
use skilllite_evolution::{EvolutionLlm, EvolutionMessage};

use super::llm::LlmClient;
use super::types::{ChatMessage, ExecutionFeedback, FeedbackSignal};

/// Adapter that makes LlmClient implement EvolutionLlm.
pub struct EvolutionLlmAdapter<'a> {
    pub llm: &'a LlmClient,
}

#[async_trait::async_trait]
impl EvolutionLlm for EvolutionLlmAdapter<'_> {
    async fn complete(
        &self,
        messages: &[EvolutionMessage],
        model: &str,
        temperature: f64,
    ) -> Result<String> {
        let chat_messages: Vec<ChatMessage> = messages
            .iter()
            .map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            })
            .collect();

        let response = self
            .llm
            .chat_completion(model, &chat_messages, None, Some(temperature))
            .await?;

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .unwrap_or("")
            .trim()
            .to_string();

        Ok(content)
    }
}

/// Convert agent's ExecutionFeedback to evolution's DecisionInput.
pub fn execution_feedback_to_decision_input(feedback: &ExecutionFeedback) -> DecisionInput {
    DecisionInput {
        total_tools: feedback.total_tools,
        failed_tools: feedback.failed_tools,
        replans: feedback.replans,
        elapsed_ms: feedback.elapsed_ms,
        task_completed: feedback.task_completed,
        task_description: feedback.task_description.clone(),
        rules_used: feedback.rules_used.clone(),
        tools_detail: feedback
            .tools_detail
            .iter()
            .map(|t| skilllite_evolution::feedback::ToolExecDetail {
                tool: t.tool.clone(),
                success: t.success,
            })
            .collect(),
    }
}

/// Convert agent's FeedbackSignal to evolution's.
pub fn to_evolution_feedback(signal: FeedbackSignal) -> EvolutionFeedbackSignal {
    match signal {
        FeedbackSignal::ExplicitPositive => EvolutionFeedbackSignal::ExplicitPositive,
        FeedbackSignal::ExplicitNegative => EvolutionFeedbackSignal::ExplicitNegative,
        FeedbackSignal::Neutral => EvolutionFeedbackSignal::Neutral,
    }
}

// Re-export evolution crate for use by chat_session and other modules.
pub use skilllite_evolution::{
    check_auto_rollback, format_evolution_changes, on_shutdown, query_changes_by_txn,
    run_evolution, EvolutionMode,
};
pub use skilllite_evolution::feedback;
pub use skilllite_evolution::seed;
