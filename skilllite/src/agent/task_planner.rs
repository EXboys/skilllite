//! Task Planner: task planning and tracking for agentic loops.
//!
//! Ported from Python `core/task_planner.py` + `config/planning_rules.py`.
//!
//! Responsibilities:
//! - Generate task list from user message using LLM
//! - Track task completion status
//! - Build execution and task system prompts
//! - Planning rules engine (rules defined in planning_rules.rs)

use anyhow::Result;

use super::llm::LlmClient;
use super::planning_rules;
use super::skills::LoadedSkill;
use super::types::*;

/// Resolve the output directory path for prompt injection.
fn resolve_output_dir() -> String {
    get_output_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".skilllite")
            .join("chat")
            .join("output")
            .to_string_lossy()
            .to_string()
    })
}

/// Load planning rules. Uses built-in rules from planning_rules.rs.
fn load_planning_rules() -> Vec<PlanningRule> {
    planning_rules::builtin_rules()
}

/// Filter rules by user message: include rules with empty keywords (always) or
/// rules whose keywords match the user message. Reduces prompt size when compact mode is on.
fn filter_rules_for_user_message<'a>(rules: &'a [PlanningRule], user_message: &str) -> Vec<&'a PlanningRule> {
    let msg_lower = user_message.to_lowercase();
    rules
        .iter()
        .filter(|r| {
            if r.keywords.is_empty() && r.context_keywords.is_empty() {
                return true; // explicit_skill etc. — always include
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
/// Ported from Python `planning_rules.py` `build_rules_section`.
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
/// Ported from Python `core/task_planner.py`.
pub struct TaskPlanner {
    /// Current task list.
    pub task_list: Vec<Task>,
    /// Planning rules.
    rules: Vec<PlanningRule>,
}

impl TaskPlanner {
    /// Create a new TaskPlanner with built-in or external rules.
    pub fn new() -> Self {
        Self {
            task_list: Vec::new(),
            rules: load_planning_rules(),
        }
    }

    /// Generate task list from user message using LLM.
    /// Ported from Python `TaskPlanner.generate_task_list`.
    pub async fn generate_task_list(
        &mut self,
        client: &LlmClient,
        model: &str,
        user_message: &str,
        skills: &[LoadedSkill],
        conversation_context: Option<&str>,
    ) -> Result<Vec<Task>> {
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

        let planning_prompt = self.build_planning_prompt(&skills_info, user_message, Some(model));

        let mut user_content = format!(
            "**PRIMARY — Plan ONLY based on this**:\nUser request: {}\n\n",
            user_message
        );
        if let Some(ctx) = conversation_context {
            // Only pass context when user says "继续" — otherwise it can confuse the model
            let needs_continue_context = user_message.contains("继续")
                || user_message.contains("继续之前")
                || user_message.contains("继续任务");
            if needs_continue_context {
                user_content.push_str(&format!(
                    "Conversation context (use ONLY when user says 继续 — to understand what to continue):\n{}\n\n",
                    ctx
                ));
            } else {
                // For new requests: pass truncated context to avoid model fixating on previous task
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
                    .unwrap_or_else(|| "[]".to_string());

                match self.parse_task_list(&raw) {
                    Ok(mut tasks) => {
                        // Auto-add SKILL.md writing task if skill creation detected
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
    fn parse_task_list(&self, raw: &str) -> Result<Vec<Task>> {
        let mut cleaned = raw.trim().to_string();

        // Strip markdown code fences
        if cleaned.starts_with("```json") {
            cleaned = cleaned[7..].to_string();
        }
        if cleaned.starts_with("```") {
            cleaned = cleaned[3..].to_string();
        }
        if cleaned.ends_with("```") {
            cleaned = cleaned[..cleaned.len() - 3].to_string();
        }
        let cleaned = cleaned.trim();

        let tasks: Vec<Task> = serde_json::from_str(cleaned)?;
        Ok(tasks)
    }

    /// Auto-enhance tasks: add SKILL.md writing if skill creation is detected.
    /// Ported from Python `generate_task_list` auto-enhancement logic.
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

    /// Build the planning prompt for task generation.
    /// When compact: filter rules by user message, use fewer examples.
    /// Weak models (7b, ollama, etc.) auto-get full prompt when SKILLLITE_COMPACT_PLANNING not set.
    fn build_planning_prompt(&self, skills_info: &str, user_message: &str, model: Option<&str>) -> String {
        let compact = get_compact_planning(model);
        let rules_section = if compact {
            let filtered: Vec<&PlanningRule> = filter_rules_for_user_message(&self.rules, user_message);
            build_rules_section(&filtered.iter().map(|r| (*r).clone()).collect::<Vec<_>>())
        } else {
            build_rules_section(&self.rules)
        };
        let examples_section = if compact {
            planning_rules::compact_examples_section(user_message)
        } else {
            planning_rules::full_examples_section()
        };
        let output_dir = resolve_output_dir();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let yesterday = (chrono::Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();

        format!(
r#"You are a task planning assistant. Based on user requirements, determine whether tools are needed and generate a task list.

**Current date**: {} (yesterday = {}; for chat_history, pass this date when user says 昨天/昨天记录)

## Core Principle: Minimize Tool Usage

**Important**: Not all tasks require tools! Follow these principles:

1. **Complete simple tasks directly**: If a task can be completed directly by the LLM (such as writing, translation, Q&A, creative generation, etc.), return an empty task list `[]` and let the LLM answer directly
2. **Use tools only when necessary**: Only plan tool-using tasks when the task truly requires external capabilities (such as calculations, HTTP requests, file operations, data analysis, browser automation, etc.)
3. **chat_history is ONLY for past conversation**: Use chat_history ONLY when the user explicitly asks to view, summarize, or analyze **past chat/conversation records** (e.g. 查看聊天记录, 分析历史消息). For analysis of external topics (places, cities, companies, products), prefer http-request for fresh data or return `[]` for LLM knowledge — do NOT use chat_history

## Examples of tasks that DON'T need tools (return empty list `[]`)

- Writing poems, articles, stories (EXCEPT 小红书/种草/图文笔记 - see below, EXCEPT when user asks to 输出到/保存到/写到文件 - see output_to_file rule)
- Translating text
- Answering knowledge-based questions (EXCEPT 天气/气象 - see below, EXCEPT when user asks for 实时/最新 - see below)
- Code explanation, code review suggestions
- Creative generation, brainstorming (EXCEPT 小红书 - see below, EXCEPT HTML/PPT rendering - see below, EXCEPT 网站/官网/网页设计 - see below)
- Summarizing, rewriting, polishing text

{rules_section}

## Examples of tasks that NEED tools

- **Complex or high-precision calculations** (use calculator only for: complex formulas, large numbers, scientific calculations, or when explicit precision is required)
  - ❌ DON'T use calculator for: simple arithmetic (e.g., 0.85 * 0.3, 1 + 2), basic math you can do directly
  - ✅ DO use calculator for: statistical formulas, matrix operations, financial calculations, or when handling large datasets
- Sending HTTP requests (use http-request)
- Reading/writing files (use built-in file operations)
- Querying real-time weather (use weather)
- Creating new Skills (use skill-creator)
- **小红书/种草/图文笔记** (use xiaohongshu-writer - generates structured content + cover image)
- **HTML/PPT/网页渲染** (use write_output to save HTML file, then preview_server to open in browser)
- **官网/网站/网页设计** (use write_output to save HTML + preview_server to open in browser; if frontend-design skill available, use it)
- **输出到 output/保存到文件** (when user says 输出到output, 保存到, 写到文件 — use write_output to persist content)
- **Browser automation / screenshots / visiting websites** (use agent-browser or any matching skill)

## Available Resources

**Available Skills**:
{skills_info}

**Built-in capabilities**: read_file, write_file, write_output (final results), list_directory, list_output (list output directory files), file_exists, chat_history (read past conversation by date), chat_plan (read task plan), **update_task_plan** (revise task list when current plan is wrong/unusable), run_command (execute shell command, requires user confirmation), preview_server (start HTTP server to preview HTML in browser)

**Output directory**: {output_dir}
(When skills produce file outputs like screenshots or PDFs, instruct them to save directly to the output directory)

## Planning Principles

1. **Task decomposition**: Break down user requirements into specific, executable steps
2. **Tool matching**: Select appropriate tools for each step (Skill or built-in file operations). **Match user intent to available skill descriptions** — if a skill's description matches what the user wants, use that skill.
3. **Dependency order**: Ensure tasks are arranged in correct dependency order
4. **Verifiability**: Each task should have clear completion criteria

## Output Format

Must return pure JSON format, no other text.
Task list is an array, each task contains:
- id: Task ID (number)
- description: Task description (concise and clear, stating what to do)
- tool_hint: Suggested tool (skill name or "file_operation" or "analysis")
- completed: Whether completed (initially false)

Example format:
[
  {{"id": 1, "description": "Use list_directory to view project structure", "tool_hint": "file_operation", "completed": false}},
  {{"id": 2, "description": "Use skill-creator to create basic skill structure", "tool_hint": "skill-creator", "completed": false}},
  {{"id": 3, "description": "Use write_file to write main skill code", "tool_hint": "file_operation", "completed": false}},
  {{"id": 4, "description": "Verify the created skill is correct", "tool_hint": "analysis", "completed": false}}
]
- If task can be completed directly by LLM, return: `[]`
- If tools are needed, return task array, each task contains:
  - id: Task ID (number)
  - description: Task description
  - tool_hint: Suggested tool (skill name or "file_operation")
  - completed: false

{examples_section}

Return only JSON, no other content."#,
            today,
            yesterday
        )
    }

    /// Build the main execution system prompt for skill selection and file operations.
    /// Ported from Python `TaskPlanner.build_execution_prompt`.
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
                    // Reference-only: make it very clear this is NOT callable
                    format!("  - **{}**: {} ⛔ [Reference Only — NOT a callable tool, do NOT call it]", s.name, desc)
                } else {
                    format!("  - **{}**: {}", s.name, desc)
                }
            })
            .collect();
        let skills_list_str = skills_list.join("\n");
        let output_dir = resolve_output_dir();

        format!(
r#"You are an intelligent task execution assistant responsible for executing tasks based on user requirements.

**Current date**: {} (yesterday = {}; when calling chat_history for 昨天/昨天记录, pass date "{}")

## CRITICAL: Plan is authority — execute strictly in order

**The task plan is the single source of truth.** You MUST:
1. Execute tasks ONE BY ONE in the given order. Do NOT skip or reorder.
2. For each task, use ONLY the tool specified in its `tool_hint`. Do NOT improvise or switch to other tools.
3. Declare "Task X completed" only after actually executing that task's required tool/action.
4. **When tasks are unusable**: If a task's result is clearly not useful (e.g. chat_history returned irrelevant data for a city comparison), call **update_task_plan** to propose a revised plan, then continue with the new tasks.

**Read the task's `tool_hint` field and follow STRICTLY:**

- **tool_hint = "file_operation"** → Use ONLY built-in tools: `write_output`, `write_file`, `preview_server`, `read_file`, `list_directory`, `file_exists`, `run_command`. ⛔ Do NOT call ANY skill tools. Generate the content yourself and save with write_output.
- **tool_hint = "analysis"** → No tools needed, produce text analysis directly.
- **tool_hint = "<skill_name>"** (e.g. "http-request", "calculator", "weather") → Call that specific skill tool directly. Do NOT use chat_history when tool_hint is http-request.

## Built-in Tools

1. **write_output**: Write final deliverables (HTML, reports, etc.) to the output directory `{output_dir}`. Path is relative to output dir. Use `append: true` to append. **For content >~6k chars**: split into multiple calls — first call overwrites, subsequent calls use `append: true`.
2. **write_file**: Write/create files within the workspace. Use `append: true` to append. Same chunking rule for large content.
3. **preview_server**: Start local HTTP server to preview files in browser
4. **read_file**: Read file content
5. **list_directory**: List directory contents
6. **file_exists**: Check if file exists
7. **run_command**: Execute shell command (requires user confirmation)
8. **update_task_plan**: When the current plan is wrong or a task's result is not useful, call with a new tasks array to replace the plan and continue with the revised tasks

## Available Skills (only use when task tool_hint matches a skill name)

{skills_list_str}

## Output Directory

**Output directory**: `{output_dir}`

- **Final deliverables**: Use **write_output** with file_path relative to output dir (e.g. `index.html`)

## Error Handling

- If a tool fails, read the error message and fix the issue
- When stuck, explain the situation to the user

## Output Guidelines

- After completing each task, explicitly declare: "Task X completed"
- Give a complete summary at the end

## ANTI-HALLUCINATION — ABSOLUTE RULE

**You MUST actually EXECUTE each task before declaring "Task X completed".**

- Execute tasks ONE BY ONE in order. Do NOT skip ahead.
- Your FIRST response must be an ACTION (tool call), NOT a summary.
- If a task requires a tool, call it FIRST, get the result, THEN declare completed.
- **Do NOT improvise**: If Task 1 says http-request, call http-request — do NOT call chat_history or other tools instead.
"#,
            today,
            yesterday,
            yesterday
        )
    }

    /// Build system prompt with task list and execution guidance.
    /// Ported from Python `TaskPlanner.build_task_system_prompt`.
    pub fn build_task_system_prompt(&self, skills: &[LoadedSkill]) -> String {
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

            // Add direct call instruction based on tool_hint type
            if let Some(ref hint) = task.tool_hint {
                if hint == "file_operation" {
                    // Explicitly tell LLM to use built-in tools, NOT skills
                    direct_call_instruction = format!(
                        "\n\n⚡ **ACTION REQUIRED**: This is a file_operation task. \
                         Call `write_output` or `preview_server` NOW.\n\
                         ⛔ Do NOT call any skill tools (skill-creator, frontend-design, etc.). \
                         Generate the content yourself and save with `write_output`."
                    );
                } else if hint != "analysis" {
                    // Check if it's a real skill
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

        format!(
            "{}\n\
             ---\n\n\
             ## Current Task List\n\n\
             {}\n\n\
             ## Execution Rules\n\n\
             1. **MATCH tool_hint**: If tool_hint is \"file_operation\" → use ONLY built-in tools (write_output, preview_server). If tool_hint is a skill name → call that skill.\n\
             2. **Strict sequential execution**: Execute tasks in order, do not skip tasks\n\
             3. **Focus on current task**: Focus only on the current task\n\
             4. **Explicit completion declaration**: After completing a task, declare: \"Task X completed\"\n\
             5. **Avoid unnecessary exploration**: Do NOT call list_directory or read_file unless the task explicitly requires it\n\
             6. **🚫 EXECUTE BEFORE COMPLETING**: Your first response must be an actual tool call, NOT a completion summary. The system will REJECT instant-completion claims.\n\
             {}{}\n\n\
             ⚠️ **Important**: You must explicitly declare after completing each task so the system can track progress.",
            execution_prompt,
            task_list_json,
            current_task_info,
            direct_call_instruction
        )
    }

    /// Check if tasks were completed based on LLM response content.
    /// Looks for "Task X completed" pattern. Returns ALL matching task IDs.
    /// Ported from Python `TaskPlanner.check_completion_in_content`.
    pub fn check_completion_in_content(&self, content: &str) -> Vec<u32> {
        if content.is_empty() {
            return Vec::new();
        }
        let content_lower = content.to_lowercase();
        let mut completed_ids = Vec::new();
        for task in &self.task_list {
            if !task.completed {
                let pattern1 = format!("task {} completed", task.id);
                let pattern2 = format!("task{} completed", task.id);
                let pattern3 = format!("task {} complete", task.id);
                let pattern4 = format!("✅ task {}", task.id);
                if content_lower.contains(&pattern1)
                    || content_lower.contains(&pattern2)
                    || content_lower.contains(&pattern3)
                    || content_lower.contains(&pattern4)
                {
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
    pub fn all_completed(&self) -> bool {
        self.task_list.iter().all(|t| t.completed)
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
    /// Ported from Python auto-nudge logic in `_run_openai`.
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
             Current task: Task {} - {}\n{}",
            task_list_json, current.id, current.description, tool_instruction
        ))
    }

    /// Build a per-task depth limit message.
    /// Sent when a single task has used too many tool calls.
    pub fn build_depth_limit_message(&self, max_calls: usize) -> String {
        format!(
            "You have used {} tool calls for the current task. \
             Based on the information gathered so far, please provide a brief summary, \
             mark this task as completed (\"Task X completed\"), and proceed to the next task.",
            max_calls
        )
    }
}
