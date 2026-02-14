"""
MCP Server implementation for SkillLite.

This module provides a secure code execution sandbox server implementing
the Model Context Protocol (MCP).

Features:
    1. **Skills Management**: List, inspect, and execute pre-defined skills
       from the skills directory.

    2. **Code Execution**: Execute arbitrary code in a secure sandbox with
       security scanning.

Security Model:
    The MCP server implements a two-phase execution model for security:

    1. **scan_code**: First, scan the code for security issues. This returns
       a detailed report of any potential risks found in the code.

    2. **execute_code**: Then, execute the code. If security issues were found,
       the caller must explicitly set `confirmed=true` to proceed.

    This allows LLM clients to present security warnings to users and get
    explicit confirmation before executing potentially dangerous code.

Environment Variables:
    SKILLBOX_SANDBOX_LEVEL: Default sandbox level (1/2/3, default: 3)
    SKILLBOX_PATH: Path to skillbox binary
    MCP_SANDBOX_TIMEOUT: Execution timeout in seconds (default: 30)
    SKILLLITE_SKILLS_DIR: Directory containing skills (default: ./.skills)
"""

import asyncio
import json
import os
from pathlib import Path
from typing import Any, Dict, List, Optional

from .handlers import SandboxExecutor
from .tools import get_mcp_tools


def _load_dotenv():
    """Load environment variables from .env file if it exists."""
    current_dir = Path.cwd()

    for search_dir in [current_dir] + list(current_dir.parents)[:3]:
        env_file = search_dir / ".env"
        if env_file.exists():
            try:
                with open(env_file, "r") as f:
                    for line in f:
                        line = line.strip()
                        if not line or line.startswith("#"):
                            continue
                        if "=" in line:
                            key, _, value = line.partition("=")
                            key = key.strip()
                            value = value.strip()
                            if value and value[0] in ('"', "'") and value[-1] == value[0]:
                                value = value[1:-1]
                            if key and key not in os.environ:
                                os.environ[key] = value
                return True
            except Exception:
                pass
    return False


# Load .env file on module import
_load_dotenv()

try:
    from mcp.server import Server
    from mcp.server.stdio import stdio_server
    from mcp.types import (
        TextContent,
        CallToolResult,
    )
    MCP_AVAILABLE = True
except ImportError:
    MCP_AVAILABLE = False
    Server = None  # type: ignore
    stdio_server = None  # type: ignore
    TextContent = None  # type: ignore
    CallToolResult = None  # type: ignore


