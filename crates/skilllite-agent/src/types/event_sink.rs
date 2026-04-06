//! Event sink trait and implementations for different output targets.

use super::string_utils::safe_truncate;
use super::task::Task;

/// Structured request asking the user for clarification before the agent stops.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClarificationRequest {
    pub reason: String,
    pub message: String,
    pub suggestions: Vec<String>,
}

/// User's response to a clarification request.
#[derive(Debug, Clone)]
pub enum ClarificationResponse {
    /// Continue execution; optional hint injected as a user message.
    Continue(Option<String>),
    /// Stop the agent loop.
    Stop,
}

/// Machine-readable severity for confirmation gating (e.g. desktop auto-approve only `Low`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    /// Safe to auto-approve when the user enabled auto-confirm (e.g. generic `run_command`).
    Low,
    /// Must not be auto-approved; user must explicitly approve.
    ConfirmRequired,
}

/// Structured confirmation request (prefer this over parsing `prompt` text in UIs).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfirmationRequest {
    pub prompt: String,
    pub risk_tier: RiskTier,
}

impl ConfirmationRequest {
    pub fn new(prompt: impl Into<String>, risk_tier: RiskTier) -> Self {
        Self {
            prompt: prompt.into(),
            risk_tier,
        }
    }
}

/// Event sink trait for different output targets (CLI, RPC, SDK).
pub trait EventSink: Send {
    /// Called at the start of each conversation turn (before any other events).
    fn on_turn_start(&mut self) {}
    /// Reset streaming UI state before a new LLM request (same user turn may call the model many times).
    fn reset_streamed_text_for_llm_call(&mut self) {}
    /// Called when the assistant produces text content.
    fn on_text(&mut self, text: &str);
    /// Called when a tool is about to be invoked.
    fn on_tool_call(&mut self, name: &str, arguments: &str);
    /// Called when a tool returns a result.
    fn on_tool_result(&mut self, name: &str, result: &str, is_error: bool);
    /// Called when a command tool starts execution.
    fn on_command_started(&mut self, _command: &str) {}
    /// Called when a command tool emits incremental stdout/stderr output.
    fn on_command_output(&mut self, _stream: &str, _chunk: &str) {}
    /// Called when a command tool finishes execution.
    fn on_command_finished(&mut self, _success: bool, _exit_code: i32, _duration_ms: u64) {}
    /// Called when preview server startup begins.
    fn on_preview_started(&mut self, _path: &str, _port: u16) {}
    /// Called when preview server is ready.
    fn on_preview_ready(&mut self, _url: &str, _port: u16) {}
    /// Called when preview server startup fails.
    fn on_preview_failed(&mut self, _message: &str) {}
    /// Called when preview server stops.
    fn on_preview_stopped(&mut self, _reason: &str) {}
    /// Called when swarm delegation starts.
    fn on_swarm_started(&mut self, _description: &str) {}
    /// Called with lightweight swarm progress updates.
    fn on_swarm_progress(&mut self, _status: &str) {}
    /// Called when swarm delegation finishes with a summary.
    fn on_swarm_finished(&mut self, _summary: &str) {}
    /// Called when swarm delegation fails or falls back.
    fn on_swarm_failed(&mut self, _message: &str) {}
    /// Called when the agent needs user confirmation (tools, L3 security, etc.).
    /// Returns true if the user approves.
    fn on_confirmation_request(&mut self, request: &ConfirmationRequest) -> bool;
    /// Called for streaming text chunks.
    fn on_text_chunk(&mut self, _chunk: &str) {}
    /// Called when a task plan is generated. (Phase 2)
    fn on_task_plan(&mut self, _tasks: &[Task]) {}
    /// Called when a task's status changes. (Phase 2)
    /// `tasks` contains the full updated task list for progress rendering.
    fn on_task_progress(&mut self, _task_id: u32, _completed: bool, _tasks: &[Task]) {}
    /// Called when the agent is about to stop and wants user clarification.
    /// Returns `Continue(hint)` to keep going or `Stop` to terminate.
    fn on_clarification_request(
        &mut self,
        _request: &ClarificationRequest,
    ) -> ClarificationResponse {
        ClarificationResponse::Stop
    }
}

/// Silent event sink for background operations (e.g. pre-compaction memory flush).
/// Swallows all output and auto-approves confirmation requests.
pub struct SilentEventSink;

