"""
Agentic Loops - Continuous tool execution loops for LLM interactions.

This module provides a unified agentic loop implementation that supports
both OpenAI-compatible APIs and Claude's native API through a single interface.
"""

import json
from typing import Any, List, Optional, TYPE_CHECKING, Dict, Callable

from ..logger import get_logger, strip_ansi, step_header, ANSI_DIM, ANSI_RESET
from ..config.env_config import get_long_text_summarize_threshold, get_tool_result_max_chars
from ..extensions.long_text import summarize_long_content, truncate_content
from .task_planner import ApiFormat, TaskPlanner
from .tools import ToolResult

if TYPE_CHECKING:
    from .manager import SkillManager

# Max chars per tool result (~2k tokens). Configurable via SKILLLITE_TOOL_RESULT_MAX_CHARS.
TOOL_RESULT_MAX_CHARS = get_tool_result_max_chars()

# Max chars for context-overflow recovery retry (more aggressive truncation)
TOOL_RESULT_RECOVERY_MAX_CHARS = 3000


# ---------------------------------------------------------------------------
# Streaming support: lightweight mock objects that mimic OpenAI SDK types
# so accumulated streaming chunks can be consumed by the same code path.
# ---------------------------------------------------------------------------

class _MockFunction:
    """Mock for ChatCompletionMessageToolCall.function."""
    __slots__ = ('name', 'arguments')

    def __init__(self, name: str, arguments: str):
        self.name = name
        self.arguments = arguments


class _MockToolCall:
    """Mock for ChatCompletionMessageToolCall."""
    __slots__ = ('id', 'type', 'function', 'index')

    def __init__(self, id: str, function: "_MockFunction"):
        self.id = id
        self.type = "function"
        self.function = function
        self.index = 0


class _MockMessage:
    """Mock for ChatCompletionMessage."""
    __slots__ = ('role', 'content', 'tool_calls')

    def __init__(self, role: str = "assistant", content: Optional[str] = None,
                 tool_calls: Optional[list] = None):
        self.role = role
        self.content = content
        self.tool_calls = tool_calls


class _MockChoice:
    """Mock for Choice."""
    __slots__ = ('message', 'finish_reason')

    def __init__(self, message: "_MockMessage", finish_reason: str):
        self.message = message
        self.finish_reason = finish_reason


class _MockResponse:
    """Mock for ChatCompletion."""
    __slots__ = ('choices',)

    def __init__(self, choices: list):
        self.choices = choices


def _is_context_overflow_error(exc: Exception) -> bool:
    """Check if exception is due to context length overflow."""
    msg = str(exc).lower()
    return (
        "maximum context length" in msg
        or "context_length_exceeded" in msg
        or "token" in msg and "exceeded" in msg
    )


