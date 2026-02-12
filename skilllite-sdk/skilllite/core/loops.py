"""
Agentic Loops - Continuous tool execution loops for LLM interactions.

This module provides a unified agentic loop implementation that supports
both OpenAI-compatible APIs and Claude's native API through a single interface.
"""

import json
from typing import Any, List, Optional, TYPE_CHECKING, Dict, Callable

from ..logger import get_logger
from .task_planner import ApiFormat, TaskPlanner

if TYPE_CHECKING:
    from .manager import SkillManager


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
        custom_tools: Optional[List[Dict[str, Any]]] = None,
        enable_task_planning: bool = True,
        verbose: bool = True,
        confirmation_callback: Optional[Callable[[str, str], bool]] = None,
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
            custom_tools: Additional tool definitions (e.g. builtin, memory)
            enable_task_planning: Whether to generate task list before execution
            verbose: Whether to print detailed logs
            confirmation_callback: Callback for security confirmation (sandbox_level=3).
                Signature: (security_report: str, scan_id: str) -> bool
                If None and sandbox_level=3, will use interactive terminal confirmation.
            **kwargs: Additional arguments passed to the LLM
        """
        self.manager = manager
        self.client = client
        self.model = model
        self.system_prompt = system_prompt
        self.max_iterations = max_iterations
        self.api_format = api_format
        self.custom_tool_handler = custom_tool_handler
        self.custom_tools = custom_tools or []
        self.enable_task_planning = enable_task_planning
        self.verbose = verbose
        self.confirmation_callback = confirmation_callback
        self.extra_kwargs = kwargs
        self._no_tools_needed = False  # Set True when task planner says no tools needed
        self._max_no_tool_retries = 3  # Max consecutive iterations without tool calls before giving up

        # Delegate task planning to TaskPlanner
        self._planner = TaskPlanner(
            client=client,
            model=model,
            api_format=api_format,
            verbose=verbose,
            extra_kwargs=kwargs
        )
        
        # Initialize logger
        self._logger = get_logger("skilllite.core.loops", verbose=verbose)

    def _log(self, message: str) -> None:
        """Log message if verbose mode is enabled."""
        if self.verbose:
            self._logger.info(message)

    def _interactive_confirmation(self, report: str, scan_id: str) -> bool:
        """Default interactive terminal confirmation."""
        self._log(f"\n{report}")
        self._log("\n" + "=" * 60)
        while True:
            response = input("⚠️  Allow execution? (y/n): ").strip().lower()
            if response in ['y', 'yes']:
                return True
            elif response in ['n', 'no']:
                return False
            self._log("Please enter 'y' or 'n'")

    # Task planning is delegated to self._planner (TaskPlanner)


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
            
            # Skip built-in tools (read_file, write_file, etc.) and memory tools
            if tool_name in ['read_file', 'write_file', 'list_directory', 'file_exists',
                            'memory_search', 'memory_write', 'memory_list', 'run_command']:
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
        timeout: Optional[int] = None,
        initial_messages: Optional[List[Dict[str, Any]]] = None,
    ) -> Any:
        """Run loop using OpenAI-compatible API."""
        messages = []

        if self.system_prompt:
            messages.append({"role": "system", "content": self.system_prompt})

        if self.enable_task_planning and self._planner.task_list:
            messages.append({"role": "system", "content": self._planner.build_task_system_prompt(self.manager)})

        if initial_messages:
            messages.extend(initial_messages)

        messages.append({"role": "user", "content": user_message})
        
        tools = None
        if not self._no_tools_needed:
            tools = self.manager.get_tools()
            if self.custom_tools:
                tools = tools + self.custom_tools
        response = None
        consecutive_no_tool = 0  # Track consecutive iterations without tool calls or task progress

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
                    completed_id = self._planner.check_completion_in_content(message.content)
                    if completed_id:
                        self._planner.update_task_list(completed_id)
                        consecutive_no_tool = 0  # Reset: made progress

                    if self._planner.check_all_completed():
                        self._log("🎯 All tasks completed, ending iteration")
                        return response
                    else:
                        # Guard: if LLM keeps not calling tools and not completing tasks, stop
                        if not completed_id:
                            consecutive_no_tool += 1
                        if consecutive_no_tool >= self._max_no_tool_retries:
                            self._log(f"⚠️  LLM failed to call tools or make progress after {self._max_no_tool_retries} consecutive attempts, returning current response")
                            return response

                        # Tasks not complete and no tool calls — nudge the LLM
                        # to continue working (mirror Claude-native behaviour).
                        self._log("⏳ There are still pending tasks, continuing execution...")
                        messages.append(message)

                        current_task = next(
                            (t for t in self._planner.task_list if not t["completed"]),
                            None,
                        )
                        task_list_str = json.dumps(
                            self._planner.task_list, ensure_ascii=False, indent=2
                        )
                        nudge = (
                            f"Task progress update:\n{task_list_str}\n\n"
                            f"Current task to execute: Task {current_task['id']} - "
                            f"{current_task['description']}\n\n"
                            "Please use the available tools to complete this task."
                        ) if current_task else "Please continue to complete the remaining tasks."
                        messages.append({"role": "user", "content": nudge})
                        continue
                else:
                    return response
            
            # Handle tool calls
            consecutive_no_tool = 0  # Reset: LLM is calling tools
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

            # Execute tools using unified execution service
            self._log(f"\n⚙️  Executing tools...")

            if self.custom_tool_handler:
                # Custom tool handler takes precedence
                tool_results = self.custom_tool_handler(
                    response, self.manager, allow_network, timeout
                )
            else:
                # Use unified execution service with confirmation callback
                # This handles security scanning, confirmation, and sandbox level management
                tool_results = self.manager.handle_tool_calls(
                    response,
                    confirmation_callback=self.confirmation_callback or self._interactive_confirmation,
                    allow_network=allow_network,
                    timeout=timeout
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
                    completed_id = self._planner.check_completion_in_content(message.content)
                    if completed_id:
                        self._planner.update_task_list(completed_id)

                if self._planner.check_all_completed():
                    self._log("🎯 All tasks completed, ending iteration")
                    final_response = self.client.chat.completions.create(
                        model=self.model, messages=messages, tools=None
                    )
                    return final_response

                # Update task focus
                current_task = next((t for t in self._planner.task_list if not t["completed"]), None)
                if current_task:
                    task_list_str = json.dumps(self._planner.task_list, ensure_ascii=False, indent=2)
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
        timeout: Optional[int] = None,
        initial_messages: Optional[List[Dict[str, Any]]] = None,
    ) -> Any:
        """Run loop using Claude's native API."""
        messages: List[Dict[str, Any]] = []
        if initial_messages:
            messages.extend(initial_messages)
        messages.append({"role": "user", "content": user_message})
        tools = None
        if not self._no_tools_needed:
            tools = self.manager.get_tools_for_claude_native()
            if self.custom_tools:
                for t in self.custom_tools:
                    if isinstance(t, dict) and t.get("type") == "function":
                        fn = t.get("function", {})
                        tools.append({
                            "name": fn.get("name", ""),
                            "description": fn.get("description", ""),
                            "input_schema": fn.get("parameters", {})
                        })

        # Build system prompt
        system = self.system_prompt or ""
        if self.enable_task_planning and self._planner.task_list:
            system = (system + "\n\n" if system else "") + self._planner.build_task_system_prompt(self.manager)
        
        response = None
        consecutive_no_tool = 0  # Track consecutive iterations without tool calls or task progress

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

                    completed_id = self._planner.check_completion_in_content(text_content)
                    if completed_id:
                        self._planner.update_task_list(completed_id)
                        consecutive_no_tool = 0  # Reset: made progress

                    if self._planner.check_all_completed():
                        self._log("🎯 All tasks completed, ending iteration")
                        return response
                    else:
                        # Guard: if LLM keeps not calling tools and not completing tasks, stop
                        if not completed_id:
                            consecutive_no_tool += 1
                        if consecutive_no_tool >= self._max_no_tool_retries:
                            self._log(f"⚠️  LLM failed to call tools or make progress after {self._max_no_tool_retries} consecutive attempts, returning current response")
                            return response

                        self._log("⏳ There are still pending tasks, continuing execution...")
                        messages.append({"role": "assistant", "content": response.content})
                        messages.append({"role": "user", "content": "Please continue to complete the remaining tasks."})
                        continue
                else:
                    return response
            
            # Handle tool calls
            consecutive_no_tool = 0  # Reset: LLM is calling tools
            tool_use_blocks = [b for b in response.content if hasattr(b, 'type') and b.type == 'tool_use']
            self._log(f"\n🔧 LLM decided to call {len(tool_use_blocks)} tools:")
            for idx, block in enumerate(tool_use_blocks, 1):
                self._log(f"   {idx}. {block.name}")
                self._log(f"      Arguments: {json.dumps(block.input, ensure_ascii=False)}")
            
            messages.append({"role": "assistant", "content": response.content})

            # Execute tools using unified execution service
            self._log(f"\n⚙️  Executing tools...")

            # Use unified execution service with confirmation callback
            # This handles security scanning, confirmation, and sandbox level management
            tool_results = self.manager.handle_tool_calls_claude_native(
                response,
                confirmation_callback=self.confirmation_callback or self._interactive_confirmation,
                allow_network=allow_network,
                timeout=timeout
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
                
                completed_id = self._planner.check_completion_in_content(text_content)
                if completed_id:
                    self._planner.update_task_list(completed_id)

                if self._planner.check_all_completed():
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
        timeout: Optional[int] = None,
        initial_messages: Optional[List[Dict[str, Any]]] = None,
    ) -> Any:
        """
        Run the agentic loop until completion.

        Args:
            user_message: The user's message
            allow_network: Override default network setting for skill execution
            timeout: Execution timeout per tool call in seconds
            initial_messages: Optional conversation history to prepend (for chat sessions)

        Returns:
            The final LLM response
        """
        # Generate task list if enabled
        if self.enable_task_planning and not initial_messages:
            self._planner.generate_task_list(user_message, self.manager)

            # If task list is empty, the task can be completed by LLM directly
            # Disable task planning mode for this run
            if not self._planner.task_list:
                self._log("\n💡 Task can be completed directly by LLM, no tools needed")
                self.enable_task_planning = False
                self._no_tools_needed = True

        # Dispatch to appropriate implementation
        if self.api_format == ApiFormat.OPENAI:
            return self._run_openai(user_message, allow_network, timeout, initial_messages)
        else:
            return self._run_claude_native(user_message, allow_network, timeout, initial_messages)


# Backward compatibility alias
AgenticLoopClaudeNative = AgenticLoop
