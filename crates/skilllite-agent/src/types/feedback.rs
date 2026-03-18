//! EVO-1: Execution feedback types for evolution engine.

use serde::{Deserialize, Serialize};

/// Structured feedback collected from each agent loop execution.
/// Used by the evolution engine to evaluate rule/skill effectiveness.
#[derive(Debug, Clone, Default)]
pub struct ExecutionFeedback {
    pub total_tools: usize,
    pub failed_tools: usize,
    pub replans: usize,
    pub iterations: usize,
    pub elapsed_ms: u64,
    pub context_overflow_retries: usize,
    pub task_completed: bool,
    /// Brief task description (generalized, not user's original text).
    pub task_description: Option<String>,
    /// Names of planning rules that were matched for this task.
    pub rules_used: Vec<String>,
    /// Per-tool execution details.
    pub tools_detail: Vec<ToolExecDetail>,
}

/// Per-tool execution outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecDetail {
    pub tool: String,
    pub success: bool,
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

/// Action type for skill evolution (generate new or refine existing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillAction {
    #[default]
    None,
    Generate,
    Refine,
}
