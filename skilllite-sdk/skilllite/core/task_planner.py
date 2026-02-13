"""
Task Planner - Task planning and tracking for agentic loops.

This module handles:
- Generating task lists from user messages using LLM
- Tracking task completion
- Building task-related system prompts
"""

import json
from enum import Enum
from typing import Any, List, Optional, Dict, TYPE_CHECKING

from ..logger import get_logger

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
        extra_kwargs: Optional[Dict] = None
    ):
        self.client = client
        self.model = model
        self.api_format = api_format
        self.verbose = verbose
        self.extra_kwargs = extra_kwargs or {}
        self.task_list: List[Dict] = []
        
        # Initialize logger
        self._logger = get_logger("skilllite.core.task_planner", verbose=verbose)
    
    def _log(self, message: str) -> None:
        """Log message if verbose mode is enabled."""
        if self.verbose:
            self._logger.info(message)

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
        
        return f"""You are an intelligent task execution assistant responsible for planning and executing tasks based on user requirements.

## Project Structure

**Skills Directory**: `{skills_dir}/`

All skills are stored in the `{skills_dir}/` directory, each skill is an independent subdirectory.

## Available Skills

{skills_list_str}

## Built-in File Operations

In addition to the above Skills, you have the following built-in file operation capabilities:

1. **read_file**: Read file content
   - Used to view existing files, understand project structure, read configurations, etc.
   - Parameter: `file_path` (string, file path)

2. **write_file**: Write/create project files (e.g. .skills/xxx/SKILL.md)
   - Parameters: `file_path`, `content`

3. **write_output**: Write final output (reports, images, generated content) to output directory
   - Keeps results separate from plan/memory/logs. Path is relative to output dir (e.g. report.md, image.png)
   - Parameters: `file_path` (filename or path under output), `content`

4. **list_directory**: List directory contents
   - Used to view directory structure, understand project layout
   - Parameter: `directory_path` (string, directory path, e.g., "." or ".skills")

5. **file_exists**: Check if file exists
   - Used to confirm file status before operations
   - Parameter: `file_path` (string, file path)

6. **run_command**: Execute shell command (requires user confirmation)
   - Use when skill output suggests running commands (e.g. "请运行: pip install playwright && playwright install chromium")
   - Parameter: `command` (string, the command to run)
   - User will be prompted to confirm before execution

**Note**: Parameter names must be used exactly as defined above, otherwise errors will occur.

## Task Execution Strategy

### 1. Task Analysis
- Carefully analyze user requirements and understand the final goal
- Break down complex tasks into executable sub-steps
- Identify the tools needed for each step (Skill or built-in file operations)

### 2. Tool Selection Principles

**IMPORTANT: Minimize Tool Usage - Do Simple Tasks Directly**

**When to calculate directly (DO NOT use calculator):**
- Simple arithmetic: addition, subtraction, multiplication, division of small numbers
- Examples: 0.85 * 0.3 = 0.255, 1 + 2 = 3, 10 / 2 = 5
- Basic weighted averages: (0.85 * 0.3) + (1.0 * 0.2) = 0.455
- These calculations should be done directly in your response, NOT by calling calculator tool

**When to use calculator tool:**
- Complex statistical formulas (standard deviation, correlation, regression)
- Matrix operations or linear algebra
- Financial calculations (compound interest, NPV, etc.)
- Large dataset operations
- Scientific calculations requiring high precision

**When to prioritize Skills:**
- Tasks involve specialized domain processing (e.g., data analysis, text processing, HTTP requests)
- Skills have encapsulated complex business logic
- Need to call external services or APIs

**When to use built-in file operations:**
- Need to read existing files to understand content or structure
- Need to create new files or modify existing files
- Need to view directory structure to locate files
- Need to prepare input data before calling Skills
- Need to save output results after calling Skills
   - **Use write_output** for final outputs (reports, images). Path relative to output dir (e.g. report.md).

### 3. Execution Order

1. **Information Gathering Phase**: Use read_file, list_directory to understand current state
2. **Planning Phase**: Determine which Skills to use and operation order
3. **Execution Phase**: Call Skills and file operations in sequence
4. **Verification Phase**: Check execution results, make corrections if necessary

### 4. Error Handling

- If Skill execution fails, analyze the error cause and try to fix it
- If file operation fails, check if the path is correct
- When encountering unsolvable problems, explain the situation to the user and request help

## Output Guidelines

- **Final output files**: Use **write_output** (path relative to output dir, e.g. report.md, image.png). Keeps results separate from plan/memory/logs.
- After completing each task step, explicitly declare: "Task X completed"
- Provide clear execution process explanations
- Give a complete summary of execution results at the end
"""

    def _build_planning_prompt(self, skills_info: str) -> str:
        """Build the planning prompt for task generation."""
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
- Creative generation, brainstorming (EXCEPT 小红书 - see below)
- Summarizing, rewriting, polishing text