class MCPServer:
    """MCP server for SkillLite sandbox execution.

    This server provides tools for skills management and secure code execution:

    Skills Tools:
    1. **list_skills**: List all available skills in the skills directory.
    2. **get_skill_info**: Get detailed information about a specific skill.
    3. **run_skill**: Execute a skill with given input parameters.

    Code Execution Tools:
    4. **scan_code**: Scan code for security issues before execution.
       Returns a detailed report and a scan_id for confirmation.
    5. **execute_code**: Execute code in a sandbox. If high-severity
       security issues are found, requires explicit confirmation.

    Example workflow for skills:
        1. Call list_skills to see available skills
        2. Call get_skill_info to understand a skill's parameters
        3. Call run_skill with the required input

    Example workflow for code execution:
        1. Call scan_code to check for security issues
        2. Review the security report with the user
        3. If user approves, call execute_code with confirmed=true and scan_id
    """

    def __init__(self, skills_dir: Optional[str] = None):
        if not MCP_AVAILABLE:
            raise ImportError(
                "MCP library not available. Please install it with: "
                "pip install skilllite[mcp]"
            )
        self.server = Server("skilllite-mcp-server")
        self.executor = SandboxExecutor()

        self.skills_dir = skills_dir or os.environ.get("SKILLLITE_SKILLS_DIR", "./.skills")
        self.skill_manager = None
        self._init_skill_manager()

        self._setup_handlers()

    def _init_skill_manager(self):
        """Initialize the SkillManager if skills directory exists."""
        try:
            from ..core.manager import SkillManager
            skills_path = Path(self.skills_dir)
            if skills_path.exists():
                self.skill_manager = SkillManager(skills_dir=str(skills_path))
        except Exception:
            pass

    def _setup_handlers(self):
        """Setup MCP server handlers."""

        @self.server.list_tools()
        async def list_tools() -> List:
            return get_mcp_tools()

        @self.server.call_tool()
        async def call_tool(
            name: str,
            arguments: Dict[str, Any]
        ) -> "CallToolResult":
            if name == "list_skills":
                return await self._handle_list_skills(arguments)
            elif name == "get_skill_info":
                return await self._handle_get_skill_info(arguments)
            elif name == "run_skill":
                return await self._handle_run_skill(arguments)
            elif name == "scan_code":
                return await self._handle_scan_code(arguments)
            elif name == "execute_code":
                return await self._handle_execute_code(arguments)
            else:
                return CallToolResult(
                    isError=True,
                    content=[
                        TextContent(
                            type="text",
                            text=f"Unknown tool: {name}"
                        )
                    ]
                )

    async def _handle_scan_code(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle scan_code tool call."""
        language = arguments.get("language")
        code = arguments.get("code")

        if not language or not code:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="Missing required arguments: language and code"
                    )
                ]
            )

        scan_result = self.executor.scan_code(language, code)
        report = scan_result.format_report()
        result_json = json.dumps(scan_result.to_dict(), indent=2)

        return CallToolResult(
            content=[
                TextContent(
                    type="text",
                    text=f"{report}\n\n---\nScan Details (JSON):\n{result_json}"
                )
            ]
        )

    async def _handle_execute_code(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle execute_code tool call."""
        language = arguments.get("language")
        code = arguments.get("code")
        confirmed = arguments.get("confirmed", False)
        scan_id = arguments.get("scan_id")
        sandbox_level = arguments.get("sandbox_level")

        if not language or not code:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="Missing required arguments: language and code"
                    )
                ]
            )

        result = self.executor.execute(
            language=language,
            code=code,
            confirmed=confirmed,
            scan_id=scan_id,
            sandbox_level=sandbox_level,
        )

        if result.get("hard_blocked"):
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=result["stderr"]
                    )
                ]
            )

        if result.get("requires_confirmation"):
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=result["stderr"]
                    )
                ]
            )

        output_lines = []
        if result["stdout"]:
            output_lines.append(f"Output:\n{result['stdout']}")
        if result["stderr"]:
            output_lines.append(f"Errors:\n{result['stderr']}")

        output_text = "\n".join(output_lines) if output_lines else "Execution completed successfully (no output)"

        if result["success"]:
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=output_text
                    )
                ]
            )
        else:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Execution failed:\n{output_text}"
                    )
                ]
            )

    async def _handle_list_skills(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle list_skills tool call."""
        if not self.skill_manager:
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=f"No skills available. Skills directory not found: {self.skills_dir}"
                    )
                ]
            )

        skills = self.skill_manager.list_skills()
        if not skills:
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=f"No skills found in directory: {self.skills_dir}"
                    )
                ]
            )

        lines = ["Available Skills:", ""]
        for skill in skills:
            lines.append(f"• **{skill.name}**")
            if skill.description:
                lines.append(f"  {skill.description}")
            if skill.language:
                lines.append(f"  Language: {skill.language}")
            lines.append("")

        return CallToolResult(
            content=[
                TextContent(
                    type="text",
                    text="\n".join(lines)
                )
            ]
        )

    async def _handle_get_skill_info(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle get_skill_info tool call."""
        skill_name = arguments.get("skill_name")

        if not skill_name:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="Missing required argument: skill_name"
                    )
                ]
            )

        if not self.skill_manager:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"No skills available. Skills directory not found: {self.skills_dir}"
                    )
                ]
            )

        skill = self.skill_manager.get_skill(skill_name)
        if not skill:
            available = ", ".join(self.skill_manager.skill_names()) or "none"
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Skill not found: {skill_name}\nAvailable skills: {available}"
                    )
                ]
            )

        full_content = skill.get_full_content()
        if not full_content:
            full_content = f"# {skill.name}\n\n{skill.description or 'No description available.'}"

        return CallToolResult(
            content=[
                TextContent(
                    type="text",
                    text=full_content
                )
            ]
        )

    async def _handle_run_skill(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle run_skill tool call using UnifiedExecutionService."""
        skill_name = arguments.get("skill_name")
        input_data = arguments.get("input", {})
        confirmed = arguments.get("confirmed", False)
        scan_id = arguments.get("scan_id")

        if not skill_name:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="Missing required argument: skill_name"
                    )
                ]
            )

        if not self.skill_manager:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"No skills available. Skills directory not found: {self.skills_dir}"
                    )
                ]
            )

        if not self.skill_manager.has_skill(skill_name):
            available = ", ".join(self.skill_manager.skill_names()) or "none"
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Skill not found: {skill_name}\nAvailable skills: {available}"
                    )
                ]
            )

        skill = self.skill_manager.get_skill(skill_name)
        if not skill:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Could not load skill: {skill_name}"
                    )
                ]
            )

        if not self.skill_manager:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="SkillManager not initialized. Skills directory may not exist."
                    )
                ]
            )

        service = self.skill_manager._execution_service
        scan_result_holder = {"result": None}

        def mcp_confirmation_callback(report: str, scan_id_from_scan: str) -> bool:
            if confirmed:
                return True
            scan_result_holder["result"] = {"report": report, "scan_id": scan_id_from_scan}
            return False

        result = service.execute_skill(
            skill_info=skill,
            input_data=input_data,
            confirmation_callback=mcp_confirmation_callback if not confirmed else lambda r, s: True,
        )

        if scan_result_holder["result"] is not None:
            report_data = scan_result_holder["result"]
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=(
                            f"🔐 Security Review Required for skill '{skill_name}'\n\n"
                            f"{report_data['report']}\n\n"
                            f"⚠️ IMPORTANT: You MUST ask the user for confirmation before proceeding.\n"
                            f"Show this security report to the user and wait for their explicit approval.\n\n"
                            f"If the user approves, call run_skill again with:\n"
                            f"  - confirmed: true\n"
                            f"  - scan_id: \"{report_data['scan_id']}\"\n"
                        )
                    )
                ]
            )

        if result.success:
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=f"Skill '{skill_name}' executed successfully.\n\nOutput:\n{result.output}"
                    )
                ]
            )
        else:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Skill '{skill_name}' execution failed.\n\nError:\n{result.error}"
                    )
                ]
            )

    async def run(self):
        """Run the MCP server."""
        async with stdio_server() as (read_stream, write_stream):
            await self.server.run(read_stream, write_stream, self.server.create_initialization_options())


async def main():
    """Main entry point for the MCP sandbox server."""
    if not MCP_AVAILABLE:
        print("Error: MCP library not available")
        print("Please install it with: pip install skilllite[mcp]")
        return

    server = MCPServer()
    await server.run()


if __name__ == "__main__":
    asyncio.run(main())
