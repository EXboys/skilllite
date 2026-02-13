"""
SkillLite Quick Start - Minimal wrapper for running Skills with one line of code.

Provides out-of-the-box convenience functions without manual LLM calls and tool calls handling.

Example:
    ```python
    from skilllite import quick_run
    
    # Run with one line of code
    result = quick_run("Calculate 15 times 27 for me")
    print(result)
    ```
"""

import os
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Union

from .core import SkillManager, AgenticLoop
from .config.env_config import parse_bool_env, get_int_env
from .logger import get_logger


def load_env(env_file: Optional[Union[str, Path]] = None) -> Dict[str, str]:
    """
    Load .env file into environment variables.
    
    Args:
        env_file: Path to .env file, defaults to .env in current directory
        
    Returns:
        Dictionary of loaded environment variables
    """
    if env_file is None:
        env_file = Path.cwd() / ".env"
    else:
        env_file = Path(env_file)
    
    loaded = {}
    if env_file.exists():
        for line in env_file.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                key, value = line.split("=", 1)
                key, value = key.strip(), value.strip()
                if value:
                    os.environ.setdefault(key, value)
                    loaded[key] = value
    return loaded


class SkillRunner:
    """
    Minimal Skill Runner - Encapsulates all initialization and invocation logic.
    
    Example:
        ```python
        from skilllite import SkillRunner
        
        # Method 1: Use .env configuration
        runner = SkillRunner()
        result = runner.run("Calculate 15 times 27 for me")
        
        # Method 2: Explicitly pass configuration
        runner = SkillRunner(
            base_url="https://api.deepseek.com",
            api_key="sk-xxx",
            model="deepseek-chat",
            skills_dir="./.skills"
        )
        result = runner.run("Calculate 15 times 27 for me")
        ```
    """
    
    def __init__(
        self,
        base_url: Optional[str] = None,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        skills_dir: Optional[Union[str, Path]] = None,
        env_file: Optional[Union[str, Path]] = None,
        include_full_instructions: bool = True,
        include_references: bool = True,
        include_assets: bool = True,
        context_mode: str = "full",
        max_tokens_per_skill: Optional[int] = None,
        max_iterations: int = 50,
        max_tool_calls_per_task: int = 30,
        verbose: bool = False,
        custom_tools: Optional[List[Dict[str, Any]]] = None,
        custom_tool_executor: Optional[Callable] = None,
        use_enhanced_loop: bool = True,
        enable_builtin_tools: bool = True,
        allow_network: Optional[bool] = None,
        enable_sandbox: Optional[bool] = None,
        execution_timeout: Optional[int] = None,
        max_memory_mb: Optional[int] = None,
        confirmation_callback: Optional[Callable[[str, str], bool]] = None
    ):
        """
        Initialize SkillRunner.
        
        Args:
            base_url: LLM API URL, defaults to BASE_URL environment variable
            api_key: API key, defaults to API_KEY environment variable
            model: Model name, defaults to MODEL environment variable or "deepseek-chat"
            skills_dir: Skills directory, defaults to "./.skills"
            env_file: Path to .env file, defaults to .env in current directory
            include_full_instructions: Whether to include full SKILL.md in system prompt (legacy)
            include_references: Whether to include references directory content
            include_assets: Whether to include assets directory content
            context_mode: System prompt mode:
                - "summary": Most minimal, only name, description and brief summary
                - "standard": Balanced mode, includes input_schema and usage summary
                - "full": Full mode, includes complete SKILL.md content
                - "progressive": Progressive, summary + on-demand detail prompts
            max_tokens_per_skill: Maximum tokens per skill (for truncation)
            max_iterations: Global max iterations (plan-based budget when task planning enabled)
            max_tool_calls_per_task: Max tool calls per task (default 30)
            verbose: Whether to output detailed logs
            custom_tools: Custom tools list (e.g., file operation tools)
            custom_tool_executor: Custom tool executor function
            use_enhanced_loop: Whether to use enhanced AgenticLoop (default: True)
            enable_builtin_tools: Whether to enable built-in file operation tools (default: True)
            allow_network: Whether to allow skill network access (defaults from .env or False)
            enable_sandbox: Whether to enable sandbox protection (defaults from .env or True)
            execution_timeout: Skill execution timeout in seconds (defaults from .env or 120)
            max_memory_mb: Maximum memory limit in MB (defaults from .env or 512)
            confirmation_callback: Callback for security confirmation when sandbox_level=3.
                Signature: (security_report: str, scan_id: str) -> bool
                If None and sandbox_level=3, will use interactive terminal confirmation.
        """
        # Load .env
        load_env(env_file)
        
        # Configuration
        self.base_url = base_url or os.environ.get("BASE_URL")
        self.api_key = api_key or os.environ.get("API_KEY")
        self.model = model or os.environ.get("MODEL", "deepseek-chat")
        self.skills_dir = skills_dir or "./.skills"
        self.include_full_instructions = include_full_instructions
        self.include_references = include_references
        self.include_assets = include_assets
        self.context_mode = context_mode
        self.max_tokens_per_skill = max_tokens_per_skill
        self.max_iterations = max_iterations
        self.max_tool_calls_per_task = max_tool_calls_per_task
        self.verbose = verbose
        self.enable_builtin_tools = enable_builtin_tools
        
        # Initialize logger
        self._logger = get_logger("skilllite.quick", verbose=verbose)
        
        # Sandbox and network configuration (read from .env or use defaults)
        # Use SKILLBOX_* prefix env vars; fall back to legacy names for compatibility
        self.allow_network = allow_network if allow_network is not None else \
            parse_bool_env("SKILLBOX_ALLOW_NETWORK", False, "ALLOW_NETWORK")
        self.enable_sandbox = enable_sandbox if enable_sandbox is not None else \
            parse_bool_env("SKILLBOX_ENABLE_SANDBOX", True, "ENABLE_SANDBOX")
        self.execution_timeout = execution_timeout or \
            get_int_env("SKILLBOX_TIMEOUT_SECS", 120, "EXECUTION_TIMEOUT")
        self.max_memory_mb = max_memory_mb or \
            get_int_env("SKILLBOX_MAX_MEMORY_MB", 512, "MAX_MEMORY_MB")
        # Read sandbox security level (from .env or default to 3)
        self.sandbox_level = os.environ.get("SKILLBOX_SANDBOX_LEVEL", "3")
        
        # Merge built-in tools and custom tools
        self.custom_tools = custom_tools or []
        if enable_builtin_tools:
            from .builtin_tools import get_builtin_file_tools
            builtin_tools = get_builtin_file_tools()
            self.custom_tools = builtin_tools + self.custom_tools
        
        self.custom_tool_executor = custom_tool_executor
        self.use_enhanced_loop = use_enhanced_loop

        # Security confirmation callback
        # If None and sandbox_level=3, will use interactive terminal confirmation
        self.confirmation_callback = confirmation_callback

        # Lazy initialization
        self._client = None
        self._manager = None
        self._system_context = None
    
    @property
    def client(self):
        """Get OpenAI client (lazy initialization)"""
        if self._client is None:
            from openai import OpenAI
            self._client = OpenAI(base_url=self.base_url, api_key=self.api_key)
        return self._client
    
    @property
    def manager(self) -> SkillManager:
        """Get SkillManager (lazy initialization)"""
        if self._manager is None:
            self._manager = SkillManager(
                skills_dir=self.skills_dir,
                allow_network=self.allow_network,
                enable_sandbox=self.enable_sandbox,
                execution_timeout=self.execution_timeout,
                max_memory_mb=self.max_memory_mb,
                sandbox_level=self.sandbox_level
            )
            if self.verbose:
                self._logger.info(f"📦 Loaded Skills: {self._manager.skill_names()}")
        return self._manager
    
    @property
    def system_context(self) -> str:
        """Get system prompt context"""
        if self._system_context is None:
            # Basic skill context, using new mode parameter
            skill_context = self.manager.get_system_prompt_context(
                include_full_instructions=self.include_full_instructions,
                include_references=self.include_references,
                include_assets=self.include_assets,
                mode=self.context_mode,
                max_tokens_per_skill=self.max_tokens_per_skill
            )
            
            # Add tool calling guidance
            tool_guidance = """
# Tool Calling Guidelines

When calling tools, follow these rules:

1. **Sequential Dependencies**: If a task depends on the result of a previous task, you MUST wait for the previous tool call to complete before making the next one. Do NOT use placeholders like `<result_of_xxx>` - always use actual values.

2. **Parallel Independence**: If multiple tasks are independent of each other, you can call them in parallel in a single turn.

3. **Always Use Real Values**: Tool parameters must be concrete values (numbers, strings, etc.), never references to other tool results.

Example of WRONG approach (don't do this):
- Task: "Calculate 100+200, then multiply by 3"
- Wrong: Call calculator(add, 100, 200) AND calculator(multiply, <result>, 3) in same turn

Example of CORRECT approach:
- Turn 1: Call calculator(add, 100, 200) → get result 300
- Turn 2: Call calculator(multiply, 300, 3) → get result 900

"""
            self._system_context = tool_guidance + skill_context
            
            if self.verbose:
                estimated_tokens = self.manager.estimate_context_tokens(
                    mode=self.context_mode,
                    include_references=self.include_references,
                    include_assets=self.include_assets
                )
                self._logger.info(f"📊 System Prompt estimated tokens: ~{estimated_tokens}")
        return self._system_context
    
    @property
    def tools(self) -> List[Dict[str, Any]]:
        """Get tool definitions list"""
        return self.manager.get_tools()
    
    def run(self, user_message: str, stream: bool = False) -> str:
        """
        Run Skill and return final result.
        
        Args:
            user_message: User input message
            stream: Whether to use streaming output (not supported yet)
            
        Returns:
            Final response content from LLM
        """
        if self.verbose:
            self._logger.info(f"👤 User: {user_message}")
            self._logger.info(f"⏳ Calling LLM...")
        
        # Prepare tool executor
        tool_executor = self.custom_tool_executor
        if self.enable_builtin_tools and tool_executor is None:
            # Create a combined executor (uses create_builtin_tool_executor for sandbox support)
            from .builtin_tools import create_builtin_tool_executor

            workspace_root = Path(self.skills_dir).resolve().parent
            builtin_executor = create_builtin_tool_executor(
                run_command_confirmation=self.confirmation_callback,
                workspace_root=workspace_root,
            )
            builtin_names = {"read_file", "write_file", "list_directory", "file_exists", "run_command"}

            def combined_executor(tool_input: Dict[str, Any]) -> str:
                tool_name = tool_input.get("tool_name")
                if tool_name in builtin_names:
                    return builtin_executor(tool_input)
                elif self.custom_tool_executor:
                    return self.custom_tool_executor(tool_input)
                else:
                    return f"Error: No executor found for tool: {tool_name}"

            tool_executor = combined_executor
        
        # Use enhanced AgenticLoop to handle complete conversation flow
        if self.use_enhanced_loop:
            loop = self.manager.create_enhanced_agentic_loop(
                client=self.client,
                model=self.model,
                max_iterations=self.max_iterations,
                max_tool_calls_per_task=self.max_tool_calls_per_task,
                custom_tools=self.custom_tools if self.custom_tools else None,
                custom_tool_executor=tool_executor,
                confirmation_callback=self.confirmation_callback
            )
        else:
            # Use basic AgenticLoop (backward compatible)
            loop = self.manager.create_agentic_loop(
                client=self.client,
                model=self.model,
                system_prompt=self.system_context,
                max_iterations=self.max_iterations,
                max_tool_calls_per_task=self.max_tool_calls_per_task,
                confirmation_callback=self.confirmation_callback
            )
        
        response = loop.run(
            user_message,
            timeout=self.execution_timeout,
        )
        result = response.choices[0].message.content or ""
        
        if self.verbose:
            self._logger.info(f"🤖 Assistant: {result}")
        
        return result
    
    def run_with_details(self, user_message: str) -> Dict[str, Any]:
        """
        Run Skill and return detailed results (including intermediate process).
        
        Args:
            user_message: User input message
            
        Returns:
            Dictionary containing complete information
        """
        messages = []
        if self.system_context:
            messages.append({"role": "system", "content": self.system_context})
        messages.append({"role": "user", "content": user_message})
        
        tools = self.tools
        tool_calls_history = []
        iterations = 0
        
        for _ in range(self.max_iterations):
            iterations += 1
            response = self.client.chat.completions.create(
                model=self.model,
                tools=tools if tools else None,
                messages=messages
            )
            
            message = response.choices[0].message
            
            if not message.tool_calls:
                return {
                    "content": message.content,
                    "iterations": iterations,
                    "tool_calls": tool_calls_history,
                    "final_response": response
                }
            
            # Record tool calls
            messages.append(message)
            results = self.manager.handle_tool_calls(response)
            
            for tc, result in zip(message.tool_calls, results):
                tool_calls_history.append({
                    "name": tc.function.name,
                    "arguments": tc.function.arguments,
                    "result": result.content
                })
                messages.append({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": result.content
                })
        
        return {
            "content": message.content if message else None,
            "iterations": iterations,
            "tool_calls": tool_calls_history,
            "final_response": response
        }


# ==================== Convenience Functions ====================

def quick_run(
    user_message: str,
    skills_dir: Optional[str] = None,
    verbose: bool = False,
    **kwargs
) -> str:
    """
    Run Skill with one line of code.
    
    Args:
        user_message: User input message
        skills_dir: Skills directory, defaults to "./.skills"
        verbose: Whether to output detailed logs
        **kwargs: Other parameters passed to SkillRunner
        
    Returns:
        Final response content from LLM
        
    Example:
        ```python
        from skilllite import quick_run
        
        # Simplest usage (requires .env configuration)
        result = quick_run("Calculate 15 times 27 for me")
        
        # With detailed output
        result = quick_run("Calculate 15 times 27 for me", verbose=True)
        
        # Specify skills directory
        result = quick_run("Calculate 15 times 27 for me", skills_dir="./my_skills")
        ```
    """
    if skills_dir:
        kwargs["skills_dir"] = skills_dir
    kwargs["verbose"] = verbose
    
    runner = SkillRunner(**kwargs)
    return runner.run(user_message)
