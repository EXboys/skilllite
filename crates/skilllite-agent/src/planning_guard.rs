//! Lightweight guards around task planning results.

use crate::types::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FallbackPlanKind {
    InspectOnly,
    ChangeAndVerify,
}

#[derive(Debug, Clone)]
pub struct EmptyPlanGuardResult {
    pub reason: &'static str,
    pub fallback_tasks: Vec<Task>,
}

fn task(id: u32, description: &str, tool_hint: Option<&str>) -> Task {
    Task {
        id,
        description: description.to_string(),
        tool_hint: tool_hint.map(|s| s.to_string()),
        completed: false,
    }
}

fn classify_fallback_plan(user_message: &str) -> Option<FallbackPlanKind> {
    let lower = user_message.to_lowercase();
    let path_signals = [
        "crates/",
        "src/",
        "docs/",
        ".rs",
        ".md",
        ".json",
        ".toml",
        ".yaml",
        ".yml",
        ".meta.json",
        "执行路径",
        "task_planner",
        "agent_loop",
        "executionfeedback",
        "feedbacksignal",
        "rules_used",
        "first_success_rate",
        "avg_replans",
        "user_correction_rate",
        "rule id",
    ];
    let action_signals = [
        "修改", "更新", "补", "修复", "接入", "接线", "增加", "添加", "整理", "写入", "输出",
        "导出", "实现", "make ", "update ", "wire ", "add ", "fix ", "edit ",
    ];
    let verification_signals = [
        "测试",
        "单测",
        "校验",
        "验证",
        "核对",
        "确保",
        "检查",
        "确认",
        "status",
        "metrics",
        "replay",
        "evolution",
        "benchmark",
        "eval",
        "verify",
        "test ",
    ];

    let has_path_signal = path_signals.iter().any(|s| lower.contains(s));
    let has_action_signal = action_signals.iter().any(|s| lower.contains(s));
    let has_verification_signal = verification_signals.iter().any(|s| lower.contains(s));

    if (has_path_signal && has_action_signal) || (has_action_signal && has_verification_signal) {
        Some(FallbackPlanKind::ChangeAndVerify)
    } else if has_path_signal && has_verification_signal {
        Some(FallbackPlanKind::InspectOnly)
    } else {
        None
    }
}

fn build_fallback_tasks(kind: FallbackPlanKind) -> Vec<Task> {
    match kind {
        FallbackPlanKind::InspectOnly => vec![
            task(
                1,
                "Use read_file to inspect the relevant files or implementation details first.",
                Some("file_read"),
            ),
            task(
                2,
                "Analyze whether the current implementation satisfies the user's request.",
                Some("analysis"),
            ),
        ],
        FallbackPlanKind::ChangeAndVerify => vec![
            task(
                1,
                "Use read_file to inspect the relevant files and confirm the current implementation.",
                Some("file_read"),
            ),
            task(
                2,
                "Use file_edit to make the required code or content changes.",
                Some("file_edit"),
            ),
            task(
                3,
                "Run a focused verification step or analyze the updated result to confirm correctness.",
                Some("command"),
            ),
        ],
    }
}

/// Reject empty plans for requests that clearly need file/code/test work.
pub fn guard_empty_plan(user_message: &str) -> Option<EmptyPlanGuardResult> {
    let kind = classify_fallback_plan(user_message)?;
    Some(EmptyPlanGuardResult {
        reason: "empty plan rejected by centralized planning guard",
        fallback_tasks: build_fallback_tasks(kind),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_empty_plan_rejects_code_change_requests() {
        let guard = guard_empty_plan(
            "在 crates/skilllite-agent/src/task_planner.rs 里补一个单测并验证 rules_used",
        )
        .unwrap();
        assert_eq!(guard.fallback_tasks.len(), 3);
        assert_eq!(
            guard.fallback_tasks[0].tool_hint.as_deref(),
            Some("file_read")
        );
        assert_eq!(
            guard.fallback_tasks[1].tool_hint.as_deref(),
            Some("file_edit")
        );
    }

    #[test]
    fn test_guard_empty_plan_allows_pure_text_requests() {
        assert!(guard_empty_plan("帮我解释一下什么是 first_success_rate").is_none());
    }
}
