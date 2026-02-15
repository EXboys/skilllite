"""
Task Planner - Task planning and tracking for agentic loops.

This module handles:
- Generating task lists from user messages using LLM
- Tracking task completion
- Building task-related system prompts
"""

import json
from pathlib import Path
from enum import Enum
from typing import Any, List, Optional, Dict, TYPE_CHECKING

from ..logger import get_logger
from ..config.planning_rules import get_rules, build_rules_section, merge_rules

if TYPE_CHECKING:
    from .manager import SkillManager


class ApiFormat(Enum):
    """Supported API formats."""
    OPENAI = "openai"
    CLAUDE_NATIVE = "claude_native"


class TaskPlanner:
    """
    Handles task planning and tracking for agentic loops.
    
    Responsibilities:
    - Generate task list from user message using LLM
    - Track task completion status
    - Build execution and task system prompts
    """
    
    def __init__(
        self,
        client: Any,
        model: str,
        api_format: ApiFormat = ApiFormat.OPENAI,
        verbose: bool = True,
        extra_kwargs: Optional[Dict] = None,
        planning_rules: Optional[List[Dict[str, Any]]] = None,
        planning_rules_path: Optional[Path] = None,
    ):
        self.client = client
        self.model = model
        self.api_format = api_format
        self.verbose = verbose
        self.extra_kwargs = extra_kwargs or {}
        self.task_list: List[Dict] = []
        self._planning_rules = planning_rules
        self._planning_rules_path = planning_rules_path

        # Initialize logger
        self._logger = get_logger("skilllite.core.task_planner", verbose=verbose)
    
    def _log(self, message: str) -> None:
        """Log message if verbose mode is enabled."""
        if self.verbose:
            self._logger.info(message)

    def _resolve_planning_rules(self) -> List[Dict[str, Any]]:
        """Resolve planning rules: custom > path > default config."""
        if self._planning_rules is not None:
            return merge_rules(extra=self._planning_rules) if self._planning_rules else []
        if self._planning_rules_path is not None:
            return get_rules(path=self._planning_rules_path, use_cache=False)
        return get_rules()

    def build_execution_prompt(self, manager: "SkillManager") -> str:
        """
        Generate the main execution system prompt for skill selection and file operations.
        """
        skills_info = []
        for skill in manager.list_skills():
            skill_desc = {
                "name": skill.name,
                "description": skill.description or "No description",
                "executable": manager.is_executable(skill.name),
                "path": str(skill.path) if hasattr(skill, 'path') else ""
            }
            skills_info.append(skill_desc)
        
        skills_list_str = "\n".join([
            f"  - **{s['name']}**: {s['description']} {'[Executable]' if s['executable'] else '[Reference Only]'}"
            for s in skills_info
        ])
        
        # Determine skills directory
        skills_dir = ".skills"
        if skills_info and skills_info[0].get("path"):
            first_path = skills_info[0]["path"]
            if ".skills" in first_path:
                skills_dir = ".skills"
            elif "skills" in first_path:
                skills_dir = "skills"
        
        return f"""You are an intelligent task execution assistant responsible for executing tasks based on user requirements.

## Available Skills

{skills_list_str}

## Built-in File Operations (Secondary Tools)

These are auxiliary tools. Only use them when the task genuinely requires file operations:

1. **read_file**: Read file content
   - Parameter: `file_path` (string)
2. **write_file**: Write/create project files
   - Parameters: `file_path`, `content`
3. **write_output**: Write final output to output directory
   - Parameters: `file_path` (relative to output dir), `content`
4. **list_directory**: List directory contents
   - ⚠️ **RESTRICTED**: Only use when the task explicitly requires exploring unknown file locations.
   - **DO NOT** use to "understand the project" or "explore structure" before calling skills.
   - Parameter: `directory_path` (string)
5. **file_exists**: Check if file exists
   - Parameter: `file_path` (string)
6. **run_command**: Execute shell command (requires user confirmation)
   - Parameter: `command` (string)

## ⭐ Critical Rule: SKILL-FIRST Execution

**When a task specifies a skill (via tool_hint), you MUST call that skill DIRECTLY as your first action.**

❌ **WRONG** (wastes iterations):
1. list_directory(".") → read_file("README.md") → list_directory(".skills") → finally call skill
2. Calling list_directory or read_file to "gather information" before calling the specified skill

✅ **CORRECT** (efficient):
1. Task says "Use xiaohongshu-writer" → Call xiaohongshu-writer immediately with appropriate parameters
2. Task says "Use weather" → Call weather skill immediately
3. Task says "Use calculator" → Call calculator skill immediately

**Only use file operations BEFORE a skill when:**
- The skill explicitly requires file content as input (e.g., "analyze this file")
- You need to read a specific file the user mentioned to get data for the skill
- The task description explicitly says to read/list files first

**Never use list_directory or read_file just to "understand context" when you already know which skill to call.**

## Tool Selection Principles

**Minimize tool usage. Do simple tasks directly.**

- Simple arithmetic: do it directly (e.g., 0.85 * 0.3 = 0.255)
- Complex calculations: use calculator skill
- Domain-specific tasks: use the matching skill directly
- File operations: only when the task genuinely needs them

## Error Handling

- If skill execution fails, analyze the error and try to fix
- If file operation fails, check the path
- When stuck, explain the situation to the user

## Output Guidelines

- **Final output files**: Use **write_output** (path relative to output dir)
- After completing each task, explicitly declare: "Task X completed"
- Give a complete summary at the end
"""

    def _build_planning_prompt(self, skills_info: str) -> str:
        """Build the planning prompt for task generation."""
        rules = self._resolve_planning_rules()
        rules_section = build_rules_section(rules)
        return f"""You are a task planning assistant. Based on user requirements, determine whether tools are needed and generate a task list.

## Core Principle: Minimize Tool Usage

**Important**: Not all tasks require tools! Follow these principles:

1. **Complete simple tasks directly**: If a task can be completed directly by the LLM (such as writing, translation, Q&A, creative generation, etc.), return an empty task list `[]` and let the LLM answer directly
2. **Use tools only when necessary**: Only plan tool-using tasks when the task truly requires external capabilities (such as calculations, HTTP requests, file operations, data analysis, etc.)

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

## Available Resources

**Available Skills**: {skills_info}

**Built-in capabilities**: read_file, write_file, write_output (final results), list_directory, file_exists, run_command (execute shell command, requires user confirmation - use when skill suggests e.g. pip install), preview_server (start HTTP server to preview HTML in browser - use after write_output)

## Planning Principles

1. **Task decomposition**: Break down user requirements into specific, executable steps
2. **Tool matching**: Select appropriate tools for each step (Skill or built-in file operations)
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
User request: "继续为我呐未完成的任务"
Conversation context: [assistant previously said: "要完成西安交大与清迈大学的对比，最关键的是获取实时信息... 需要您行动: 获取2024年最新排名数据（需访问QS官网）..."]
Return: [{{"id": 1, "description": "Use http-request to fetch QS rankings and university official data for Xi'an Jiaotong vs Chiang Mai comparison", "tool_hint": "http-request", "completed": false}}, {{"id": 2, "description": "Analyze and present comparison based on fetched data", "tool_hint": "analysis", "completed": false}}]

Example 7 - HTML/PPT rendering (MUST use write_output + preview_server, user wants browser preview):
User request: "帮我设计一个关于skilllite的介绍和分析的ppt，你可以通过html渲染出来给我"
Return: [{{"id": 1, "description": "Use write_output to save HTML presentation to output/index.html", "tool_hint": "file_operation", "completed": false}}, {{"id": 2, "description": "Use preview_server to start local server and open in browser", "tool_hint": "file_operation", "completed": false}}]

Return only JSON, no other content."""

    def generate_task_list(
        self,
        user_message: str,
        manager: "SkillManager",
        conversation_context: Optional[str] = None,
    ) -> List[Dict]:
        """Generate task list from user message using LLM.

        Args:
            user_message: Current user message
            manager: SkillManager for available skills
            conversation_context: Optional recent conversation (for "继续" etc.) to infer task
        """
        skills_names = manager.skill_names()
        skills_info = ", ".join(skills_names) if skills_names else "None"
        planning_prompt = self._build_planning_prompt(skills_info)

        user_content = f"User request:\n{user_message}\n\n"
        if conversation_context:
            user_content += f"Conversation context (recent messages - use this to understand what task to continue):\n{conversation_context}\n\n"
        user_content += "Please generate task list:"

        try:
            if self.api_format == ApiFormat.OPENAI:
                response = self.client.chat.completions.create(
                    model=self.model,
                    messages=[
                        {"role": "system", "content": planning_prompt},
                        {"role": "user", "content": user_content},
                    ],
                    temperature=0.3,
                )
                result = response.choices[0].message.content.strip()
            else:  # CLAUDE_NATIVE
                response = self.client.messages.create(
                    model=self.model,
                    max_tokens=2048,
                    system=planning_prompt,
                    messages=[{"role": "user", "content": user_content}],
                )
                result = response.content[0].text.strip()

            # Parse JSON
            if result.startswith("```json"):
                result = result[7:]
            if result.startswith("```"):
                result = result[3:]
            if result.endswith("```"):
                result = result[:-3]

            task_list = json.loads(result.strip())

            for task in task_list:
                if "completed" not in task:
                    task["completed"] = False

            # Auto-add SKILL.md writing task if skill creation is detected
            has_skill_creation = any(
                "skill-creator" in task.get("description", "").lower() or
                "skill-creator" in task.get("tool_hint", "").lower()
                for task in task_list
            )
            has_skillmd_task = any(
                "skill.md" in task.get("description", "").lower() or
                "skill.md" in task.get("tool_hint", "").lower()
                for task in task_list
            )

            if has_skill_creation and not has_skillmd_task:
                max_id = max((task["id"] for task in task_list), default=0)
                task_list.append({
                    "id": max_id + 1,
                    "description": "Use write_file to write actual SKILL.md content (skill description, usage, parameter documentation, etc.)",
                    "tool_hint": "file_operation",
                    "completed": False
                })
                self._log(f"\n💡 Detected skill creation task, automatically adding SKILL.md writing task")

            self._log(f"\n📋 Generated task list ({len(task_list)} tasks):")
            for task in task_list:
                status = "✅" if task["completed"] else "⬜"
                self._log(f"   {status} [{task['id']}] {task['description']}")

            self.task_list = task_list
            return task_list

        except Exception as e:
            self._log(f"⚠️  Failed to generate task list: {e}")
            self.task_list = [{"id": 1, "description": user_message, "completed": False}]
            return self.task_list

    def update_task_list(self, completed_task_id: Optional[int] = None) -> None:
        """Update task list display."""
        if completed_task_id is not None:
            for task in self.task_list:
                if task["id"] == completed_task_id:
                    task["completed"] = True
                    break

        completed = sum(1 for t in self.task_list if t["completed"])
        self._log(f"\n📋 Current task progress ({completed}/{len(self.task_list)}):")
        for task in self.task_list:
            status = "✅" if task["completed"] else "⬜"
            self._log(f"   {status} [{task['id']}] {task['description']}")

    def check_all_completed(self) -> bool:
        """Check if all tasks are completed."""
        return all(task["completed"] for task in self.task_list)

    def check_completion_in_content(self, content: str) -> Optional[int]:
        """Check if any task was completed based on LLM response content."""
        if not content:
            return None
        content_lower = content.lower()
        for task in self.task_list:
            if not task["completed"]:
                if f"task {task['id']} completed" in content_lower or f"task{task['id']} completed" in content_lower:
                    return task["id"]
        return None

    def build_task_system_prompt(self, manager: "SkillManager") -> str:
        """Generate system prompt with task list and execution guidance."""
        execution_prompt = self.build_execution_prompt(manager)

        task_list_str = json.dumps(self.task_list, ensure_ascii=False, indent=2)
        current_task = next((t for t in self.task_list if not t["completed"]), None)
        current_task_info = ""
        direct_call_instruction = ""
        if current_task:
            tool_hint = current_task.get("tool_hint", "")
            hint_str = f"(Suggested tool: {tool_hint})" if tool_hint else ""
            current_task_info = f"\n\n🎯 **Current task to execute**: Task {current_task['id']} - {current_task['description']} {hint_str}"
            
            # Add direct call instruction when tool_hint points to a specific skill
            if tool_hint and tool_hint not in ("file_operation", "analysis"):
                # Check if it's a real skill
                if manager.has_skill(tool_hint) or manager.is_executable(tool_hint):
                    direct_call_instruction = f"""

⚡ **DIRECT ACTION REQUIRED**: Call `{tool_hint}` NOW with appropriate parameters.
Do NOT call list_directory, read_file, or any other tool first. The skill `{tool_hint}` is ready to use.
If you're unsure about parameters, call the skill with your best guess — the system will inject documentation to help you correct parameters if needed."""

        task_rules = f"""
---

## Current Task List

{task_list_str}

## Execution Rules

1. **SKILL-FIRST**: When a task specifies a skill tool, call it DIRECTLY as your first action. Do NOT explore files first.
2. **Strict sequential execution**: Execute tasks in order, do not skip tasks
3. **Focus on current task**: Focus only on the current task
4. **Explicit completion declaration**: After completing a task, declare: "Task X completed" (X is task ID)
5. **Sequential progression**: Only start next task after current task is completed
6. **Avoid unnecessary exploration**: Do NOT call list_directory or read_file unless the task explicitly requires reading specific files
7. **Multi-step tasks**: If a task requires multiple tool calls, continue until truly completed
{current_task_info}{direct_call_instruction}

⚠️ **Important**: You must explicitly declare after completing each task so the system can track progress.
"""

        return execution_prompt + task_rules