def _truncate_tool_messages_in_place(messages: List[Dict[str, Any]], max_chars: int) -> None:
    """Truncate content of all tool messages in messages list (in place)."""
    for m in messages:
        if m.get("role") == "tool" and "content" in m:
            content = m["content"]
            if len(content) > max_chars:
                m["content"] = content[:max_chars] + f"\n\n[... 已截断至 {max_chars} 字符 ...]"


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
        max_iterations: int = 50,
        max_tool_calls_per_task: int = 30,
        api_format: ApiFormat = ApiFormat.OPENAI,
        custom_tool_handler: Optional[Callable] = None,
        custom_tools: Optional[List[Dict[str, Any]]] = None,
        enable_task_planning: bool = True,
        verbose: bool = True,
        confirmation_callback: Optional[Callable[[str, str], bool]] = None,
        planning_rules: Optional[List[Dict[str, Any]]] = None,
        planning_rules_path: Optional[Any] = None,
        **kwargs
    ):
        """
        Initialize the agentic loop.

        Args:
            manager: SkillManager instance
            client: LLM client (OpenAI or Anthropic)
            model: Model name to use
            system_prompt: Optional system prompt
            max_iterations: Global maximum iterations (safety cap). With task planning,
                effective limit = min(max_iterations, num_tasks * max_tool_calls_per_task).
            max_tool_calls_per_task: Max tool-call rounds per task. Prevents one task
                from consuming all iterations. Reset when task completes.
            api_format: API format to use (OPENAI or CLAUDE_NATIVE)
            custom_tool_handler: Optional custom tool handler function
            custom_tools: Additional tool definitions (e.g. builtin, memory)
            enable_task_planning: Whether to generate task list before execution
            verbose: Whether to print detailed logs
            confirmation_callback: Callback for security confirmation (sandbox_level=3).
                Signature: (security_report: str, scan_id: str) -> bool
                If None and sandbox_level=3, will use interactive terminal confirmation.
            planning_rules: Optional custom planning rules (merged with defaults by id).
            planning_rules_path: Optional path to planning_rules.json (overrides default).
            **kwargs: Additional arguments passed to the LLM
        """
        self.manager = manager
        self.client = client
        self.model = model
        self.system_prompt = system_prompt
        self.max_iterations = max_iterations
        self.max_tool_calls_per_task = max_tool_calls_per_task
        self.api_format = api_format
        self.custom_tool_handler = custom_tool_handler
        self.custom_tools = custom_tools or []
        self.enable_task_planning = enable_task_planning
        self.verbose = verbose
        self.confirmation_callback = confirmation_callback
        self.extra_kwargs = kwargs
        self._no_tools_needed = False  # Set True when task planner says no tools needed
        self._max_no_tool_retries = 3  # Max consecutive iterations without tool calls before giving up
        self._on_plan_updated: Optional[Callable[[List[Dict[str, Any]]], None]] = None

        # Delegate task planning to TaskPlanner
        self._planner = TaskPlanner(
            client=client,
            model=model,
            api_format=api_format,
            verbose=verbose,
            extra_kwargs=kwargs,
            planning_rules=planning_rules,
            planning_rules_path=planning_rules_path,
        )
        
        # Initialize logger
        self._logger = get_logger("skilllite.core.loops", verbose=verbose)

    def _log(self, message: str) -> None:
        """Log message if verbose mode is enabled."""
        if self.verbose:
            self._logger.info(message)

    # ---- Formatting helpers for clean log output ----

    @staticmethod
    def _fmt_tool_args(args_json: str) -> str:
        """Format tool arguments for compact display."""
        try:
            args = json.loads(args_json)
            if isinstance(args, dict):
                parts = []
                for k, v in args.items():
                    v_str = str(v)
                    if len(v_str) > 100:
                        v_str = v_str[:100] + "..."
                    parts.append(f"{k}={v_str}")
                return ", ".join(parts)
        except (json.JSONDecodeError, TypeError):
            pass
        return args_json[:200] + ("..." if len(args_json) > 200 else "")

    @staticmethod
    def _fmt_tool_result(content: str, max_len: int = 300) -> str:
        """Format tool result for compact one-line display."""
        content = strip_ansi(content)
        # Try to extract stdout from JSON envelope
        try:
            data = json.loads(content)
            if isinstance(data, dict) and "stdout" in data:
                content = data["stdout"]
                if data.get("stderr"):
                    content += f" [stderr: {data['stderr']}]"
        except (json.JSONDecodeError, TypeError):
            pass
        content = content.strip().replace('\n', ' ↵ ')
        if len(content) > max_len:
            content = content[:max_len] + "..."
        return content

    def _get_bash_tool_skills_context(self) -> str:
        """Build system prompt section with full SKILL.md for all bash-tool skills.

        Bash-tool skills need their full documentation injected at startup
        (not progressively), because the SKILL.md *is* the operational manual
        that tells the LLM how to use the CLI commands.

        Returns:
            Formatted context string, or empty string if no bash-tool skills.
        """
        registry = self.manager._registry
        bash_skills = registry.list_bash_tool_skills()
        if not bash_skills:
            return ""

        parts = ["\n# Bash Tool Skills — Full Documentation\n"]
        parts.append("The following skills are CLI tools. Use the tool function to send bash commands.\n")

        for info in bash_skills:
            content = info.get_full_content()
            if content:
                parts.append(f"\n## {info.name}\n")
                parts.append(content)

                # Include references if available
                refs = info.get_references()
                if refs:
                    parts.append(f"\n### References for {info.name}\n")
                    for ref_name, ref_content in refs.items():
                        parts.append(f"\n#### {ref_name}\n")
                        parts.append(ref_content)

        return "\n".join(parts)

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

    def _process_tool_result_content(self, content: str) -> str:
        """Process long tool result: chunked summarization if very long, else truncate."""
        max_chars = get_tool_result_max_chars()
        threshold = get_long_text_summarize_threshold()
        if len(content) <= max_chars:
            return content
        if len(content) <= threshold:
            return truncate_content(content, max_chars)
        self._log(f"📝 Long content ({len(content)} chars), summarize 开头+结尾 (head+tail)...")
        api_fmt = self.api_format.value if hasattr(self.api_format, "value") else "openai"
        return summarize_long_content(
            self.client,
            self.model,
            content,
            api_format=api_fmt,
            max_output_chars=max_chars,
            logger=self._log,
        )

    # Task planning is delegated to self._planner (TaskPlanner)

    # ==================== Streaming helpers ====================

    @staticmethod
    def _message_to_dict(message: Any) -> Dict[str, Any]:
        """Convert a ChatCompletionMessage (or mock) to a plain dict.

        This is safe for both real SDK objects and _MockMessage instances,
        and produces a dict that the OpenAI SDK accepts in the messages list.
        """
        if isinstance(message, dict):
            return message
        d: Dict[str, Any] = {
            "role": getattr(message, "role", "assistant"),
            "content": message.content,
        }
        if message.tool_calls:
            d["tool_calls"] = [
                {
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.function.name,
                        "arguments": tc.function.arguments,
                    },
                }
                for tc in message.tool_calls
            ]
        return d

    def _call_openai(
        self,
        stream_callback: Optional[Callable[[str], None]],
        **api_kwargs: Any,
    ) -> Any:
        """Call OpenAI-compatible API. Streams text to *stream_callback* when provided.

        All keyword arguments are forwarded directly to
        ``client.chat.completions.create``.  Returns a response object
        (real ``ChatCompletion`` or ``_MockResponse``) whose interface is
        identical for downstream code.
        """
        if stream_callback:
            api_kwargs["stream"] = True
            stream_iter = self.client.chat.completions.create(**api_kwargs)
            return self._accumulate_openai_stream(stream_iter, stream_callback)
        api_kwargs.pop("stream", None)
        return self.client.chat.completions.create(**api_kwargs)

    @staticmethod
    def _accumulate_openai_stream(stream, stream_callback: Callable[[str], None]) -> "_MockResponse":
        """Iterate over a streaming response, forward text chunks, return accumulated mock."""
        content_parts: List[str] = []
        tool_calls_acc: Dict[int, Dict[str, str]] = {}
        finish_reason: Optional[str] = None

        for chunk in stream:
            if not chunk.choices:
                continue
            choice = chunk.choices[0]
            delta = choice.delta

            if choice.finish_reason:
                finish_reason = choice.finish_reason

            # --- text content ---
            if hasattr(delta, "content") and delta.content:
                content_parts.append(delta.content)
                stream_callback(delta.content)

            # --- tool calls (accumulated across chunks) ---
            if hasattr(delta, "tool_calls") and delta.tool_calls:
                for tc_delta in delta.tool_calls:
                    idx = tc_delta.index
                    if idx not in tool_calls_acc:
                        tool_calls_acc[idx] = {"id": "", "name": "", "arguments": ""}
                    if tc_delta.id:
                        tool_calls_acc[idx]["id"] = tc_delta.id
                    if tc_delta.function:
                        if tc_delta.function.name:
                            tool_calls_acc[idx]["name"] += tc_delta.function.name
                        if tc_delta.function.arguments:
                            tool_calls_acc[idx]["arguments"] += tc_delta.function.arguments

        content = "".join(content_parts) if content_parts else None

        mock_tcs: Optional[List[_MockToolCall]] = None
        if tool_calls_acc:
            mock_tcs = [
                _MockToolCall(
                    id=tool_calls_acc[i]["id"],
                    function=_MockFunction(
                        name=tool_calls_acc[i]["name"],
                        arguments=tool_calls_acc[i]["arguments"],
                    ),
                )
                for i in sorted(tool_calls_acc.keys())
            ]

        msg = _MockMessage(content=content, tool_calls=mock_tcs)
        ch = _MockChoice(message=msg, finish_reason=finish_reason or "stop")
        return _MockResponse(choices=[ch])

    def _call_claude(
        self,
        stream_callback: Optional[Callable[[str], None]],
        **api_kwargs: Any,
    ) -> Any:
        """Call Claude native API. Streams text to *stream_callback* when provided.

        Uses the Anthropic SDK ``messages.stream()`` context manager which
        yields the full ``Message`` object via ``get_final_message()``, so
        no mock is needed.
        """
        if stream_callback:
            try:
                with self.client.messages.stream(**api_kwargs) as stream_obj:
                    for text in stream_obj.text_stream:
                        stream_callback(text)
                    return stream_obj.get_final_message()
            except AttributeError:
                # Fallback: older SDK without streaming support
                return self.client.messages.create(**api_kwargs)
        return self.client.messages.create(**api_kwargs)

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
            if tool_name in ['read_file', 'write_file', 'write_output', 'list_directory', 'file_exists',
                            'memory_search', 'memory_write', 'memory_list', 'run_command', 'preview_server']:
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
        stream_callback: Optional[Callable[[str], None]] = None,
    ) -> Any:
        """Run loop using OpenAI-compatible API."""
        messages = []

        if self.system_prompt:
            messages.append({"role": "system", "content": self.system_prompt})

        # Inject full SKILL.md for bash-tool skills (they need docs upfront, not progressive)
        bash_ctx = self._get_bash_tool_skills_context()
        if bash_ctx:
            messages.append({"role": "system", "content": bash_ctx})

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
        tool_calls_count_current_task = 0  # Reset when task completes

        # Plan-based budget: effective_max = min(global, num_tasks * per_task)
        if self.enable_task_planning and self._planner.task_list:
            plan_budget = len(self._planner.task_list) * self.max_tool_calls_per_task
            effective_max = min(self.max_iterations, plan_budget)
        else:
            effective_max = self.max_iterations

        for iteration in range(effective_max):
            self._log(f"\n{step_header(iteration + 1, effective_max)}")

            try:
                response = self._call_openai(
                    stream_callback,
                    model=self.model,
                    messages=messages,
                    tools=tools if tools else None,
                    **self.extra_kwargs
                )
            except Exception as e:
                if _is_context_overflow_error(e):
                    self._log(f"⚠️  Context overflow detected, attempting recovery...")
                    _truncate_tool_messages_in_place(messages, TOOL_RESULT_RECOVERY_MAX_CHARS)
                    try:
                        response = self._call_openai(
                            stream_callback,
                            model=self.model,
                            messages=messages,
                            tools=tools if tools else None,
                            **self.extra_kwargs
                        )
                    except Exception as retry_e:
                        self._log(f"❌ Recovery failed: {retry_e}")
                        raise RuntimeError(
                            f"上下文长度超限，恢复失败。建议：1) 使用 /clear 清空对话 2) 避免请求返回超大内容的操作（如抓取整页 HTML）"
                        ) from retry_e
                else:
                    raise

            message = response.choices[0].message
            finish_reason = response.choices[0].finish_reason

            # Ensure newline after streamed text so logs don't collide
            if stream_callback and message.content:
                print()

            # No tool calls
            if not message.tool_calls:

                if self.enable_task_planning:
                    completed_id = self._planner.check_completion_in_content(message.content)
                    if completed_id:
                        self._planner.update_task_list(completed_id)
                        if self._on_plan_updated:
                            self._on_plan_updated(self._planner.task_list)
                        consecutive_no_tool = 0  # Reset: made progress
                        tool_calls_count_current_task = 0  # Reset for next task

                    if self._planner.check_all_completed():
                        self._log("🎯 All tasks completed, ending iteration")
                        return response
                    else:
                        # Guard: if LLM keeps not calling tools and not completing tasks, stop
                        if not completed_id:
                            consecutive_no_tool += 1
                        if consecutive_no_tool >= self._max_no_tool_retries:
                            self._log(f"⚠️  LLM failed to make progress after {self._max_no_tool_retries} attempts, stopping")
                            return response

                        # Tasks not complete and no tool calls — nudge the LLM
                        # to continue working (mirror Claude-native behaviour).
                        self._log(f"  {ANSI_DIM}↳ pending tasks remain, continuing...{ANSI_RESET}")
                        messages.append(self._message_to_dict(message))

                        current_task = next(
                            (t for t in self._planner.task_list if not t["completed"]),
                            None,
                        )
                        task_list_str = json.dumps(
                            self._planner.task_list, ensure_ascii=False, indent=2
                        )
                        if current_task:
                            tool_hint = current_task.get("tool_hint", "")
                            if tool_hint and tool_hint not in ("file_operation", "analysis"):
                                nudge = (
                                    f"Task progress update:\n{task_list_str}\n\n"
                                    f"Current task to execute: Task {current_task['id']} - "
                                    f"{current_task['description']}\n\n"
                                    f"⚡ Call `{tool_hint}` DIRECTLY now. Do NOT call list_directory or read_file first."
                                )
                            else:
                                nudge = (
                                    f"Task progress update:\n{task_list_str}\n\n"
                                    f"Current task to execute: Task {current_task['id']} - "
                                    f"{current_task['description']}\n\n"
                                    "Please use the available tools to complete this task."
                                )
                        else:
                            nudge = "Please continue to complete the remaining tasks."
                        messages.append({"role": "user", "content": nudge})
                        continue
                else:
                    return response
            
            # Handle tool calls
            consecutive_no_tool = 0  # Reset: LLM is calling tools
            tool_calls_count_current_task += 1

            for tc in message.tool_calls:
                args_display = self._fmt_tool_args(tc.function.arguments)
                self._log(f"  🔧 {tc.function.name}")
                self._log(f"     {ANSI_DIM}{args_display}{ANSI_RESET}")

            # Get full SKILL.md content for tools that haven't been documented yet
            skill_docs = self._get_skill_docs_for_tools(message.tool_calls)
            
            # If we have new skill docs, inject them into the prompt first
            # and ask LLM to re-call with correct parameters
            if skill_docs:
                self._log(f"  📖 Injecting Skill docs...")
                messages.append({
                    "role": "system", 
                    "content": skill_docs
                })
                messages.append({
                    "role": "user",
                    "content": "Please re-call the tools with correct parameters based on the complete Skill documentation above."
                })
                continue
            
            messages.append(self._message_to_dict(message))

            # Execute tools
            if self.custom_tool_handler:
                tool_results = self.custom_tool_handler(
                    response, self.manager, allow_network, timeout
                )
            else:
                tool_results = self.manager.handle_tool_calls(
                    response,
                    confirmation_callback=self.confirmation_callback or self._interactive_confirmation,
                    allow_network=allow_network,
                    timeout=timeout
                )

            for result, tc in zip(tool_results, message.tool_calls):
                icon = "❌" if result.is_error else "✅"
                output = self._fmt_tool_result(result.content)
                self._log(f"  {icon} {tc.function.name} → {output}")

            for result in tool_results:
                processed = self._process_tool_result_content(result.content)
                messages.append({
                    "role": "tool",
                    "tool_call_id": result.tool_use_id,
                    "content": processed,
                })

            # Per-task depth limit: after executing tools, ask LLM to wrap up if over limit
            if (self.enable_task_planning and self._planner.task_list
                    and tool_calls_count_current_task >= self.max_tool_calls_per_task):
                current_task = next((t for t in self._planner.task_list if not t["completed"]), None)
                if current_task:
                    self._log(f"\n⚠️  Task {current_task['id']} reached max tool calls ({self.max_tool_calls_per_task}), requesting summary...")
                    nudge = (
                        f"You have used {self.max_tool_calls_per_task} tool calls for the current task. "
                        f"Based on the information gathered so far, please provide a brief summary, "
                        f"mark this task as completed, and proceed to the next task."
                    )
                    messages.append({"role": "user", "content": nudge})
                    continue
            
            # Check task completion
            if self.enable_task_planning:
                if message.content:
                    completed_id = self._planner.check_completion_in_content(message.content)
                    if completed_id:
                        self._planner.update_task_list(completed_id)
                        if self._on_plan_updated:
                            self._on_plan_updated(self._planner.task_list)
                        tool_calls_count_current_task = 0  # Reset for next task

                if self._planner.check_all_completed():
                    self._log("🎯 All tasks completed, ending iteration")
                    try:
                        final_response = self._call_openai(
                            stream_callback,
                            model=self.model, messages=messages, tools=None
                        )
                    except Exception as e:
                        if _is_context_overflow_error(e):
                            self._log(f"⚠️  Context overflow on final response, attempting recovery...")
                            _truncate_tool_messages_in_place(messages, TOOL_RESULT_RECOVERY_MAX_CHARS)
                            try:
                                final_response = self._call_openai(
                                    stream_callback,
                                    model=self.model, messages=messages, tools=None
                                )
                            except Exception as retry_e:
                                raise RuntimeError(
                                    f"上下文长度超限。请使用 /clear 清空对话后重试。"
                                ) from retry_e
                        else:
                            raise
                    return final_response

                # Update task focus
                current_task = next((t for t in self._planner.task_list if not t["completed"]), None)
                if current_task:
                    task_list_str = json.dumps(self._planner.task_list, ensure_ascii=False, indent=2)
                    tool_hint = current_task.get("tool_hint", "")
                    if tool_hint and tool_hint not in ("file_operation", "analysis"):
                        focus_msg = (
                            f"Task progress update:\n{task_list_str}\n\n"
                            f"Current task to execute: Task {current_task['id']} - {current_task['description']}\n\n"
                            f"⚡ Call `{tool_hint}` DIRECTLY. Do NOT explore files first."
                        )
                    else:
                        focus_msg = (
                            f"Task progress update:\n{task_list_str}\n\n"
                            f"Current task to execute: Task {current_task['id']} - {current_task['description']}\n\n"
                            "Please continue to focus on completing the current task."
                        )
                    messages.append({
                        "role": "system",
                        "content": focus_msg
                    })
        
        self._log(f"\n⚠️  Reached maximum iterations ({effective_max}), stopping execution")
        return response
    
    # ==================== Claude Native API ====================
    
    def _run_claude_native(
        self,
        user_message: str,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None,
        initial_messages: Optional[List[Dict[str, Any]]] = None,
        stream_callback: Optional[Callable[[str], None]] = None,
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

        # Inject full SKILL.md for bash-tool skills (they need docs upfront, not progressive)
        bash_ctx = self._get_bash_tool_skills_context()
        if bash_ctx:
            system = (system + "\n\n" if system else "") + bash_ctx

        if self.enable_task_planning and self._planner.task_list:
            system = (system + "\n\n" if system else "") + self._planner.build_task_system_prompt(self.manager)
        
        response = None
        consecutive_no_tool = 0
        tool_calls_count_current_task = 0

        # Plan-based budget (same as OpenAI path)
        if self.enable_task_planning and self._planner.task_list:
            plan_budget = len(self._planner.task_list) * self.max_tool_calls_per_task
            effective_max = min(self.max_iterations, plan_budget)
        else:
            effective_max = self.max_iterations

        for iteration in range(effective_max):
            self._log(f"\n{step_header(iteration + 1, effective_max)}")

            kwargs = {
                "model": self.model,
                "max_tokens": self.extra_kwargs.get("max_tokens", 4096),
                "tools": tools,
                "messages": messages,
                **{k: v for k, v in self.extra_kwargs.items() if k != "max_tokens"}
            }
            if system:
                kwargs["system"] = system

            try:
                response = self._call_claude(stream_callback, **kwargs)
            except Exception as e:
                if _is_context_overflow_error(e):
                    self._log(f"⚠️  Context overflow detected (Claude), cannot auto-recover. Use /clear to reset.")
                raise

            # Ensure newline after streamed text so logs don't collide
            text_content_blocks = [b for b in response.content if hasattr(b, 'text')]
            if stream_callback and text_content_blocks:
                print()

            # No tool use
            if response.stop_reason != "tool_use":

                if self.enable_task_planning:
                    # Extract text content
                    text_content = ""
                    for block in response.content:
                        if hasattr(block, 'text'):
                            text_content += block.text

                    completed_id = self._planner.check_completion_in_content(text_content)
                    if completed_id:
                        self._planner.update_task_list(completed_id)
                        if self._on_plan_updated:
                            self._on_plan_updated(self._planner.task_list)
                        consecutive_no_tool = 0
                        tool_calls_count_current_task = 0

                    if self._planner.check_all_completed():
                        self._log("🎯 All tasks completed, ending iteration")
                        return response
                    else:
                        if not completed_id:
                            consecutive_no_tool += 1
                        if consecutive_no_tool >= self._max_no_tool_retries:
                            self._log(f"⚠️  LLM failed to make progress after {self._max_no_tool_retries} attempts, stopping")
                            return response

                        self._log(f"  {ANSI_DIM}↳ pending tasks remain, continuing...{ANSI_RESET}")
                        messages.append({"role": "assistant", "content": response.content})
                        messages.append({"role": "user", "content": "Please continue to complete the remaining tasks."})
                        continue
                else:
                    return response
            
            # Handle tool calls
            consecutive_no_tool = 0
            tool_calls_count_current_task += 1
            tool_use_blocks = [b for b in response.content if hasattr(b, 'type') and b.type == 'tool_use']
            for block in tool_use_blocks:
                args_display = self._fmt_tool_args(json.dumps(block.input, ensure_ascii=False))
                self._log(f"  🔧 {block.name}")
                self._log(f"     {ANSI_DIM}{args_display}{ANSI_RESET}")
            
            messages.append({"role": "assistant", "content": response.content})

            # Execute tools
            tool_results = self.manager.handle_tool_calls_claude_native(
                response,
                confirmation_callback=self.confirmation_callback or self._interactive_confirmation,
                allow_network=allow_network,
                timeout=timeout
            )

            for idx, result in enumerate(tool_results):
                icon = "❌" if result.is_error else "✅"
                output = self._fmt_tool_result(result.content)
                tool_name = tool_use_blocks[idx].name if idx < len(tool_use_blocks) else "tool"
                self._log(f"  {icon} {tool_name} → {output}")

            # Process long content (summarize or truncate) before adding to context
            processed_results = [
                ToolResult(
                    result.tool_use_id,
                    self._process_tool_result_content(result.content),
                    result.is_error,
                )
                for result in tool_results
            ]
            formatted_results = self.manager.format_tool_results_claude_native(processed_results)
            messages.append({"role": "user", "content": formatted_results})

            # Per-task depth limit (same as OpenAI path)
            if (self.enable_task_planning and self._planner.task_list
                    and tool_calls_count_current_task >= self.max_tool_calls_per_task):
                current_task = next((t for t in self._planner.task_list if not t["completed"]), None)
                if current_task:
                    self._log(f"\n⚠️  Task {current_task['id']} reached max tool calls ({self.max_tool_calls_per_task}), requesting summary...")
                    nudge = (
                        f"You have used {self.max_tool_calls_per_task} tool calls for the current task. "
                        f"Based on the information gathered so far, please provide a brief summary, "
                        f"mark this task as completed, and proceed to the next task."
                    )
                    messages.append({"role": "user", "content": nudge})
                    continue
            
            # Check task completion
            if self.enable_task_planning:
                text_content = ""
                for block in response.content:
                    if hasattr(block, 'text'):
                        text_content += block.text
                
                completed_id = self._planner.check_completion_in_content(text_content)
                if completed_id:
                    self._planner.update_task_list(completed_id)
                    if self._on_plan_updated:
                        self._on_plan_updated(self._planner.task_list)
                    tool_calls_count_current_task = 0

                if self._planner.check_all_completed():
                    self._log("🎯 All tasks completed, ending iteration")
                    try:
                        final_response = self._call_claude(
                            stream_callback,
                            model=self.model,
                            max_tokens=self.extra_kwargs.get("max_tokens", 4096),
                            system=system if system else None,
                            messages=messages
                        )
                    except Exception as e:
                        if _is_context_overflow_error(e):
                            raise RuntimeError("上下文长度超限。请使用 /clear 清空对话后重试。") from e
                        raise
                    return final_response
        
        self._log(f"\n⚠️  Reached maximum iterations ({effective_max}), stopping execution")
        return response
    
    # ==================== Public API ====================
    
    def run(
        self,
        user_message: str,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None,
        initial_messages: Optional[List[Dict[str, Any]]] = None,
        conversation_context: Optional[str] = None,
        on_plan_generated: Optional[Callable[[List[Dict[str, Any]]], None]] = None,
        on_plan_updated: Optional[Callable[[List[Dict[str, Any]]], None]] = None,
        stream_callback: Optional[Callable[[str], None]] = None,
    ) -> Any:
        """
        Run the agentic loop until completion.

        Args:
            user_message: The user's message
            allow_network: Override default network setting for skill execution
            timeout: Execution timeout per tool call in seconds
            initial_messages: Optional conversation history to prepend (for chat sessions)
            conversation_context: Optional recent conversation summary for planner (for "继续" etc.)
            on_plan_generated: Optional callback when task plan is generated (task_list)
            on_plan_updated: Optional callback when task list is updated (e.g. step completed)
            stream_callback: Optional callback for streaming text output.
                When provided, LLM text responses are forwarded chunk-by-chunk
                via ``stream_callback(chunk: str)``.  Internal calls (task
                planning, compaction) remain non-streaming so that complete
                content is available for JSON parsing / decision logic.

        Returns:
            The final LLM response
        """
        self._on_plan_updated = on_plan_updated
        # Generate task list if enabled (every turn, so user always sees plan when tasks are needed)
        # NOTE: Task planning LLM call remains non-streaming (needs complete JSON for parsing).
        if self.enable_task_planning:
            self._planner.generate_task_list(user_message, self.manager, conversation_context=conversation_context)

            # If task list is empty, the task can be completed by LLM directly
            if not self._planner.task_list:
                self._log("\n💡 Task can be completed directly by LLM, no tools needed")
                self.enable_task_planning = False
                self._no_tools_needed = True
            else:
                if on_plan_generated:
                    on_plan_generated(self._planner.task_list)

        # Dispatch to appropriate implementation
        if self.api_format == ApiFormat.OPENAI:
            return self._run_openai(user_message, allow_network, timeout, initial_messages, stream_callback)
        else:
            return self._run_claude_native(user_message, allow_network, timeout, initial_messages, stream_callback)


# Backward compatibility alias
AgenticLoopClaudeNative = AgenticLoop