## CRITICAL: When user explicitly requests a Skill, ALWAYS use it

**If user says "使用 XX skill" / "用 XX 技能" / "use XX skills"**, you MUST add that skill to the task list. Do NOT return empty list.

**天气/气象/天气预报**: When the user asks about weather (天气、气温、气象、今天天气、明天天气、某地天气、适合出行吗、适合出去玩吗 etc.), you MUST use **weather** skill. The LLM cannot provide real-time weather data; only the weather skill can. Return a task with tool_hint: "weather".

**实时/最新/实时信息**: When the user explicitly asks for 实时、最新、实时信息、最新数据、实时数据、最新排名、实时查询、抓取网页、获取最新、fetch live data etc., you MUST use **http-request** skill. The LLM's knowledge has a cutoff; only HTTP requests can fetch current information. Return a task with tool_hint: "http-request".

**继续/继续未完成的任务**: When the user says 继续、继续未完成、继续之前、继续任务 etc., you MUST use the **conversation context** (if provided) to understand what task to continue. If the context mentions: real-time data, rankings, university comparison, fees, QS ranking, 实时、最新、需要用户自行查询、请访问官网 etc., you MUST plan **http-request** to fetch the data. The AI must DO the work using tools, NOT ask the user to do it. Only return empty list [] when the continued task is truly LLM-only (e.g. creative writing).

**小红书/种草/图文笔记**: When the task involves 小红书、种草文案、小红书图文、小红书笔记, you MUST use **xiaohongshu-writer** skill. It generates structured content + thumbnail image. Return a task with tool_hint: "xiaohongshu-writer".

## Examples of tasks that NEED tools

- **Complex or high-precision calculations** (use calculator only for: complex formulas, large numbers, scientific calculations, or when explicit precision is required)
  - ❌ DON'T use calculator for: simple arithmetic (e.g., 0.85 * 0.3, 1 + 2), basic math you can do directly
  - ✅ DO use calculator for: statistical formulas, matrix operations, financial calculations, or when handling large datasets
- Sending HTTP requests (use http-request)
- Reading/writing files (use built-in file operations)
- Querying real-time weather (use weather)
- Creating new Skills (use skill-creator)
- **小红书/种草/图文笔记** (use xiaohongshu-writer - generates structured content + cover image)

## Available Resources

**Available Skills**: {skills_info}

**Built-in capabilities**: read_file, write_file, write_output (final results), list_directory, file_exists, run_command (execute shell command, requires user confirmation - use when skill suggests e.g. pip install)

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
        if current_task:
            tool_hint = current_task.get("tool_hint", "")
            hint_str = f"(Suggested tool: {tool_hint})" if tool_hint else ""
            current_task_info = f"\n\n🎯 **Current task to execute**: Task {current_task['id']} - {current_task['description']} {hint_str}"

        task_rules = f"""
---

## Current Task List

{task_list_str}

## Execution Rules

1. **Strict sequential execution**: Must execute tasks in order, do not skip tasks
2. **Focus on current task**: Focus only on executing the current task at a time
3. **Explicit completion declaration**: After completing a task, must explicitly declare in response: "Task X completed" (X is task ID)
4. **Sequential progression**: Can only start next task after current task is completed
5. **Avoid repetition**: Do not repeat already completed tasks
6. **Multi-step tasks**: If a task requires multiple tool calls to complete, continue calling tools until the task is truly completed before declaring
{current_task_info}

⚠️ **Important**: You must explicitly declare after completing each task so the system can track progress and know when to end.
"""

        return execution_prompt + task_rules

