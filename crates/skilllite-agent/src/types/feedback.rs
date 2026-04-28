//! EVO-1: Execution feedback types for evolution engine.

use super::llm_usage::LlmUsageTotals;
use serde::{Deserialize, Serialize};

pub const WIKI_CONSECUTIVE_TOOL_FAILURE_THRESHOLD: usize = 3;

/// Task completion classification from model-reported structured completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskCompletionType {
    #[default]
    Success,
    PartialSuccess,
    Failure,
}

impl TaskCompletionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::PartialSuccess => "partial_success",
            Self::Failure => "failure",
        }
    }
}

/// Structured feedback collected from each agent loop execution.
/// Used by the evolution engine to evaluate rule/skill effectiveness.
#[derive(Debug, Clone, Default)]
pub struct ExecutionFeedback {
    pub total_tools: usize,
    pub failed_tools: usize,
    pub replans: usize,
    pub max_consecutive_tool_failures: usize,
    pub max_repeated_tool_failures: usize,
    pub iterations: usize,
    pub elapsed_ms: u64,
    pub context_overflow_retries: usize,
    pub task_completed: bool,
    pub completion_type: TaskCompletionType,
    /// Brief task description (generalized, not user's original text).
    pub task_description: Option<String>,
    /// Names of planning rules that were matched for this task.
    pub rules_used: Vec<String>,
    /// Per-tool execution details.
    pub tools_detail: Vec<ToolExecDetail>,
    /// Sum of API-reported token usage across LLM calls in this run (when provided by the provider).
    pub llm_usage: LlmUsageTotals,
}

/// Per-tool execution outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecDetail {
    pub tool: String,
    pub success: bool,
}

/// Structured prompt payload for asking the user whether a difficult run should
/// become a project Repo Wiki lesson.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiUpdateSuggestion {
    pub trigger: WikiUpdateTrigger,
    pub replan_count: usize,
    pub failed_tool_count: usize,
    pub failed_tools: Vec<String>,
    pub error_summaries: Vec<String>,
    pub proposed_title: String,
    pub proposed_lesson: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WikiUpdateTrigger {
    Replan,
    ConsecutiveToolFailures,
    RepeatedToolFailures,
}

pub fn build_wiki_update_suggestion(
    feedback: &ExecutionFeedback,
    error_summaries: Vec<String>,
) -> Option<WikiUpdateSuggestion> {
    let trigger = if feedback.replans > 0 {
        WikiUpdateTrigger::Replan
    } else if feedback.max_consecutive_tool_failures >= WIKI_CONSECUTIVE_TOOL_FAILURE_THRESHOLD {
        WikiUpdateTrigger::ConsecutiveToolFailures
    } else if feedback.max_repeated_tool_failures >= WIKI_CONSECUTIVE_TOOL_FAILURE_THRESHOLD {
        WikiUpdateTrigger::RepeatedToolFailures
    } else {
        return None;
    };

    let mut failed_tools = Vec::new();
    for detail in feedback
        .tools_detail
        .iter()
        .filter(|detail| !detail.success)
    {
        if !failed_tools.contains(&detail.tool) {
            failed_tools.push(detail.tool.clone());
        }
    }

    let task = feedback
        .task_description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("agent conversation");
    let proposed_title = match trigger {
        WikiUpdateTrigger::Replan => {
            format!("Conversation lesson: replan while working on {}", task)
        }
        WikiUpdateTrigger::ConsecutiveToolFailures => {
            format!(
                "Conversation lesson: consecutive tool failures while working on {}",
                task
            )
        }
        WikiUpdateTrigger::RepeatedToolFailures => {
            format!(
                "Conversation lesson: repeated tool failure while working on {}",
                task
            )
        }
    };
    let failed_tool_text = if failed_tools.is_empty() {
        "None recorded".to_string()
    } else {
        failed_tools.join(", ")
    };
    let error_text = if error_summaries.is_empty() {
        "- No concrete error summary was captured.".to_string()
    } else {
        error_summaries
            .iter()
            .map(|summary| format!("- {}", summary))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let proposed_lesson = format!(
        "## What Happened\n\nA chat/Assistant run triggered `{:?}` while working on `{}`.\n\n- Replans: {}\n- Failed tools: {}\n\n## Error Signals\n\n{}\n\n## Root Cause\n\nReview the failed tool output and identify why the previous approach did not work.\n\n## Optimization\n\nBefore retrying, change the approach based on the failure signal instead of repeating the same tool call or plan.\n\n## Next Time\n\n- Check the relevant path, command, schema, or dependency before retrying.\n- If the plan is invalid, replan once with a narrower task list.\n- Record the confirmed fix here after editing this lesson.",
        trigger, task, feedback.replans, failed_tool_text, error_text
    );

    Some(WikiUpdateSuggestion {
        trigger,
        replan_count: feedback.replans,
        failed_tool_count: feedback.failed_tools,
        failed_tools,
        error_summaries,
        proposed_title,
        proposed_lesson,
    })
}

/// User feedback signal classified from the next user message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FeedbackSignal {
    ExplicitPositive,
    ExplicitNegative,
    #[default]
    Neutral,
}

