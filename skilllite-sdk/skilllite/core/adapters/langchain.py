"""
LangChain adapter for SkillLite.

Provides SkillLiteTool and SkillLiteToolkit for integrating SkillLite
skills into LangChain agents.

Usage:
    from skilllite import SkillManager
    from skilllite.core.adapters.langchain import SkillLiteToolkit

    manager = SkillManager(skills_dir="./skills")
    tools = SkillLiteToolkit.from_manager(manager)

    # Use with LangChain agent
    from langchain.agents import create_openai_tools_agent, AgentExecutor
    agent = create_openai_tools_agent(llm, tools, prompt)
    executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)

Security Confirmation:
    For sandbox level 3, the adapter supports security confirmation callbacks:

    def my_confirmation_callback(security_report: str, scan_id: str) -> bool:
        print(security_report)
        return input("Continue? [y/N]: ").lower() == 'y'

    tools = SkillLiteToolkit.from_manager(
        manager,
        sandbox_level=3,
        confirmation_callback=my_confirmation_callback
    )

Requirements:
    pip install skilllite[langchain]
"""

from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional, Type, TYPE_CHECKING
import asyncio
import time

try:
    from langchain_core.tools import BaseTool
    from langchain_core.callbacks import CallbackManagerForToolRun, AsyncCallbackManagerForToolRun
    from pydantic import BaseModel, Field, ConfigDict
except ImportError as e:
    raise ImportError(
        "LangChain adapter requires langchain-core. "
        "Install with: pip install skilllite[langchain]"
    ) from e

if TYPE_CHECKING:
    from ..manager import SkillManager


# Type alias for confirmation callback
# Signature: (security_report: str, scan_id: str) -> bool
ConfirmationCallback = Callable[[str, str], bool]
AsyncConfirmationCallback = Callable[[str, str], "asyncio.Future[bool]"]


@dataclass
class SecurityScanResult:
    """Result of a security scan for LangChain adapter."""

    is_safe: bool
    issues: List[Dict[str, Any]] = field(default_factory=list)
    scan_id: str = ""
    code_hash: str = ""
    high_severity_count: int = 0
    medium_severity_count: int = 0
    low_severity_count: int = 0
    timestamp: float = field(default_factory=time.time)

    @property
    def requires_confirmation(self) -> bool:
        """Check if user confirmation is required."""
        return self.high_severity_count > 0

    def to_dict(self) -> Dict[str, Any]:
        return {
            "is_safe": self.is_safe,
            "issues": self.issues,
            "scan_id": self.scan_id,
            "code_hash": self.code_hash,
            "high_severity_count": self.high_severity_count,
            "medium_severity_count": self.medium_severity_count,
            "low_severity_count": self.low_severity_count,
            "requires_confirmation": self.requires_confirmation,
        }

    def format_report(self) -> str:
        """Format a human-readable security report."""
        if not self.issues:
            return "✅ Security scan passed. No issues found."

        lines = [
            f"📋 Security Scan Report (ID: {self.scan_id[:8]})",
            f"   Found {len(self.issues)} item(s) for review:",
            "",
        ]

        severity_icons = {
            "Critical": "🔴",
            "High": "🟠",
            "Medium": "🟡",
            "Low": "🟢",
        }

        for idx, issue in enumerate(self.issues, 1):
            severity = issue.get("severity", "Medium")
            icon = severity_icons.get(severity, "⚪")
            lines.append(f"  {icon} #{idx} [{severity}] {issue.get('issue_type', 'Unknown')}")
            lines.append(f"     ├─ Rule: {issue.get('rule_id', 'N/A')}")
            lines.append(f"     ├─ Line {issue.get('line_number', '?')}: {issue.get('description', '')}")
            snippet = issue.get('code_snippet', '')
            lines.append(f"     └─ Code: {snippet[:60]}{'...' if len(snippet) > 60 else ''}")
            lines.append("")

        if self.high_severity_count > 0:
            lines.append("⚠️  High severity issues found. Confirmation required to execute.")
        else:
            lines.append("ℹ️  Only low/medium severity issues found. Safe to execute.")

        return "\n".join(lines)