impl EventSink for SilentEventSink {
    fn on_text(&mut self, _text: &str) {}
    fn on_tool_call(&mut self, _name: &str, _arguments: &str) {}
    fn on_tool_result(&mut self, _name: &str, _result: &str, _is_error: bool) {}
    fn on_confirmation_request(&mut self, _request: &ConfirmationRequest) -> bool {
        true // Auto-approve for silent operations (memory flush may rarely need run_command)
    }
}

/// Separator for CLI section headers.
const SECTION_SEP: &str = "──────────────────────────────────────";

/// Simple terminal event sink for CLI chat.
pub struct TerminalEventSink {
    pub verbose: bool,
    streamed_text: bool,
    /// Whether we've shown the "执行" section header this turn.
    execution_section_shown: bool,
    /// Whether we've shown the "结果" section header this turn.
    result_section_shown: bool,
}

impl TerminalEventSink {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            streamed_text: false,
            execution_section_shown: false,
            result_section_shown: false,
        }
    }

    #[inline]
    fn msg(&self, s: &str) {
        eprintln!("{}", s);
    }

    #[inline]
    fn msg_opt(&self, s: &str) {
        if !s.is_empty() {
            for line in s.lines() {
                eprintln!("{}", line);
            }
        }
    }

    fn show_execution_section(&mut self) {
        if !self.execution_section_shown {
            self.execution_section_shown = true;
            self.msg(&format!("─── 🔧 执行 ─── {}", SECTION_SEP));
        }
    }

    fn show_result_section(&mut self) {
        if !self.result_section_shown {
            self.result_section_shown = true;
            self.msg(&format!("─── 📄 结果 ─── {}", SECTION_SEP));
            self.msg("");
        }
    }
}

impl EventSink for TerminalEventSink {
    fn on_turn_start(&mut self) {
        self.execution_section_shown = false;
        self.result_section_shown = false;
    }

    fn reset_streamed_text_for_llm_call(&mut self) {
        self.streamed_text = false;
    }

    fn on_text(&mut self, text: &str) {
        if self.streamed_text {
            // Text was already displayed chunk-by-chunk via on_text_chunk.
            // The trailing newline was also added by accumulate_stream.
            // Just reset the flag for the next response.
            self.streamed_text = false;
            return;
        }
        // Non-streaming path: display full text + newline
        // Only show result section when we have actual content (avoids empty "结果" between plan and execution)
        if !text.trim().is_empty() {
            self.show_result_section();
        }
        use std::io::Write;
        print!("{}", text);
        let _ = std::io::stdout().flush();
        println!();
    }

    fn on_text_chunk(&mut self, chunk: &str) {
        self.streamed_text = true;
        // Only show result section when we have actual content (avoids empty "结果" between plan and execution)
        if !chunk.trim().is_empty() {
            self.show_result_section();
        }
        use std::io::Write;
        print!("{}", chunk);
        let _ = std::io::stdout().flush();
    }

    fn on_tool_call(&mut self, name: &str, arguments: &str) {
        self.show_execution_section();
        if self.verbose {
            // Truncate long JSON args for display
            let args_display = if arguments.len() > 200 {
                format!("{}…", safe_truncate(arguments, 200))
            } else {
                arguments.to_string()
            };
            self.msg(&format!("🔧 Tool: {}  args={}", name, args_display));
        } else {
            self.msg(&format!("🔧 {}", name));
        }
    }

