//! Task Planner: task planning and tracking for agentic loops.
//!
//! Ported from Python `core/task_planner.py` + `config/planning_rules.py`.
//!
//! Responsibilities:
//! - Generate task list from user message using LLM
//! - Track task completion status
//! - Build execution and task system prompts
//! - Planning rules engine (built-in + external JSON)

use anyhow::Result;

use super::llm::LlmClient;
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

// ─── Planning Rules ─────────────────────────────────────────────────────────

/// Get default built-in planning rules.
/// Ported from Python `planning_rules.py` `_builtin_rules` + `planning_rules.json`.
fn builtin_rules() -> Vec<PlanningRule> {
    vec![
        PlanningRule {
            id: "explicit_skill".into(),
            priority: 100,
            keywords: vec![],
            context_keywords: vec![],
            tool_hint: None,
            instruction: "**If user says \"使用 XX skill\" / \"用 XX 技能\" / \"use XX skills\"**, you MUST add that skill to the task list. Do NOT return empty list.".into(),
        },
        PlanningRule {
            id: "weather".into(),
            priority: 90,
            keywords: vec!["天气".into(), "气温".into(), "气象".into(), "今天天气".into(), "明天天气".into(), "适合出行吗".into(), "适合出去玩吗".into()],
            context_keywords: vec![],
            tool_hint: Some("weather".into()),
            instruction: "**天气/气象/天气预报**: When the user asks about weather, you MUST use **weather** skill. The LLM cannot provide real-time weather data; only the weather skill can. Return a task with tool_hint: \"weather\".".into(),
        },
        PlanningRule {
            id: "realtime_http".into(),
            priority: 90,
            keywords: vec!["实时".into(), "最新".into(), "实时信息".into(), "最新数据".into(), "实时数据".into(), "最新排名".into(), "实时查询".into(), "抓取网页".into(), "获取最新".into(), "fetch live data".into()],
            context_keywords: vec![],
            tool_hint: Some("http-request".into()),
            instruction: "**实时/最新/实时信息**: When the user explicitly asks for real-time or latest data, you MUST use **http-request** skill. The LLM's knowledge has a cutoff; only HTTP requests can fetch current information. Return a task with tool_hint: \"http-request\".".into(),
        },
        PlanningRule {
            id: "continue_context".into(),
            priority: 85,
            keywords: vec!["继续".into(), "继续未完成".into(), "继续之前".into(), "继续任务".into()],
            context_keywords: vec!["实时".into(), "最新".into(), "排名".into(), "university".into(), "QS".into(), "官网".into(), "需要用户自行查询".into(), "请访问官网".into()],
            tool_hint: Some("http-request".into()),
            instruction: "**继续/继续未完成的任务**: When the user says 继续, you MUST use the **conversation context** to understand what task to continue. If the context mentions real-time data, rankings, or similar, plan **http-request** to fetch the data.".into(),
        },
        PlanningRule {
            id: "xiaohongshu".into(),
            priority: 90,
            keywords: vec!["小红书".into(), "种草文案".into(), "小红书图文".into(), "小红书笔记".into()],
            context_keywords: vec![],
            tool_hint: Some("xiaohongshu-writer".into()),
            instruction: "**小红书/种草/图文笔记**: When the task involves 小红书 content, you MUST use **xiaohongshu-writer** skill.".into(),
        },
        PlanningRule {
            id: "html_preview".into(),
            priority: 90,
            keywords: vec!["html渲染".into(), "渲染出来".into(), "预览".into(), "在浏览器中打开".into(), "html呈现".into(), "网页".into(), "PPT".into()],
            context_keywords: vec![],
            tool_hint: Some("file_operation".into()),
            instruction: "**HTML/PPT/网页渲染/预览**: When the user asks for HTML rendering or browser preview, use **write_output** + **preview_server**.".into(),
        },
    ]
}

/// Load planning rules: external JSON path > built-in defaults.
/// Ported from Python `planning_rules.py` `get_rules` + `load_rules`.
fn load_planning_rules() -> Vec<PlanningRule> {
    if let Some(path) = get_planning_rules_path() {
        match load_rules_from_json(&path) {
            Ok(rules) if !rules.is_empty() => return rules,
            Ok(_) => tracing::debug!("External planning rules empty, using built-in"),
            Err(e) => tracing::warn!("Failed to load planning rules from {}: {}", path, e),
        }
    }
    builtin_rules()
}

