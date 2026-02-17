"""
SkillLite - A lightweight Skills execution engine with LLM integration.

Python SDK delegates agent loop, tools, and execution to skillbox (Rust).
"""

# Import from core module
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

# Import from non-core modules
from .quick import SkillRunner, quick_run, load_env
from .core.metadata import get_skill_summary
from .logger import get_logger, setup_logging, LoggerMixin
from .sandbox.core import (
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
from .extensions import ExtensionsContext, register_extensions
from .extensions.memory import build_memory_context

__version__ = "0.1.4"
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
    # Binary Management
    "install_binary",
    "uninstall_binary",
    "is_binary_installed",
    "find_binary",
    "ensure_installed",
    "get_installed_version",
    "BINARY_VERSION",
    # Extensions
    "ExtensionsContext",
    "register_extensions",
    "build_memory_context",
    # Logging
    "get_logger",
    "setup_logging",
    "LoggerMixin",
]
