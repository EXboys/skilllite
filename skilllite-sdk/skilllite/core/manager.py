"""
SkillManager - Main interface for managing and executing skills.

This module uses OpenAI-compatible API format as the unified interface,
which is supported by most LLM providers including:
- OpenAI (GPT-4, GPT-3.5, etc.)
- Anthropic Claude (via OpenAI-compatible endpoint or native)
- Azure OpenAI
- Google Gemini (via OpenAI-compatible endpoint)
- Local models (Ollama, vLLM, LMStudio, etc.)
- DeepSeek, Qwen, Moonshot, Zhipu, and other providers

Usage with different providers:

    # OpenAI
    from openai import OpenAI
    client = OpenAI()
    
    # Azure OpenAI
    from openai import AzureOpenAI
    client = AzureOpenAI(azure_endpoint="...", api_key="...")
    
    # Ollama (local)
    from openai import OpenAI
    client = OpenAI(base_url="http://localhost:11434/v1", api_key="ollama")
    
    # DeepSeek
    from openai import OpenAI
    client = OpenAI(base_url="https://api.deepseek.com/v1", api_key="...")
    
    # Qwen (Alibaba Cloud)
    from openai import OpenAI
    client = OpenAI(base_url="https://dashscope.aliyuncs.com/compatible-mode/v1", api_key="...")
    
    # Moonshot (Kimi)
    from openai import OpenAI
    client = OpenAI(base_url="https://api.moonshot.cn/v1", api_key="...")
"""

from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Union

from .executor import ExecutionResult, SkillExecutor
from .loops import AgenticLoop, AgenticLoopClaudeNative, ApiFormat
from .registry import SkillRegistry
from .tool_builder import ToolBuilder
from .prompt_builder import PromptBuilder
from .handler import ToolCallHandler
from .skill_info import SkillInfo
from .tools import ToolDefinition, ToolResult, ToolUseRequest