    fn on_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        let icon = if is_error { "❌" } else { "✅" };
        if self.verbose {
            let brief = if result.len() > 400 {
                format!("{}…", safe_truncate(result, 400))
            } else {
                result.to_string()
            };
            self.msg(&format!("  {} {}: {}", icon, name, brief));
        } else {
            let first = result.lines().next().unwrap_or("(ok)");
            let brief = if first.len() > 80 {
                format!("{}…", safe_truncate(first, 80))
            } else {
                first.to_string()
            };
            self.msg(&format!("  {} {} {}", icon, name, brief));
        }
    }

    fn on_command_started(&mut self, command: &str) {
        self.show_execution_section();
        let brief = if command.len() > 120 {
            format!("{}…", safe_truncate(command, 120))
        } else {
            command.to_string()
        };
        self.msg(&format!("  ▶ command started: {}", brief));
    }

    fn on_command_output(&mut self, stream: &str, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        self.show_execution_section();
        let prefix = if stream == "stderr" { "  ! " } else { "  │ " };
        for line in chunk.lines() {
            self.msg(&format!("{}{}", prefix, line));
        }
    }

    fn on_command_finished(&mut self, success: bool, exit_code: i32, duration_ms: u64) {
        self.show_execution_section();
        let icon = if success { "  ■" } else { "  ✗" };
        self.msg(&format!(
            "{} command finished: exit {} ({} ms)",
            icon, exit_code, duration_ms
        ));
    }

    fn on_preview_started(&mut self, path: &str, port: u16) {
        self.show_execution_section();
        self.msg(&format!("  ▶ preview started: {} (port {})", path, port));
    }

    fn on_preview_ready(&mut self, url: &str, _port: u16) {
        self.show_execution_section();
        self.msg(&format!("  ■ preview ready: {}", url));
    }

    fn on_preview_failed(&mut self, message: &str) {
        self.show_execution_section();
        self.msg(&format!("  ✗ preview failed: {}", message));
    }

    fn on_preview_stopped(&mut self, reason: &str) {
        self.show_execution_section();
        self.msg(&format!("  ■ preview stopped: {}", reason));
    }

    fn on_swarm_started(&mut self, description: &str) {
        self.show_execution_section();
        let brief = if description.len() > 120 {
            format!("{}…", safe_truncate(description, 120))
        } else {
            description.to_string()
        };
        self.msg(&format!("  ▶ swarm started: {}", brief));
    }

    fn on_swarm_progress(&mut self, status: &str) {
        self.show_execution_section();
        self.msg(&format!("  … swarm: {}", status));
    }

    fn on_swarm_finished(&mut self, summary: &str) {
        self.show_execution_section();
        let brief = if summary.len() > 160 {
            format!("{}…", safe_truncate(summary, 160))
        } else {
            summary.to_string()
        };
        self.msg(&format!("  ■ swarm finished: {}", brief));
    }

    fn on_swarm_failed(&mut self, message: &str) {
        self.show_execution_section();
        let brief = if message.len() > 160 {
            format!("{}…", safe_truncate(message, 160))
        } else {
            message.to_string()
        };
        self.msg(&format!("  ✗ swarm failed: {}", brief));
    }

    fn on_confirmation_request(&mut self, request: &ConfirmationRequest) -> bool {
        use std::io::Write;
        self.msg_opt(&request.prompt);
        eprint!("确认执行? [y/N] ");
        let _ = std::io::stderr().flush();
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            trimmed == "y" || trimmed == "yes"
        } else {
            false
        }
    }

    fn on_clarification_request(
        &mut self,
        request: &ClarificationRequest,
    ) -> ClarificationResponse {
        use std::io::Write;
        self.msg(&format!("─── ⚠ 需要确认 ─── {}", SECTION_SEP));
        self.msg(&format!("原因: {}", request.reason));
        self.msg(&request.message);
        self.msg("");
        for (i, s) in request.suggestions.iter().enumerate() {
            self.msg(&format!("  [{}] {}", i + 1, s));
        }
        self.msg("  [0] 停止");
        eprint!("请选择 (或直接输入补充信息): ");
        let _ = std::io::stderr().flush();
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim();
            if trimmed == "0" {
                return ClarificationResponse::Stop;
            }
            if let Ok(idx) = trimmed.parse::<usize>() {
                if idx >= 1 && idx <= request.suggestions.len() {
                    return ClarificationResponse::Continue(Some(
                        request.suggestions[idx - 1].clone(),
                    ));
                }
            }
            if !trimmed.is_empty() {
                return ClarificationResponse::Continue(Some(trimmed.to_string()));
            }
        }
        ClarificationResponse::Stop
    }

    fn on_task_plan(&mut self, tasks: &[Task]) {
        self.msg(&format!("─── 📋 计划 ─── {}", SECTION_SEP));
        self.msg(&format!("Task plan ({} tasks):", tasks.len()));
        for task in tasks {
            let status = if task.completed { "✅" } else { "○" };
            let hint = task
                .tool_hint
                .as_deref()
                .map(|h| format!(" [{}]", h))
                .unwrap_or_default();
            self.msg(&format!(
                "   {}. {} {}{}",
                task.id, status, task.description, hint
            ));
        }
    }

    fn on_task_progress(&mut self, task_id: u32, completed: bool, tasks: &[Task]) {
        if completed {
            self.msg(&format!("  ✅ Task {} completed", task_id));
        }
        if !tasks.is_empty() {
            let completed_count = tasks.iter().filter(|t| t.completed).count();
            self.msg(&format!("  📋 进度 ({}/{}):", completed_count, tasks.len()));
            for task in tasks {
                let status = if task.completed {
                    "✅"
                } else if task.id
                    == tasks
                        .iter()
                        .find(|t| !t.completed)
                        .map(|t| t.id)
                        .unwrap_or(0)
                {
                    "▶"
                } else {
                    "○"
                };
                let hint = task
                    .tool_hint
                    .as_deref()
                    .map(|h| format!(" [{}]", h))
                    .unwrap_or_default();
                self.msg(&format!(
                    "     {}. {} {}{}",
                    task.id, status, task.description, hint
                ));
            }
        }
    }
}

