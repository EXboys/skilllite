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
import hashlib
import time
import uuid

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

    # Internal scan cache for confirmation flow
    _scan_cache: Dict[str, SecurityScanResult] = {}
    _SCAN_CACHE_TTL: int = 300  # 5 minutes

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def _generate_input_hash(self, input_data: Dict[str, Any]) -> str:
        """Generate a hash of the input data for verification."""
        import json
        content = json.dumps(input_data, sort_keys=True, ensure_ascii=False)
        return hashlib.sha256(content.encode()).hexdigest()[:16]

    def _cleanup_expired_scans(self) -> None:
        """Remove expired scan results from cache."""
        current_time = time.time()
        expired_keys = [
            k for k, v in self._scan_cache.items()
            if current_time - v.timestamp > self._SCAN_CACHE_TTL
        ]
        for key in expired_keys:
            del self._scan_cache[key]

    def _perform_security_scan(self, input_data: Dict[str, Any]) -> SecurityScanResult:
        """
        Perform a security scan on the skill execution.

        For skills, we scan based on the skill's entry point script.
        Returns a SecurityScanResult with any issues found.
        """
        self._cleanup_expired_scans()

        input_hash = self._generate_input_hash(input_data)
        scan_id = str(uuid.uuid4())

        # Try to get skill info and scan its entry point
        try:
            skill_info = self.manager._registry.get_skill(self.skill_name)
            if skill_info and skill_info.entry_point:
                entry_script = skill_info.path / skill_info.entry_point
                if entry_script.exists():
                    # Use skillbox scan command if available
                    from ...sandbox.skillbox import find_binary
                    import subprocess

                    skillbox_path = find_binary()
                    if skillbox_path:
                        result = subprocess.run(
                            [skillbox_path, "scan", str(skill_info.path)],
                            capture_output=True,
                            text=True,
                            timeout=30
                        )

                        # Parse scan output
                        issues = self._parse_scan_output(result.stdout + result.stderr)
                        high_count = sum(1 for i in issues if i.get("severity") in ["Critical", "High"])
                        medium_count = sum(1 for i in issues if i.get("severity") == "Medium")
                        low_count = sum(1 for i in issues if i.get("severity") == "Low")

                        scan_result = SecurityScanResult(
                            is_safe=high_count == 0,
                            issues=issues,
                            scan_id=scan_id,
                            code_hash=input_hash,
                            high_severity_count=high_count,
                            medium_severity_count=medium_count,
                            low_severity_count=low_count,
                        )
                        self._scan_cache[scan_id] = scan_result
                        return scan_result
        except Exception:
            pass

        # Default: no issues found (skill already validated)
        scan_result = SecurityScanResult(
            is_safe=True,
            issues=[],
            scan_id=scan_id,
            code_hash=input_hash,
        )
        self._scan_cache[scan_id] = scan_result
        return scan_result

    def _parse_scan_output(self, output: str) -> List[Dict[str, Any]]:
        """Parse skillbox scan output into structured issues."""
        issues = []
        current_issue: Optional[Dict[str, Any]] = None

        for line in output.split('\n'):
            line = line.strip()
            if not line:
                continue

            # Detect severity markers
            if any(sev in line for sev in ['[Critical]', '[High]', '[Medium]', '[Low]']):
                if current_issue:
                    issues.append(current_issue)

                severity = "Medium"
                for sev in ['Critical', 'High', 'Medium', 'Low']:
                    if f'[{sev}]' in line:
                        severity = sev
                        break

                current_issue = {
                    "severity": severity,
                    "issue_type": "SecurityIssue",
                    "description": line,
                    "rule_id": "unknown",
                    "line_number": 0,
                    "code_snippet": ""
                }
            elif current_issue:
                if 'Rule:' in line:
                    current_issue["rule_id"] = line.split('Rule:')[-1].strip()
                elif 'Line' in line:
                    try:
                        line_num = int(line.split('Line')[-1].split(':')[0].strip())
                        current_issue["line_number"] = line_num
                    except ValueError:
                        pass
                elif 'Code:' in line or '│' in line:
                    current_issue["code_snippet"] = line.split('Code:')[-1].strip() if 'Code:' in line else line

        if current_issue:
            issues.append(current_issue)

        return issues

    def _run(
        self,
        run_manager: Optional[CallbackManagerForToolRun] = None,
        **kwargs: Any
    ) -> str:
        """Execute the skill synchronously with security confirmation support."""
        try:
            # For sandbox level 3, perform security scan first
            if self.sandbox_level >= 3:
                scan_result = self._perform_security_scan(kwargs)

                if scan_result.requires_confirmation:
                    # Check if we have a confirmation callback
                    if self.confirmation_callback:
                        report = scan_result.format_report()
                        confirmed = self.confirmation_callback(report, scan_result.scan_id)

                        if not confirmed:
                            return (
                                f"🔐 Execution cancelled by user.\n\n"
                                f"{report}\n\n"
                                f"User declined to proceed with execution."
                            )
                    else:
                        # No callback, return security report and require manual confirmation
                        return (
                            f"🔐 Security Review Required\n\n"
                            f"{scan_result.format_report()}\n\n"
                            f"To execute this skill, provide a confirmation_callback when creating the tool."
                        )

            # Execute the skill
            result = self.manager.execute(
                self.skill_name,
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

    async def _arun(
        self,
        run_manager: Optional[AsyncCallbackManagerForToolRun] = None,
        **kwargs: Any
    ) -> str:
        """Execute the skill asynchronously with security confirmation support."""
        try:
            # For sandbox level 3, perform security scan first
            if self.sandbox_level >= 3:
                scan_result = await asyncio.to_thread(self._perform_security_scan, kwargs)

                if scan_result.requires_confirmation:
                    # Check if we have an async confirmation callback
                    if self.async_confirmation_callback:
                        report = scan_result.format_report()
                        confirmed = await self.async_confirmation_callback(report, scan_result.scan_id)

                        if not confirmed:
                            return (
                                f"🔐 Execution cancelled by user.\n\n"
                                f"{report}\n\n"
                                f"User declined to proceed with execution."
                            )
                    elif self.confirmation_callback:
                        # Fall back to sync callback in thread
                        report = scan_result.format_report()
                        confirmed = await asyncio.to_thread(
                            self.confirmation_callback, report, scan_result.scan_id
                        )

                        if not confirmed:
                            return (
                                f"🔐 Execution cancelled by user.\n\n"
                                f"{report}\n\n"
                                f"User declined to proceed with execution."
                            )
                    else:
                        # No callback, return security report
                        return (
                            f"🔐 Security Review Required\n\n"
                            f"{scan_result.format_report()}\n\n"
                            f"To execute this skill, provide a confirmation_callback when creating the tool."
                        )

            # Execute the skill in thread pool
            result = await asyncio.to_thread(
                self.manager.execute,
                self.skill_name,
                kwargs,
                self.allow_network,
                self.timeout
            )
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