class SkillLiteTool(BaseTool):
    """
    LangChain BaseTool adapter for a single SkillLite skill.

    This wraps a SkillLite skill as a LangChain tool, enabling it to be
    used with LangChain agents.

    Attributes:
        name: Tool name (same as skill name)
        description: Tool description
        manager: SkillManager instance
        skill_name: Name of the skill to execute
        allow_network: Whether to allow network access
        timeout: Execution timeout in seconds
        sandbox_level: Sandbox security level (1/2/3, default: 3)
        confirmation_callback: Callback for security confirmation (sync)
        async_confirmation_callback: Callback for security confirmation (async)
    """

    name: str = Field(description="Tool name")
    description: str = Field(description="Tool description")
    args_schema: Optional[Type[BaseModel]] = Field(default=None, description="Pydantic schema for arguments")

    # SkillLite specific fields
    manager: Any = Field(exclude=True)  # SkillManager instance
    skill_name: str = Field(description="SkillLite skill name")
    allow_network: bool = Field(default=False, description="Allow network access")
    timeout: Optional[int] = Field(default=None, description="Execution timeout in seconds")

    # Security confirmation fields
    sandbox_level: int = Field(default=3, description="Sandbox security level (1/2/3)")
    confirmation_callback: Optional[Any] = Field(
        default=None,
        exclude=True,
        description="Sync callback for security confirmation: (report: str, scan_id: str) -> bool"
    )
    async_confirmation_callback: Optional[Any] = Field(
        default=None,
        exclude=True,
        description="Async callback for security confirmation: (report: str, scan_id: str) -> Future[bool]"
    )

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def _run(
        self,
        run_manager: Optional[CallbackManagerForToolRun] = None,
        **kwargs: Any
    ) -> str:
        """
        Execute the skill synchronously using UnifiedExecutionService.

        This method uses the unified execution layer which:
        1. Reads sandbox level at runtime
        2. Handles security scanning and confirmation
        3. Properly downgrades sandbox level after confirmation
        """
        try:
            # Get skill info
            skill_info = self.manager._registry.get_skill(self.skill_name)
            if not skill_info:
                return f"Error: Skill '{self.skill_name}' not found"

            # Use UnifiedExecutionService
            from ...sandbox.execution_service import UnifiedExecutionService

            service = UnifiedExecutionService.get_instance()
            result = service.execute_skill(
                skill_info=skill_info,
                input_data=kwargs,
                confirmation_callback=self.confirmation_callback,
                allow_network=self.allow_network,
                timeout=self.timeout,
            )

            if result.success:
                return result.output or "Execution completed successfully"
            else:
                return f"Error: {result.error}"
        except Exception as e:
            return f"Execution failed: {str(e)}"

    async def _arun(
        self,
        run_manager: Optional[AsyncCallbackManagerForToolRun] = None,
        **kwargs: Any
    ) -> str:
        """
        Execute the skill asynchronously using UnifiedExecutionService.

        This method uses the unified execution layer which:
        1. Reads sandbox level at runtime
        2. Handles security scanning and confirmation
        3. Properly downgrades sandbox level after confirmation
        """
        try:
            # Get skill info
            skill_info = self.manager._registry.get_skill(self.skill_name)
            if not skill_info:
                return f"Error: Skill '{self.skill_name}' not found"

            # Use UnifiedExecutionService in thread pool
            from ...sandbox.execution_service import UnifiedExecutionService

            def execute_sync():
                service = UnifiedExecutionService.get_instance()
                # Use async confirmation callback if available, otherwise sync
                callback = self.confirmation_callback
                return service.execute_skill(
                    skill_info=skill_info,
                    input_data=kwargs,
                    confirmation_callback=callback,
                    allow_network=self.allow_network,
                    timeout=self.timeout,
                )

            result = await asyncio.to_thread(execute_sync)

            if result.success:
                return result.output or "Execution completed successfully"
            else:
                return f"Error: {result.error}"
        except Exception as e:
            return f"Execution failed: {str(e)}"


class SkillLiteToolkit:
    """
    LangChain Toolkit for SkillLite.

    Provides a convenient way to create LangChain tools from all skills
    registered in a SkillManager.

    Usage:
        manager = SkillManager(skills_dir="./skills")
        tools = SkillLiteToolkit.from_manager(manager)

        # Or with options
        tools = SkillLiteToolkit.from_manager(
            manager,
            skill_names=["calculator", "web_search"],  # Only specific skills
            allow_network=True,
            timeout=60
        )

        # With security confirmation callback (for sandbox level 3)
        def confirm_execution(report: str, scan_id: str) -> bool:
            print(report)
            return input("Continue? [y/N]: ").lower() == 'y'

        tools = SkillLiteToolkit.from_manager(
            manager,
            sandbox_level=3,
            confirmation_callback=confirm_execution
        )
    """

    @staticmethod
    def from_manager(
        manager: "SkillManager",
        skill_names: Optional[List[str]] = None,
        allow_network: bool = False,
        timeout: Optional[int] = None,
        sandbox_level: int = 3,
        confirmation_callback: Optional[ConfirmationCallback] = None,
        async_confirmation_callback: Optional[AsyncConfirmationCallback] = None,
    ) -> List[SkillLiteTool]:
        """
        Create LangChain tools from a SkillManager.

        Args:
            manager: SkillManager instance with registered skills
            skill_names: Optional list of skill names to include (default: all)
            allow_network: Whether to allow network access for all tools
            timeout: Execution timeout in seconds for all tools
            sandbox_level: Sandbox security level (1/2/3, default: 3)
                - Level 1: No sandbox - direct execution
                - Level 2: Sandbox isolation only
                - Level 3: Sandbox isolation + security scanning (requires confirmation for high-severity issues)
            confirmation_callback: Sync callback for security confirmation.
                Signature: (security_report: str, scan_id: str) -> bool
                Return True to proceed, False to cancel.
            async_confirmation_callback: Async callback for security confirmation.
                Signature: (security_report: str, scan_id: str) -> Future[bool]
                Return True to proceed, False to cancel.

        Returns:
            List of SkillLiteTool instances

        Example with confirmation callback:
            def my_callback(report: str, scan_id: str) -> bool:
                print(f"Security Report:\\n{report}")
                response = input("Proceed with execution? [y/N]: ")
                return response.lower() == 'y'

            tools = SkillLiteToolkit.from_manager(
                manager,
                sandbox_level=3,
                confirmation_callback=my_callback
            )
        """
        tools = []

        # Get executable skills
        skills = manager.list_executable_skills()

        for skill in skills:
            # Filter by name if specified
            if skill_names and skill.name not in skill_names:
                continue

            # Create tool with security confirmation support
            tool = SkillLiteTool(
                name=skill.name,
                description=skill.description or f"Execute the {skill.name} skill",
                manager=manager,
                skill_name=skill.name,
                allow_network=allow_network,
                timeout=timeout,
                sandbox_level=sandbox_level,
                confirmation_callback=confirmation_callback,
                async_confirmation_callback=async_confirmation_callback,
            )
            tools.append(tool)

        return tools


__all__ = [
    "SkillLiteTool",
    "SkillLiteToolkit",
    "SecurityScanResult",
    "ConfirmationCallback",
    "AsyncConfirmationCallback",
]

