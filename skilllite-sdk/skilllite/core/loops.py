"""
Agentic Loops - Continuous tool execution loops for LLM interactions.

This module provides a unified agentic loop implementation that supports
both OpenAI-compatible APIs and Claude's native API through a single interface.
"""

import json
from enum import Enum
from typing import Any, List, Optional, TYPE_CHECKING, Dict, Callable

if TYPE_CHECKING:
    from .manager import SkillManager


class ApiFormat(Enum):
    """Supported API formats."""
    OPENAI = "openai"
    CLAUDE_NATIVE = "claude_native"


class AgenticLoop:
    """
    Unified agentic loop for LLM-tool interactions.
    
    Supports both OpenAI-compatible APIs and Claude's native API through
    a single interface. Handles the back-and-forth between the LLM and
    tool execution until completion.
    
    Works with:
    - OpenAI (GPT-4, GPT-3.5, etc.)
    - Azure OpenAI
    - Anthropic Claude (both OpenAI-compatible and native)
    - Ollama, vLLM, LMStudio
    - DeepSeek, Qwen, Moonshot, etc.
    
    Example:
        ```python
        # OpenAI-compatible (default)
        loop = AgenticLoop(manager, client, model="gpt-4")
        
        # Claude native API
        loop = AgenticLoop(manager, client, model="claude-3-opus",
                          api_format=ApiFormat.CLAUDE_NATIVE)
        ```
    """
    
    def __init__(
        self,
        manager: "SkillManager",
        client: Any,
        model: str,
        system_prompt: Optional[str] = None,
        max_iterations: int = 10,
        api_format: ApiFormat = ApiFormat.OPENAI,
        custom_tool_handler: Optional[Callable] = None,
        enable_task_planning: bool = True,
        verbose: bool = True,
        **kwargs
    ):
        """
        Initialize the agentic loop.
        
        Args:
            manager: SkillManager instance
            client: LLM client (OpenAI or Anthropic)
            model: Model name to use
            system_prompt: Optional system prompt
            max_iterations: Maximum number of iterations
            api_format: API format to use (OPENAI or CLAUDE_NATIVE)
            custom_tool_handler: Optional custom tool handler function
            enable_task_planning: Whether to generate task list before execution
            verbose: Whether to print detailed logs
            **kwargs: Additional arguments passed to the LLM
        """
        self.manager = manager
        self.client = client
        self.model = model
        self.system_prompt = system_prompt
        self.max_iterations = max_iterations
        self.api_format = api_format
        self.custom_tool_handler = custom_tool_handler
        self.enable_task_planning = enable_task_planning
        self.verbose = verbose
        self.extra_kwargs = kwargs
        self.task_list: List[Dict] = []
    
    def _log(self, message: str) -> None:
        """Print log message if verbose mode is enabled."""
        if self.verbose:
            print(message)
    
    def _get_execution_system_prompt(self) -> str:
        """
        Generate the main execution system prompt for skill selection and file operations.
        
        This prompt guides the LLM to:
        1. Analyze tasks and select appropriate skills
        2. Determine when to use built-in file read/write capabilities
        3. Execute tasks step by step
        """
        # Get available skills info
        skills_info = []
        for skill in self.manager.list_skills():
            skill_desc = {
                "name": skill.name,
                "description": skill.description or "No description",
                "executable": self.manager.is_executable(skill.name),
                "path": str(skill.path) if hasattr(skill, 'path') else ""
            }
            skills_info.append(skill_desc)
        
        skills_list_str = "\n".join([
            f"  - **{s['name']}**: {s['description']} {'[Executable]' if s['executable'] else '[Reference Only]'}"
            for s in skills_info
        ])
        
        # Determine skills directory
        skills_dir = ".skills"  # Default
        if skills_info and skills_info[0].get("path"):
            # Extract skills directory from first skill path
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

    def _generate_task_list(self, user_message: str) -> List[Dict]:
        """Generate task list from user message using LLM."""
        # Get available skills for context
        skills_names = self.manager.skill_names()
        skills_info = ", ".join(skills_names) if skills_names else "None"
        
        planning_prompt = f"""You are a task planning assistant. Based on user requirements, determine whether tools are needed and generate a task list.

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
            
            # Check if any task involves creating a skill using skill-creator
            # If so, automatically add a task to write SKILL.md content (if not already present)
            has_skill_creation = any(
                "skill-creator" in task.get("description", "").lower() or 
                "skill-creator" in task.get("tool_hint", "").lower()
                for task in task_list
            )
            
            # Check if SKILL.md writing task already exists
            has_skillmd_task = any(
                "skill.md" in task.get("description", "").lower() or
                "skill.md" in task.get("tool_hint", "").lower()
                for task in task_list
            )
            
            if has_skill_creation and not has_skillmd_task:
                # Add task to write SKILL.md actual content
                max_id = max((task["id"] for task in task_list), default=0)
                new_task = {
                    "id": max_id + 1,
                    "description": "Use write_file to write actual SKILL.md content (skill description, usage, parameter documentation, etc.)",
                    "tool_hint": "file_operation",
                    "completed": False
                }
                task_list.append(new_task)
                self._log(f"\n💡 Detected skill creation task, automatically adding SKILL.md writing task")
            
            self._log(f"\n📋 Generated task list ({len(task_list)} tasks):")
            for task in task_list:
                status = "✅" if task["completed"] else "⬜"
                self._log(f"   {status} [{task['id']}] {task['description']}")
            
            return task_list
            
        except Exception as e:
            self._log(f"⚠️  Failed to generate task list: {e}")
            return [{"id": 1, "description": user_message, "completed": False}]
    
    def _update_task_list(self, completed_task_id: Optional[int] = None) -> None:
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
    
    def _check_all_tasks_completed(self) -> bool:
        """Check if all tasks are completed."""
        return all(task["completed"] for task in self.task_list)
    
    def _check_task_completion_in_content(self, content: str) -> Optional[int]:
        """Check if any task was completed based on LLM response content."""
        if not content:
            return None
        content_lower = content.lower()
        for task in self.task_list:
            if not task["completed"]:
                if f"task {task['id']} completed" in content_lower or f"task{task['id']} completed" in content_lower:
                    return task["id"]
        return None
    
    def _get_task_system_prompt(self) -> str:
        """Generate system prompt with task list and execution guidance."""
        # Get the main execution system prompt
        execution_prompt = self._get_execution_system_prompt()
        
        # Format task list
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
    
    def _get_skill_docs_for_tools(self, tool_calls: List[Any]) -> Optional[str]:
        """
        Get full SKILL.md documentation for the tools being called.
        
        This implements progressive disclosure - the LLM only gets the full
        documentation when it decides to use a specific skill.
        
        Tracks which skills have already been documented to avoid duplicates.
        
        Args:
            tool_calls: List of tool calls from LLM response
            
        Returns:
            Formatted string with full SKILL.md content for each skill,
            or None if no new skill documentation is available
        """
        # Initialize the set to track documented skills if not exists
        if not hasattr(self, '_documented_skills'):
            self._documented_skills = set()
        
        docs_parts = []
        
        for tc in tool_calls:
            tool_name = tc.function.name if hasattr(tc, 'function') else tc.get('function', {}).get('name', '')
            
            # Skip built-in tools (read_file, write_file, etc.)
            if tool_name in ['read_file', 'write_file', 'list_directory', 'file_exists']:
                continue
            
            # Skip if already documented in this session
            if tool_name in self._documented_skills:
                continue
            
            # Get skill info - handle both regular skills and multi-script tools
            skill_info = self.manager.get_skill(tool_name)
            if not skill_info:
                # Try to get parent skill for multi-script tools (e.g., "skill-creator:init-skill")
                if ':' in tool_name:
                    parent_name = tool_name.split(':')[0]
                    skill_info = self.manager.get_skill(parent_name)
                    # Mark both the parent and the specific tool as documented
                    if skill_info:
                        self._documented_skills.add(parent_name)
            
            if skill_info:
                full_content = skill_info.get_full_content()
                if full_content:
                    # Mark this skill as documented
                    self._documented_skills.add(tool_name)
                    
                    docs_parts.append(f"""