class SkillManager:
    """
    Main interface for managing and executing skills.
    
    This is a facade class that composes:
    - SkillRegistry: Skill registration and discovery
    - ToolBuilder: Tool definition generation and schema inference
    - PromptBuilder: System prompt context generation
    - ToolCallHandler: LLM response handling and tool execution
    
    Example:
        ```python
        from openai import OpenAI
        from skilllite import SkillManager
        
        # Works with any OpenAI-compatible client
        client = OpenAI()  # or OpenAI(base_url="...", api_key="...")
        manager = SkillManager(skills_dir="./my_skills")
        
        # Get tools in OpenAI format (universal)
        tools = manager.get_tools()
        
        # Call any OpenAI-compatible API
        response = client.chat.completions.create(
            model="gpt-4",
            tools=tools,
            messages=[{"role": "user", "content": "..."}]
        )
        
        # Handle tool calls
        if response.choices[0].message.tool_calls:
            results = manager.handle_tool_calls(response)
        ```
    """
    
    def __init__(
        self,
        skills_dir: Optional[Union[str, Path]] = None,
        binary_path: Optional[str] = None,
        cache_dir: Optional[str] = None,
        allow_network: bool = False,
        enable_sandbox: bool = True,
        execution_timeout: Optional[int] = None,
        max_memory_mb: Optional[int] = None,
        sandbox_level: Optional[str] = None
    ):
        """
        Initialize the SkillManager.
        
        Args:
            skills_dir: Directory containing skills. If None, no skills are loaded initially.
            binary_path: Path to the skillbox binary. If None, searches PATH.
            cache_dir: Directory for caching virtual environments.
            allow_network: Whether to allow network access by default.
            enable_sandbox: Whether to enable sandbox protection (default: True).
            execution_timeout: Skill execution timeout in seconds (default: 120).
            max_memory_mb: Maximum memory limit in MB (default: 512).
            sandbox_level: Sandbox security level (1/2/3, default from env or 3).
        """
        # Initialize executor
        self._executor = SkillExecutor(
            binary_path=binary_path,
            cache_dir=cache_dir,
            allow_network=allow_network,
            enable_sandbox=enable_sandbox,
            execution_timeout=execution_timeout,
            max_memory_mb=max_memory_mb,
            sandbox_level=sandbox_level
        )
        
        # Initialize registry
        self._registry = SkillRegistry()
        
        # Initialize builders and handler
        self._tool_builder = ToolBuilder(self._registry)
        self._prompt_builder = PromptBuilder(self._registry)
        self._handler = ToolCallHandler(self._registry, self._executor)
        
        # Scan skills directory if provided
        if skills_dir:
            self.scan_directory(Path(skills_dir))
    
    # ==================== Skill Registration (delegated to registry) ====================
    
    def scan_directory(self, directory: Path) -> int:
        """Scan a directory for skills."""
        return self._registry.scan_directory(directory)
    
    def register_skill(self, skill_dir: Path) -> SkillInfo:
        """Register a single skill from a directory."""
        return self._registry.register_skill(skill_dir)
    
    def get_skill(self, name: str) -> Optional[SkillInfo]:
        """Get a skill by name."""
        return self._registry.get_skill(name)
    
    def list_skills(self) -> List[SkillInfo]:
        """Get all registered skills."""
        return self._registry.list_skills()
    
    def skill_names(self) -> List[str]:
        """Get names of all registered skills."""
        return self._registry.skill_names()
    
    def has_skill(self, name: str) -> bool:
        """Check if a skill exists."""
        return self._registry.has_skill(name)
    
    def is_executable(self, name: str) -> bool:
        """Check if a skill or tool is executable."""
        return self._registry.is_executable(name)
    
    def list_executable_skills(self) -> List[SkillInfo]:
        """Get all executable skills."""
        return self._registry.list_executable_skills()
    
    def list_prompt_only_skills(self) -> List[SkillInfo]:
        """Get all prompt-only skills."""
        return self._registry.list_prompt_only_skills()
    
    def list_multi_script_tools(self) -> List[str]:
        """Get all multi-script tool names."""
        return self._registry.list_multi_script_tools()
    
    # ==================== Tool Definition (delegated to tool_builder) ====================
    
    def get_tool_definitions(self, include_prompt_only: bool = False) -> List[ToolDefinition]:
        """Get tool definitions for registered skills."""
        return self._tool_builder.get_tool_definitions(include_prompt_only)
    
    def get_tools(self) -> List[Dict[str, Any]]:
        """Get tool definitions in OpenAI-compatible format."""
        return self._tool_builder.get_tools_openai()
    
    def get_tools_for_claude_native(self) -> List[Dict[str, Any]]:
        """Get tool definitions in Claude's native API format."""
        return self._tool_builder.get_tools_claude_native()
    
    def infer_all_schemas(self, force: bool = False) -> Dict[str, Dict[str, Any]]:
        """Infer schemas for all skills that don't have one defined."""
        return self._tool_builder.infer_all_schemas(force)
    
    # ==================== System Prompt (delegated to prompt_builder) ====================
    
    def get_system_prompt_context(
        self,
        include_full_instructions: bool = True,
        include_references: bool = False,
        include_assets: bool = False,
        skills: Optional[List[str]] = None,
        mode: str = "full",
        max_tokens_per_skill: Optional[int] = None
    ) -> str:
        """Generate system prompt context containing skill information."""
        return self._prompt_builder.get_system_prompt_context(
            include_full_instructions=include_full_instructions,
            include_references=include_references,
            include_assets=include_assets,
            skills=skills,
            mode=mode,
            max_tokens_per_skill=max_tokens_per_skill
        )
    
    def get_skill_details(self, skill_name: str) -> Optional[str]:
        """Get full details for a specific skill."""
        return self._prompt_builder.get_skill_details(skill_name)
    
    def get_skills_summary(self) -> str:
        """Get a compact summary of all available skills."""
        return self._prompt_builder.get_skills_summary()
    
    def estimate_context_tokens(
        self,
        mode: str = "full",
        include_references: bool = False,
        include_assets: bool = False
    ) -> int:
        """Estimate the number of tokens the system prompt context will use."""
        return self._prompt_builder.estimate_context_tokens(mode, include_references, include_assets)
    
    def get_skill_context(self, skill_name: str) -> Optional[Dict[str, Any]]:
        """Get complete context for a specific skill."""
        return self._prompt_builder.get_skill_context(skill_name)
    
    def get_all_skill_contexts(self) -> Dict[str, Dict[str, Any]]:
        """Get complete context for all skills."""
        return self._prompt_builder.get_all_skill_contexts()
    
    # ==================== Skills Status (delegated to prompt_builder) ====================
    
    def get_skills_status(self) -> Dict[str, Any]:
        """Get structured status information about all loaded skills."""
        return self._prompt_builder.get_skills_status()
    
    def print_skills_status(self, verbose: bool = False) -> None:
        """Print a formatted status of all loaded skills."""
        self._prompt_builder.print_skills_status(verbose)
    
    def get_prompt_only_status(self) -> List[Dict[str, str]]:
        """Get status info for prompt-only skills."""
        return self._prompt_builder.get_prompt_only_status()
    
    def print_prompt_only_status(self) -> None:
        """Print status of prompt-only skills."""
        self._prompt_builder.print_prompt_only_status()
    
    # ==================== Skill Execution (delegated to handler) ====================
    
    def execute(
        self,
        skill_name: str,
        input_data: Dict[str, Any],
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> ExecutionResult:
        """Execute a skill or multi-script tool with the given input."""
        return self._handler.execute(skill_name, input_data, allow_network, timeout)
    
    def execute_tool_call(
        self,
        request: ToolUseRequest,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> ToolResult:
        """Execute a tool call request from an LLM."""
        return self._handler.execute_tool_call(request, allow_network, timeout)
    
    # ==================== LLM Response Handling (delegated to handler) ====================
    
    def parse_tool_calls(self, response: Any) -> List[ToolUseRequest]:
        """Parse tool calls from an OpenAI-compatible LLM response."""
        return self._handler.parse_tool_calls(response)
    
    def parse_tool_calls_claude_native(self, response: Any) -> List[ToolUseRequest]:
        """Parse tool calls from Claude's native API response."""
        return self._handler.parse_tool_calls_claude_native(response)
    
    def handle_tool_calls(
        self,
        response: Any,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> List[ToolResult]:
        """Parse and execute all tool calls from an OpenAI-compatible LLM response."""
        return self._handler.handle_tool_calls(response, allow_network, timeout)
    
    def handle_tool_calls_claude_native(
        self,
        response: Any,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> List[ToolResult]:
        """Parse and execute all tool calls from Claude's native API response."""
        return self._handler.handle_tool_calls_claude_native(response, allow_network, timeout)
    
    def format_tool_results_claude_native(self, results: List[ToolResult]) -> List[Dict[str, Any]]:
        """Format tool results for Claude's native API."""
        return self._handler.format_tool_results_claude_native(results)
    
    # ==================== Enhanced Workflow (delegated to handler) ====================
    
    def create_enhanced_skill_workflow(
        self,
        skill_name: str,
        user_request: str,
        llm_client: Any,
        llm_model: str = "gpt-4"
    ) -> Dict[str, Any]:
        """Create an enhanced workflow for a skill that involves planning and execution."""
        return self._handler.create_enhanced_skill_workflow(
            skill_name, user_request, llm_client, llm_model
        )
    
    # ==================== Agentic Loop Creation ====================
    
    def create_agentic_loop(
        self,
        client: Any,
        model: str,
        system_prompt: Optional[str] = None,
        max_iterations: int = 10,
        api_format: str = "openai",
        custom_tool_handler: Optional[Callable] = None,
        enable_task_planning: bool = True,
        verbose: bool = True,
        **kwargs
    ) -> AgenticLoop:
        """
        Create a unified agentic loop for LLM-tool interactions.
        
        Supports both OpenAI-compatible APIs and Claude's native API.
        
        Args:
            client: LLM client (OpenAI or Anthropic)
            model: Model name to use
            system_prompt: Optional system prompt
            max_iterations: Maximum number of iterations
            api_format: API format - "openai" or "claude_native"
            custom_tool_handler: Optional custom tool handler
            enable_task_planning: Whether to generate task list before execution
            verbose: Whether to print detailed logs
            **kwargs: Additional arguments passed to the LLM
            
        Returns:
            AgenticLoop instance
            
        Example:
            # OpenAI-compatible (default)
            loop = manager.create_agentic_loop(client, "gpt-4")
            
            # Claude native API
            loop = manager.create_agentic_loop(client, "claude-3-opus",
                                               api_format="claude_native")
        """
        format_enum = ApiFormat.CLAUDE_NATIVE if api_format == "claude_native" else ApiFormat.OPENAI
        return AgenticLoop(
            manager=self,
            client=client,
            model=model,
            system_prompt=system_prompt,
            max_iterations=max_iterations,
            api_format=format_enum,
            custom_tool_handler=custom_tool_handler,
            enable_task_planning=enable_task_planning,
            verbose=verbose,
            **kwargs
        )
    
    def create_agentic_loop_claude_native(
        self,
        client: Any,
        model: str,
        system_prompt: Optional[str] = None,
        max_iterations: int = 10,
        **kwargs
    ) -> AgenticLoop:
        """
        Create an agentic loop for Claude's native API.
        
        This is a convenience method that calls create_agentic_loop with
        api_format="claude_native".
        
        Args:
            client: Anthropic client
            model: Model name to use
            system_prompt: Optional system prompt
            max_iterations: Maximum number of iterations
            **kwargs: Additional arguments passed to the LLM
            
        Returns:
            AgenticLoop instance configured for Claude native API
        """
        return self.create_agentic_loop(
            client=client,
            model=model,
            system_prompt=system_prompt,
            max_iterations=max_iterations,
            api_format="claude_native",
            **kwargs
        )
    
    def create_enhanced_agentic_loop(
        self,
        client: Any,
        model: str,
        system_prompt: Optional[str] = None,
        max_iterations: int = 10,
        custom_tools: Optional[List[Dict[str, Any]]] = None,
        custom_tool_executor: Optional[Callable] = None,
        enable_task_planning: bool = True,
        verbose: bool = True,
        **kwargs
    ) -> AgenticLoop:
        """
        Create an enhanced agentic loop with custom tools support.
        
        This method creates an AgenticLoop that can handle both skill tools
        and custom tools (like file operations).
        
        Args:
            client: LLM client (OpenAI-compatible)
            model: Model name to use
            system_prompt: Optional system prompt
            max_iterations: Maximum number of iterations
            custom_tools: Additional custom tool definitions
            custom_tool_executor: Executor function for custom tools
            enable_task_planning: Whether to generate task list before execution
            verbose: Whether to print detailed logs
            **kwargs: Additional arguments passed to the LLM
            
        Returns:
            AgenticLoop instance with enhanced capabilities
        """
        # Create custom tool handler that combines skill tools and custom tools
        def combined_tool_handler(response, manager, allow_network, timeout):
            from .tools import ToolUseRequest, ToolResult
            
            requests = ToolUseRequest.parse_from_openai_response(response)
            results = []
            
            # Get skill tool names
            skill_tool_names = set(self.skill_names())
            skill_tool_names.update(self._registry.list_multi_script_tools())
            
            for request in requests:
                if request.name in skill_tool_names:
                    # Execute as skill tool
                    result = self._handler.execute_tool_call(
                        request, allow_network=allow_network, timeout=timeout
                    )
                    results.append(result)
                elif custom_tool_executor:
                    # Execute as custom tool
                    try:
                        tool_input = {"tool_name": request.name, **request.input}
                        output = custom_tool_executor(tool_input)
                        results.append(ToolResult.success(request.id, output))
                    except Exception as e:
                        results.append(ToolResult.error(request.id, str(e)))
                else:
                    results.append(ToolResult.error(
                        request.id, f"No executor found for tool: {request.name}"
                    ))
            
            return results
        
        return AgenticLoop(
            manager=self,
            client=client,
            model=model,
            system_prompt=system_prompt,
            max_iterations=max_iterations,
            api_format=ApiFormat.OPENAI,
            custom_tool_handler=combined_tool_handler if custom_tool_executor else None,
            enable_task_planning=enable_task_planning,
            verbose=verbose,
            **kwargs
        )
    
    # ==================== Compatibility Properties ====================
    
    @property
    def _skills(self) -> Dict[str, SkillInfo]:
        """Direct access to skills dict (for backward compatibility)."""
        return self._registry.skills
    
    @property
    def _multi_script_tools(self) -> Dict[str, Dict[str, str]]:
        """Direct access to multi-script tools dict (for backward compatibility)."""
        return self._registry.multi_script_tools
    
    @property
    def _inferred_schemas(self) -> Dict[str, Dict[str, Any]]:
        """Direct access to inferred schemas (for backward compatibility)."""
        return self._tool_builder.inferred_schemas
