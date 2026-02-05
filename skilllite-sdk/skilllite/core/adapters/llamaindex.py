"""
LlamaIndex adapter for SkillLite.

Provides SkillLiteToolSpec for integrating SkillLite skills into LlamaIndex agents.

Usage:
    from skilllite import SkillManager
    from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

    manager = SkillManager(skills_dir="./skills")
    tool_spec = SkillLiteToolSpec.from_manager(manager)
    tools = tool_spec.to_tool_list()

    # Use with LlamaIndex agent
    from llama_index.core.agent import ReActAgent
    agent = ReActAgent.from_tools(tools, llm=llm, verbose=True)

Security Confirmation:
    For sandbox level 3, the adapter supports security confirmation callbacks:

    def my_confirmation_callback(security_report: str, scan_id: str) -> bool:
        print(security_report)
        return input("Continue? [y/N]: ").lower() == 'y'

    tool_spec = SkillLiteToolSpec.from_manager(
        manager,
        sandbox_level=3,
        confirmation_callback=my_confirmation_callback
    )

Requirements:
    pip install skilllite[llamaindex]
"""

from typing import Any, Callable, Dict, List, Optional, TYPE_CHECKING

try:
    from llama_index.core.tools import FunctionTool, ToolMetadata
    from llama_index.core.tools.types import BaseTool as LlamaBaseTool
except ImportError as e:
    raise ImportError(
        "LlamaIndex adapter requires llama-index. "
        "Install with: pip install skilllite[llamaindex]"
    ) from e

if TYPE_CHECKING:
    from ..manager import SkillManager


# Type alias for confirmation callback
# Signature: (security_report: str, scan_id: str) -> bool
ConfirmationCallback = Callable[[str, str], bool]


# Import SecurityScanResult from langchain adapter to share the implementation
# This avoids code duplication
try:
    from .langchain import SecurityScanResult
except ImportError:
    # Fallback: define a minimal SecurityScanResult if langchain adapter not available
    from dataclasses import dataclass, field

    @dataclass
    class SecurityScanResult:
        """Result of a security scan for LlamaIndex adapter."""

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
            return self.high_severity_count > 0

        def format_report(self) -> str:
            if not self.issues:
                return "✅ Security scan passed. No issues found."

            lines = [
                f"📋 Security Scan Report (ID: {self.scan_id[:8]})",
                f"   Found {len(self.issues)} item(s) for review:",
                "",
            ]

            severity_icons = {"Critical": "🔴", "High": "🟠", "Medium": "🟡", "Low": "🟢"}

            for idx, issue in enumerate(self.issues, 1):
                severity = issue.get("severity", "Medium")
                icon = severity_icons.get(severity, "⚪")
                lines.append(f"  {icon} #{idx} [{severity}] {issue.get('issue_type', 'Unknown')}")
                lines.append(f"     └─ {issue.get('description', '')}")

            if self.high_severity_count > 0:
                lines.append("\n⚠️  High severity issues found. Confirmation required.")

            return "\n".join(lines)


class SkillLiteToolSpec:
    """
    LlamaIndex ToolSpec for SkillLite.

    Provides a way to create LlamaIndex tools from SkillLite skills.

    Usage:
        manager = SkillManager(skills_dir="./skills")
        tool_spec = SkillLiteToolSpec.from_manager(manager)
        tools = tool_spec.to_tool_list()

        # Use with ReActAgent
        agent = ReActAgent.from_tools(tools, llm=llm)
        response = agent.chat("Your query")

    Security Confirmation (sandbox_level=3):
        def confirm(report: str, scan_id: str) -> bool:
            print(report)
            return input("Continue? [y/N]: ").lower() == 'y'

        tool_spec = SkillLiteToolSpec.from_manager(
            manager, sandbox_level=3, confirmation_callback=confirm
        )
    """

    def __init__(
        self,
        manager: "SkillManager",
        skill_names: Optional[List[str]] = None,
        allow_network: bool = False,
        timeout: Optional[int] = None,
        sandbox_level: int = 3,
        confirmation_callback: Optional[ConfirmationCallback] = None
    ):
        """
        Initialize SkillLiteToolSpec.

        Args:
            manager: SkillManager instance with registered skills
            skill_names: Optional list of skill names to include (default: all)
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            sandbox_level: Sandbox security level (1=no sandbox, 2=sandbox only, 3=sandbox+scan)
            confirmation_callback: Callback for security confirmation (report, scan_id) -> bool
        """
        self.manager = manager
        self.skill_names = skill_names
        self.allow_network = allow_network
        self.timeout = timeout
        self.sandbox_level = sandbox_level
        self.confirmation_callback = confirmation_callback

    @classmethod
    def from_manager(
        cls,
        manager: "SkillManager",
        skill_names: Optional[List[str]] = None,
        allow_network: bool = False,
        timeout: Optional[int] = None,
        sandbox_level: int = 3,
        confirmation_callback: Optional[ConfirmationCallback] = None
    ) -> "SkillLiteToolSpec":
        """
        Create a SkillLiteToolSpec from a SkillManager.

        Args:
            manager: SkillManager instance
            skill_names: Optional list of skill names to include
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            sandbox_level: Sandbox security level (1/2/3, default: 3)
            confirmation_callback: Callback for security confirmation

        Returns:
            SkillLiteToolSpec instance
        """
        return cls(
            manager=manager,
            skill_names=skill_names,
            allow_network=allow_network,
            timeout=timeout,
            sandbox_level=sandbox_level,
            confirmation_callback=confirmation_callback
        )

    def _create_skill_function(self, skill_name: str):
        """
        Create a callable function for a skill using UnifiedExecutionService.

        This method uses the unified execution layer which:
        1. Reads sandbox level at runtime
        2. Handles security scanning and confirmation
        3. Properly downgrades sandbox level after confirmation
        """
        def skill_fn(**kwargs) -> str:
            try:
                # Get skill info
                skill_info = self.manager.get_skill(skill_name)
                if not skill_info:
                    return f"Error: Skill '{skill_name}' not found"

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

        return skill_fn
    
    def to_tool_list(self) -> List[LlamaBaseTool]:
        """
        Convert all skills to a list of LlamaIndex tools.
        
        Returns:
            List of FunctionTool instances
        """
        tools = []
        
        # Get executable skills
        skills = self.manager.list_executable_skills()
        
        for skill in skills:
            # Filter by name if specified
            if self.skill_names and skill.name not in self.skill_names:
                continue
            
            # Create function for this skill
            fn = self._create_skill_function(skill.name)
            
            # Create tool metadata
            metadata = ToolMetadata(
                name=skill.name,
                description=skill.description or f"Execute the {skill.name} skill"
            )
            
            # Create FunctionTool
            tool = FunctionTool.from_defaults(
                fn=fn,
                name=skill.name,
                description=skill.description or f"Execute the {skill.name} skill"
            )
            tools.append(tool)
        
        return tools


__all__ = ["SkillLiteToolSpec", "SecurityScanResult", "ConfirmationCallback"]

