//! Task Planner: task planning and tracking for agentic loops.
//!
//! EVO-2: Prompt templates are loaded from `~/.skilllite/chat/prompts/` at runtime.
//! Compiled-in seed data provides the fallback when no external file exists.
//!
//! Responsibilities:
//! - Generate task list from user message using LLM
//! - Track task completion status
//! - Build execution and task system prompts (from external templates)
//! - Planning rules engine (rules loaded from file or seed)

use std::path::Path;

use anyhow::Result;

use skilllite_evolution::seed;
use super::goal_boundaries::GoalBoundaries;
use super::llm::LlmClient;
use super::planning_rules;
use super::skills::LoadedSkill;
use super::soul::Soul;
use super::types::*;

/// Resolve the output directory path for prompt injection.
fn resolve_output_dir() -> String {
    get_output_dir().unwrap_or_else(|| {
        skilllite_executor::skilllite_data_root()
            .join("chat")
            .join("output")
            .to_string_lossy()
            .to_string()
    })
}

/// Filter rules by user message: include rules with empty keywords (always) or
/// rules whose keywords match the user message.
fn filter_rules_for_user_message<'a>(rules: &'a [PlanningRule], user_message: &str) -> Vec<&'a PlanningRule> {
    let msg_lower = user_message.to_lowercase();
    rules
        .iter()
        .filter(|r| {
            if r.keywords.is_empty() && r.context_keywords.is_empty() {
                return true;
            }
            let matches_keywords = r.keywords.iter().any(|k| {
                user_message.contains(k.as_str()) || msg_lower.contains(&k.to_lowercase())
            });
            let matches_context = r.context_keywords.iter().any(|k| {
                user_message.contains(k.as_str()) || msg_lower.contains(&k.to_lowercase())
            });
            matches_keywords || matches_context
        })
        .collect()
}

/// Build the "## CRITICAL" rules section for the planning prompt.
fn build_rules_section(rules: &[PlanningRule]) -> String {
    if rules.is_empty() {
        return String::new();
    }
    let mut lines = vec![
        "## CRITICAL: When user explicitly requests a Skill, ALWAYS use it".to_string(),
        String::new(),
    ];
    for r in rules {
        let inst = r.instruction.trim();
        if !inst.is_empty() {
            lines.push(inst.to_string());
            lines.push(String::new());
        }
    }
    lines.join("\n").trim_end().to_string()
}

// ─── TaskPlanner ────────────────────────────────────────────────────────────

/// Task planner: generates and tracks task lists for the agentic loop.
pub struct TaskPlanner {
    /// Current task list.
    pub task_list: Vec<Task>,
    /// Planning rules (loaded from file or seed).
    rules: Vec<PlanningRule>,
    /// Rules filtered by actually available skills (computed lazily).
    available_rules: Vec<PlanningRule>,
    /// Chat data root for loading prompt templates.
    chat_root: Option<std::path::PathBuf>,
    /// EVO-5: Workspace path for project-level prompt overrides.
    workspace: Option<std::path::PathBuf>,
}

impl TaskPlanner {
    /// Create a new TaskPlanner.
    ///
    /// `workspace`: per-project override directory.
    /// `chat_root`: `~/.skilllite/chat/` for loading prompts from the seed system.
    pub fn new(workspace: Option<&Path>, chat_root: Option<&Path>) -> Self {
        let rules = planning_rules::load_rules(workspace, chat_root);
        Self {
            task_list: Vec::new(),
            available_rules: rules.clone(),
            rules,
            chat_root: chat_root.map(|p| p.to_path_buf()),
            workspace: workspace.map(|p| p.to_path_buf()),
        }
    }