## 📖 Skill Documentation: {tool_name}

Below is the complete documentation for `{tool_name}`. Please read the documentation to understand how to use this tool correctly:

---
{full_content}
---
""")
        
        if docs_parts:
            header = """
# 🔍 Skill Detailed Documentation

You are calling the following Skills. Here is their complete documentation. Please read carefully to understand:
1. The functionality and purpose of this Skill
2. What parameters need to be passed
3. The format and type of parameters
4. Usage examples

Based on the documentation, call the tools with correct parameters.
"""
            return header + "\n".join(docs_parts)
        
        return None
    
    # ==================== OpenAI-compatible API ====================
    
    def _run_openai(
        self,
        user_message: str,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> Any:
        """Run loop using OpenAI-compatible API."""
        messages = []
        
        if self.system_prompt:
            messages.append({"role": "system", "content": self.system_prompt})
        
        if self.enable_task_planning and self.task_list:
            messages.append({"role": "system", "content": self._get_task_system_prompt()})
        
        messages.append({"role": "user", "content": user_message})
        
        tools = self.manager.get_tools()
        response = None
        
        for iteration in range(self.max_iterations):
            self._log(f"\n🔄 Iteration #{iteration + 1}/{self.max_iterations}")
            
            self._log("⏳ Calling LLM...")
            response = self.client.chat.completions.create(
                model=self.model,
                messages=messages,
                tools=tools if tools else None,
                **self.extra_kwargs
            )
            
            message = response.choices[0].message
            finish_reason = response.choices[0].finish_reason
            
            self._log(f"✅ LLM response completed (finish_reason: {finish_reason})")
            
            # No tool calls
            if not message.tool_calls:
                self._log("📝 LLM did not call any tools")

                if self.enable_task_planning:
                    completed_id = self._check_task_completion_in_content(message.content)
                    if completed_id:
                        self._update_task_list(completed_id)

                    if self._check_all_tasks_completed():
                        self._log("🎯 All tasks completed, ending iteration")
                        return response
                    else:
                        # Tasks not complete and no tool calls - continue to next iteration
                        continue
                else:
                    return response
            
            # Handle tool calls
            self._log(f"\n🔧 LLM decided to call {len(message.tool_calls)} tools:")
            for idx, tc in enumerate(message.tool_calls, 1):
                self._log(f"   {idx}. {tc.function.name}")
                self._log(f"      Arguments: {tc.function.arguments}")
            
            # Get full SKILL.md content for tools that haven't been documented yet
            skill_docs = self._get_skill_docs_for_tools(message.tool_calls)
            
            # If we have new skill docs, inject them into the prompt first
            # and ask LLM to re-call with correct parameters
            if skill_docs:
                self._log(f"\n📖 Injecting Skill documentation into prompt...")
                messages.append({
                    "role": "system", 
                    "content": skill_docs
                })
                messages.append({
                    "role": "user",
                    "content": "Please re-call the tools with correct parameters based on the complete Skill documentation above."
                })
                continue
            
            messages.append(message)
            
            self._log(f"\n⚙️  Executing tools...")
            if self.custom_tool_handler:
                tool_results = self.custom_tool_handler(
                    response, self.manager, allow_network, timeout
                )
            else:
                tool_results = self.manager.handle_tool_calls(
                    response, allow_network=allow_network, timeout=timeout
                )
            
            self._log(f"\n📊 Tool execution results:")
            for idx, (result, tc) in enumerate(zip(tool_results, message.tool_calls), 1):
                output = result.content
                if len(output) > 500:
                    output = output[:500] + "... (truncated)"
                self._log(f"   {idx}. {tc.function.name}")
                self._log(f"      Result: {output}")
            
            for result in tool_results:
                messages.append(result.to_openai_format())
            
            # Check task completion
            if self.enable_task_planning:
                if message.content:
                    completed_id = self._check_task_completion_in_content(message.content)
                    if completed_id:
                        self._update_task_list(completed_id)
                
                if self._check_all_tasks_completed():
                    self._log("🎯 All tasks completed, ending iteration")
                    final_response = self.client.chat.completions.create(
                        model=self.model, messages=messages, tools=None
                    )
                    return final_response
                
                # Update task focus
                current_task = next((t for t in self.task_list if not t["completed"]), None)
                if current_task:
                    task_list_str = json.dumps(self.task_list, ensure_ascii=False, indent=2)
                    messages.append({
                        "role": "system",
                        "content": f"Task progress update:\n{task_list_str}\n\nCurrent task to execute: Task {current_task['id']} - {current_task['description']}\n\nPlease continue to focus on completing the current task."
                    })
        
        self._log(f"\n⚠️  Reached maximum iterations ({self.max_iterations}), stopping execution")
        return response
    
    # ==================== Claude Native API ====================
    
    def _run_claude_native(
        self,
        user_message: str,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> Any:
        """Run loop using Claude's native API."""
        messages = [{"role": "user", "content": user_message}]
        tools = self.manager.get_tools_for_claude_native()
        
        # Build system prompt
        system = self.system_prompt or ""
        if self.enable_task_planning and self.task_list:
            system = (system + "\n\n" if system else "") + self._get_task_system_prompt()
        
        response = None
        
        for iteration in range(self.max_iterations):
            self._log(f"\n🔄 Iteration #{iteration + 1}/{self.max_iterations}")
            
            self._log("⏳ Calling LLM...")
            
            kwargs = {
                "model": self.model,
                "max_tokens": self.extra_kwargs.get("max_tokens", 4096),
                "tools": tools,
                "messages": messages,
                **{k: v for k, v in self.extra_kwargs.items() if k != "max_tokens"}
            }
            if system:
                kwargs["system"] = system
            
            response = self.client.messages.create(**kwargs)
            
            self._log(f"✅ LLM response completed (stop_reason: {response.stop_reason})")
            
            # No tool use
            if response.stop_reason != "tool_use":
                self._log("📝 LLM did not call any tools")
                
                if self.enable_task_planning:
                    # Extract text content
                    text_content = ""
                    for block in response.content:
                        if hasattr(block, 'text'):
                            text_content += block.text
                    
                    completed_id = self._check_task_completion_in_content(text_content)
                    if completed_id:
                        self._update_task_list(completed_id)
                    
                    if self._check_all_tasks_completed():
                        self._log("🎯 All tasks completed, ending iteration")
                        return response
                    else:
                        self._log("⏳ There are still pending tasks, continuing execution...")
                        messages.append({"role": "assistant", "content": response.content})
                        messages.append({"role": "user", "content": "Please continue to complete the remaining tasks."})
                        continue
                else:
                    return response
            
            # Handle tool calls
            tool_use_blocks = [b for b in response.content if hasattr(b, 'type') and b.type == 'tool_use']
            self._log(f"\n🔧 LLM decided to call {len(tool_use_blocks)} tools:")
            for idx, block in enumerate(tool_use_blocks, 1):
                self._log(f"   {idx}. {block.name}")
                self._log(f"      Arguments: {json.dumps(block.input, ensure_ascii=False)}")
            
            messages.append({"role": "assistant", "content": response.content})
            
            self._log(f"\n⚙️  Executing tools...")
            tool_results = self.manager.handle_tool_calls_claude_native(
                response, allow_network=allow_network, timeout=timeout
            )
            
            self._log(f"\n📊 Tool execution results:")
            for idx, result in enumerate(tool_results, 1):
                output = result.content
                if len(output) > 500:
                    output = output[:500] + "... (truncated)"
                self._log(f"   {idx}. Result: {output}")
            
            formatted_results = self.manager.format_tool_results_claude_native(tool_results)
            messages.append({"role": "user", "content": formatted_results})
            
            # Check task completion
            if self.enable_task_planning:
                text_content = ""
                for block in response.content:
                    if hasattr(block, 'text'):
                        text_content += block.text
                
                completed_id = self._check_task_completion_in_content(text_content)
                if completed_id:
                    self._update_task_list(completed_id)
                
                if self._check_all_tasks_completed():
                    self._log("🎯 All tasks completed, ending iteration")
                    final_response = self.client.messages.create(
                        model=self.model,
                        max_tokens=self.extra_kwargs.get("max_tokens", 4096),
                        system=system if system else None,
                        messages=messages
                    )
                    return final_response
        
        self._log(f"\n⚠️  Reached maximum iterations ({self.max_iterations}), stopping execution")
        return response
    
    # ==================== Public API ====================
    
    def run(
        self,
        user_message: str,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> Any:
        """
        Run the agentic loop until completion.
        
        Args:
            user_message: The user's message
            allow_network: Override default network setting for skill execution
            timeout: Execution timeout per tool call in seconds
            
        Returns:
            The final LLM response
        """
        # Generate task list if enabled
        if self.enable_task_planning:
            self.task_list = self._generate_task_list(user_message)
            
            # If task list is empty, the task can be completed by LLM directly
            # Disable task planning mode for this run
            if not self.task_list:
                self._log("\n💡 Task can be completed directly by LLM, no tools needed")
                self.enable_task_planning = False
        
        # Dispatch to appropriate implementation
        if self.api_format == ApiFormat.OPENAI:
            return self._run_openai(user_message, allow_network, timeout)
        else:
            return self._run_claude_native(user_message, allow_network, timeout)


# Backward compatibility alias
AgenticLoopClaudeNative = AgenticLoop
