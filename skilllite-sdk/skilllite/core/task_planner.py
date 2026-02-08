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

2. **write_file**: Write/create files
   - Used to create new files or modify existing file content
   - Parameters: `file_path` (string, file path), `content` (string, file content)

3. **list_directory**: List directory contents
   - Used to view directory structure, understand project layout
   - Parameter: `directory_path` (string, directory path, e.g., "." or ".skills")

4. **file_exists**: Check if file exists
   - Used to confirm file status before operations
   - Parameter: `file_path` (string, file path)

**Note**: Parameter names must be used exactly as defined above, otherwise errors will occur.

## Task Execution Strategy

### 1. Task Analysis
- Carefully analyze user requirements and understand the final goal
- Break down complex tasks into executable sub-steps
- Identify the tools needed for each step (Skill or built-in file operations)

### 2. Tool Selection Principles

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

- Writing poems, articles, stories
- Translating text
- Answering knowledge-based questions
- Code explanation, code review suggestions
- Creative generation, brainstorming
- Summarizing, rewriting, polishing text

## Examples of tasks that NEED tools

- Precise calculations (use calculator)
- Sending HTTP requests (use http-request)
- Reading/writing files (use built-in file operations)
- Querying real-time weather (use weather)
- Creating new Skills (use skill-creator)

## Available Resources

**Available Skills**: {skills_info}

**Built-in capabilities**: read_file (read files), write_file (write files), list_directory (list directory), file_exists (check file existence)

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

Return only JSON, no other content."""

    def generate_task_list(self, user_message: str, manager: "SkillManager") -> List[Dict]:
        """Generate task list from user message using LLM."""
        skills_names = manager.skill_names()
        skills_info = ", ".join(skills_names) if skills_names else "None"
        planning_prompt = self._build_planning_prompt(skills_info)

        try:
            if self.api_format == ApiFormat.OPENAI:
                response = self.client.chat.completions.create(
                    model=self.model,
                    messages=[
                        {"role": "system", "content": planning_prompt},
                        {"role": "user", "content": f"User request:\n{user_message}\n\nPlease generate task list:"}
                    ],
                    temperature=0.3
                )
                result = response.choices[0].message.content.strip()
            else:  # CLAUDE_NATIVE
                response = self.client.messages.create(
                    model=self.model,
                    max_tokens=2048,
                    system=planning_prompt,
                    messages=[
                        {"role": "user", "content": f"User request:\n{user_message}\n\nPlease generate task list:"}
                    ]
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

