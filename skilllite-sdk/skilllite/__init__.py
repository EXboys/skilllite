"""
SkillLite - A lightweight Skills execution engine with LLM integration.

This package provides a Python interface to the SkillLite execution engine,
using OpenAI-compatible API format as the unified interface.

Supported LLM providers:
- OpenAI (GPT-4, GPT-3.5, etc.)
- Azure OpenAI
- Anthropic Claude (via OpenAI-compatible endpoint or native)
- Google Gemini (via OpenAI-compatible endpoint)
- Local models (Ollama, vLLM, LMStudio, etc.)
- DeepSeek, Qwen, Moonshot, Zhipu, and other providers

Quick Start (Enhanced):
    ```python
    from skilllite import SkillRunner
    
    # One-line execution with intelligent features
    runner = SkillRunner()
    result = runner.run("Create a data analysis skill for me")
    print(result)
    ```

Advanced Usage:
    ```python
    from openai import OpenAI
    from skilllite import SkillManager
    
    # Works with any OpenAI-compatible client
    client = OpenAI()  # or OpenAI(base_url="...", api_key="...")
    manager = SkillManager(skills_dir="./my_skills")
    
    # Enhanced agentic loop with task list based execution
    loop = manager.create_enhanced_agentic_loop(
        client=client,
        model="gpt-4"
    )
    response = loop.run("Analyze this data and generate a report")
    ```

Legacy Usage:
    ```python
    from openai import OpenAI
    from skilllite import SkillManager
    
    client = OpenAI()
    manager = SkillManager(skills_dir="./my_skills")
    
    # Get tools (OpenAI-compatible format)
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

# Import from core module (protected core functionality)
from .core import (
    SkillManager,
    SkillInfo,
    AgenticLoop,
    AgenticLoopClaudeNative,
    ApiFormat,
    ToolDefinition, 
    ToolUseRequest,
    ToolResult,
    SkillExecutor,
    ExecutionResult,
    SkillMetadata,
    NetworkPolicy,
    parse_skill_metadata,
)

# Import from non-core modules (utilities, quick start, etc.)
from .quick import SkillRunner, quick_run, load_env, get_runner
from .core.metadata import get_skill_summary
from .sandbox.skillbox import (
    install as install_binary,
    uninstall as uninstall_binary,
    is_installed as is_binary_installed,
    find_binary,
    ensure_installed,
    get_installed_version,
    BINARY_VERSION,
)
from .analyzer import (
    ScriptAnalyzer,
    ScriptInfo,
    SkillScanResult,
    ExecutionRecommendation,
    scan_skill,
    analyze_skill,
)
from .builtin_tools import (
    get_builtin_file_tools,
    execute_builtin_file_tool,
    create_builtin_tool_executor,
)

# Try to import MCP module (optional dependency)
try:
    from .mcp import MCPServer, SandboxExecutor
    MCP_AVAILABLE = True
except ImportError:
    MCP_AVAILABLE = False

__version__ = "0.1.1"
__all__ = [
    # Core
    "SkillManager",
    "SkillInfo",
    "AgenticLoop",
    "AgenticLoopClaudeNative",
    "ApiFormat",
    "ToolDefinition", 
    "ToolUseRequest",
    "ToolResult",
    "SkillExecutor",
    "ExecutionResult",
    # Script Analysis
    "ScriptAnalyzer",
    "ScriptInfo",
    "SkillScanResult",
    "ExecutionRecommendation",
    "scan_skill",
    "analyze_skill",
    # Schema Inference
    "get_skill_summary",
    # Quick Start
    "SkillRunner",
    "quick_run",
    "load_env",
    "get_runner",
    # Binary Management
    "install_binary",
    "uninstall_binary",
    "is_binary_installed",
    "find_binary",
    "ensure_installed",
    "get_installed_version",
    "BINARY_VERSION",
    # Built-in Tools
    "get_builtin_file_tools",
    "execute_builtin_file_tool",
    "create_builtin_tool_executor",
    # MCP Integration (optional)
    "MCPServer",
    "SandboxExecutor",
    "MCP_AVAILABLE",
]