/// Event sink for unattended run mode: same output as TerminalEventSink,
/// but auto-approves confirmation requests (run_command, L3 skill scan).
/// Replan (update_task_plan) never waits — agent continues immediately.
pub struct RunModeEventSink {
    inner: TerminalEventSink,
}

impl RunModeEventSink {
    pub fn new(verbose: bool) -> Self {
        Self {
            inner: TerminalEventSink::new(verbose),
        }
    }
}

impl EventSink for RunModeEventSink {
    fn on_turn_start(&mut self) {
        self.inner.on_turn_start();
    }
    fn reset_streamed_text_for_llm_call(&mut self) {
        self.inner.reset_streamed_text_for_llm_call();
    }
    fn on_text(&mut self, text: &str) {
        self.inner.on_text(text);
    }
    fn on_text_chunk(&mut self, chunk: &str) {
        self.inner.on_text_chunk(chunk);
    }
    fn on_tool_call(&mut self, name: &str, arguments: &str) {
        self.inner.on_tool_call(name, arguments);
    }
    fn on_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        self.inner.on_tool_result(name, result, is_error);
    }
    fn on_command_started(&mut self, command: &str) {
        self.inner.on_command_started(command);
    }
    fn on_command_output(&mut self, stream: &str, chunk: &str) {
        self.inner.on_command_output(stream, chunk);
    }
    fn on_command_finished(&mut self, success: bool, exit_code: i32, duration_ms: u64) {
        self.inner
            .on_command_finished(success, exit_code, duration_ms);
    }
    fn on_preview_started(&mut self, path: &str, port: u16) {
        self.inner.on_preview_started(path, port);
    }
    fn on_preview_ready(&mut self, url: &str, port: u16) {
        self.inner.on_preview_ready(url, port);
    }
    fn on_preview_failed(&mut self, message: &str) {
        self.inner.on_preview_failed(message);
    }
    fn on_preview_stopped(&mut self, reason: &str) {
        self.inner.on_preview_stopped(reason);
    }
    fn on_swarm_started(&mut self, description: &str) {
        self.inner.on_swarm_started(description);
    }
    fn on_swarm_progress(&mut self, status: &str) {
        self.inner.on_swarm_progress(status);
    }
    fn on_swarm_finished(&mut self, summary: &str) {
        self.inner.on_swarm_finished(summary);
    }
    fn on_swarm_failed(&mut self, message: &str) {
        self.inner.on_swarm_failed(message);
    }
    fn on_confirmation_request(&mut self, request: &ConfirmationRequest) -> bool {
        if !request.prompt.is_empty() {
            for line in request.prompt.lines() {
                eprintln!("{}", line);
            }
        }
        eprintln!("  [run mode: auto-approved]");
        true
    }
    fn on_clarification_request(
        &mut self,
        request: &ClarificationRequest,
    ) -> ClarificationResponse {
        eprintln!(
            "  [run mode: auto-stop on clarification] reason={} msg={}",
            request.reason, request.message
        );
        ClarificationResponse::Stop
    }
    fn on_task_plan(&mut self, tasks: &[Task]) {
        self.inner.on_task_plan(tasks);
    }
    fn on_task_progress(&mut self, task_id: u32, completed: bool, tasks: &[Task]) {
        self.inner.on_task_progress(task_id, completed, tasks);
    }
}
