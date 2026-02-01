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
import hashlib
import time
import uuid

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

    # Class-level scan cache for confirmation flow
    _scan_cache: Dict[str, SecurityScanResult] = {}
    _SCAN_CACHE_TTL: int = 300  # 5 minutes

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

    def _generate_input_hash(self, skill_name: str, input_data: Dict[str, Any]) -> str:
        """Generate a hash of the input data for verification."""
        import json
        content = json.dumps({"skill": skill_name, "inputs": input_data}, sort_keys=True)
        return hashlib.sha256(content.encode()).hexdigest()

    def _cleanup_expired_scans(self) -> None:
        """Remove expired scan results from cache."""
        current_time = time.time()
        expired = [
            scan_id for scan_id, result in self._scan_cache.items()
            if current_time - result.timestamp > self._SCAN_CACHE_TTL
        ]
        for scan_id in expired:
            del self._scan_cache[scan_id]

    def _perform_security_scan(self, skill_name: str, input_data: Dict[str, Any]) -> SecurityScanResult:
        """Perform security scan on skill code before execution."""
        import subprocess
        import os

        # Get skill info to find the code file
        skill = self.manager.get_skill(skill_name)
        if not skill or not skill.path:
            return SecurityScanResult(
                is_safe=True,
                scan_id=str(uuid.uuid4()),
                code_hash=self._generate_input_hash(skill_name, input_data)
            )

        # Try to find skillbox binary
        skillbox_paths = [
            os.path.join(os.path.dirname(__file__), "..", "..", "sandbox", "skillbox", "skillbox"),
            os.path.expanduser("~/.skilllite/bin/skillbox"),
            "/usr/local/bin/skillbox"
        ]

        skillbox_path = None
        for path in skillbox_paths:
            if os.path.exists(path) and os.access(path, os.X_OK):
                skillbox_path = path
                break

        if not skillbox_path:
            # No skillbox available, assume safe
            return SecurityScanResult(
                is_safe=True,
                scan_id=str(uuid.uuid4()),
                code_hash=self._generate_input_hash(skill_name, input_data)
            )

        try:
            result = subprocess.run(
                [skillbox_path, "scan", skill.path],
                capture_output=True,
                text=True,
                timeout=30
            )
            return self._parse_scan_output(result.stdout, skill_name, input_data)
        except Exception:
            return SecurityScanResult(
                is_safe=True,
                scan_id=str(uuid.uuid4()),
                code_hash=self._generate_input_hash(skill_name, input_data)
            )

    def _parse_scan_output(self, output: str, skill_name: str, input_data: Dict[str, Any]) -> SecurityScanResult:
        """Parse skillbox scan output into SecurityScanResult."""
        import json

        scan_id = str(uuid.uuid4())
        code_hash = self._generate_input_hash(skill_name, input_data)

        try:
            data = json.loads(output)
            issues = data.get("issues", [])

            high_count = sum(1 for i in issues if i.get("severity") in ["Critical", "High"])
            medium_count = sum(1 for i in issues if i.get("severity") == "Medium")
            low_count = sum(1 for i in issues if i.get("severity") == "Low")

            return SecurityScanResult(
                is_safe=len(issues) == 0,
                issues=issues,
                scan_id=scan_id,
                code_hash=code_hash,
                high_severity_count=high_count,
                medium_severity_count=medium_count,
                low_severity_count=low_count
            )
        except json.JSONDecodeError:
            return SecurityScanResult(
                is_safe=True,
                scan_id=scan_id,
                code_hash=code_hash
            )

    def _create_skill_function(self, skill_name: str):
        """Create a callable function for a skill with security confirmation support."""
        def skill_fn(**kwargs) -> str:
            # Cleanup expired scans
            self._cleanup_expired_scans()

            # For sandbox_level < 3, skip security scan
            if self.sandbox_level < 3:
                return self._execute_skill(skill_name, kwargs)

            # Perform security scan
            scan_result = self._perform_security_scan(skill_name, kwargs)

            # If no high severity issues, execute directly
            if not scan_result.requires_confirmation:
                return self._execute_skill(skill_name, kwargs)

            # Store scan result for confirmation
            self._scan_cache[scan_result.scan_id] = scan_result

            # If no callback, return security report
            if not self.confirmation_callback:
                report = scan_result.format_report()
                return (
                    f"⚠️ Security confirmation required but no callback configured.\n\n"
                    f"{report}\n\n"
                    f"To proceed, configure a confirmation_callback when creating SkillLiteToolSpec."
                )

            # Call confirmation callback
            report = scan_result.format_report()
            try:
                confirmed = self.confirmation_callback(report, scan_result.scan_id)
            except Exception as e:
                return f"Error during security confirmation: {str(e)}"

            if not confirmed:
                return "❌ Execution cancelled by user after security review."

            # User confirmed, execute
            return self._execute_skill(skill_name, kwargs)

        return skill_fn

    def _execute_skill(self, skill_name: str, kwargs: Dict[str, Any]) -> str:
        """Execute a skill and return the result."""
        try:
            result = self.manager.execute(
                skill_name,
                kwargs,
                allow_network=self.allow_network,
                timeout=self.timeout
            )
            if result.success:
                return result.output or "Execution completed successfully"
            else:
                return f"Error: {result.error}"
        except Exception as e:
            return f"Execution failed: {str(e)}"
    
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