    /// Builtin tool_hint values that don't require a loaded skill.
    const BUILTIN_HINTS: &'static [&'static str] = &[
        "file_operation", "chat_history", "memory_write", "memory_search", "analysis",
    ];

    /// Filter out rules whose tool_hint references a skill that isn't loaded.
    fn filter_rules_by_available_skills(rules: &[PlanningRule], skills: &[LoadedSkill]) -> Vec<PlanningRule> {
        rules.iter().filter(|r| {
            match r.tool_hint.as_deref() {
                None => true,
                Some(hint) => Self::is_hint_available(hint, skills),
            }
        }).cloned().collect()
    }

    /// Check if a tool_hint is available (builtin or loaded skill).
    fn is_hint_available(hint: &str, skills: &[LoadedSkill]) -> bool {
        Self::BUILTIN_HINTS.contains(&hint)
            || skills.iter().any(|s| {
                s.name == hint
                || s.name.replace('-', "_") == hint.replace('-', "_")
                || s.tool_definitions.iter().any(|td| td.function.name == hint.replace('-', "_"))
            })
    }

    /// Strip tool_hints that reference unavailable skills from LLM-generated tasks.
    /// The LLM may hallucinate tool names even when they're not in the prompt.
    fn sanitize_task_hints(tasks: &mut [Task], skills: &[LoadedSkill]) {
        for task in tasks.iter_mut() {
            if let Some(ref hint) = task.tool_hint {
                if !Self::is_hint_available(hint, skills) {
                    tracing::info!(
                        "Stripped unavailable tool_hint '{}' from task {}: {}",
                        hint, task.id, task.description
                    );
                    task.tool_hint = None;
                }
            }
        }
    }

    /// Generate task list from user message using LLM.
    ///
    /// `goal_boundaries`: Optional extracted boundaries (scope, exclusions, completion conditions)
    /// to inject into planning. Used in run mode for long-running tasks.
    /// `soul`: Optional SOUL identity document; when present, Scope & Boundaries are injected (A8).
    pub async fn generate_task_list(
        &mut self,
        client: &LlmClient,
        model: &str,
        user_message: &str,
        skills: &[LoadedSkill],
        conversation_context: Option<&str>,
        goal_boundaries: Option<&GoalBoundaries>,
        soul: Option<&Soul>,
    ) -> Result<Vec<Task>> {
        self.available_rules = Self::filter_rules_by_available_skills(&self.rules, skills);

        let skills_info = if skills.is_empty() {
            "None".to_string()
        } else {
            skills
                .iter()
                .map(|s| {
                    let desc = s
                        .metadata
                        .description
                        .as_deref()
                        .unwrap_or("No description");
                    format!("- **{}**: {}", s.name, desc)
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let planning_prompt = self.build_planning_prompt(&skills_info, user_message, Some(model), soul);

        let mut user_content = format!(
            "**PRIMARY — Plan ONLY based on this**:\nUser request: {}\n\n",
            user_message
        );
        // A5: Inject goal boundaries when available (run mode)
        if let Some(gb) = goal_boundaries {
            if !gb.is_empty() {
                user_content.push_str(&gb.to_planning_block());
                user_content.push_str("\n\n");
            }
        }
        if let Some(ctx) = conversation_context {
            let needs_continue_context = user_message.contains("继续")
                || user_message.contains("继续之前")
                || user_message.contains("继续任务");
            if needs_continue_context {
                user_content.push_str(&format!(
                    "Conversation context (use ONLY when user says 继续 — to understand what to continue):\n{}\n\n",
                    ctx
                ));
            } else {
                let max_ctx_bytes = 2000;
                let ctx_truncated = if ctx.len() > max_ctx_bytes {
                    format!(
                        "{}...[truncated, {} bytes total]",
                        safe_truncate(ctx, max_ctx_bytes),
                        ctx.len()
                    )
                } else {
                    ctx.to_string()
                };
                user_content.push_str(&format!(
                    "Conversation context (REFERENCE ONLY — user's request above is the PRIMARY input; do NOT plan based on previous tasks in context):\n{}\n\n",
                    ctx_truncated
                ));
            }
        }
        user_content.push_str("Generate task list based on the User request above:");

        let messages = vec![
            ChatMessage::system(&planning_prompt),
            ChatMessage::user(&user_content),
        ];

        match client
            .chat_completion(model, &messages, None, Some(0.3))
            .await
        {
            Ok(resp) => {
                let raw = resp
                    .choices
                    .first()
                    .and_then(|c| c.message.content.clone())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "[]".to_string());

                match self.parse_task_list(&raw) {
                    Ok(mut tasks) => {
                        Self::sanitize_task_hints(&mut tasks, skills);
                        self.auto_enhance_tasks(&mut tasks);
                        self.task_list = tasks.clone();
                        Ok(tasks)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse task list: {}", e);
                        let fallback = vec![Task {
                            id: 1,
                            description: user_message.to_string(),
                            tool_hint: None,
                            completed: false,
                        }];
                        self.task_list = fallback.clone();
                        Ok(fallback)
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Task planning LLM call failed: {}", e);
                let fallback = vec![Task {
                    id: 1,
                    description: user_message.to_string(),
                    tool_hint: None,
                    completed: false,
                }];
                self.task_list = fallback.clone();
                Ok(fallback)
            }
        }
    }

    /// Parse the LLM response into a task list.
    ///
    /// Handles common LLM output quirks:
    /// - `<think>…</think>` reasoning blocks (MiniMax, DeepSeek, Qwen3)
    /// - Markdown code fences around JSON
    /// - Natural-language preamble before the JSON array
    /// - Empty or whitespace-only responses
    pub(crate) fn parse_task_list(&self, raw: &str) -> Result<Vec<Task>> {
        let mut cleaned = raw.trim().to_string();

        // Strip <think>…</think> blocks (reasoning models)
        while let Some(start) = cleaned.find("<think>") {
            if let Some(end) = cleaned.find("</think>") {
                let end_tag_end = end + "</think>".len();
                cleaned = format!("{}{}", &cleaned[..start], &cleaned[end_tag_end..]);
            } else {
                // Unclosed <think> — drop everything from <think> onwards
                cleaned = cleaned[..start].to_string();
                break;
            }
        }
        let cleaned = cleaned.trim().to_string();

        if cleaned.is_empty() {
            anyhow::bail!("LLM returned empty content (after stripping think blocks)");
        }

        // Strip markdown code fences
        let cleaned = Self::strip_code_fences(&cleaned);

        // Try direct parse first (fast path)
        if let Ok(tasks) = serde_json::from_str::<Vec<Task>>(&cleaned) {
            return Ok(tasks);
        }

        // Fallback: extract the first JSON array from the text
        if let Some(json_str) = Self::extract_json_array(&cleaned) {
            if let Ok(tasks) = serde_json::from_str::<Vec<Task>>(&json_str) {
                return Ok(tasks);
            }
        }

        tracing::debug!("parse_task_list raw (first 500 chars): {}", &raw[..raw.len().min(500)]);
        anyhow::bail!("No valid JSON task array found in LLM response")
    }

    /// Strip markdown code fences (```` ```json ... ``` ````) from content.
    fn strip_code_fences(s: &str) -> String {
        let mut cleaned = s.trim().to_string();
        if cleaned.starts_with("```json") {
            cleaned = cleaned[7..].to_string();
        } else if cleaned.starts_with("```") {
            cleaned = cleaned[3..].to_string();
        }
        if cleaned.ends_with("```") {
            cleaned = cleaned[..cleaned.len() - 3].to_string();
        }
        cleaned.trim().to_string()
    }

    /// Try to extract the first JSON array from mixed text content.
    /// Scans for `[` and finds the matching `]`, handling nesting.
    fn extract_json_array(s: &str) -> Option<String> {
        let start = s.find('[')?;
        let bytes = s.as_bytes();
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;
        for (i, &b) in bytes[start..].iter().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }
            match b {
                b'\\' if in_string => escape_next = true,
                b'"' => in_string = !in_string,
                b'[' if !in_string => depth += 1,
                b']' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(s[start..start + i + 1].to_string());
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Apply sanitize_task_hints and auto_enhance_tasks to a task list (e.g. from replan).
    /// Use this when accepting a new plan from update_task_plan so replan has the same
    /// validation and enhancement as initial planning.
    pub fn sanitize_and_enhance_tasks(&self, tasks: &mut Vec<Task>, skills: &[LoadedSkill]) {
        Self::sanitize_task_hints(tasks, skills);
        self.auto_enhance_tasks(tasks);
    }

    /// Auto-enhance tasks: add SKILL.md writing if skill creation is detected.
    fn auto_enhance_tasks(&self, tasks: &mut Vec<Task>) {
        let has_skill_creation = tasks.iter().any(|t| {
            let desc_lower = t.description.to_lowercase();
            let hint_lower = t
                .tool_hint
                .as_deref()
                .unwrap_or("")
                .to_lowercase();
            desc_lower.contains("skill-creator") || hint_lower.contains("skill-creator")
        });

        let has_skillmd_task = tasks.iter().any(|t| {
            let desc_lower = t.description.to_lowercase();
            desc_lower.contains("skill.md")
        });

        if has_skill_creation && !has_skillmd_task {
            let max_id = tasks.iter().map(|t| t.id).max().unwrap_or(0);
            tasks.push(Task {
                id: max_id + 1,
                description: "Use write_file to write actual SKILL.md content (skill description, usage, parameter documentation, etc.)".to_string(),
                tool_hint: Some("file_operation".to_string()),
                completed: false,
            });
        }
    }

    /// Build the planning prompt from the external template.
    /// Placeholders: {{TODAY}}, {{YESTERDAY}}, {{RULES_SECTION}}, {{SKILLS_INFO}},
    /// {{OUTPUT_DIR}}, {{EXAMPLES_SECTION}}, {{SOUL_SCOPE_BLOCK}} (A8).
    pub(crate) fn build_planning_prompt(
        &self,
        skills_info: &str,
        user_message: &str,
        model: Option<&str>,
        soul: Option<&Soul>,
    ) -> String {
        let compact = get_compact_planning(model);
        let rules_section = if compact {
            let filtered: Vec<&PlanningRule> = filter_rules_for_user_message(&self.available_rules, user_message);
            build_rules_section(&filtered.iter().map(|r| (*r).clone()).collect::<Vec<_>>())
        } else {
            build_rules_section(&self.available_rules)
        };
        let examples_section = if compact {
            planning_rules::compact_examples_section(user_message)
        } else {
            planning_rules::load_full_examples(self.chat_root.as_deref())
        };
        let output_dir = resolve_output_dir();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let yesterday = (chrono::Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();

        let template = seed::load_prompt_file_with_project(
            self.chat_root.as_deref().unwrap_or(Path::new("/nonexistent")),
            self.workspace.as_deref(),
            "planning.md",
            include_str!("seed/planning.seed.md"),
        );

        let soul_scope = soul
            .and_then(|s| s.to_planning_scope_block())
            .unwrap_or_default();

        template
            .replace("{{TODAY}}", &today)
            .replace("{{YESTERDAY}}", &yesterday)
            .replace("{{RULES_SECTION}}", &rules_section)
            .replace("{{SKILLS_INFO}}", skills_info)
            .replace("{{OUTPUT_DIR}}", &output_dir)
            .replace("{{EXAMPLES_SECTION}}", &examples_section)
            .replace("{{SOUL_SCOPE_BLOCK}}", &soul_scope)
    }

    /// Build the main execution system prompt from the external template.
    /// Placeholders: {{TODAY}}, {{YESTERDAY}}, {{SKILLS_LIST}}, {{OUTPUT_DIR}}.
    pub fn build_execution_prompt(&self, skills: &[LoadedSkill]) -> String {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let yesterday = (chrono::Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        let skills_list: Vec<String> = skills
            .iter()
            .map(|s| {
                let desc = s
                    .metadata
                    .description
                    .as_deref()
                    .unwrap_or("No description");
                if s.metadata.entry_point.is_empty() && !s.metadata.is_bash_tool_skill() {
                    format!("  - **{}**: {} ⛔ [Reference Only — NOT a callable tool, do NOT call it]", s.name, desc)
                } else {
                    format!("  - **{}**: {}", s.name, desc)
                }
            })
            .collect();
        let skills_list_str = skills_list.join("\n");
        let output_dir = resolve_output_dir();

        let template = seed::load_prompt_file_with_project(
            self.chat_root.as_deref().unwrap_or(Path::new("/nonexistent")),
            self.workspace.as_deref(),
            "execution.md",
            include_str!("seed/execution.seed.md"),
        );

        template
            .replace("{{TODAY}}", &today)
            .replace("{{YESTERDAY}}", &yesterday)
            .replace("{{SKILLS_LIST}}", &skills_list_str)
            .replace("{{OUTPUT_DIR}}", &output_dir)
    }

    /// Build system prompt with task list and execution guidance.
    ///
    /// `goal_boundaries`: Optional extracted boundaries to inject (A5, run mode).
    pub fn build_task_system_prompt(
        &self,
        skills: &[LoadedSkill],
        goal_boundaries: Option<&GoalBoundaries>,
    ) -> String {
        let execution_prompt = self.build_execution_prompt(skills);
        let task_list_json =
            serde_json::to_string_pretty(&self.task_list).unwrap_or_else(|_| "[]".to_string());

        let current_task = self.task_list.iter().find(|t| !t.completed);
        let mut current_task_info = String::new();
        let mut direct_call_instruction = String::new();

        if let Some(task) = current_task {
            let hint_str = task
                .tool_hint
                .as_deref()
                .map(|h| format!("(Suggested tool: {})", h))
                .unwrap_or_default();
            current_task_info = format!(
                "\n\n🎯 **Current task to execute**: Task {} - {} {}",
                task.id, task.description, hint_str
            );

            if let Some(ref hint) = task.tool_hint {
                if hint == "file_operation" {
                    direct_call_instruction = format!(
                        "\n\n⚡ **ACTION REQUIRED**: This is a file_operation task. \
                         Call `write_output` or `preview_server` NOW.\n\
                         ⛔ Do NOT call any skill tools (skill-creator, frontend-design, etc.). \
                         Generate the content yourself and save with `write_output`."
                    );
                } else if hint == "memory_search" {
                    direct_call_instruction = "\n\n⚡ **ACTION REQUIRED**: This is an exploration task. Call `memory_search` NOW with appropriate query to find relevant memory.".to_string();
                } else if hint != "analysis" {
                    let is_skill = skills.iter().any(|s| {
                        s.name == *hint
                            || s.tool_definitions.iter().any(|td| td.function.name == *hint)
                    });
                    if is_skill {
                        direct_call_instruction = format!(
                            "\n\n⚡ **DIRECT ACTION REQUIRED**: Call `{}` NOW with appropriate parameters.\n\
                             Do NOT call list_directory, read_file, or any other tool first. \
                             The skill `{}` is ready to use.",
                            hint, hint
                        );
                    }
                }
            }
        }

        let boundaries_block = match goal_boundaries {
            Some(gb) if !gb.is_empty() => format!("\n\n{}\n", gb.to_planning_block()),
            _ => String::new(),
        };

        format!(
            "{}{}\n\
             ---\n\n\
             ## Current Task List\n\n\
             {}\n\n\
             ## Execution Rules\n\n\
             1. **MATCH tool_hint**: If tool_hint is \"file_operation\" → use built-in tools (read_file, write_output, preview_server). If tool_hint is \"memory_search\" → call memory_search. If tool_hint is a skill name → call that skill.\n\
             2. **Strict sequential execution**: Execute tasks in order, do not skip tasks\n\
             3. **Focus on current task**: Focus only on the current task\n\
             4. **Structured completion**: After finishing a task, call `complete_task(task_id=N)`. Writing \"Task N completed\" in text is NOT sufficient.\n\
             5. **Avoid unnecessary exploration**: Do NOT call list_directory or read_file unless the task explicitly requires it\n\
             6. **🚫 EXECUTE BEFORE COMPLETING**: Your first response must be an actual tool call, NOT a completion summary. Call `complete_task` only AFTER the work is done.\n\
             {}{}\n\n\
             ⚠️ **Important**: After completing each task, you MUST call `complete_task(task_id=N)` so the system can track progress. Text declarations are ignored.",
            execution_prompt,
            boundaries_block,
            task_list_json,
            current_task_info,
            direct_call_instruction
        )
    }

    /// Check if tasks were completed based on LLM response content.
    /// Supports both English and Chinese completion phrases.
    pub fn check_completion_in_content(&self, content: &str) -> Vec<u32> {
        if content.is_empty() {
            return Vec::new();
        }
        let content_lower = content.to_lowercase();
        let content_zh = content;
        let mut completed_ids = Vec::new();
        for task in &self.task_list {
            if !task.completed {
                // English patterns
                let pattern1 = format!("task {} completed", task.id);
                let pattern2 = format!("task{} completed", task.id);
                let pattern3 = format!("task {} complete", task.id);
                let pattern4 = format!("✅ task {}", task.id);
                let en_match = content_lower.contains(&pattern1)
                    || content_lower.contains(&pattern2)
                    || content_lower.contains(&pattern3)
                    || content_lower.contains(&pattern4);

                // Chinese patterns (LLM often responds in Chinese). Require explicit task id
                // to avoid false completion on phrases like "我将把任务标记为已完成".
                let zh_task = format!("任务{}已完成", task.id);
                let zh_task_spaced = format!("任务 {} 已完成", task.id);
                let zh_marked = format!("任务{}为已完成", task.id);
                let zh_marked_spaced = format!("任务 {} 为已完成", task.id);
                let zh_match = content_zh.contains(&zh_task)
                    || content_zh.contains(&zh_task_spaced)
                    || content_zh.contains(&zh_marked)
                    || content_zh.contains(&zh_marked_spaced);

                if en_match || zh_match {
                    completed_ids.push(task.id);
                }
            }
        }
        completed_ids
    }

    /// Mark a task as completed and return whether it was found.
    pub fn mark_completed(&mut self, task_id: u32) -> bool {
        for task in &mut self.task_list {
            if task.id == task_id {
                task.completed = true;
                return true;
            }
        }
        false
    }

    /// Check if all tasks are completed.
    ///
    /// Returns `false` when the task list is empty: an empty plan means the LLM
    /// decided no explicit tasks were needed, which is *not* the same as having
    /// finished a set of tasks.  Treating an empty list as "all done" causes
    /// `run_with_task_planning` to fire the "final summary" branch immediately
    /// after the first batch of tool calls, ending the loop prematurely.
    pub fn all_completed(&self) -> bool {
        !self.task_list.is_empty() && self.task_list.iter().all(|t| t.completed)
    }

    /// Check if the task list is empty (LLM decided no tools needed).
    pub fn is_empty(&self) -> bool {
        self.task_list.is_empty()
    }

    /// Get the current (first uncompleted) task.
    pub fn current_task(&self) -> Option<&Task> {
        self.task_list.iter().find(|t| !t.completed)
    }

    /// Build a nudge message to push the LLM to continue working on tasks.
    pub fn build_nudge_message(&self) -> Option<String> {
        let current = self.current_task()?;
        let task_list_json =
            serde_json::to_string_pretty(&self.task_list).unwrap_or_else(|_| "[]".to_string());

        let tool_instruction = if let Some(ref hint) = current.tool_hint {
            if hint != "file_operation" && hint != "analysis" {
                format!(
                    "\n⚡ Call `{}` DIRECTLY now. Do NOT call list_directory or read_file first.",
                    hint
                )
            } else {
                "\nPlease use the available tools to complete this task.".to_string()
            }
        } else {
            "\nPlease use the available tools to complete this task.".to_string()
        };

        Some(format!(
            "There are still pending tasks. Please continue.\n\n\
             Updated task list:\n{}\n\n\
             Current task: Task {} - {}\n{}\n\n\
             ⚠️ After completing this task, call `complete_task(task_id={})` to record completion.\n\
             If the current plan no longer fits the goal, you may call `update_task_plan` to revise the plan, then continue.",
            task_list_json, current.id, current.description, tool_instruction, current.id
        ))
    }

    /// Build a per-task depth limit message.
    /// Suggests complete_task first; if the approach is wrong, suggests update_task_plan.
    pub fn build_depth_limit_message(&self, max_calls: usize) -> String {
        let current_id = self.current_task().map(|t| t.id).unwrap_or(0);
        format!(
            "You have used {} tool calls for the current task. \
             Based on the information gathered so far, call \
             `complete_task(task_id={})` to record completion, then proceed to the next task. \
             If the current approach is clearly wrong or the plan no longer fits the goal, \
             you may call `update_task_plan` with a revised task list instead, then continue with the new plan.",
            max_calls, current_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planning_prompt_phase1_balance() {
        let planner = TaskPlanner::new(None, None);
        let prompt = planner.build_planning_prompt("None", "hello", None, None);

        assert!(
            prompt.contains("First: Check if `[]` is correct"),
            "prompt should contain First check for []"
        );
        assert!(
            prompt.contains("Prefer `[]`"),
            "prompt should contain Prefer [] in output format"
        );
        assert!(
            prompt.contains("Minimize Tool Usage"),
            "prompt should contain Core Principle"
        );
        assert!(
            prompt.contains("Only when tools are needed"),
            "prompt should qualify when to apply heuristics"
        );
        assert!(
            prompt.contains("Three-phase model"),
            "prompt should contain three-phase model"
        );
    }

    #[test]
    fn test_parse_task_list() {
        let planner = TaskPlanner::new(None, None);

        let json = r#"[{"id": 1, "description": "Use grep_files", "tool_hint": "file_operation", "completed": false}]"#;
        let tasks = planner.parse_task_list(json).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].description, "Use grep_files");
        assert_eq!(tasks[0].tool_hint.as_deref(), Some("file_operation"));

        let empty = planner.parse_task_list("[]").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_planning_prompt_contains_placeholders_resolved() {
        let planner = TaskPlanner::new(None, None);
        let prompt = planner.build_planning_prompt("None", "hello", None, None);

        // All placeholders should be resolved (no {{...}} remaining)
        assert!(!prompt.contains("{{TODAY}}"));
        assert!(!prompt.contains("{{YESTERDAY}}"));
        assert!(!prompt.contains("{{RULES_SECTION}}"));
        assert!(!prompt.contains("{{SKILLS_INFO}}"));
        assert!(!prompt.contains("{{OUTPUT_DIR}}"));
        assert!(!prompt.contains("{{EXAMPLES_SECTION}}"));
        assert!(!prompt.contains("{{SOUL_SCOPE_BLOCK}}"));
    }

    #[test]
    fn test_execution_prompt_contains_placeholders_resolved() {
        let planner = TaskPlanner::new(None, None);
        let prompt = planner.build_execution_prompt(&[]);

        assert!(!prompt.contains("{{TODAY}}"));
        assert!(!prompt.contains("{{YESTERDAY}}"));
        assert!(!prompt.contains("{{SKILLS_LIST}}"));
        assert!(!prompt.contains("{{OUTPUT_DIR}}"));
    }
}