/// Load rules from a JSON file.
fn load_rules_from_json(path: &str) -> Result<Vec<PlanningRule>> {
    let content = std::fs::read_to_string(path)?;
    let data: serde_json::Value = serde_json::from_str(&content)?;
    let rules_val = data
        .get("rules")
        .ok_or_else(|| anyhow::anyhow!("Missing 'rules' key in planning rules JSON"))?;
    let mut rules: Vec<PlanningRule> = serde_json::from_value(rules_val.clone())?;
    // Sort by priority descending
    rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    Ok(rules)
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

        let planning_prompt = self.build_planning_prompt(&skills_info);

        let mut user_content = format!("User request:\n{}\n\n", user_message);
        if let Some(ctx) = conversation_context {
            user_content.push_str(&format!(
                "Conversation context (recent messages - use this to understand what task to continue):\n{}\n\n",
                ctx
            ));
        }
        user_content.push_str("Please generate task list:");

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
    /// Ported from Python `TaskPlanner._build_planning_prompt`.
    fn build_planning_prompt(&self, skills_info: &str) -> String {
        let rules_section = build_rules_section(&self.rules);
        let output_dir = resolve_output_dir();

        format!(
r#"You are a task planning assistant. Based on user requirements, determine whether tools are needed and generate a task list.

## Core Principle: Minimize Tool Usage

**Important**: Not all tasks require tools! Follow these principles:

1. **Complete simple tasks directly**: If a task can be completed directly by the LLM (such as writing, translation, Q&A, creative generation, etc.), return an empty task list `[]` and let the LLM answer directly
2. **Use tools only when necessary**: Only plan tool-using tasks when the task truly requires external capabilities (such as calculations, HTTP requests, file operations, data analysis, browser automation, etc.)

## Examples of tasks that DON'T need tools (return empty list `[]`)

- Writing poems, articles, stories (EXCEPT 小红书/种草/图文笔记 - see below)
- Translating text
- Answering knowledge-based questions (EXCEPT 天气/气象 - see below, EXCEPT when user asks for 实时/最新 - see below)
- Code explanation, code review suggestions
- Creative generation, brainstorming (EXCEPT 小红书 - see below, EXCEPT HTML/PPT rendering - see below)
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
- **Browser automation / screenshots / visiting websites** (use agent-browser or any matching skill)

## Available Resources

**Available Skills**:
{skills_info}

**Built-in capabilities**: read_file, write_file, write_output (final results), list_directory, file_exists, run_command (execute shell command, requires user confirmation), preview_server (start HTTP server to preview HTML in browser)

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

Example 1 - Simple task (writing poetry):
User request: "Write a poem praising spring"
Return: []

Example 2 - Task requiring tools:
User request: "Calculate 123 * 456 + 789 for me"
Return: [{{"id": 1, "description": "Use calculator to compute expression", "tool_hint": "calculator", "completed": false}}]

Example 3 - User explicitly asks to use a skill (MUST use that skill):
User request: "写一个关于本项目推广的小红书的图文，使用小红书的skills"
Return: [{{"id": 1, "description": "Use xiaohongshu-writer to generate 小红书 content with thumbnail", "tool_hint": "xiaohongshu-writer", "completed": false}}]

Example 4 - Weather query (MUST use weather skill, LLM cannot provide real-time data):
User request: "深圳今天天气怎样，适合出去玩吗？"
Return: [{{"id": 1, "description": "Use weather skill to query real-time weather in Shenzhen", "tool_hint": "weather", "completed": false}}]

Example 5 - User asks for real-time/latest info (MUST use http-request):
User request: "我需要更实时的信息" or "分析西安交大和清迈大学的对比，要最新数据"
Return: [{{"id": 1, "description": "Use http-request to fetch latest data from authoritative sources (QS, official sites)", "tool_hint": "http-request", "completed": false}}, {{"id": 2, "description": "Analyze and compare based on fetched data", "tool_hint": "analysis", "completed": false}}]

Example 6 - User says "继续" with context (MUST use context to infer task):
User request: "继续为我那未完成的任务"
Conversation context: [assistant previously said: "要完成西安交大与清迈大学的对比，最关键的是获取实时信息... 需要您行动: 获取2024年最新排名数据（需访问QS官网）..."]
Return: [{{"id": 1, "description": "Use http-request to fetch QS rankings and university official data for Xi'an Jiaotong vs Chiang Mai comparison", "tool_hint": "http-request", "completed": false}}, {{"id": 2, "description": "Analyze and present comparison based on fetched data", "tool_hint": "analysis", "completed": false}}]

Example 7 - HTML/PPT rendering (MUST use write_output + preview_server, user wants browser preview):
User request: "帮我设计一个关于skilllite的介绍和分析的ppt，你可以通过html渲染出来给我"
Return: [{{"id": 1, "description": "Use write_output to save HTML presentation to output/index.html", "tool_hint": "file_operation", "completed": false}}, {{"id": 2, "description": "Use preview_server to start local server and open in browser", "tool_hint": "file_operation", "completed": false}}]

Return only JSON, no other content."#
        )
    }

    /// Build the main execution system prompt for skill selection and file operations.
    /// Ported from Python `TaskPlanner.build_execution_prompt`.
    pub fn build_execution_prompt(&self, skills: &[LoadedSkill]) -> String {
        let skills_list: Vec<String> = skills
            .iter()
            .map(|s| {
                let desc = s
                    .metadata
                    .description
                    .as_deref()
                    .unwrap_or("No description");
                let executable = if s.metadata.entry_point.is_empty() {
                    "[Reference Only]"
                } else {
                    "[Executable]"
                };
                format!("  - **{}**: {} {}", s.name, desc, executable)
            })
            .collect();
        let skills_list_str = skills_list.join("\n");
        let output_dir = resolve_output_dir();

        format!(
r#"You are an intelligent task execution assistant responsible for executing tasks based on user requirements.

## Available Skills

{skills_list_str}

## Built-in File Operations (Secondary Tools)

These are auxiliary tools. Only use them when the task genuinely requires file operations:

1. **read_file**: Read file content
2. **write_file**: Write/create project files
3. **write_output**: Write final text output to output directory
4. **list_directory**: List directory contents
5. **file_exists**: Check if file exists
6. **run_command**: Execute shell command (requires user confirmation)
7. **preview_server**: Start local HTTP server for preview

## Output Directory

**Output directory**: `{output_dir}`

- **Final text output files**: Use **write_output** (path relative to output dir)
- **File outputs from skills** (screenshots, PDFs, images, etc.): Tell the skill to save directly to the output directory by specifying the full path. For example: `agent-browser screenshot {output_dir}/screenshot.png`

## Critical Rule: SKILL-FIRST Execution

**When a task specifies a skill (via tool_hint), you MUST call that skill DIRECTLY as your first action.**

## Tool Selection Principles

**Minimize tool usage. Do simple tasks directly.**

## Error Handling

- If skill execution fails, analyze the error and try to fix
- If file operation fails, check the path
- When stuck, explain the situation to the user

## Output Guidelines

- After completing each task, explicitly declare: "Task X completed"
- Give a complete summary at the end

## ANTI-HALLUCINATION — ABSOLUTE RULE

**You MUST actually EXECUTE each task before declaring "Task X completed".**

- Execute tasks ONE BY ONE in order. Do NOT skip ahead.
- Your FIRST response after seeing the task plan must be an ACTION (tool call or actual work), NOT a completion summary.
- NEVER declare multiple tasks completed in your first response — execute them step by step.
- If a task requires a tool (e.g. agent-browser, calculator), call it FIRST, get the result, THEN declare completed.
- If a task is pure analysis/text, produce the actual output FIRST, THEN declare completed.
"#
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

            // Add direct call instruction when tool_hint points to a specific skill
            if let Some(ref hint) = task.tool_hint {
                if hint != "file_operation" && hint != "analysis" {
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
             1. **SKILL-FIRST**: When a task specifies a skill tool, call it DIRECTLY as your first action.\n\
             2. **Strict sequential execution**: Execute tasks in order, do not skip tasks\n\
             3. **Focus on current task**: Focus only on the current task\n\
             4. **Explicit completion declaration**: After completing a task, declare: \"Task X completed\"\n\
             5. **Sequential progression**: Only start next task after current task is completed\n\
             6. **Avoid unnecessary exploration**: Do NOT call list_directory or read_file unless the task explicitly requires it\n\
             7. **Multi-step tasks**: If a task requires multiple tool calls, continue until truly completed\n\
             8. **🚫 EXECUTE BEFORE COMPLETING**: Your first response after seeing the plan must be actual execution (tool calls or real work), NOT a completion summary. The system will REJECT instant-completion claims.\n\
             {}{}\n\n\
             ⚠️ **Important**: You must explicitly declare after completing each task so the system can track progress. \
             The system enforces: completion claims without actual tool calls will be REJECTED and you will be asked to retry.",
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