impl FeedbackSignal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExplicitPositive => "pos",
            Self::ExplicitNegative => "neg",
            Self::Neutral => "neutral",
        }
    }
}

/// Classify user feedback from the next message (simple keyword matching).
/// ~80% accuracy is sufficient; evolution is gradual and tolerates noise.
pub fn classify_user_feedback(next_user_message: &str) -> FeedbackSignal {
    let msg = next_user_message.to_lowercase();
    let negative = [
        "不对",
        "错了",
        "重来",
        "重新",
        "wrong",
        "redo",
        "fix",
        "不是这样",
        "不行",
        "有问题",
        "bug",
        "失败",
    ];
    let positive = [
        "好的",
        "谢谢",
        "完美",
        "不错",
        "thanks",
        "great",
        "perfect",
        "可以",
        "没问题",
        "很好",
        "nice",
        "done",
        "ok",
    ];
    if negative.iter().any(|k| msg.contains(k)) {
        FeedbackSignal::ExplicitNegative
    } else if positive.iter().any(|k| msg.contains(k)) {
        FeedbackSignal::ExplicitPositive
    } else {
        FeedbackSignal::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_suggestion_triggers_on_replan() {
        let feedback = ExecutionFeedback {
            replans: 1,
            failed_tools: 1,
            task_description: Some("fix wiki refresh".to_string()),
            tools_detail: vec![ToolExecDetail {
                tool: "run_command".to_string(),
                success: false,
            }],
            ..ExecutionFeedback::default()
        };

        let suggestion =
            build_wiki_update_suggestion(&feedback, vec!["cargo test failed".to_string()])
                .expect("suggestion");

        assert_eq!(suggestion.trigger, WikiUpdateTrigger::Replan);
        assert_eq!(suggestion.replan_count, 1);
        assert_eq!(suggestion.failed_tools, vec!["run_command"]);
        assert!(suggestion.proposed_title.contains("fix wiki refresh"));
        assert!(suggestion.proposed_lesson.contains("## What Happened"));
        assert!(suggestion.proposed_lesson.contains("## Root Cause"));
        assert!(suggestion.proposed_lesson.contains("## Optimization"));
        assert!(suggestion.proposed_lesson.contains("## Next Time"));
    }

    #[test]
    fn wiki_suggestion_triggers_on_three_consecutive_tool_failures() {
        let feedback = ExecutionFeedback {
            failed_tools: 3,
            max_consecutive_tool_failures: 3,
            tools_detail: vec![
                ToolExecDetail {
                    tool: "read_file".to_string(),
                    success: false,
                },
                ToolExecDetail {
                    tool: "read_file".to_string(),
                    success: false,
                },
            ],
            ..ExecutionFeedback::default()
        };

        let suggestion = build_wiki_update_suggestion(&feedback, Vec::new()).expect("suggestion");

        assert_eq!(
            suggestion.trigger,
            WikiUpdateTrigger::ConsecutiveToolFailures
        );
        assert_eq!(suggestion.failed_tool_count, 3);
        assert_eq!(suggestion.failed_tools, vec!["read_file"]);
    }

    #[test]
    fn wiki_suggestion_skips_clean_runs() {
        let feedback = ExecutionFeedback {
            total_tools: 2,
            tools_detail: vec![ToolExecDetail {
                tool: "read_file".to_string(),
                success: true,
            }],
            ..ExecutionFeedback::default()
        };

        assert!(build_wiki_update_suggestion(&feedback, Vec::new()).is_none());
    }
}

/// Action type for skill evolution (generate new or refine existing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillAction {
    #[default]
    None,
    Generate,
    Refine,
}
