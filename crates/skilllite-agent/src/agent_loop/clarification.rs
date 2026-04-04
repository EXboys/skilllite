//! Clarification sub-module: reusable clarification-request pattern.
//!
//! Encapsulates the repeated pattern of requesting clarification from the user
//! when the agent hits a stopping condition (max iterations, consecutive
//! failures, tool call limits, etc.).

use super::super::types::{safe_truncate, ToolExecDetail, *};

use crate::task_planner::TaskPlanner;

/// Maximum number of clarification round-trips before the agent stops unconditionally.
pub(super) const MAX_CLARIFICATIONS: usize = 3;

/// UTF-8 byte cap for embedding task descriptions in user-visible clarification text.
const TASK_DESC_SNIPPET_BYTES: usize = 200;

/// Simple loop: model stopped making tool calls — tailored copy + one quick-reply (injected as user text).
pub(super) fn no_progress_simple_copy(
    iterations: usize,
    no_tool_retries: usize,
    tools_len: usize,
) -> (String, Vec<String>) {
    let tools_note = if tools_len == 0 {
        "当前无可调用工具，适合纯问答；若你其实需要改文件或跑命令，请改在支持工具的会话里执行。"
    } else {
        "当前有可调用工具，但模型多轮未使用；常见于任务描述过泛或模型在「说明」而非「执行」。"
    };
    let msg = format!(
        "执行已暂停（简单模式）。{tools_note}\n\
         已记录约 {iterations} 轮循环、无工具进展计数 {no_tool_retries}。\n\
         你可以直接点「继续运行」重试，或用下方快捷句补充路径、验收标准或约束。"
    );
    let chip = if tools_len > 0 {
        "请先调用工具实际推进：若缺仓库路径、文件位置或权限，请逐条告诉我。"
    } else {
        "若只需解释不需要执行，请说明「仅回答、不调用工具」；若需要执行，请切换到带工具能力的会话。"
    };
    (msg, vec![chip.to_string()])
}

/// Planning loop: same stop reason but grounded in the current pending task when possible.
pub(super) fn no_progress_planning_copy(
    planner: &TaskPlanner,
    consecutive_no_tool: usize,
) -> (String, Vec<String>) {
    let current = planner.current_task();
    let (focus_line, chip) = match current {
        Some(t) => {
            let brief = safe_truncate(t.description.trim(), TASK_DESC_SNIPPET_BYTES);
            let line = format!("当前待办：Task {} — {}", t.id, brief);
            let c = format!(
                "请优先推进 Task {}：{}。若被路径/权限/依赖卡住，请说明错误信息或你还缺什么输入。",
                t.id,
                safe_truncate(t.description.trim(), 80)
            );
            (line, c)
        }
        None => {
            if planner.is_empty() {
                (
                    "当前没有任务步骤（模型按纯对话处理）。若需要多步执行，请说明具体目标与交付物。"
                        .to_string(),
                    "请把目标拆成可验证的步骤（例如要先读哪些文件、改哪里、如何验收）。"
                        .to_string(),
                )
            } else {
                (
                    "待办列表非空，但暂时无法定位「当前步骤」（可考虑在下一轮要求模型更新任务计划）。"
                        .to_string(),
                    "若目标已变化，请用一句话写清新的优先级或交付物，我会据此继续。".to_string(),
                )
            }
        }
    };
    let msg = format!(
        "执行已暂停（任务规划模式）。已连续 {consecutive_no_tool} 轮没有看到有效的工具调用或结构化推进。\n{focus_line}"
    );
    (msg, vec![chip])
}

/// Last failed tool name when available (best-effort).
pub(super) fn too_many_failures_message(
    consecutive: usize,
    tools_detail: &[ToolExecDetail],
) -> String {
    let last_fail = tools_detail
        .iter()
        .rev()
        .find(|d| !d.success)
        .map(|d| d.tool.as_str());
    match last_fail {
        Some(name) => format!(
            "工具「{name}」等已连续失败 {consecutive} 次，可能是参数、环境或权限问题。"
        ),
        None => format!("工具执行已连续失败 {consecutive} 次，可能是环境或权限问题。"),
    }
}

/// Single quick-reply for iteration cap (UI already offers bare «continue»).
pub(super) const CHIP_NARROW_SCOPE: &str = "请先完成我最关心的这一部分（可缩小范围）：";

/// Single quick-reply after global tool-call budget.
pub(super) fn tool_limit_chip(total_tool_calls: usize) -> String {
    format!(
        "工具已用 {total_tool_calls} 次仍不够的话，请帮我砍掉非核心步骤，只保留必须交付："
    )
}

/// What the caller should do after a clarification attempt.
pub(super) enum ClarifyAction {
    /// User chose to continue; any hint was already pushed to `messages`.
    Continue,
    /// Clarification was declined or the limit was reached.
    Declined,
}

/// Attempt a clarification request with the user.
///
/// If `clarification_count < MAX_CLARIFICATIONS` and the user responds with
/// `Continue`, increments `clarification_count`, pushes the hint (if any) to
/// `messages`, and returns `ClarifyAction::Continue`.
///
/// Otherwise returns `ClarifyAction::Declined`.
pub(super) fn try_clarify(
    reason: &str,
    message: &str,
    suggestions: &[&str],
    clarification_count: &mut usize,
    event_sink: &mut dyn EventSink,
    messages: &mut Vec<ChatMessage>,
) -> ClarifyAction {
    if *clarification_count >= MAX_CLARIFICATIONS {
        return ClarifyAction::Declined;
    }
    let req = ClarificationRequest {
        reason: reason.into(),
        message: message.into(),
        suggestions: suggestions.iter().map(|s| s.to_string()).collect(),
    };
    match event_sink.on_clarification_request(&req) {
        ClarificationResponse::Continue(hint) => {
            *clarification_count += 1;
            if let Some(h) = hint {
                messages.push(ChatMessage::user(&h));
            }
            ClarifyAction::Continue
        }
        ClarificationResponse::Stop => ClarifyAction::Declined,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_planner::TaskPlanner;
    use crate::types::Task;

    #[test]
    fn no_progress_planning_mentions_current_task() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 3,
            description: "Run tests in crates/foo".to_string(),
            tool_hint: None,
            completed: false,
        }];
        let (msg, sugg) = no_progress_planning_copy(&planner, 2);
        assert!(msg.contains("Task 3"));
        assert!(msg.contains("Run tests"));
        assert_eq!(sugg.len(), 1);
        assert!(sugg[0].contains("Task 3"));
    }

    #[test]
    fn no_progress_planning_empty_planner_copy_is_honest() {
        let planner = TaskPlanner::new(None, None, None);
        let (msg, sugg) = no_progress_planning_copy(&planner, 0);
        assert!(
            msg.contains("没有任务步骤") || msg.contains("纯对话"),
            "msg={msg}"
        );
        assert!(
            !msg.contains("待办列表非空"),
            "empty plan must not say 待办列表非空: {msg}"
        );
        assert_eq!(sugg.len(), 1);
    }

    #[test]
    fn too_many_failures_names_last_failed_tool() {
        let details = vec![
            ToolExecDetail {
                tool: "read_file".to_string(),
                success: true,
            },
            ToolExecDetail {
                tool: "run_command".to_string(),
                success: false,
            },
        ];
        let m = too_many_failures_message(2, &details);
        assert!(m.contains("run_command"));
        assert!(m.contains('2'));
    }
}
